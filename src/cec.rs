pub mod enums;
pub mod vchi;
pub mod vchiq_ioctl;

use cec_rs::{CecConnection, CecConnectionResultError};
use enums::{LogicalAddress, Opcode, PowerStatus, UserControl};
use log::debug;
use std::cmp;
use std::fmt;

#[derive(Debug)]
pub struct CECError {
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

type CECPhysicalAddress = [u8; 4];

#[derive(Debug)]
enum CECOpcode {
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

impl CECOpcode {
    fn get_opcode(&self) -> Opcode {
        match &self {
            CECOpcode::None => Opcode::FeatureAbort,
            CECOpcode::ImageViewOn => Opcode::ImageViewOn,
            CECOpcode::Standby => Opcode::Standby,
            CECOpcode::ActiveSource { .. } => Opcode::ActiveSource,
            CECOpcode::GivePhysicalAddress => Opcode::GivePhysicalAddress,
            CECOpcode::ReportPhysicalAddress { .. } => Opcode::ReportPhysicalAddress,
            CECOpcode::GiveOSDName => Opcode::GiveOSDName,
            CECOpcode::SetOSDName { .. } => Opcode::SetOSDName,
            CECOpcode::GiveDevicePowerStatus => Opcode::GiveDevicePowerStatus,
            CECOpcode::ReportPowerStatus { .. } => Opcode::ReportPowerStatus,
            CECOpcode::UserControlPressed { .. } => Opcode::UserControlPressed,
            CECOpcode::UserControlReleased => Opcode::UserControlReleased,
        }
    }
    fn get_parameters(&self) -> Vec<u8> {
        match &self {
            CECOpcode::ActiveSource { physical_address } => physical_address.to_vec(),
            CECOpcode::ReportPhysicalAddress {
                physical_address,
                device_type,
            } => {
                let mut params: Vec<u8> = physical_address.to_vec();
                params.push(*device_type as u8);
                params
            }
            CECOpcode::SetOSDName { name } => name.as_bytes().to_vec(),
            CECOpcode::ReportPowerStatus { power_status } => {
                let code = *power_status as u32;
                vec![((code >> 8) & 0xf) as u8, ((code >> 0) & 0xf) as u8]
            }
            CECOpcode::UserControlPressed { user_control_code } => {
                let code = *user_control_code as u32;
                vec![((code >> 8) & 0xf) as u8, ((code >> 0) & 0xf) as u8]
            }
            CECOpcode::None
            | CECOpcode::ImageViewOn
            | CECOpcode::Standby
            | CECOpcode::GivePhysicalAddress
            | CECOpcode::GiveOSDName
            | CECOpcode::UserControlReleased
            | CECOpcode::GiveDevicePowerStatus => vec![],
        }
    }
}

struct CECCommand {
    initiator: LogicalAddress,
    destination: LogicalAddress,
    opcode: CECOpcode,
}

impl Into<libcec_sys::cec_command> for CECCommand {
    fn into(self) -> libcec_sys::cec_command {
        let opcode_set = match self.opcode {
            CECOpcode::None => 0,
            _ => 1,
        };
        let params = self.opcode.get_parameters();
        let mut data: [u8; 64] = [0; 64];
        for (from, to) in params.iter().zip(data.iter_mut()) {
            *to = *from;
        }
        libcec_sys::cec_command {
            initiator: self.initiator as libcec_sys::cec_logical_address,
            destination: self.destination as libcec_sys::cec_logical_address,
            ack: 0,
            eom: 0,
            opcode_set: opcode_set,
            opcode: self.opcode.get_opcode() as u32,
            parameters: libcec_sys::cec_datapacket {
                data: data,
                size: cmp::min(params.len(), 64) as u8,
            },
            transmit_timeout: 1000,
        }
    }
}

pub struct CEC {
    conn: CecConnection,
}

impl CEC {
    pub fn new(conn: CecConnection) -> Self {
        CEC { conn }
    }

    fn transmit(&self, destination: LogicalAddress, opcode: CECOpcode) -> Result<(), CECError> {
        debug!("sending {:?} to {:?}", opcode, destination);
        self.conn.transmit(
            CECCommand {
                // TODO(stvn): Don't do this implicitly
                initiator: LogicalAddress::RecordingDevice1,
                destination: destination,
                opcode: opcode,
            }
            .into(),
        )?;
        Ok(())
    }

    fn broadcast(&self, code: CECOpcode) -> Result<(), CECError> {
        self.transmit(LogicalAddress::Broadcast, code)
    }

    fn press_key(&self, code: UserControl) -> Result<(), CECError> {
        self.transmit(
            LogicalAddress::TV,
            CECOpcode::UserControlPressed {
                user_control_code: code,
            },
        )?;
        self.transmit(LogicalAddress::TV, CECOpcode::UserControlReleased)
    }

    pub fn volume_change(&self, relative_steps: i32) -> Result<(), CECError> {
        if relative_steps > 0 {
            for _ in 0..relative_steps {
                self.conn.volume_up(true)?
                // self.press_key(UserControl::VolumeUp)?
            }
        } else if relative_steps < 0 {
            for _ in relative_steps..0 {
                self.conn.volume_down(true)?
                // self.press_key(UserControl::VolumeDown)?
            }
        }
        Ok(())
    }
    pub fn mute(&self, mute: bool) -> Result<(), CECError> {
        if mute {
            self.conn.audio_mute()?;
        } else {
            self.conn.audio_unmute()?;
        }
        Ok(())
    }
    pub fn on_off(&self, on: bool) -> Result<(), CECError> {
        if on {
            self.transmit(LogicalAddress::TV, CECOpcode::ImageViewOn)
        } else {
            self.transmit(LogicalAddress::TV, CECOpcode::Standby)
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
        self.broadcast(CECOpcode::ReportPhysicalAddress {
            physical_address: new_addr,
            device_type: LogicalAddress::RecordingDevice1,
        })?;
        self.broadcast(CECOpcode::ActiveSource {
            physical_address: new_addr,
        })?;
        self.broadcast(CECOpcode::ReportPhysicalAddress {
            physical_address: old_addr,
            device_type: LogicalAddress::RecordingDevice1,
        })
    }
}
unsafe impl Send for CEC {}
