use crate::action::devices::{Execution, FulfillmentRequest, FulfillmentResponse, RequestPayload};
use crate::tv::{self, TVConnection, TVError};
use crate::wol;
use log::{error, info};
use serde_json::{json, Map, Value};
use std::sync::{Arc, Mutex};

const DEVICE_ID: &str = "1";
const MAX_RELATIVE_VOLUME_STEPS: i32 = 25;

#[derive(Clone)]
pub struct FulfillmentHandler {
    pub cec: Arc<Mutex<Box<dyn TVConnection + Sync + Send>>>,
    pub server_mac_addr: [u8; 6],
    pub volume_step_size: i32,
}

fn available_inputs() -> Value {
    json!([
        {
            "key": "hdmi_1",
            "names": [{ "lang": "en", "name_synonym": ["HDMI 1", "HDMI one", "input 1"] }]
        },
        {
            "key": "hdmi_2",
            "names": [{ "lang": "en", "name_synonym": ["HDMI 2", "HDMI two", "input 2"] }]
        },
        {
            "key": "hdmi_3",
            "names": [{ "lang": "en", "name_synonym": ["HDMI 3", "HDMI three", "input 3"] }]
        },
        {
            "key": "hdmi_4",
            "names": [{ "lang": "en", "name_synonym": ["HDMI 4", "HDMI four", "input 4"] }]
        }
    ])
}

fn parse_input(raw: &str) -> Option<tv::Input> {
    let normalized: String = raw
        .chars()
        .filter(|c| !matches!(c, ' ' | '_' | '-'))
        .map(|c| c.to_ascii_lowercase())
        .collect();
    match normalized.as_str() {
        "1" | "hdmi1" | "input1" => Some(tv::Input::HDMI1),
        "2" | "hdmi2" | "input2" => Some(tv::Input::HDMI2),
        "3" | "hdmi3" | "input3" => Some(tv::Input::HDMI3),
        "4" | "hdmi4" | "input4" => Some(tv::Input::HDMI4),
        _ => None,
    }
}

fn map_tv_error(err: &TVError) -> &'static str {
    match err {
        TVError::Unsupported(_) => "functionNotSupported",
        TVError::Other(_) => "transientError",
    }
}

impl FulfillmentHandler {
    pub fn handle(&self, req: &FulfillmentRequest, agent_user_id: &str) -> Value {
        let request_id = req.request_id.clone();

        if let Some(input) = req.inputs.first() {
            match input {
                RequestPayload::Sync => {
                    info!("SYNC req={} user={}", request_id, agent_user_id);
                    let supports_state = self.cec.lock().unwrap().supports_state_query();
                    let command_only = !supports_state;
                    let mut traits = vec![
                        "action.devices.traits.InputSelector",
                        "action.devices.traits.OnOff",
                        "action.devices.traits.TransportControl",
                        "action.devices.traits.Volume",
                    ];
                    let mut attributes = json!({
                        "availableInputs": available_inputs(),
                        "commandOnlyInputSelector": true,
                        "orderedInputs": false,
                        "commandOnlyOnOff": command_only,
                        "queryOnlyOnOff": false,
                        "transportControlSupportedCommands": ["PAUSE", "RESUME"],
                        "volumeMaxLevel": 100,
                        "volumeCanMuteAndUnmute": true,
                        "volumeDefaultPercentage": 15,
                        "levelStepSize": self.volume_step_size,
                        "commandOnlyVolume": command_only
                    });
                    if supports_state {
                        traits.push("action.devices.traits.MediaState");
                        let attrs = attributes.as_object_mut().unwrap();
                        attrs.insert("supportActivityState".to_string(), json!(true));
                        attrs.insert("supportPlaybackState".to_string(), json!(false));
                    }
                    return serde_json::to_value(&FulfillmentResponse {
                        request_id,
                        payload: json!({
                            "agentUserId": agent_user_id,
                            "devices": [
                                {
                                    "id": DEVICE_ID,
                                    "type": "action.devices.types.AUDIO_VIDEO_RECEIVER",
                                    "traits": traits,
                                    "name": {
                                        "name": "cecvol"
                                    },
                                    "willReportState": false,
                                    "deviceInfo": {
                                        "manufacturer": "LG",
                                        "model": "cecvol"
                                    },
                                    "attributes": attributes
                                }
                            ]
                        }),
                    })
                    .unwrap();
                }
                RequestPayload::Query { devices } => {
                    let ids: Vec<&str> = devices.iter().map(|d| d.id.as_str()).collect();
                    info!("QUERY req={} devices={:?}", request_id, ids);
                    let mut device_map = Map::new();
                    let queried_state = if devices.iter().any(|d| d.id == DEVICE_ID) {
                        let mut cec = self.cec.lock().unwrap();
                        if cec.supports_state_query() {
                            match cec.get_volume() {
                                Ok(Some(vol)) => {
                                    let is_muted = match cec.get_mute() {
                                        Ok(Some(m)) => m,
                                        Ok(None) => false,
                                        Err(e) => {
                                            error!("Failed to query mute state: {}", e);
                                            false
                                        }
                                    };
                                    Some((true, vol, is_muted))
                                }
                                Ok(None) => None,
                                Err(e) => {
                                    error!("Failed to query volume: {}", e);
                                    Some((false, 0, false))
                                }
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    for d in devices {
                        if d.id == DEVICE_ID {
                            let mut state = Map::new();
                            state.insert("online".to_string(), json!(true));
                            state.insert("status".to_string(), json!("SUCCESS"));
                            if let Some((on, vol, is_muted)) = queried_state {
                                state.insert("on".to_string(), json!(on));
                                state.insert(
                                    "activityState".to_string(),
                                    json!(if on { "ACTIVE" } else { "STANDBY" }),
                                );
                                state.insert("currentVolume".to_string(), json!(vol));
                                state.insert("isMuted".to_string(), json!(is_muted));
                            }
                            device_map.insert(d.id.clone(), Value::Object(state));
                        } else {
                            device_map.insert(
                                d.id.clone(),
                                json!({
                                    "online": false,
                                    "status": "ERROR",
                                    "errorCode": "deviceNotFound"
                                }),
                            );
                        }
                    }
                    return serde_json::to_value(&FulfillmentResponse {
                        request_id,
                        payload: json!({
                            "devices": device_map,
                        }),
                    })
                    .unwrap();
                }
                RequestPayload::Execute { commands } => {
                    let mut cec = self.cec.lock().unwrap();
                    let supports_state = cec.supports_state_query();
                    let mut results = Vec::new();

                    for c in commands {
                        let ids: Vec<String> = c.devices.iter().map(|d| d.id.clone()).collect();
                        let mut error_code: Option<&'static str> = None;
                        let mut states = Map::new();
                        states.insert("online".to_string(), json!(true));

                        for e in &c.execution {
                            info!("EXECUTE req={} devices={:?} {:?}", request_id, ids, e);
                            let res: Result<(), &'static str> = match e {
                                Execution::VolumeRelative { relative_steps } => {
                                    let steps = (*relative_steps).clamp(
                                        -MAX_RELATIVE_VOLUME_STEPS,
                                        MAX_RELATIVE_VOLUME_STEPS,
                                    );
                                    cec.volume_change(steps)
                                        .map(|()| {
                                            if supports_state {
                                                states.insert("on".to_string(), json!(true));
                                                states.insert(
                                                    "activityState".to_string(),
                                                    json!("ACTIVE"),
                                                );
                                                if let Ok(Some(vol)) = cec.get_volume() {
                                                    states.insert(
                                                        "currentVolume".to_string(),
                                                        json!(vol),
                                                    );
                                                }
                                                let muted =
                                                    cec.get_mute().ok().flatten().unwrap_or(false);
                                                states.insert("isMuted".to_string(), json!(muted));
                                            }
                                        })
                                        .map_err(|err| {
                                            error!("Failed to change volume: {}", err);
                                            map_tv_error(&err)
                                        })
                                }
                                Execution::SetVolume { volume_level } => {
                                    let clamped = (*volume_level).clamp(0, 100);
                                    cec.set_volume(clamped)
                                        .map(|()| {
                                            if supports_state {
                                                states.insert("on".to_string(), json!(true));
                                                states.insert(
                                                    "activityState".to_string(),
                                                    json!("ACTIVE"),
                                                );
                                                states.insert(
                                                    "currentVolume".to_string(),
                                                    json!(clamped),
                                                );
                                                let muted =
                                                    cec.get_mute().ok().flatten().unwrap_or(false);
                                                states.insert("isMuted".to_string(), json!(muted));
                                            }
                                        })
                                        .map_err(|err| {
                                            error!("Failed to set volume: {}", err);
                                            map_tv_error(&err)
                                        })
                                }
                                Execution::Mute { mute } => cec
                                    .mute(*mute)
                                    .map(|()| {
                                        if supports_state {
                                            states.insert("on".to_string(), json!(true));
                                            states.insert(
                                                "activityState".to_string(),
                                                json!("ACTIVE"),
                                            );
                                            states.insert("isMuted".to_string(), json!(*mute));
                                            if let Ok(Some(vol)) = cec.get_volume() {
                                                states.insert(
                                                    "currentVolume".to_string(),
                                                    json!(vol),
                                                );
                                            }
                                        }
                                    })
                                    .map_err(|err| {
                                        error!("Failed to mute: {}", err);
                                        map_tv_error(&err)
                                    }),
                                Execution::OnOff { on } => cec
                                    .on_off(*on)
                                    .map(|()| {
                                        if supports_state {
                                            states.insert("on".to_string(), json!(*on));
                                            states.insert(
                                                "activityState".to_string(),
                                                json!(if *on { "ACTIVE" } else { "STANDBY" }),
                                            );
                                        }
                                    })
                                    .map_err(|err| {
                                        error!("Failed to turn on/off: {}", err);
                                        map_tv_error(&err)
                                    }),
                                Execution::WakeOnLan {} => {
                                    wol::wake(self.server_mac_addr).map_err(|err| {
                                        error!("Failed to wake on lan: {}", err);
                                        "transientError"
                                    })
                                }
                                Execution::MediaPause {} | Execution::MediaResume {} => {
                                    cec.play_pause().map_err(|err| {
                                        error!("Failed to toggle play/pause: {}", err);
                                        map_tv_error(&err)
                                    })
                                }
                                Execution::SendKey { key } => {
                                    let key = match key.as_str() {
                                        "up" => Some(tv::Key::Up),
                                        "down" => Some(tv::Key::Down),
                                        "left" => Some(tv::Key::Left),
                                        "right" => Some(tv::Key::Right),
                                        "ok" => Some(tv::Key::Ok),
                                        "back" => Some(tv::Key::Back),
                                        "home" => Some(tv::Key::Home),
                                        "settingmenu" | "settings" => Some(tv::Key::Settings),
                                        _ => None,
                                    };
                                    match key {
                                        Some(k) => cec.send_key(k).map_err(|err| {
                                            error!("Failed to send key: {}", err);
                                            map_tv_error(&err)
                                        }),
                                        None => {
                                            error!("Unsupported key");
                                            Err("functionNotSupported")
                                        }
                                    }
                                }
                                Execution::SetInput { new_input } => match parse_input(new_input) {
                                    Some(input) => cec.set_input(input).map_err(|err| {
                                        error!("Failed to set input: {}", err);
                                        map_tv_error(&err)
                                    }),
                                    None => {
                                        error!("Unsupported input: {}", new_input);
                                        Err("unsupportedInput")
                                    }
                                },
                            };

                            if let Err(code) = res {
                                error_code = Some(code);
                                break;
                            }
                        }

                        match error_code {
                            None => results.push(json!({
                                "ids": ids,
                                "status": "SUCCESS",
                                "states": states
                            })),
                            Some(code) => results.push(json!({
                                "ids": ids,
                                "status": "ERROR",
                                "errorCode": code
                            })),
                        }
                    }

                    return serde_json::to_value(&FulfillmentResponse {
                        request_id,
                        payload: json!({
                            "commands": results,
                        }),
                    })
                    .unwrap();
                }
                RequestPayload::Disconnect => {
                    info!("DISCONNECT req={} user={}", request_id, agent_user_id);
                    return json!({});
                }
            }
        }

        serde_json::to_value(&FulfillmentResponse {
            request_id,
            payload: json!({
                "errorCode": "notSupported",
                "debugString": "no inputs provided",
            }),
        })
        .unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct MockTV {
        calls: Arc<Mutex<Vec<String>>>,
        fail_next: bool,
        stateful: bool,
        muted: bool,
    }

    impl TVConnection for MockTV {
        fn supports_state_query(&self) -> bool {
            self.stateful
        }
        fn on_off(&mut self, on: bool) -> Result<(), TVError> {
            if self.fail_next {
                return Err(TVError::Other(Box::new(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "tv offline",
                ))));
            }
            self.calls.lock().unwrap().push(format!("on_off({})", on));
            Ok(())
        }
        fn volume_change(&mut self, relative_steps: i32) -> Result<(), TVError> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("volume_change({})", relative_steps));
            Ok(())
        }
        fn set_volume(&mut self, level: i32) -> Result<(), TVError> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("set_volume({})", level));
            Ok(())
        }
        fn get_volume(&mut self) -> Result<Option<i32>, TVError> {
            if !self.stateful {
                return Ok(None);
            }
            if self.fail_next {
                return Err(TVError::Other(Box::new(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "tv offline",
                ))));
            }
            Ok(Some(19))
        }
        fn mute(&mut self, mute: bool) -> Result<(), TVError> {
            self.muted = mute;
            self.calls.lock().unwrap().push(format!("mute({})", mute));
            Ok(())
        }
        fn get_mute(&mut self) -> Result<Option<bool>, TVError> {
            if !self.stateful {
                return Ok(None);
            }
            if self.fail_next {
                return Err(TVError::Other(Box::new(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "tv offline",
                ))));
            }
            Ok(Some(self.muted))
        }
        fn set_input(&mut self, input: tv::Input) -> Result<(), TVError> {
            let name = match input {
                tv::Input::HDMI1 => "HDMI1",
                tv::Input::HDMI2 => "HDMI2",
                tv::Input::HDMI3 => "HDMI3",
                tv::Input::HDMI4 => "HDMI4",
            };
            self.calls
                .lock()
                .unwrap()
                .push(format!("set_input({})", name));
            Ok(())
        }
        fn play_pause(&mut self) -> Result<(), TVError> {
            self.calls.lock().unwrap().push("play_pause".to_string());
            Ok(())
        }
        fn send_key(&mut self, _key: tv::Key) -> Result<(), TVError> {
            self.calls.lock().unwrap().push("send_key".to_string());
            Ok(())
        }
    }

    fn make_handler_with(
        fail_next: bool,
        stateful: bool,
    ) -> (FulfillmentHandler, Arc<Mutex<Vec<String>>>) {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let mock = MockTV {
            calls: calls.clone(),
            fail_next,
            stateful,
            muted: false,
        };
        let handler = FulfillmentHandler {
            server_mac_addr: [0; 6],
            cec: Arc::new(Mutex::new(Box::new(mock))),
            volume_step_size: 3,
        };
        (handler, calls)
    }

    fn make_handler(fail_next: bool) -> (FulfillmentHandler, Arc<Mutex<Vec<String>>>) {
        make_handler_with(fail_next, true)
    }

    #[test]
    fn test_sync_response() {
        let (handler, _) = make_handler(false);
        let req: FulfillmentRequest = serde_json::from_value(json!({
            "requestId": "req-sync",
            "inputs": [{ "intent": "action.devices.SYNC" }]
        }))
        .unwrap();

        let resp = handler.handle(&req, "user@example.com");
        assert_eq!(resp["requestId"], "req-sync");
        assert_eq!(resp["payload"]["agentUserId"], "user@example.com");
        let dev = &resp["payload"]["devices"][0];
        assert_eq!(dev["id"], "1");
        assert_eq!(dev["type"], "action.devices.types.AUDIO_VIDEO_RECEIVER");
        assert_eq!(dev["name"], json!({ "name": "cecvol" }));
        assert!(dev.get("roomHint").is_none());
        assert_eq!(dev["attributes"]["commandOnlyOnOff"], false);
        assert_eq!(dev["attributes"]["commandOnlyVolume"], false);
        assert_eq!(dev["attributes"]["supportActivityState"], true);
        assert_eq!(dev["attributes"]["supportPlaybackState"], false);
        assert!(dev["traits"]
            .as_array()
            .unwrap()
            .contains(&json!("action.devices.traits.MediaState")));
        assert_eq!(
            dev["attributes"]["transportControlSupportedCommands"],
            json!(["PAUSE", "RESUME"])
        );
        assert_eq!(dev["attributes"]["levelStepSize"], 3);
    }

    #[test]
    fn test_sync_and_query_command_only_backend() {
        let (handler, _) = make_handler_with(false, false);
        let sync_req: FulfillmentRequest = serde_json::from_value(json!({
            "requestId": "req-sync-cmd",
            "inputs": [{ "intent": "action.devices.SYNC" }]
        }))
        .unwrap();
        let sync_resp = handler.handle(&sync_req, "user@example.com");
        let dev = &sync_resp["payload"]["devices"][0];
        assert_eq!(dev["attributes"]["commandOnlyOnOff"], true);
        assert_eq!(dev["attributes"]["commandOnlyVolume"], true);
        assert!(!dev["traits"]
            .as_array()
            .unwrap()
            .contains(&json!("action.devices.traits.MediaState")));

        let query_req: FulfillmentRequest = serde_json::from_value(json!({
            "requestId": "req-query-cmd",
            "inputs": [{
                "intent": "action.devices.QUERY",
                "payload": { "devices": [{ "id": "1" }] }
            }]
        }))
        .unwrap();
        let query_resp = handler.handle(&query_req, "user@example.com");
        let state = &query_resp["payload"]["devices"]["1"];
        assert_eq!(state["online"], true);
        assert_eq!(state["status"], "SUCCESS");
        assert!(state.get("on").is_none());
        assert!(state.get("activityState").is_none());
        assert!(state.get("currentVolume").is_none());
        assert!(state.get("isMuted").is_none());
    }

    #[test]
    fn test_query_response() {
        let (handler, _) = make_handler(false);
        let req: FulfillmentRequest = serde_json::from_value(json!({
            "requestId": "req-query",
            "inputs": [{
                "intent": "action.devices.QUERY",
                "payload": {
                    "devices": [{ "id": "1" }, { "id": "999" }]
                }
            }]
        }))
        .unwrap();

        let resp = handler.handle(&req, "user@example.com");
        assert_eq!(resp["payload"]["devices"]["1"]["online"], true);
        assert_eq!(resp["payload"]["devices"]["1"]["status"], "SUCCESS");
        assert_eq!(resp["payload"]["devices"]["1"]["on"], true);
        assert_eq!(resp["payload"]["devices"]["1"]["activityState"], "ACTIVE");
        assert_eq!(resp["payload"]["devices"]["1"]["currentVolume"], 19);
        assert_eq!(resp["payload"]["devices"]["1"]["isMuted"], false);
        assert_eq!(resp["payload"]["devices"]["999"]["online"], false);
        assert_eq!(
            resp["payload"]["devices"]["999"]["errorCode"],
            "deviceNotFound"
        );
    }

    #[test]
    fn test_query_response_when_tv_off() {
        let (handler, _) = make_handler(true);
        let req: FulfillmentRequest = serde_json::from_value(json!({
            "requestId": "req-query-off",
            "inputs": [{
                "intent": "action.devices.QUERY",
                "payload": {
                    "devices": [{ "id": "1" }]
                }
            }]
        }))
        .unwrap();

        let resp = handler.handle(&req, "user@example.com");
        assert_eq!(resp["payload"]["devices"]["1"]["online"], true);
        assert_eq!(resp["payload"]["devices"]["1"]["status"], "SUCCESS");
        assert_eq!(resp["payload"]["devices"]["1"]["on"], false);
        assert_eq!(resp["payload"]["devices"]["1"]["activityState"], "STANDBY");
        assert_eq!(resp["payload"]["devices"]["1"]["currentVolume"], 0);
        assert_eq!(resp["payload"]["devices"]["1"]["isMuted"], false);
    }

    #[test]
    fn test_execute_volume_pause_resume_and_multiple_commands() {
        let (handler, calls) = make_handler(false);
        let req: FulfillmentRequest = serde_json::from_value(json!({
            "requestId": "req-exec",
            "inputs": [{
                "intent": "action.devices.EXECUTE",
                "payload": {
                    "commands": [
                        {
                            "devices": [{ "id": "1" }],
                            "execution": [
                                {
                                    "command": "action.devices.commands.volumeRelative",
                                    "params": { "relativeSteps": 50 }
                                },
                                {
                                    "command": "action.devices.commands.setVolume",
                                    "params": { "volumeLevel": 20 }
                                }
                            ]
                        },
                        {
                            "devices": [{ "id": "1" }],
                            "execution": [
                                {
                                    "command": "action.devices.commands.mediaPause",
                                    "params": {}
                                },
                                {
                                    "command": "action.devices.commands.mediaResume",
                                    "params": {}
                                }
                            ]
                        }
                    ]
                }
            }]
        }))
        .unwrap();

        let resp = handler.handle(&req, "user@example.com");
        assert_eq!(resp["payload"]["commands"][0]["status"], "SUCCESS");
        assert_eq!(
            resp["payload"]["commands"][0]["states"],
            json!({
                "online": true,
                "on": true,
                "activityState": "ACTIVE",
                "currentVolume": 20,
                "isMuted": false
            })
        );
        assert_eq!(resp["payload"]["commands"][1]["status"], "SUCCESS");
        assert_eq!(
            resp["payload"]["commands"][1]["states"],
            json!({ "online": true })
        );
        assert_eq!(
            *calls.lock().unwrap(),
            vec![
                "volume_change(25)",
                "set_volume(20)",
                "play_pause",
                "play_pause"
            ]
        );
    }

    #[test]
    fn test_execute_error_returns_smart_home_error() {
        let (handler, _) = make_handler(true);
        let req: FulfillmentRequest = serde_json::from_value(json!({
            "requestId": "req-err",
            "inputs": [{
                "intent": "action.devices.EXECUTE",
                "payload": {
                    "commands": [{
                        "devices": [{ "id": "1" }],
                        "execution": [{
                            "command": "action.devices.commands.OnOff",
                            "params": { "on": false }
                        }]
                    }]
                }
            }]
        }))
        .unwrap();

        let resp = handler.handle(&req, "user@example.com");
        assert_eq!(resp["payload"]["commands"][0]["status"], "ERROR");
        assert_eq!(resp["payload"]["commands"][0]["errorCode"], "transientError");
    }

    #[test]
    fn test_disconnect_returns_empty_json_object() {
        let (handler, _) = make_handler(false);
        let req: FulfillmentRequest = serde_json::from_value(json!({
            "requestId": "req-disc",
            "inputs": [{ "intent": "action.devices.DISCONNECT" }]
        }))
        .unwrap();

        let resp = handler.handle(&req, "user@example.com");
        assert_eq!(resp, json!({}));
    }

    #[test]
    fn test_parse_input_variants() {
        assert!(matches!(parse_input("hdmi_2"), Some(tv::Input::HDMI2)));
        assert!(matches!(parse_input("HDMI 2"), Some(tv::Input::HDMI2)));
        assert!(matches!(parse_input("hdmi-3"), Some(tv::Input::HDMI3)));
        assert!(matches!(parse_input("input 4"), Some(tv::Input::HDMI4)));
        assert!(matches!(parse_input("1"), Some(tv::Input::HDMI1)));
        assert!(parse_input("hdmi_5").is_none());
    }
}
