pub mod enums;
pub mod vchi;
pub mod vchiq_ioctl;

use enums::{LogicalAddress, Opcode, PowerStatus, UserControl};
use log::debug;
use std::cmp;
use std::fmt;

#[derive(Debug)]
pub struct CECError {}
impl actix_http::ResponseError for CECError {}
impl fmt::Display for CECError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

type CECPhysicalAddress = [u8; 4];

#[derive(Debug)]
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

struct CECCommand {
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
