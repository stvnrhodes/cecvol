use cec_rs::{CecConnection, CecConnectionResultError, CecLogicalAddress, CecUserControlCode};
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

const EMPTY_DATAPACKET: libcec_sys::cec_datapacket = libcec_sys::cec_datapacket {
    data: [0; 64],
    size: 0,
};

type CECPhysicalAddress = [u8; 4];

#[repr(u32)]
#[derive(Copy, Clone, Debug)]
#[allow(dead_code)]
enum PowerStatus {
    On = libcec_sys::CEC_POWER_STATUS_ON,
    Standby = libcec_sys::CEC_POWER_STATUS_STANDBY,
    InTransitionStandbyToOn = libcec_sys::CEC_POWER_STATUS_IN_TRANSITION_STANDBY_TO_ON,
    InTransitionOnToStandby = libcec_sys::CEC_POWER_STATUS_IN_TRANSITION_ON_TO_STANDBY,
    Unknown = libcec_sys::CEC_POWER_STATUS_UNKNOWN,
}

#[allow(dead_code)]
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
        device_type: CecLogicalAddress,
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
        user_control_code: CecUserControlCode,
    },
    UserControlRelease,
}

impl CECOpcode {
    fn get_opcode(&self) -> libcec_sys::cec_opcode {
        match &self {
            CECOpcode::None => 0,
            CECOpcode::ImageViewOn => libcec_sys::CEC_OPCODE_IMAGE_VIEW_ON,
            CECOpcode::Standby => libcec_sys::CEC_OPCODE_STANDBY,
            CECOpcode::ActiveSource { .. } => libcec_sys::CEC_OPCODE_ACTIVE_SOURCE,
            CECOpcode::GivePhysicalAddress => libcec_sys::CEC_OPCODE_GIVE_PHYSICAL_ADDRESS,
            CECOpcode::ReportPhysicalAddress { .. } => {
                libcec_sys::CEC_OPCODE_REPORT_PHYSICAL_ADDRESS
            }
            CECOpcode::GiveOSDName => libcec_sys::CEC_OPCODE_GIVE_OSD_NAME,
            CECOpcode::SetOSDName { .. } => libcec_sys::CEC_OPCODE_SET_OSD_NAME,
            CECOpcode::GiveDevicePowerStatus => libcec_sys::CEC_OPCODE_GIVE_DEVICE_POWER_STATUS,
            CECOpcode::ReportPowerStatus { .. } => libcec_sys::CEC_OPCODE_REPORT_POWER_STATUS,
            CECOpcode::UserControlPressed { .. } => libcec_sys::CEC_OPCODE_USER_CONTROL_PRESSED,
            CECOpcode::UserControlRelease => libcec_sys::CEC_OPCODE_USER_CONTROL_RELEASE,
        }
    }
    fn get_parameters(&self) -> libcec_sys::cec_datapacket {
        match &self {
            CECOpcode::ActiveSource { physical_address } => {
                let mut data: [u8; 64] = [0; 64];
                for (from, to) in physical_address.iter().zip(data.iter_mut()) {
                    *to = *from;
                }
                libcec_sys::cec_datapacket { data, size: 4 }
            }
            CECOpcode::ReportPhysicalAddress {
                physical_address,
                device_type,
            } => {
                let mut data: [u8; 64] = [0; 64];
                for (from, to) in physical_address.iter().zip(data.iter_mut()) {
                    *to = *from;
                }
                data[4] = *device_type as u8;
                libcec_sys::cec_datapacket { data, size: 5 }
            }
            CECOpcode::SetOSDName { name } => {
                let mut data: [u8; 64] = [0; 64];
                for (from, to) in name.as_bytes().iter().zip(data.iter_mut()) {
                    *to = *from;
                }
                libcec_sys::cec_datapacket {
                    data,
                    size: cmp::min(name.len(), 64) as u8,
                }
            }
            CECOpcode::ReportPowerStatus { power_status } => {
                let code = *power_status as u32;
                let mut data: [u8; 64] = [0; 64];
                data[0] = ((code >> 8) & 0xf) as u8;
                data[1] = ((code >> 0) & 0xf) as u8;
                libcec_sys::cec_datapacket { data, size: 2 }
            }
            CECOpcode::UserControlPressed { user_control_code } => {
                let code = *user_control_code as u32;
                let mut data: [u8; 64] = [0; 64];
                data[0] = ((code >> 8) & 0xf) as u8;
                data[1] = ((code >> 0) & 0xf) as u8;
                libcec_sys::cec_datapacket { data, size: 2 }
            }
            CECOpcode::None
            | CECOpcode::ImageViewOn
            | CECOpcode::Standby
            | CECOpcode::GivePhysicalAddress
            | CECOpcode::GiveOSDName
            | CECOpcode::UserControlRelease
            | CECOpcode::GiveDevicePowerStatus => EMPTY_DATAPACKET,
        }
    }
}

struct CECCommand {
    initiator: CecLogicalAddress,
    destination: CecLogicalAddress,
    opcode: CECOpcode,
}

impl Into<libcec_sys::cec_command> for CECCommand {
    fn into(self) -> libcec_sys::cec_command {
        let opcode_set = match self.opcode {
            CECOpcode::None => 0,
            _ => 1,
        };
        libcec_sys::cec_command {
            initiator: self.initiator as libcec_sys::cec_logical_address,
            destination: self.destination as libcec_sys::cec_logical_address,
            ack: 0,
            eom: 0,
            opcode_set: opcode_set,
            opcode: self.opcode.get_opcode(),
            parameters: self.opcode.get_parameters(),
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

    fn transmit(&self, destination: CecLogicalAddress, opcode: CECOpcode) -> Result<(), CECError> {
        debug!("sending {:?} to {:?}", opcode, destination);
        self.conn.transmit(
            CECCommand {
                // TODO(stvn): Don't do this implicitly
                initiator: CecLogicalAddress::Recordingdevice1,
                destination: destination,
                opcode: opcode,
            }
            .into(),
        )?;
        Ok(())
    }

    fn broadcast(&self, code: CECOpcode) -> Result<(), CECError> {
        self.transmit(CecLogicalAddress::Unregistered, code)
    }

    fn press_key(&self, code: CecUserControlCode) -> Result<(), CECError> {
        self.transmit(
            CecLogicalAddress::Tv,
            CECOpcode::UserControlPressed {
                user_control_code: code,
            },
        )?;
        self.transmit(CecLogicalAddress::Tv, CECOpcode::UserControlRelease)
    }

    pub fn volume_change(&self, relative_steps: i32) -> Result<(), CECError> {
        if relative_steps > 0 {
            for _ in 0..relative_steps {
                self.press_key(CecUserControlCode::VolumeUp)?
            }
        } else if relative_steps < 0 {
            for _ in relative_steps..0 {
                self.press_key(CecUserControlCode::VolumeDown)?
            }
        }
        Ok(())
    }
    pub fn mute(&self, _mute: bool) -> Result<(), CECError> {
        self.press_key(CecUserControlCode::Mute)
    }
    pub fn on_off(&self, on: bool) -> Result<(), CECError> {
        if on {
            self.transmit(CecLogicalAddress::Tv, CECOpcode::ImageViewOn)
        } else {
            self.transmit(CecLogicalAddress::Tv, CECOpcode::Standby)
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
            device_type: CecLogicalAddress::Recordingdevice1,
        })?;
        self.broadcast(CECOpcode::ActiveSource {
            physical_address: new_addr,
        })?;
        self.broadcast(CECOpcode::ReportPhysicalAddress {
            physical_address: old_addr,
            device_type: CecLogicalAddress::Recordingdevice1,
        })
    }
}
unsafe impl Send for CEC {}
