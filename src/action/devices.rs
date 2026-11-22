use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Deserialize)]
#[serde(tag = "intent", content = "payload", rename_all = "camelCase")]
pub enum RequestPayload {
    #[serde(rename = "action.devices.SYNC")]
    Sync,

    #[serde(rename = "action.devices.QUERY")]
    Query { devices: Vec<DeviceId> },

    #[serde(rename = "action.devices.EXECUTE")]
    Execute { commands: Vec<ExecuteCommand> },

    #[serde(rename = "action.devices.DISCONNECT")]
    Disconnect,
}

#[derive(Deserialize)]
pub struct ExecuteCommand {
    pub devices: Vec<DeviceId>,
    #[serde(deserialize_with = "deserialize_executions")]
    pub execution: Vec<Execution>,
}

/// Google Home omits `"params"` on parameterless commands like `mediaPause` / `mediaResume`,
/// which serde's adjacently-tagged enum derive rejects unless `"params": {}` is present.
fn deserialize_executions<'de, D>(deserializer: D) -> Result<Vec<Execution>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let mut items: Vec<Value> = Vec::deserialize(deserializer)?;
    for item in &mut items {
        if let Some(obj) = item.as_object_mut() {
            if obj.get("params").is_none_or(Value::is_null) {
                obj.insert("params".to_string(), Value::Object(Default::default()));
            }
        }
    }
    items
        .into_iter()
        .map(|v| serde_json::from_value(v).map_err(serde::de::Error::custom))
        .collect()
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DeviceId {
    pub id: String,
}

#[derive(Deserialize, Debug, PartialEq)]
#[serde(tag = "command", content = "params", rename_all = "camelCase")]
pub enum Execution {
    // Mutes (sets the volume to 0) or unmutes the device.
    #[serde(rename = "action.devices.commands.mute")]
    Mute {
        mute: bool, // Whether to mute a device or unmute a device.
    },

    // Set volume to the requested level, based on volumeMaxLevel.
    #[serde(rename = "action.devices.commands.setVolume", rename_all = "camelCase")]
    SetVolume {
        volume_level: i32, // New volume, from 0 to volumeMaxLevel.
    },

    // Set volume up or down n steps, based on volumeMaxLevel.
    #[serde(
        rename = "action.devices.commands.volumeRelative",
        rename_all = "camelCase"
    )]
    VolumeRelative {
        relative_steps: i32, // negative for 'decrease'.
    },

    // Set the media input.
    #[serde(rename = "action.devices.commands.SetInput", rename_all = "camelCase")]
    SetInput {
        new_input: String, // Key of the new input.
    },

    // The basic on and off functionality for any device that has binary on and off.
    #[serde(rename = "action.devices.commands.OnOff")]
    OnOff {
        on: bool, // Whether to turn the device on or off.
    },

    // For triggering WakeOnLAN packet, not a real Google Smart Home command.
    #[serde(rename = "action.devices.commands.wol")]
    WakeOnLan {},

    // Pause media playback.
    #[serde(rename = "action.devices.commands.mediaPause")]
    MediaPause {},

    // Resume media playback.
    #[serde(rename = "action.devices.commands.mediaResume")]
    MediaResume {},

    // Send a remote navigation/select key (custom command, not a real Google Smart Home command).
    #[serde(rename = "action.devices.commands.sendKey")]
    SendKey { key: String },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FulfillmentRequest {
    pub request_id: String,
    pub inputs: Vec<RequestPayload>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FulfillmentResponse {
    // ID of the corresponding request.
    pub request_id: String,
    pub payload: Value,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_deserialize_sync_request() {
        let json = json!({
            "requestId": "ff36a3cc-ec34-11e6-b1a0-64510650abcf",
            "inputs": [{
                "intent": "action.devices.SYNC"
            }]
        });

        let req: FulfillmentRequest = serde_json::from_value(json).unwrap();
        assert_eq!(req.request_id, "ff36a3cc-ec34-11e6-b1a0-64510650abcf");
        match &req.inputs[0] {
            RequestPayload::Sync => {}
            _ => panic!("Expected Sync payload"),
        }
    }

    #[test]
    fn test_deserialize_execute_request() {
        let json = json!({
            "requestId": "ff36a3cc-ec34-11e6-b1a0-64510650abcf",
            "inputs": [{
                "intent": "action.devices.EXECUTE",
                "payload": {
                    "commands": [{
                        "devices": [{"id": "123"}],
                        "execution": [{
                            "command": "action.devices.commands.setVolume",
                            "params": {
                                "volumeLevel": 35
                            }
                        }]
                    }]
                }
            }]
        });

        let req: FulfillmentRequest = serde_json::from_value(json).unwrap();
        match &req.inputs[0] {
            RequestPayload::Execute { commands } => {
                let cmd = &commands[0];
                assert_eq!(cmd.devices[0].id, "123");
                match &cmd.execution[0] {
                    Execution::SetVolume { volume_level } => {
                        assert_eq!(*volume_level, 35);
                    }
                    _ => panic!("Expected SetVolume execution"),
                }
            }
            _ => panic!("Expected Execute payload"),
        }
    }

    #[test]
    fn test_deserialize_media_pause_and_resume() {
        let json = json!({
            "requestId": "req-1",
            "inputs": [{
                "intent": "action.devices.EXECUTE",
                "payload": {
                    "commands": [{
                        "devices": [{"id": "1"}],
                        "execution": [
                            {
                                "command": "action.devices.commands.mediaPause",
                                "params": {}
                            },
                            {
                                "command": "action.devices.commands.mediaResume"
                            },
                            {
                                "command": "action.devices.commands.mediaPause",
                                "params": null
                            }
                        ]
                    }]
                }
            }]
        });

        let req: FulfillmentRequest = serde_json::from_value(json).unwrap();
        match &req.inputs[0] {
            RequestPayload::Execute { commands } => {
                assert_eq!(commands[0].execution[0], Execution::MediaPause {});
                assert_eq!(commands[0].execution[1], Execution::MediaResume {});
                assert_eq!(commands[0].execution[2], Execution::MediaPause {});
            }
            _ => panic!("Expected Execute payload"),
        }
    }

    #[test]
    fn test_deserialize_send_key_request() {
        let json = json!({
            "requestId": "no-id",
            "inputs": [{
                "intent": "action.devices.EXECUTE",
                "payload": {
                    "commands": [{
                        "devices": [{"id": "123"}],
                        "execution": [{
                            "command": "action.devices.commands.sendKey",
                            "params": {
                                "key": "ok"
                            }
                        }]
                    }]
                }
            }]
        });

        let req: FulfillmentRequest = serde_json::from_value(json).unwrap();
        match &req.inputs[0] {
            RequestPayload::Execute { commands } => match &commands[0].execution[0] {
                Execution::SendKey { key } => assert_eq!(key, "ok"),
                _ => panic!("Expected SendKey execution"),
            },
            _ => panic!("Expected Execute payload"),
        }
    }

    #[test]
    fn test_deserialize_query_request() {
        let json = json!({
            "requestId": "no-id",
            "inputs": [{
                "intent": "action.devices.QUERY",
                "payload": {
                    "devices": [{"id": "1"}]
                }
            }]
        });

        let req: FulfillmentRequest = serde_json::from_value(json).unwrap();
        match &req.inputs[0] {
            RequestPayload::Query { devices } => assert_eq!(devices[0].id, "1"),
            _ => panic!("Expected Query payload"),
        }
    }

    #[test]
    fn test_deserialize_disconnect() {
        let disconnect = json!({
            "requestId": "req-disconnect",
            "inputs": [{
                "intent": "action.devices.DISCONNECT"
            }]
        });
        let req: FulfillmentRequest = serde_json::from_value(disconnect).unwrap();
        assert!(matches!(req.inputs[0], RequestPayload::Disconnect));
    }
}
