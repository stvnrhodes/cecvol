pub mod noop;
pub mod vchi;
pub mod vchiq_ioctl;

use crate::tv;
use crate::tv::TVError;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use log::info;
use num_enum::{TryFromPrimitive, TryFromPrimitiveError};
use std::array::TryFromSliceError;
use std::convert::{TryFrom, TryInto};
use std::fmt;
use std::str;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, TryFromPrimitive)]
pub enum PowerStatus {
    On = 0,
    Standby = 1,
    InTransitionStandbyToOn = 2,
    InTransitionOnToStandby = 3,
    Unknown = 153,
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, TryFromPrimitive)]
pub enum DeckStatus {
    On = 1,
    Off = 2,
    Once = 3,
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, TryFromPrimitive)]
pub enum AbortReason {
    UnrecognisedOpcode = 0,
    WrongMode = 1,
    CannotProvideSource = 2,
    InvalidOperand = 3,
    Refused = 4,
    Undetermined = 5,
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, TryFromPrimitive)]
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

#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, TryFromPrimitive)]
pub enum LogicalAddress {
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

impl LogicalAddress {
    pub fn to_device_type(&self) -> DeviceType {
        match self {
            Self::TV => DeviceType::TV,
            Self::AudioSystem => DeviceType::AudioSystem,
            Self::RecordingDevice1 | Self::RecordingDevice2 | Self::RecordingDevice3 => {
                DeviceType::RecordingDevice
            }
            Self::PlaybackDevice1 | Self::PlaybackDevice2 | Self::PlaybackDevice3 => {
                DeviceType::PlaybackDevice
            }
            Self::Tuner1 | Self::Tuner2 | Self::Tuner3 | Self::Tuner4 => DeviceType::Tuner,
            Self::Reserved1 | Self::Reserved2 | Self::FreeUse | Self::Broadcast => {
                DeviceType::Reserved
            }
        }
    }
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, TryFromPrimitive)]
pub enum DeviceType {
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
#[derive(Copy, Clone, Debug, PartialEq, TryFromPrimitive)]
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
pub enum CECError {
    UnknownInputDevice(String),
    ParsingError(Error),
    Other(Box<dyn std::error::Error + Sync + Send>),
}

impl std::error::Error for CECError {}
impl fmt::Display for CECError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::UnknownInputDevice(err) => write!(f, "Unknown input device: {}", err),
            Self::ParsingError(err) => write!(f, "Parsing error: {}", err),
            Self::Other(err) => write!(f, "Application-specific error: {}", err),
        }
    }
}
impl From<Error> for CECError {
    fn from(err: Error) -> Self {
        Self::ParsingError(err)
    }
}

impl IntoResponse for CECError {
    fn into_response(self) -> Response {
        StatusCode::IM_A_TEAPOT.into_response()
    }
}

impl From<CECError> for TVError {
    fn from(err: CECError) -> Self {
        Self::Other(Box::new(err))
    }
}

type PhysicalAddress = u16;
fn physical_address_from_bytes(b: &[u8]) -> Result<PhysicalAddress, TryFromSliceError> {
    Ok(u16::from_be_bytes(b.try_into()?))
}

#[derive(Clone, Debug, PartialEq)]
pub enum CECMessage {
    FeatureAbort {
        feature_opcode: Opcode,
        abort_reason: AbortReason,
    },
    ImageViewOn,
    Standby,
    RequestActiveSource,
    ActiveSource {
        physical_address: PhysicalAddress,
    },
    GivePhysicalAddress,
    ReportPhysicalAddress {
        physical_address: PhysicalAddress,
        device_type: DeviceType,
    },
    SetStreamPath {
        physical_address: PhysicalAddress,
    },
    GiveOSDName,
    SetOSDName {
        name: String,
    },
    GiveDevicePowerStatus,
    ReportPowerStatus {
        power_status: PowerStatus,
    },
    GiveDeviceVendorID,
    DeviceVendorID {
        vendor_id: u32,
    },
    GiveDeckStatus {
        status_request: DeckStatus,
    },
    GiveAudioStatus,
    RoutingChange {
        original_address: PhysicalAddress,
        new_address: PhysicalAddress,
    },
    UserControlPressed {
        user_control_code: UserControl,
    },
    UserControlReleased,
    VendorCommand {
        vendor_data: Vec<u8>,
    },
}

impl CECMessage {
    fn get_opcode(&self) -> Opcode {
        match &self {
            CECMessage::FeatureAbort { .. } => Opcode::FeatureAbort,
            CECMessage::ImageViewOn => Opcode::ImageViewOn,
            CECMessage::Standby => Opcode::Standby,
            CECMessage::ActiveSource { .. } => Opcode::ActiveSource,
            CECMessage::RequestActiveSource => Opcode::RequestActiveSource,
            CECMessage::GivePhysicalAddress => Opcode::GivePhysicalAddress,
            CECMessage::ReportPhysicalAddress { .. } => Opcode::ReportPhysicalAddress,
            CECMessage::SetStreamPath { .. } => Opcode::SetStreamPath,
            CECMessage::GiveOSDName => Opcode::GiveOSDName,
            CECMessage::SetOSDName { .. } => Opcode::SetOSDName,
            CECMessage::GiveDevicePowerStatus => Opcode::GiveDevicePowerStatus,
            CECMessage::ReportPowerStatus { .. } => Opcode::ReportPowerStatus,
            CECMessage::GiveDeviceVendorID => Opcode::GiveDeviceVendorID,
            CECMessage::DeviceVendorID { .. } => Opcode::DeviceVendorID,
            CECMessage::GiveAudioStatus => Opcode::GiveAudioStatus,
            CECMessage::RoutingChange { .. } => Opcode::RoutingChange,
            CECMessage::UserControlPressed { .. } => Opcode::UserControlPressed,
            CECMessage::UserControlReleased => Opcode::UserControlReleased,
            CECMessage::VendorCommand { .. } => Opcode::VendorCommand,
            CECMessage::GiveDeckStatus { .. } => Opcode::GiveDeckStatus,
        }
    }
    fn get_parameters(&self) -> Vec<u8> {
        match &self {
            CECMessage::FeatureAbort {
                feature_opcode,
                abort_reason,
            } => vec![*feature_opcode as u8, *abort_reason as u8],
            CECMessage::ActiveSource { physical_address }
            | CECMessage::SetStreamPath { physical_address } => {
                physical_address.to_be_bytes().to_vec()
            }
            CECMessage::ReportPhysicalAddress {
                physical_address,
                device_type,
            } => {
                let mut params: Vec<u8> = physical_address.to_be_bytes().to_vec();
                params.push(*device_type as u8);
                params
            }
            CECMessage::SetOSDName { name } => name.as_bytes().to_vec(),
            CECMessage::ReportPowerStatus { power_status } => vec![*power_status as u8],
            CECMessage::DeviceVendorID { vendor_id } => {
                let code = *vendor_id as u32;
                code.to_be_bytes()[1..].to_vec()
            }
            CECMessage::RoutingChange {
                original_address,
                new_address,
            } => {
                let mut params: Vec<u8> = original_address.to_be_bytes().to_vec();
                params.extend(&new_address.to_be_bytes());
                params
            }
            CECMessage::UserControlPressed { user_control_code } => vec![*user_control_code as u8],
            CECMessage::VendorCommand { vendor_data } => vendor_data.to_vec(),
            CECMessage::GiveDeckStatus { status_request } => vec![*status_request as u8],
            CECMessage::ImageViewOn
            | CECMessage::Standby
            | CECMessage::RequestActiveSource
            | CECMessage::GivePhysicalAddress
            | CECMessage::GiveOSDName
            | CECMessage::UserControlReleased
            | CECMessage::GiveDevicePowerStatus
            | CECMessage::GiveDeviceVendorID
            | CECMessage::GiveAudioStatus => vec![],
        }
    }

    fn payload(&self) -> Vec<u8> {
        let mut p = vec![self.get_opcode() as u8];
        p.extend(self.get_parameters());
        p
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Command is too short")]
    InputTooShort,
    #[error("Parsing for this opcode hasn't been implemented")]
    OpcodeNotImplemented,
    #[error("Command has invalid logical address")]
    BadLogicalAddr(#[from] TryFromPrimitiveError<LogicalAddress>),
    #[error("Command has invalid opcode")]
    BadOpcode(#[from] TryFromPrimitiveError<Opcode>),
    #[error("Command has invalid abort reason")]
    BadAbortReason(#[from] TryFromPrimitiveError<AbortReason>),
    #[error("Command has invalid power status")]
    BadPowerStatus(#[from] TryFromPrimitiveError<PowerStatus>),
    #[error("Command has invalid user control code")]
    BadUserControlCode(#[from] TryFromPrimitiveError<UserControl>),
    #[error("Command has invalid device type")]
    BadDeviceType(#[from] TryFromPrimitiveError<DeviceType>),
    #[error("Command has invalid device type")]
    BadDeckStatus(#[from] TryFromPrimitiveError<DeckStatus>),
    #[error("Bad internal slicing")]
    BadInternalSlicing(#[from] TryFromSliceError),
    #[error("Command has invalid string")]
    BadString(#[from] str::Utf8Error),
}

#[derive(Clone, Debug)]
pub struct CECCommand {
    initiator: Option<LogicalAddress>,
    destination: LogicalAddress,
    message: CECMessage,
}
impl CECCommand {
    pub fn from_raw(input: &[u8]) -> Result<CECCommand, Error> {
        if input.len() == 0 {
            return Err(Error::InputTooShort);
        }
        let initiator = LogicalAddress::try_from((input[0] & 0xf0) >> 4)?;
        let destination = LogicalAddress::try_from(input[0] & 0x0f)?;
        if input.len() < 2 {
            return Err(Error::InputTooShort);
        }
        let opcode = Opcode::try_from(input[1])?;
        let min_len = match opcode {
            Opcode::ImageViewOn
            | Opcode::Standby
            | Opcode::GivePhysicalAddress
            | Opcode::RequestActiveSource
            | Opcode::GiveOSDName
            | Opcode::GiveDeviceVendorID
            | Opcode::GiveAudioStatus
            | Opcode::UserControlReleased => 2,
            Opcode::SetOSDName
            | Opcode::GiveDeckStatus
            | Opcode::ReportPowerStatus
            | Opcode::UserControlPressed
            | Opcode::VendorCommand => 3,
            Opcode::ActiveSource | Opcode::SetStreamPath | Opcode::FeatureAbort => 4,
            Opcode::ReportPhysicalAddress | Opcode::DeviceVendorID => 5,
            Opcode::RoutingChange => 6,
            _ => 0,
        };
        if input.len() < min_len {
            return Err(Error::InputTooShort);
        }
        let message = match opcode {
            Opcode::FeatureAbort => CECMessage::FeatureAbort {
                feature_opcode: Opcode::try_from(input[2])?,
                abort_reason: AbortReason::try_from(input[3])?,
            },
            Opcode::ImageViewOn => CECMessage::ImageViewOn,
            Opcode::Standby => CECMessage::Standby,
            Opcode::GivePhysicalAddress => CECMessage::GivePhysicalAddress,
            Opcode::RequestActiveSource => CECMessage::RequestActiveSource,
            Opcode::GiveOSDName => CECMessage::GiveOSDName,
            Opcode::GiveDevicePowerStatus => CECMessage::GiveDevicePowerStatus,
            Opcode::GiveDeckStatus => CECMessage::GiveDeckStatus {
                status_request: DeckStatus::try_from(input[2])?,
            },
            Opcode::ActiveSource => CECMessage::ActiveSource {
                physical_address: physical_address_from_bytes(&input[2..4])?,
            },
            Opcode::ReportPhysicalAddress => CECMessage::ReportPhysicalAddress {
                physical_address: physical_address_from_bytes(&input[2..4])?,
                device_type: DeviceType::try_from(input[4])?,
            },
            Opcode::SetOSDName => CECMessage::SetOSDName {
                name: str::from_utf8(&input[2..])?.to_string(),
            },
            Opcode::ReportPowerStatus => CECMessage::ReportPowerStatus {
                power_status: PowerStatus::try_from(input[2])?,
            },
            Opcode::SetStreamPath => CECMessage::SetStreamPath {
                physical_address: physical_address_from_bytes(&input[2..4])?,
            },
            Opcode::GiveDeviceVendorID => CECMessage::GiveDeviceVendorID,
            Opcode::DeviceVendorID => CECMessage::DeviceVendorID {
                vendor_id: (input[2] as u32) << 16 | (input[3] as u32) << 8 | (input[4] as u32),
            },
            Opcode::GiveAudioStatus => CECMessage::GiveAudioStatus,
            Opcode::RoutingChange => CECMessage::RoutingChange {
                original_address: physical_address_from_bytes(&input[2..4])?,
                new_address: physical_address_from_bytes(&input[4..6])?,
            },
            Opcode::UserControlPressed => CECMessage::UserControlPressed {
                user_control_code: UserControl::try_from(input[2])?,
            },
            Opcode::UserControlReleased => CECMessage::UserControlReleased,
            Opcode::VendorCommand => CECMessage::VendorCommand {
                vendor_data: input[2..].to_vec(),
            },
            _ => return Err(Error::OpcodeNotImplemented),
        };
        Ok(CECCommand {
            initiator: Some(initiator),
            destination: destination,
            message: message,
        })
    }
}

pub trait CECConnection: Sync + Send {
    fn transmit(&self, cmd: CECCommand) -> Result<(), CECError>;
    fn get_logical_address(&self) -> Result<LogicalAddress, CECError>;
    fn get_physical_address(&self) -> Result<PhysicalAddress, CECError>;
    fn set_tx_callback(&self, func: Box<dyn FnMut(&CECCommand) + Send>);
    fn set_rx_callback(&self, func: Box<dyn FnMut(&CECCommand) + Send>);
}

impl tv::TVConnection for CEC {
    fn on_off(&mut self, on: bool) -> Result<(), TVError> {
        Ok(self.on_off(on)?)
    }
    fn volume_change(&mut self, relative_steps: i32) -> Result<(), TVError> {
        Ok(self.volume_change(relative_steps)?)
    }
    fn mute(&mut self, mute: bool) -> Result<(), TVError> {
        Ok(self.mute(mute)?)
    }
    fn set_input(&mut self, input: tv::Input) -> Result<(), TVError> {
        Ok(self.set_input(input)?)
    }
}

pub struct CEC {
    conn: Arc<dyn CECConnection>,
    tx_signal: Arc<(Mutex<Option<CECCommand>>, Condvar)>,

    // Internal state.
    power_state: Arc<Mutex<bool>>,
    input_state: Arc<Mutex<PhysicalAddress>>,
}

impl CEC {
    pub fn new(
        conn: Arc<dyn CECConnection>,
        osd_name: &str,
        vendor_id: u32,
    ) -> Result<Self, CECError> {
        let tx_signal = Arc::new((Mutex::new(None), Condvar::new()));
        let power_state = Arc::new(Mutex::new(false));
        let input_state = Arc::new(Mutex::new(0));
        let inner_tx_signal = tx_signal.clone();
        let inner_conn = conn.clone();
        let inner_input_state = input_state.clone();
        let inner_power_state = power_state.clone();
        let osd_name = osd_name.to_string();
        let mut logical_to_physical = [0; 0xf];
        conn.set_rx_callback(Box::new(move |msg| {
            info!("rx {:x?} {:02x?}", msg, msg.message.payload());
            match &msg.message {
                CECMessage::GiveOSDName => inner_conn
                    .transmit(CECCommand {
                        initiator: None,
                        destination: msg.initiator.unwrap(),
                        message: CECMessage::SetOSDName {
                            name: osd_name.clone(),
                        },
                    })
                    .unwrap(),
                CECMessage::GiveDeviceVendorID => inner_conn
                    .transmit(CECCommand {
                        initiator: None,
                        destination: msg.initiator.unwrap(),
                        message: CECMessage::DeviceVendorID {
                            vendor_id: vendor_id,
                        },
                    })
                    .unwrap(),
                CECMessage::GiveDevicePowerStatus => inner_conn
                    .transmit(CECCommand {
                        initiator: None,
                        destination: msg.initiator.unwrap(),
                        message: CECMessage::ReportPowerStatus {
                            power_status: PowerStatus::On,
                        },
                    })
                    .unwrap(),
                CECMessage::GivePhysicalAddress => inner_conn
                    .transmit(CECCommand {
                        initiator: None,
                        destination: msg.initiator.unwrap(),
                        message: CECMessage::ReportPhysicalAddress {
                            physical_address: inner_conn.get_physical_address().unwrap(),
                            device_type: inner_conn.get_logical_address().unwrap().to_device_type(),
                        },
                    })
                    .unwrap(),
                CECMessage::ReportPhysicalAddress {
                    physical_address, ..
                } => {
                    logical_to_physical[msg.initiator.unwrap() as usize] = *physical_address;
                }
                CECMessage::RoutingChange {
                    original_address: _,
                    new_address,
                } => {
                    *inner_input_state.lock().unwrap() = *new_address;
                    *inner_power_state.lock().unwrap() = true;
                }
                CECMessage::SetStreamPath { physical_address } => {
                    *inner_input_state.lock().unwrap() = *physical_address;
                    *inner_power_state.lock().unwrap() = true;
                }
                CECMessage::ActiveSource { physical_address } => {
                    *inner_input_state.lock().unwrap() = *physical_address;
                    *inner_power_state.lock().unwrap() = true;
                }
                CECMessage::Standby => {
                    *inner_power_state.lock().unwrap() = false;
                }
                CECMessage::ImageViewOn => {
                    *inner_power_state.lock().unwrap() = true;
                }
                CECMessage::VendorCommand { vendor_data } => {
                    // See https://github.com/Pulse-Eight/libcec/blob/master/src/libcec/implementations/SLCommandHandler.cpp
                    match vendor_data[0] {
                        0x01 => {
                            inner_conn
                                .transmit(CECCommand {
                                    initiator: None,
                                    destination: msg.initiator.unwrap(),
                                    message: CECMessage::VendorCommand {
                                        vendor_data: vec![0x02, 0x05],
                                    },
                                })
                                .unwrap();
                        }
                        0x04 => {
                            inner_conn
                                .transmit(CECCommand {
                                    initiator: None,
                                    destination: msg.initiator.unwrap(),
                                    message: CECMessage::VendorCommand {
                                        vendor_data: vec![0x05, DeviceType::RecordingDevice as u8],
                                    },
                                })
                                .unwrap();
                            inner_conn
                                .transmit(CECCommand {
                                    initiator: None,
                                    destination: msg.initiator.unwrap(),
                                    message: CECMessage::ReportPowerStatus {
                                        power_status: PowerStatus::On,
                                    },
                                })
                                .unwrap();
                        }
                        0x03 | 0x0b | 0xa0 => {
                            inner_conn
                                .transmit(CECCommand {
                                    initiator: None,
                                    destination: msg.initiator.unwrap(),
                                    message: CECMessage::ReportPowerStatus {
                                        power_status: PowerStatus::InTransitionStandbyToOn,
                                    },
                                })
                                .unwrap();
                            inner_conn
                                .transmit(CECCommand {
                                    initiator: None,
                                    destination: msg.initiator.unwrap(),
                                    message: CECMessage::ReportPowerStatus {
                                        power_status: PowerStatus::On,
                                    },
                                })
                                .unwrap();
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }));
        conn.set_tx_callback(Box::new(move |msg| {
            info!("tx {:x?} {:02x?}", msg, msg.message.payload());
            let (lock, cvar) = &*inner_tx_signal;
            *lock.lock().unwrap() = Some(msg.clone());
            cvar.notify_all();
        }));
        let mut cec = CEC {
            conn,
            tx_signal,
            input_state: input_state,
            power_state: power_state,
        };
        // Force the tv into a well-known state
        cec.on_off(true)?;

        Ok(cec)
    }

    pub fn poll_all(&self) -> Result<(), CECError> {
        for &addr in &[
            LogicalAddress::TV,
            LogicalAddress::AudioSystem,
            LogicalAddress::PlaybackDevice1,
            LogicalAddress::PlaybackDevice2,
            LogicalAddress::PlaybackDevice3,
            LogicalAddress::RecordingDevice1,
            LogicalAddress::RecordingDevice2,
            LogicalAddress::RecordingDevice3,
            LogicalAddress::Tuner1,
            LogicalAddress::Tuner2,
            LogicalAddress::Tuner3,
            LogicalAddress::Tuner4,
        ] {
            self.transmit(addr, CECMessage::GiveOSDName)?;
            self.transmit(addr, CECMessage::GivePhysicalAddress)?;
        }
        Ok(())
    }

    fn transmit(&self, destination: LogicalAddress, message: CECMessage) -> Result<(), CECError> {
        info!("sending {:x?} to {:?}", message, destination);
        let payload = message.payload();
        self.conn.transmit(CECCommand {
            initiator: None,
            destination: destination,
            message: message,
        })?;
        let (lock, cvar) = &*self.tx_signal;
        let _ = cvar
            .wait_timeout_while(lock.lock().unwrap(), Duration::from_millis(200), |tx| {
                if let Some(CECCommand { message: sent, .. }) = tx {
                    return !sent.payload().eq(&payload);
                }
                true
            })
            .unwrap();
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

    pub fn volume_change(&mut self, relative_steps: i32) -> Result<(), CECError> {
        if relative_steps > 0 {
            for _ in 0..relative_steps {
                self.press_key(UserControl::VolumeUp)?;
            }
        } else if relative_steps < 0 {
            for _ in relative_steps..0 {
                self.press_key(UserControl::VolumeDown)?;
            }
        }
        Ok(())
    }

    pub fn mute(&mut self, mute: bool) -> Result<(), CECError> {
        // Volume changes always cause the tv to become unmuted, so use them to
        // force the tv into the proper state.
        if mute {
            self.volume_change(-1)?;
            self.volume_change(1)?;
            self.press_key(UserControl::Mute)?;
        } else {
            self.press_key(UserControl::Mute)?;
            self.volume_change(-1)?;
            self.volume_change(1)?;
        }
        Ok(())
    }
    pub fn on_off(&mut self, on: bool) -> Result<(), CECError> {
        *self.power_state.lock().unwrap() = on;
        if on {
            self.transmit(LogicalAddress::TV, CECMessage::ImageViewOn)
        } else {
            self.transmit(LogicalAddress::TV, CECMessage::Standby)
        }
    }
    pub fn set_input(&mut self, new_input: tv::Input) -> Result<(), CECError> {
        let new_addr = match new_input {
            tv::Input::HDMI1 => 0x1000,
            tv::Input::HDMI2 => 0x2000,
            tv::Input::HDMI3 => 0x3000,
            tv::Input::HDMI4 => 0x4000,
        };
        self.broadcast(CECMessage::ReportPhysicalAddress {
            physical_address: new_addr,
            device_type: DeviceType::RecordingDevice,
        })?;
        self.broadcast(CECMessage::ActiveSource {
            physical_address: new_addr,
        })?;
        *self.input_state.lock().unwrap() = new_addr;
        Ok(())
    }

    pub fn transmit_raw(&self, input: &[u8]) -> Result<(), CECError> {
        let cmd = CECCommand::from_raw(input)?;
        info!("sending {:x?}", cmd);
        self.conn.transmit(cmd)?;
        Ok(())
    }

    pub fn is_on(&self) -> bool {
        *self.power_state.lock().unwrap()
    }
    pub fn current_input(&self) -> PhysicalAddress {
        *self.input_state.lock().unwrap()
    }
}

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

    test_cec_msg! {image_view, CECMessage::ImageViewOn, "04"}
    test_cec_msg! {active_source, CECMessage::ActiveSource{
        physical_address:0x1000,
    }, "82:10:00"}
    test_cec_msg! {report_physical_address, CECMessage::ReportPhysicalAddress{
        physical_address:0x1000,
        device_type: DeviceType::Tuner,
    }, "84:10:00:03"}
    test_cec_msg! {report_power_status, CECMessage::ReportPowerStatus{
        power_status: PowerStatus::Standby,
    }, "90:01"}
    test_cec_msg! {set_osd_name, CECMessage::SetOSDName{
        name:"example".to_string(),
    }, "47:65:78:61:6d:70:6c:65"}
    test_cec_msg! {user_control_pressed, CECMessage::UserControlPressed{
        user_control_code:UserControl::Enter,
    }, "44:2b"}
}
