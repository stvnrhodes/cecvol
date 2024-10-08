// ioctl functions for VideoCore Hardware Interface Queue
//
// Inspired by the following files:
//
// https://github.com/raspberrypi/userland/blob/master/interface/vchi/vchi_common.h
// https://github.com/raspberrypi/userland/blob/master/interface/vchiq_arm/vchiq_if.h
// https://github.com/raspberrypi/userland/blob/master/interface/vchiq_arm/vchiq_ioctl.h

use core::ffi::c_void;
use core::mem::size_of;
use nix::{ioctl_none, ioctl_readwrite, ioctl_write_ptr};
use num_enum::TryFromPrimitive;
use std::ptr;

// The ioctls for vchiq incorrectly take an input when specifying ioctl_none,
// so we create our own ioctl_none function that's equivalent to ioctl_write_int.
macro_rules! ioctl_none_write_int {
    ($(#[$attr:meta])* $name:ident, $ioty:expr, $nr:expr) => (
        $(#[$attr])*
        pub unsafe fn $name(fd:  std::os::raw::c_int,
                            data:usize)
                            -> nix::Result<std::os::raw::c_int> {
            nix::convert_ioctl_res!(nix::libc::ioctl(fd, nix::request_code_none!($ioty, $nr) as nix::sys::ioctl::ioctl_num_type, data))
        }
    )
}

const VCHIQ_IOC_MAGIC: u8 = 0xc4;
ioctl_none_write_int!(connect, VCHIQ_IOC_MAGIC, 0);
ioctl_none!(shutdown, VCHIQ_IOC_MAGIC, 1);
ioctl_readwrite!(create_service, VCHIQ_IOC_MAGIC, 2, CreateService);
ioctl_none!(remove_service, VCHIQ_IOC_MAGIC, 3);
ioctl_write_ptr!(queue_message, VCHIQ_IOC_MAGIC, 4, QueueMessage);
ioctl_readwrite!(queue_bulk_transmit, VCHIQ_IOC_MAGIC, 5, QueueBulkTransfer);
ioctl_readwrite!(queue_bulk_receive, VCHIQ_IOC_MAGIC, 6, QueueBulkTransfer);
ioctl_readwrite!(await_completion, VCHIQ_IOC_MAGIC, 7, AwaitCompletion);
ioctl_readwrite!(dequeue_message, VCHIQ_IOC_MAGIC, 8, DequeueMessage);
ioctl_none!(get_client_id, VCHIQ_IOC_MAGIC, 9);
ioctl_readwrite!(get_config, VCHIQ_IOC_MAGIC, 10, GetConfig);
ioctl_none!(close_service, VCHIQ_IOC_MAGIC, 11);
ioctl_none_write_int!(use_service, VCHIQ_IOC_MAGIC, 12);
ioctl_none_write_int!(release_service, VCHIQ_IOC_MAGIC, 13);
ioctl_write_ptr!(set_service_option, VCHIQ_IOC_MAGIC, 14, SetServiceOption);
ioctl_write_ptr!(dump_phys_mem, VCHIQ_IOC_MAGIC, 15, DumpPhysMem);
ioctl_none_write_int!(lib_version, VCHIQ_IOC_MAGIC, 16);
ioctl_none_write_int!(close_delivered, VCHIQ_IOC_MAGIC, 17);

pub type ServiceHandle = usize;
pub type VersionNum = i16;

#[repr(i8)]
#[derive(Copy, Clone, Debug, PartialEq, TryFromPrimitive)]
pub enum Status {
    Error = -1,
    Success = 0,
    Retry = 1,
}

// Callback reasons when an event occurs on a service
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq)]
#[allow(dead_code)]
pub enum Reason {
    ServiceOpened,       // service, -, -
    ServiceClosed,       // service, -, -
    MessageAvailable,    // service, header, -
    BulkTransmitDone,    // service, -, bulk_userdata
    BulkReceiveDone,     // service, -, bulk_userdata
    BulkTransmitAborted, // service, -, bulk_userdata
    BulkReceiveAborted,  // service, -, bulk_userdata
}

#[repr(u8)]
#[allow(dead_code)]
pub enum BulkMode {
    Callback,
    Blocking,
    Nocallback,
}

#[repr(u8)]
#[allow(dead_code)]
pub enum ServiceOption {
    Autoclose,
    SlotQuota,
    MessageQuota,
    Synchronous,
    Trace,
}

#[repr(C)]
#[derive(Debug)]
pub struct Header {
    msgid: i32,    /* The message identifier - opaque to applications. */
    size: u32,     /* Size of message data. */
    data: *mut i8, /* message */
}

pub type Callback = extern "C" fn(Reason, *const Header, ServiceHandle, *mut c_void) -> Status;

#[repr(C)]
#[derive(Debug)]
pub struct ServiceParams {
    pub fourcc: u32,
    pub callback: Callback,
    pub userdata: *const c_void,
    pub version: VersionNum,     /* Increment for non-trivial changes */
    pub version_min: VersionNum, /* Update for incompatible changes */
}

#[repr(C)]
#[derive(Debug)]
pub struct CreateService {
    pub service_params: ServiceParams,
    pub is_open: i32,
    pub is_vchi: i32,          // True if callback is non-nil
    pub handle: ServiceHandle, /* OUT */
}

#[repr(C)]
pub struct Element {
    data: *const c_void,
    size: i32,
}
impl Element {
    pub fn new<T>(item: &T) -> Element {
        Element {
            data: item as *const _ as *const c_void,
            size: size_of::<T>() as i32,
        }
    }
}

#[repr(C)]
pub struct QueueMessage {
    handle: ServiceHandle,
    count: u32,
    elements: *const Element,
}
impl QueueMessage {
    pub fn new(handle: ServiceHandle, elements: &[Element]) -> QueueMessage {
        QueueMessage {
            handle: handle,
            count: elements.len() as u32,
            elements: elements.as_ptr(),
        }
    }
}

#[repr(C)]
#[allow(dead_code)]
pub struct QueueBulkTransfer {
    pub handle: ServiceHandle,
    pub data: *mut c_void,
    pub size: u32,
    pub userdata: *mut c_void,
    pub mode: BulkMode,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct CompletionData {
    pub reason: Reason,
    pub header: *mut Header,
    pub service_userdata: *mut c_void,
    pub bulk_userdata: *mut c_void,
}
impl Default for CompletionData {
    fn default() -> Self {
        CompletionData {
            reason: Reason::ServiceOpened,
            header: ptr::null_mut(),
            service_userdata: ptr::null_mut(),
            bulk_userdata: ptr::null_mut(),
        }
    }
}

#[repr(C)]
pub struct AwaitCompletion {
    pub count: usize,
    pub buf: *mut CompletionData,
    pub msgbufsize: usize,
    pub msgbufcount: usize, /* IN/OUT */
    pub msgbufs: *mut *mut c_void,
}

#[repr(C)]
pub struct DequeueMessage {
    pub handle: ServiceHandle,
    pub blocking: i32,
    pub bufsize: u32,
    pub buf: *mut c_void,
}

#[repr(C)]
#[derive(Default, Debug)]
pub struct Config {
    pub max_msg_size: i32,
    pub bulk_threshold: i32, /* The message size above which it is better to use
                             a bulk transfer (<= max_msg_size) */
    pub max_outstanding_bulks: i32,
    pub max_services: i32,
    pub version: VersionNum,     /* The version of VCHIQ */
    pub version_min: VersionNum, /* The minimum compatible version of VCHIQ */
}

#[repr(C)]
pub struct GetConfig {
    pub config_size: usize,
    pub pconfig: *mut Config,
}

#[repr(C)]
#[allow(dead_code)]
pub struct SetServiceOption {
    pub handle: ServiceHandle,
    pub option: ServiceOption,
    pub value: i32,
}

#[repr(C)]
#[allow(dead_code)]
pub struct DumpPhysMem {
    pub virt_addr: *mut c_void,
    pub num_bytes: u32,
}
