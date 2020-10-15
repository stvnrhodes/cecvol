pub mod vchi;
pub mod vchiq_ioctl;

use log::debug;
use num_enum::TryFromPrimitive;
use std::fmt;

#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq)]
#[allow(dead_code)]
pub enum PowerStatus {
    On = 0,
    Standby = 1,
    InTransitionStandbyToOn = 2,
    InTransitionOnToStandby = 3,
    Unknown = 153,
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq)]
#[allow(dead_code)]
pub enum AbortReason {
    UnrecognisedOpcode = 0,
    WrongMode = 1,
    CannotProvideSource = 2,
    InvalidOperand = 3,
    Refused = 4,
    Undetermined = 5,
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq)]
#[allow(dead_code)]
pub enum Opcode {
    FeatureAbort = 0x00,
    ImageViewOn = 0x04,
    TunerStepIncrement = 0x05,
    TunerStepDecrement = 0x06,
    TunerDeviceStatus = 0x07,
    GiveTunerDeviceStatus = 0x08,
    RecordOn = 0x09,
    RecordStatus = 0x0A,
    RecordOff = 0x0B,
    TextViewOn = 0x0D,
    RecordTVScreen = 0x0F,
    GiveDeckStatus = 0x1A,
    DeckStatus = 0x1B,
    SetMenuLanguage = 0x32,
    ClearAnalogTimer = 0x33,
    SetAnalogTimer = 0x34,
    TimerStatus = 0x35,
    Standby = 0x36,
    Play = 0x41,
    DeckControl = 0x42,
    TimerClearedStatus = 0x43,
    UserControlPressed = 0x44,
    UserControlReleased = 0x45,
    GiveOSDName = 0x46,
    SetOSDName = 0x47,
    SetOSDString = 0x64,
    SetTimerProgramTitle = 0x67,
    SystemAudioModeRequest = 0x70,
    GiveAudioStatus = 0x71,
    SetSystemAudioMode = 0x72,
    ReportAudioStatus = 0x7A,
    GiveSystemAudioModeStatus = 0x7D,
    SystemAudioModeStatus = 0x7E,
    RoutingChange = 0x80,
    RoutingInformation = 0x81,
    ActiveSource = 0x82,
    GivePhysicalAddress = 0x83,
    ReportPhysicalAddress = 0x84,
    RequestActiveSource = 0x85,
    SetStreamPath = 0x86,
    DeviceVendorID = 0x87,
    VendorCommand = 0x89,
    VendorRemoteButtonDown = 0x8A,
    VendorRemoteButtonUp = 0x8B,
    GiveDeviceVendorID = 0x8C,
    MenuRequest = 0x8D,
    MenuStatus = 0x8E,
    GiveDevicePowerStatus = 0x8F,
    ReportPowerStatus = 0x90,
    GetMenuLanguage = 0x91,
    SelectAnalogService = 0x92,
    SelectDigitalService = 0x93,
    SetDigitalTimer = 0x97,
    ClearDigitalTimer = 0x99,
    SetAudioRate = 0x9A,
    InactiveSource = 0x9D,
    CECVersion = 0x9E,
    GetCECVersion = 0x9F,
    VendorCommandWithID = 0xA0,
    ClearExternalTimer = 0xA1,
    SetExternalTimer = 0xA2,
    ReportShortAudioDescriptor = 0xA3,
    RequestShortAudioDescriptor = 0xA4,
    InitARC = 0xC0,
    ReportARCInited = 0xC1,
    ReportARCTerminated = 0xC2,
    RequestARCInit = 0xC3,
    RequestARCTermination = 0xC4,
    TerminateARC = 0xC5,
    CDC = 0xF8,
    Abort = 0xFF,
}

#[repr(i8)]
#[derive(Copy, Clone, Debug, PartialEq, TryFromPrimitive)]
#[allow(dead_code)]
pub enum LogicalAddress {
    Unknown = -1,
    TV = 0,
    RecordingDevice1 = 1,
    RecordingDevice2 = 2,
    Tuner1 = 3,
    PlaybackDevice1 = 4,
    AudioSystem = 5,
    Tuner2 = 6,
    Tuner3 = 7,
    PlaybackDevice2 = 8,
    RecordingDevice3 = 9,
    Tuner4 = 10,
    PlaybackDevice3 = 11,
    Reserved1 = 12,
    Reserved2 = 13,
    FreeUse = 14,
    Broadcast = 15,
}

#[repr(i8)]
#[derive(Copy, Clone, Debug, PartialEq)]
#[allow(dead_code)]
pub enum DeviceTypes {
    TV = 0,
    RecordingDevice = 1,
    Reserved = 2,
    Tuner = 3,
    PlaybackDevice = 4,
    AudioSystem = 5,
    Switch = 6,
    VideoProcessor = 7,
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq)]
#[allow(dead_code)]
pub enum UserControl {
    Select = 0x00,
    Up = 0x01,
    Down = 0x02,
    Left = 0x03,
    Right = 0x04,
    RightUp = 0x05,
    RightDown = 0x06,
    LeftUp = 0x07,
    LeftDown = 0x08,
    RootMenu = 0x09,
    SetupMenu = 0x0A,
    ContentsMenu = 0x0B,
    FavoriteMenu = 0x0C,
    Exit = 0x0D,
    Number0 = 0x20,
    Number1 = 0x21,
    Number2 = 0x22,
    Number3 = 0x23,
    Number4 = 0x24,
    Number5 = 0x25,
    Number6 = 0x26,
    Number7 = 0x27,
    Number8 = 0x28,
    Number9 = 0x29,
    Dot = 0x2A,
    Enter = 0x2B,
    Clear = 0x2C,
    ChannelUp = 0x30,
    ChannelDown = 0x31,
    PreviousChannel = 0x32,
    SoundSelect = 0x33,
    InputSelect = 0x34,
    DisplayInformation = 0x35,
    Help = 0x36,
    PageUp = 0x37,
    PageDown = 0x38,
    Power = 0x40,
    VolumeUp = 0x41,
    VolumeDown = 0x42,
    Mute = 0x43,
    Play = 0x44,
    Stop = 0x45,
    Pause = 0x46,
    Record = 0x47,
    Rewind = 0x48,
    FastForward = 0x49,
    Eject = 0x4A,
    Forward = 0x4B,
    Backward = 0x4C,
    Angle = 0x50,
    Subpicture = 0x51,
    VideoOnDemand = 0x52,
    EPG = 0x53,
    TimerProgramming = 0x54,
    InitialConfig = 0x55,
    PlayFunction = 0x60,
    PausePlayFunction = 0x61,
    RecordFunction = 0x62,
    PauseRecordFunction = 0x63,
    StopFunction = 0x64,
    MuteFunction = 0x65,
    RestoreVolumeFunction = 0x66,
    TuneFunction = 0x67,
    SelectDiskFunction = 0x68,
    SelectAVInputFunction = 0x69,
    SelectAudioInputFunction = 0x6A,
    F1Blue = 0x71,
    F2Red = 0x72,
    F3Green = 0x73,
    F4Yellow = 0x74,
    F5 = 0x75,
}

#[derive(Debug)]
pub struct CECError {}
impl actix_http::ResponseError for CECError {}
impl fmt::Display for CECError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

type CECPhysicalAddress = [u8; 4];

#[derive(Debug, PartialEq)]
#[allow(dead_code)]
enum CECMessage {
    None,
    ImageViewOn,
    Standby,
    ActiveSource {
        physical_address: CECPhysicalAddress,
    },
    GivePhysicalAddress,
    ReportPhysicalAddress {
        physical_address: CECPhysicalAddress,
        device_type: LogicalAddress,
    },
    GiveOSDName,
    SetOSDName {
        name: String,
    },
    GiveDevicePowerStatus,
    ReportPowerStatus {
        power_status: PowerStatus,
    },
    UserControlPressed {
        user_control_code: UserControl,
    },
    UserControlReleased,
}

impl CECMessage {
    fn get_opcode(&self) -> Opcode {
        match &self {
            CECMessage::None => Opcode::FeatureAbort,
            CECMessage::ImageViewOn => Opcode::ImageViewOn,
            CECMessage::Standby => Opcode::Standby,
            CECMessage::ActiveSource { .. } => Opcode::ActiveSource,
            CECMessage::GivePhysicalAddress => Opcode::GivePhysicalAddress,
            CECMessage::ReportPhysicalAddress { .. } => Opcode::ReportPhysicalAddress,
            CECMessage::GiveOSDName => Opcode::GiveOSDName,
            CECMessage::SetOSDName { .. } => Opcode::SetOSDName,
            CECMessage::GiveDevicePowerStatus => Opcode::GiveDevicePowerStatus,
            CECMessage::ReportPowerStatus { .. } => Opcode::ReportPowerStatus,
            CECMessage::UserControlPressed { .. } => Opcode::UserControlPressed,
            CECMessage::UserControlReleased => Opcode::UserControlReleased,
        }
    }
    fn get_parameters(&self) -> Vec<u8> {
        match &self {
            CECMessage::ActiveSource { physical_address } => physical_address.to_vec(),
            CECMessage::ReportPhysicalAddress {
                physical_address,
                device_type,
            } => {
                let mut params: Vec<u8> = physical_address.to_vec();
                params.push(*device_type as u8);
                params
            }
            CECMessage::SetOSDName { name } => name.as_bytes().to_vec(),
            CECMessage::ReportPowerStatus { power_status } => {
                let code = *power_status as u32;
                vec![((code >> 8) & 0xf) as u8, ((code >> 0) & 0xf) as u8]
            }
            CECMessage::UserControlPressed { user_control_code } => {
                let code = *user_control_code as u32;
                vec![((code >> 8) & 0xf) as u8, ((code >> 0) & 0xf) as u8]
            }
            CECMessage::None
            | CECMessage::ImageViewOn
            | CECMessage::Standby
            | CECMessage::GivePhysicalAddress
            | CECMessage::GiveOSDName
            | CECMessage::UserControlReleased
            | CECMessage::GiveDevicePowerStatus => vec![],
        }
    }
}

#[derive(Debug)]
pub struct CECCommand {
    initiator: LogicalAddress,
    destination: LogicalAddress,
    message: CECMessage,
}

pub trait CECConnection {
    fn transmit(&self, cmd: CECCommand) -> Result<(), CECError>;
}

pub struct CEC {
    conn: Box<dyn CECConnection>,
}

impl CEC {
    pub fn new(conn: Box<dyn CECConnection>) -> Self {
        CEC { conn }
    }

    fn transmit(&self, destination: LogicalAddress, message: CECMessage) -> Result<(), CECError> {
        debug!("sending {:?} to {:?}", message, destination);
        self.conn.transmit(CECCommand {
            // TODO(stvn): Don't do this implicitly
            initiator: LogicalAddress::RecordingDevice1,
            destination: destination,
            message: message,
        })?;
        Ok(())
    }

    fn broadcast(&self, code: CECMessage) -> Result<(), CECError> {
        self.transmit(LogicalAddress::Broadcast, code)
    }

    fn press_key(&self, code: UserControl) -> Result<(), CECError> {
        self.transmit(
            LogicalAddress::TV,
            CECMessage::UserControlPressed {
                user_control_code: code,
            },
        )?;
        self.transmit(LogicalAddress::TV, CECMessage::UserControlReleased)
    }

    pub fn volume_change(&self, relative_steps: i32) -> Result<(), CECError> {
        if relative_steps > 0 {
            for _ in 0..relative_steps {
                // self.conn.volume_up(true)?
                self.press_key(UserControl::VolumeUp)?
            }
        } else if relative_steps < 0 {
            for _ in relative_steps..0 {
                // self.conn.volume_down(true)?
                self.press_key(UserControl::VolumeDown)?
            }
        }
        Ok(())
    }
    pub fn mute(&self, mute: bool) -> Result<(), CECError> {
        if mute {
            // self.conn.audio_mute()?;
        } else {
            // self.conn.audio_unmute()?;
        }
        Ok(())
    }
    pub fn on_off(&self, on: bool) -> Result<(), CECError> {
        if on {
            self.transmit(LogicalAddress::TV, CECMessage::ImageViewOn)
        } else {
            self.transmit(LogicalAddress::TV, CECMessage::Standby)
        }
    }
    pub fn set_input(&self, new_input: String) -> Result<(), CECError> {
        // TODO(stvn): Fix this assumption!
        let old_addr = [0, 0, 0, 3];
        let new_addr = match new_input.as_str() {
            "hdmi1" => [0, 0, 0, 1],
            "hdmi2" => [0, 0, 0, 2],
            "hdmi3" => [0, 0, 0, 3],
            "hdmi4" => [0, 0, 0, 4],
            _ => [0, 0, 0, 0],
        };
        self.broadcast(CECMessage::ReportPhysicalAddress {
            physical_address: new_addr,
            device_type: LogicalAddress::RecordingDevice1,
        })?;
        self.broadcast(CECMessage::ActiveSource {
            physical_address: new_addr,
        })?;
        self.broadcast(CECMessage::ReportPhysicalAddress {
            physical_address: old_addr,
            device_type: LogicalAddress::RecordingDevice1,
        })
    }
}
unsafe impl Send for CEC {}

#[cfg(test)]
mod tests {
    use crate::cec::*;
    macro_rules! test_cec_msg {
        ($name:ident, $s:expr, $o:expr) => {
            #[test]
            fn $name() {
                let cmd: CECMessage = $s;
                let mut code = vec![format!("{:02x}", cmd.get_opcode() as u8)];
                code.extend(cmd.get_parameters().iter().map(|p| format!("{:02x}", p)));
                assert_eq!(code.join(":"), $o);
            }
        };
    }

    test_cec_msg! {none_msg, CECMessage::None, "00"}
    test_cec_msg! {image_view, CECMessage::ImageViewOn, "04"}
    test_cec_msg! {active_source, CECMessage::ActiveSource{
        physical_address:[0,0,0,1],
    }, "82:00:00:00:01"}
    test_cec_msg! {report_physical_address, CECMessage::ReportPhysicalAddress{
        physical_address:[0,0,0,1],
        device_type:LogicalAddress::Tuner1,
    }, "84:00:00:00:01:03"}
    test_cec_msg! {report_power_status, CECMessage::ReportPowerStatus{
        power_status:PowerStatus::Standby,
    }, "90:00:01"}
    test_cec_msg! {set_osd_name, CECMessage::SetOSDName{
        name:"example".to_string(),
    }, "47:65:78:61:6d:70:6c:65"}
    test_cec_msg! {user_control_pressed, CECMessage::UserControlPressed{
        user_control_code:UserControl::Enter,
    }, "44:00:0b"}
}
