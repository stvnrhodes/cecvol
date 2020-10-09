// VideoCore Hardware Interface
//
// Inspired by the following files:
//
// https://github.com/raspberrypi/userland/blob/master/interface/vchiq_arm/vchiq_lib.c
// https://github.com/raspberrypi/userland/blob/master/interface/vchiq_arm/vchiq_ioctl.h

use crate::cec::vchiq_ioctl;
use lazy_static::lazy_static;
use std::fs::File;
use std::mem::size_of;
use std::os::unix::io::AsRawFd;
use std::sync::Mutex;
use thiserror::Error;

const DEV_VCHIQ: &str = "/dev/vchiq";

/* The version of VCHIQ - change with any non-trivial change */
const VCHIQ_VERSION: i16 = 8;
/* The minimum compatible version - update to match VCHIQ_VERSION with any
** incompatible change */
const VCHIQ_VERSION_MIN: i16 = 3;
/* The version that introduced the VCHIQ_IOC_LIB_VERSION ioctl */
const VCHIQ_VERSION_LIB_VERSION: i16 = 7;

lazy_static! {
    static ref INITIALIZED: Mutex<bool> = Mutex::new(false);
}

// https://rust-lang.github.io/unsafe-code-guidelines/layout/function-pointers.html

fn get_config(dev_vchiq: &File) -> Result<vchiq_ioctl::Config, nix::Error> {
    let mut config = vchiq_ioctl::Config {
        ..Default::default()
    };
    let mut arg = vchiq_ioctl::GetConfig {
        config_size: size_of::<vchiq_ioctl::Config>(),
        pconfig: &mut config,
    };
    let fd = dev_vchiq.as_raw_fd();
    unsafe {
        vchiq_ioctl::get_config(fd, &mut arg)?;
    }
    Ok(config)
}

pub struct Service {
    handle: usize,
}

pub struct HardwareInterface {
    dev_vchiq: File,
    services: Vec<Service>,
}

#[derive(Error, Debug)]
pub enum CreationError {
    #[error("Could not retrieve driver version")]
    CouldNotRetrieveDriverVersion,
    #[error("VHCIQ was already initialized")]
    AlreadyInitialized,
    #[error("Could not open vchiq device")]
    IOError(#[from] std::io::Error),
    #[error("ioctl call failed")]
    IoctlError(#[from] nix::Error),
}

impl HardwareInterface {
    pub fn init() -> Result<HardwareInterface, CreationError> {
        // Ensure that only a single HardwareInterface exists.
        let mut already_initialized = INITIALIZED.lock().unwrap();
        if *already_initialized {
            return Err(CreationError::AlreadyInitialized);
        }
        *already_initialized = true;

        // Open the /dev/vchiq file and set up the correct library version
        let dev_vchiq = File::open(DEV_VCHIQ)?;
        let config = get_config(&dev_vchiq)?;
        if config.version < VCHIQ_VERSION_MIN || config.version_min > VCHIQ_VERSION {
            return Err(CreationError::CouldNotRetrieveDriverVersion);
        }
        if config.version >= VCHIQ_VERSION_LIB_VERSION {
            let fd = dev_vchiq.as_raw_fd();
            unsafe {
                vchiq_ioctl::lib_version(fd, VCHIQ_VERSION as libc::c_ulong)?;
            }
        }
        Ok(HardwareInterface {
            dev_vchiq: dev_vchiq,
            services: vec![],
        })
    }

    pub fn msg_queuev(&self) {
        // struct Element {
        //     data: *const c_void,
        //     size: i32,
        // }
        // #[repr(C)]
        // pub struct QueueMessage {
        //     handle: u32,
        //     count: u32,
        //     elements: *const Element,
        // }
        //handle vector
        // Remove all services
    }
}
