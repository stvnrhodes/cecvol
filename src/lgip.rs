use crate::tv;
use crate::tv::TVError;
use crate::wol;
use log::info;
use std::net::IpAddr;

pub struct LGTV {
    ip_addr: IpAddr,
    mac_address: [u8; 6],
    keycode: String,
}

// info!("faking command {:?}", cmd);

impl LGTV {}

// impl tv::TVConnection for LGTV {
//     fn power_on(&self) -> Result<(), TVError> {
//         wol.
//     }
//     fn power_off(&self) -> Result<(), TVError>;
//     fn vol_up(&self) -> Result<(), TVError>;
//     fn vol_down(&self) -> Result<(), TVError>;
//     fn mute(&self, bool) -> Result<(), TVError>;
//     fn input(&self) -> Result<(), TVError>;
// }
