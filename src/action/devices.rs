use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub enum DeviceType {
    // Media remotes are used to control media devices. Examples of this device
    // type include hubs, universal remotes, and media controllers.
    #[serde(rename = "action.devices.types.REMOTECONTROL")]
    RemoteControl,
}

#[derive(Serialize, Deserialize)]
pub enum DeviceTrait {
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
pub struct DeviceNames {
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
pub struct DeviceInfo {
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
pub struct DeviceWithAttributes {
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
pub enum ErrorCodes {
    // Sorry, something went wrong controlling <device(s)>. Please try again.
    TransientError,
    // Sorry, that mode isn't available for <device(s)>.
    NotSupported,
}

#[derive(Serialize, Deserialize)]
pub struct ResponseErrors {
    // An error code for the entire transaction for auth failures and developer
    // system unavailability.
    pub error_code: ErrorCodes,
    // Detailed error which will never be presented to users but may be logged
    // or used during development.
    pub debug_string: String,
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum ResponsePayload {
    Error(ResponseErrors),
    Sync(SyncResponsePayload),
    Execute(ExecuteResponsePayload),
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncResponsePayload {
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
pub struct InputNames {
    lang: String, // Language code.
    // User-friendly names for the input, in a given language. The first
    // ssynonym is used in Google Assistant's response to the user.
    name_synonym: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct InputKey {
    // Unique key for the input. The key should not be exposed to users in
    // speech or response.
    key: String,
    // List of names for the input for all available languages.
    names: Vec<InputNames>,
}

#[derive(Serialize, Deserialize)]
pub struct DeviceAttributes {
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    input_selector_attributes: Option<InputSelectorAttributes>,
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    on_off_attributes: Option<OnOffAttributes>,
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    volume_attributes: Option<VolumeAttributes>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InputSelectorAttributes {
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
pub struct OnOffAttributes {
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
pub struct VolumeAttributes {
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
    pub command_only_volume: bool,
}

#[derive(Serialize)]
pub struct ExecuteResponsePayload {
    // Devices associated with the third-party user.
    pub commands: Vec<CommandResults>,
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub errors: Option<ResponseErrors>,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DeviceState {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub online: Option<bool>, // Indicates if the device is online (that is, reachable) or not.

    // The current volume percentage. It must be between >0 and volumeMaxLevel.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_volume: Option<i32>,
    // Required if volumeCanMuteAndUnmute attribute is set to true. True if
    // the device is muted; false otherwise. If isMuted is true, the device
    // still returns currentVolume for the remembered point.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_muted: Option<bool>,
    // Key of the input currently in use.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_input: Option<String>,
    // Whether a device with an on/off switch is on or off.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on: Option<bool>,
}

#[derive(Serialize, Deserialize)]
pub enum CommandStatus {
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
pub enum CommandErrors {
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
pub struct CommandResults {
    pub ids: Vec<String>,      // List of device IDs corresponding to this status.
    pub status: CommandStatus, // Result of the execute operation.
    // Expanding ERROR state if needed from the preset error codes, which will
    // map to the errors presented to users.
    pub states: DeviceState,
    #[serde(skip_serializing_if = "CommandErrors::is_none")]
    pub error_code: CommandErrors,
}

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
    pub execution: Vec<Execution>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DeviceId {
    pub id: String,
}

#[derive(Deserialize)]
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
pub struct FulfillmentRequest {
    pub request_id: String,
    pub inputs: Vec<RequestPayload>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FulfillmentResponse {
    // ID of the corresponding request.
    pub request_id: String,
    pub payload: ResponsePayload,
}
