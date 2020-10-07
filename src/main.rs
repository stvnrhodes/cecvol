use actix_http::Response;
use actix_web::{get, middleware, post, web, App, HttpServer, Responder};
use cec_rs::{
    CecCommand, CecConnection, CecConnectionCfgBuilder, CecConnectionResultError, CecDeviceType,
    CecDeviceTypeVec, CecKeypress,
};
use env_logger::Env;
use log::info;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Mutex;

#[derive(Deserialize)]
struct AuthInfo {
    // client_id: String,
// redirect_uri: String,
// state: String,
// scope: Option<String>,
// response_type: String, // always code
// user_locale: String,
}

async fn index() -> impl Responder {
    let resp: &'static [u8] = include_bytes!("index.html");
    //.set_header(http::header::CONTENT_TYPE, "text/html; charset=UTF-8")
    Response::Ok().content_type("text/html").body(resp)
}

#[get("/auth")]
async fn auth(_info: web::Query<AuthInfo>) -> impl Responder {
    // https://developers.google.com/assistant/smarthome/develop/implement-oauth#implement_oauth_account_linking
    // Todo: verify client id and redirect uri is correct
    // check that user is signed in
    // generate auth code, expire after 10m
    //redirect browser to redirect_uri, include code and state
    "Unimplemented"
}

#[derive(Serialize, Deserialize)]
enum DeviceType {
    // Media remotes are used to control media devices. Examples of this device
    // type include hubs, universal remotes, and media controllers.
    #[serde(rename = "action.devices.types.REMOTECONTROL")]
    RemoteControl,
}

#[derive(Serialize, Deserialize)]
enum DeviceTrait {
    // This trait belongs to devices that support media applications, typically
    // from third parties.
    #[serde(rename = "action.devices.traits.AppSelector")]
    AppSelector,
    // Trait for devices that can change media inputs. These inputs can have
    // dynamic names per device, and may represent audio or video feeds,
    // hardwired or networked.
    #[serde(rename = "action.devices.traits.InputSelector")]
    InputSelector,
    // This trait is used for devices which are able to report media states.
    #[serde(rename = "action.devices.traits.MediaState")]
    MediaState,
    // The basic on and off functionality for any device that has binary on and
    //  off, including plugs and switches as well as many future devices.
    #[serde(rename = "action.devices.traits.OnOff")]
    OnOff,
    // This trait belongs to any devices with settings that can only exist in
    // one of two states. These settings can represent a physical button with
    // an on/off or active/inactive state, a checkbox in HTML, or any other
    // sort of specifically enabled/disabled element.
    #[serde(rename = "action.devices.traits.Toggles")]
    Toggles,
    // This trait supports media devices which are able to control media
    // playback (for example, resuming music that's paused).
    #[serde(rename = "action.devices.traits.TransportControl")]
    TransportControl,
    // This trait belongs to devices which are able to change volume (for
    // example, setting the volume to a certain level, mute, or unmute).
    #[serde(rename = "action.devices.traits.Volume")]
    Volume,
}

// Identifiers used to describe the device.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeviceNames {
    // Primary name of the device, generally provided by the user. This is also
    // the name the Assistant will prefer to describe the device in responses.
    name: String,
    // Additional names provided by the user for the device.
    nicknames: Vec<String>,
    // List of names provided by the manufacturer rather than the user, such
    // as serial numbers, SKUs, etc.
    default_names: Vec<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeviceInfo {
    // Especially useful when the developer is a hub for other devices. Google
    // may provide a standard list of manufacturers here so that e.g. TP-Link
    // and Smartthings both describe 'osram' the same way.
    manufacturer: Option<String>,
    // The model or SKU identifier of the particular device.
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    // Specific version number attached to the hardware if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    hw_version: Option<String>,
    // Specific version number attached to the software/firmware, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    sw_version: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeviceWithAttributes {
    // The ID of the device in the developer's cloud. This must be unique for
    // the user and for the developer, as in cases of sharing we may use this
    // to dedupe multiple views of the same device. It should be immutable for
    // the device; if it changes, the Assistant will treat it as a new device.
    id: String,
    // The hardware type of device.
    #[serde(rename = "type")]
    device_type: DeviceType,
    // List of traits this device has. This defines the commands, attributes,
    // and states that the device supports.
    traits: Vec<DeviceTrait>,
    // Names of this device.
    name: DeviceNames,
    // Indicates whether this device will have its states updated by the Real
    // Time Feed. (true to use the Real Time Feed for reporting state, and
    // false to use the polling model.)
    will_report_state: bool,
    // Provides the current room of the device in the user's home to simplify setup.
    #[serde(skip_serializing_if = "Option::is_none")]
    room_hint: Option<String>,
    // Contains fields describing the device for use in one-off logic if needed
    // (e.g. 'broken firmware version X of light Y requires adjusting color',
    // or 'security flaw requires notifying all users of firmware Z').
    #[serde(skip_serializing_if = "Option::is_none")]
    device_info: Option<DeviceInfo>,
    attributes: DeviceAttributes,
    // Not present due to not yet being needed:
    // custom_data
    // other_device_ids
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum ErrorCodes {
    // Sorry, something went wrong controlling <device(s)>. Please try again.
    TransientError,
    // Sorry, that mode isn't available for <device(s)>.
    NotSupported,
}

#[derive(Serialize, Deserialize)]
struct ResponseErrors {
    // An error code for the entire transaction for auth failures and developer
    // system unavailability.
    error_code: ErrorCodes,
    // Detailed error which will never be presented to users but may be logged
    // or used during development.
    debug_string: String,
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum ResponsePayload {
    Error(ResponseErrors),
    Sync(SyncResponsePayload),
    Execute(ExecuteResponsePayload),
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SyncResponsePayload {
    // Reflects the unique (and immutable) user ID on the agent's platform. The
    // string is opaque to Google, so if there's an immutable form vs a mutable
    // form on the agent side, use the immutable form (e.g. an account number
    // rather than email).
    agent_user_id: String,
    // Devices associated with the third-party user.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    devices: Vec<DeviceWithAttributes>,
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    errors: Option<ResponseErrors>,
}

#[derive(Serialize, Deserialize)]
struct InputNames {
    lang: String, // Language code.
    // User-friendly names for the input, in a given language. The first
    // ssynonym is used in Google Assistant's response to the user.
    name_synonym: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct InputKey {
    // Unique key for the input. The key should not be exposed to users in
    // speech or response.
    key: String,
    // List of names for the input for all available languages.
    names: Vec<InputNames>,
}

#[derive(Serialize, Deserialize)]
struct DeviceAttributes {
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    input_selector_attributes: Option<InputSelectorAttributes>,
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    on_off_attributes: Option<OnOffAttributes>,
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    volume_attributes: Option<VolumeAttributes>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InputSelectorAttributes {
    // List of objects representing input audio or video feeds. Feeds can be
    // hardwired or networked. Each feed should be named and reasonably
    // persistent. Make sure to define your synonyms carefully to prevent
    // undesired (over-)triggering.
    available_inputs: Vec<InputKey>,
    // Indicates if the device supports using one-way (true) or two-way (false)
    // communication. Set this attribute to true if the device cannot respond
    // to a QUERY intent or Report State for this trait.
    command_only_input_selector: bool,
    // True if the list of output is ordered. This also indicates that the
    // 'next' and 'previous' functionality is available.
    ordered_inputs: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OnOffAttributes {
    // Indicates if the device supports using one-way (true) or two-way (false)
    // communication. Set this attribute to true if the device cannot respond
    // to a QUERY intent or Report State for this trait.
    command_only_on_off: bool,
    // Indicates if the device or sensor can only be queried for state
    // information and cannot be controlled. Set this attribute to true if the
    // device can only respond to QUERY intents and cannot respond to EXECUTE
    // intents.
    query_only_on_off: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VolumeAttributes {
    // The maximum volume level, assuming a baseline of 0 (mute). Assistant
    // will adjust adverbial commands (e.g. 'make the tv a little louder')
    // accordingly.
    volume_max_level: i32,
    // Indicates if the device can mute and unmute the volume. Mute is a
    // separate option as the 'mute' behavior takes the volume to 0 while
    // remembering the previous volume, so that unmute restores it. This is
    // reflected in volume state—if volume is 5, and the user mutes, the volume
    // remains 5 and isMuted is true.
    volume_can_mute_and_unmute: bool,
    // The volume (in percentage) for the default volume defined by user or
    // manufacturer. The scale must be 0-100.
    volume_default_percentage: i32,
    // The default step size for relative volume queries like 'volume up on
    // <device_name>.
    level_step_size: i32,
    // Indicates if the device operates using one-way (true) or two-way (false)
    // communication. For example, if the controller can confirm the new device
    // state after sending the request, this field would be false. If it's not
    // possible to confirm if the request is successfully executed or to get
    // the state of the device (for example, if the device is a traditional
    // infrared remote), set this field to true.
    command_only_volume: bool,
}

#[derive(Serialize, Deserialize)]
struct ExecuteResponsePayload {
    // Devices associated with the third-party user.
    commands: Vec<CommandResults>,
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    errors: Option<ResponseErrors>,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct DeviceState {
    #[serde(skip_serializing_if = "Option::is_none")]
    online: Option<bool>, // Indicates if the device is online (that is, reachable) or not.

    // The current volume percentage. It must be between >0 and volumeMaxLevel.
    #[serde(skip_serializing_if = "Option::is_none")]
    current_volume: Option<i32>,
    // Required if volumeCanMuteAndUnmute attribute is set to true. True if
    // the device is muted; false otherwise. If isMuted is true, the device
    // still returns currentVolume for the remembered point.
    #[serde(skip_serializing_if = "Option::is_none")]
    is_muted: Option<bool>,
    // Key of the input currently in use.
    #[serde(skip_serializing_if = "Option::is_none")]
    current_input: Option<String>,
    // Whether a device with an on/off switch is on or off.
    #[serde(skip_serializing_if = "Option::is_none")]
    on: Option<bool>,
}

#[derive(Serialize, Deserialize)]
enum CommandStatus {
    SUCCESS, // Confirm that the command succeeded.
    PENDING, // Command is enqueued but expected to succeed.
    OFFLINE, // Target device is in offline state or unreachable.
    ERROR,   // Target device is unable to perform the command.
    // There is an issue or alert associated with a command. The command could
    // succeed or fail. This status type is typically set when you want to
    // send additional information about another connected device.
    EXCEPTIONS,
}

#[derive(Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
enum CommandErrors {
    None,
    // Device receives VolumeUp command when it is already at highest volume.
    VolumeAlreadyMax,
    // Device receives VolumeDown command when it is already at lowest volume.
    VolumeAlreadyMin,
    // The input is not currently supported.
    UnsupportedInput,
    ActionNotAvailable,
    TransientError,
}

impl CommandErrors {
    fn is_none(&self) -> bool {
        self == &CommandErrors::None
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommandResults {
    ids: Vec<String>,      // List of device IDs corresponding to this status.
    status: CommandStatus, // Result of the execute operation.
    // Expanding ERROR state if needed from the preset error codes, which will
    // map to the errors presented to users.
    states: DeviceState,
    #[serde(skip_serializing_if = "CommandErrors::is_none")]
    error_code: CommandErrors,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "intent", content = "payload", rename_all = "camelCase")]
enum RequestPayload {
    #[serde(rename = "action.devices.SYNC")]
    Sync,

    #[serde(rename = "action.devices.QUERY")]
    Query { devices: Vec<Device> },

    #[serde(rename = "action.devices.EXECUTE")]
    Execute { commands: Vec<ExecuteCommand> },

    #[serde(rename = "action.devices.DISCONNECT")]
    Disconnect,
}

#[derive(Serialize, Deserialize)]
struct ExecuteCommand {
    devices: Vec<Device>,
    execution: Vec<Execution>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Device {
    id: String,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "command", content = "params", rename_all = "camelCase")]
enum Execution {
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
    //
    // For commands that use a relative scale, the Assistant will select n
    // appropriately to scale to the available steps. For example, "Make the TV
    // much louder" will set a higher number of steps than "Make the TV a tiny
    // bit louder".
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
    // Select the next input. Only applicable when the orderedInputs attribute
    // is set to true.
    #[serde(rename = "action.devices.commands.NextInput")]
    NextInput {},

    // Select the previous input. Only applicable when the orderedInputs attribute
    // is set to true.
    #[serde(rename = "action.devices.commands.PreviousInput")]
    PreviousInput {},

    // The basic on and off functionality for any device that has binary on and off.
    #[serde(rename = "action.devices.commands.OnOff")]
    OnOff {
        on: bool, // Whether to turn the device on or off.
    },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FulfillmentRequest {
    request_id: String,
    inputs: Vec<RequestPayload>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FulfillmentResponse {
    // ID of the corresponding request.
    request_id: String,
    payload: ResponsePayload,
}

#[post("/fulfillment")]
async fn fulfillment(
    req: web::Json<FulfillmentRequest>,
    cec: CECData,
) -> Result<web::Json<FulfillmentResponse>, actix_web::Error> {
    let request_id = req.request_id.clone();
    for input in &req.inputs {
        match input {
            RequestPayload::Sync => println!("Sync"),
            RequestPayload::Query { devices: _ } => println!("Query"),
            RequestPayload::Execute { commands } => {
                for c in commands {
                    for e in &c.execution {
                        match e {
                            Execution::SetVolume { volume_level } => println!("{}", volume_level),
                            Execution::VolumeRelative { relative_steps } => {
                                println!("{}", relative_steps);
                                info!("changing volume of {:?} by {}", c.devices, relative_steps);
                                cec.lock().unwrap().volume_change(*relative_steps)?;
                                let result = CommandResults {
                                    ids: c.devices.iter().map(|d| d.id.clone()).collect(),
                                    status: CommandStatus::SUCCESS,
                                    error_code: CommandErrors::None,
                                    states: DeviceState {
                                        online: Some(true),
                                        current_volume: Some(5),
                                        ..Default::default()
                                    },
                                };
                                return Ok(web::Json(FulfillmentResponse {
                                    request_id: request_id,
                                    payload: ResponsePayload::Execute(ExecuteResponsePayload {
                                        commands: vec![result],
                                        errors: None,
                                    }),
                                }));
                            }
                            _ => {
                                return Ok(web::Json(FulfillmentResponse {
                                    request_id: request_id,
                                    payload: ResponsePayload::Error(ResponseErrors {
                                        error_code: ErrorCodes::NotSupported,
                                        debug_string: "unknown command".into(),
                                    }),
                                }))
                            }
                        }
                    }
                }
            }
            RequestPayload::Disconnect => println!("Disconnect"),
        }
    }

    Ok(web::Json(FulfillmentResponse {
        request_id: request_id,
        payload: ResponsePayload::Error(ResponseErrors {
            error_code: ErrorCodes::NotSupported,
            debug_string: "no inputs provided".into(),
        }),
    }))
}

#[derive(Debug)]
struct CECError {
    err: CecConnectionResultError,
}
impl actix_http::ResponseError for CECError {}
impl fmt::Display for CECError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.err)
    }
}
impl From<CecConnectionResultError> for CECError {
    fn from(err: CecConnectionResultError) -> CECError {
        CECError { err: err }
    }
}

struct CEC {
    conn: CecConnection,
}

type CECData = web::Data<Mutex<CEC>>;
impl CEC {
    fn volume_change(&self, relative_steps: i32) -> Result<(), CECError> {
        if relative_steps > 0 {
            for _ in 0..relative_steps {
                self.conn.volume_up(true)?;
            }
        } else if relative_steps < 0 {
            for _ in relative_steps..0 {
                self.conn.volume_down(true)?;
            }
        }
        Ok(())
    }
}
unsafe impl Send for CEC {}

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    env_logger::from_env(Env::default().default_filter_or("debug")).init();

    let cfg = CecConnectionCfgBuilder::default()
        .port("RPI".into())
        .device_name("Pi".into())
        .activate_source(false) /* Don't auto-turn on the TV */
        .device_types(CecDeviceTypeVec::new(CecDeviceType::RecordingDevice))
        .key_press_callback(Box::new(on_key_press))
        .command_received_callback(Box::new(on_command_received))
        .build()
        .unwrap();
    let conn = web::Data::new(Mutex::new(CEC {
        conn: cfg.open().unwrap(),
    }));

    HttpServer::new(move || {
        App::new()
            .app_data(conn.clone())
            .wrap(middleware::Logger::default())
            .wrap(middleware::Compress::default())
            .service(fulfillment)
            .service(auth)
            .route("/", web::get().to(index))
    })
    .bind("0.0.0.0:8080")?
    .run()
    .await?;
    Ok(())
}

fn on_key_press(keypress: CecKeypress) {
    info!(
        "onKeyPress: {:?}, keycode: {:?}, duration: {:?}",
        keypress, keypress.keycode, keypress.duration
    );
}

fn on_command_received(command: CecCommand) {
    info!(
        "onCommandReceived:  opcode: {:?}, initiator: {:?}",
        command.opcode, command.initiator
    );
}
