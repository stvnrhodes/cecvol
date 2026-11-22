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
            "key": "HDMI 1",
            "names": [{ "lang": "en", "name_synonym": ["HDMI 1", "HDMI one", "input 1"] }]
        },
        {
            "key": "HDMI 2",
            "names": [{ "lang": "en", "name_synonym": ["HDMI 2", "HDMI two", "input 2"] }]
        },
        {
            "key": "HDMI 3",
            "names": [{ "lang": "en", "name_synonym": ["HDMI 3", "HDMI three", "input 3"] }]
        },
        {
            "key": "HDMI 4",
            "names": [{ "lang": "en", "name_synonym": ["HDMI 4", "HDMI four", "input 4"] }]
        }
    ])
}

fn parse_input(raw: &str) -> Option<tv::Input> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "1" | "hdmi 1" | "hdmi1" => Some(tv::Input::HDMI1),
        "2" | "hdmi 2" | "hdmi2" => Some(tv::Input::HDMI2),
        "3" | "hdmi 3" | "hdmi3" => Some(tv::Input::HDMI3),
        "4" | "hdmi 4" | "hdmi4" => Some(tv::Input::HDMI4),
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
                    return serde_json::to_value(&FulfillmentResponse {
                        request_id,
                        payload: json!({
                            "agentUserId": agent_user_id,
                            "devices": [
                                {
                                    "id": DEVICE_ID,
                                    "type": "action.devices.types.TV",
                                    "traits": [
                                        "action.devices.traits.InputSelector",
                                        "action.devices.traits.OnOff",
                                        "action.devices.traits.TransportControl",
                                        "action.devices.traits.Volume"
                                    ],
                                    "name": {
                                        "name": "cecvol"
                                    },
                                    "willReportState": false,
                                    "deviceInfo": {
                                        "manufacturer": "LG",
                                        "model": "cecvol"
                                    },
                                    "attributes": {
                                        "availableInputs": available_inputs(),
                                        "commandOnlyInputSelector": true,
                                        "orderedInputs": false,
                                        "commandOnlyOnOff": true,
                                        "queryOnlyOnOff": false,
                                        "transportControlSupportedCommands": ["PAUSE", "RESUME"],
                                        "volumeMaxLevel": 100,
                                        "volumeCanMuteAndUnmute": true,
                                        "volumeDefaultPercentage": 15,
                                        "levelStepSize": self.volume_step_size,
                                        "commandOnlyVolume": true
                                    }
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
                    let (is_on, current_volume) = if devices.iter().any(|d| d.id == DEVICE_ID) {
                        let mut cec = self.cec.lock().unwrap();
                        match cec.get_volume() {
                            Ok(Some(vol)) => (Some(true), Some(vol)),
                            Ok(None) => (None, None),
                            Err(e) => {
                                error!("Failed to query volume: {}", e);
                                (Some(false), None)
                            }
                        }
                    } else {
                        (None, None)
                    };
                    for d in devices {
                        if d.id == DEVICE_ID {
                            let mut state = Map::new();
                            state.insert("online".to_string(), json!(true));
                            state.insert("status".to_string(), json!("SUCCESS"));
                            if let Some(on) = is_on {
                                state.insert("on".to_string(), json!(on));
                            }
                            if let Some(vol) = current_volume {
                                state.insert("currentVolume".to_string(), json!(vol));
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
                    let mut results = Vec::new();

                    for c in commands {
                        let ids: Vec<String> = c.devices.iter().map(|d| d.id.clone()).collect();
                        let mut error_code: Option<&'static str> = None;

                        for e in &c.execution {
                            info!("EXECUTE req={} devices={:?} {:?}", request_id, ids, e);
                            let res: Result<(), &'static str> = match e {
                                Execution::VolumeRelative { relative_steps } => {
                                    let steps = (*relative_steps).clamp(
                                        -MAX_RELATIVE_VOLUME_STEPS,
                                        MAX_RELATIVE_VOLUME_STEPS,
                                    );
                                    cec.volume_change(steps).map_err(|err| {
                                        error!("Failed to change volume: {}", err);
                                        map_tv_error(&err)
                                    })
                                }
                                Execution::SetVolume { volume_level } => {
                                    cec.set_volume(*volume_level).map_err(|err| {
                                        error!("Failed to set volume: {}", err);
                                        map_tv_error(&err)
                                    })
                                }
                                Execution::Mute { mute } => cec.mute(*mute).map_err(|err| {
                                    error!("Failed to mute: {}", err);
                                    map_tv_error(&err)
                                }),
                                Execution::OnOff { on } => cec.on_off(*on).map_err(|err| {
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
                                "status": "SUCCESS"
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
    }

    impl TVConnection for MockTV {
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
            if self.fail_next {
                return Err(TVError::Other(Box::new(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "tv offline",
                ))));
            }
            Ok(Some(19))
        }
        fn mute(&mut self, mute: bool) -> Result<(), TVError> {
            self.calls.lock().unwrap().push(format!("mute({})", mute));
            Ok(())
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

    fn make_handler(fail_next: bool) -> (FulfillmentHandler, Arc<Mutex<Vec<String>>>) {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let mock = MockTV {
            calls: calls.clone(),
            fail_next,
        };
        let handler = FulfillmentHandler {
            server_mac_addr: [0; 6],
            cec: Arc::new(Mutex::new(Box::new(mock))),
            volume_step_size: 3,
        };
        (handler, calls)
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
        assert_eq!(dev["type"], "action.devices.types.TV");
        assert_eq!(dev["name"], json!({ "name": "cecvol" }));
        assert!(dev.get("roomHint").is_none());
        assert_eq!(
            dev["attributes"]["transportControlSupportedCommands"],
            json!(["PAUSE", "RESUME"])
        );
        assert_eq!(dev["attributes"]["levelStepSize"], 3);
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
        assert_eq!(resp["payload"]["devices"]["1"]["currentVolume"], 19);
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
        assert!(resp["payload"]["devices"]["1"].get("currentVolume").is_none());
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
        assert_eq!(resp["payload"]["commands"][1]["status"], "SUCCESS");
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
}
