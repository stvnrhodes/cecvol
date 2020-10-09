// ioctl functions for VideoCore Hardware Interface Queue
//
// Inspired by the following files:
//
// https://github.com/raspberrypi/userland/blob/master/interface/vchiq_arm/vchiq_if.h
// https://github.com/raspberrypi/userland/blob/master/interface/vchiq_arm/vchiq_ioctl.h

use core::ffi::c_void;
use nix::{ioctl_none, ioctl_readwrite, ioctl_write_int, ioctl_write_ptr};

const VCHIQ_IOC_MAGIC: u8 = 0xc4;
ioctl_none!(connect, VCHIQ_IOC_MAGIC, 0);
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
ioctl_none!(use_service, VCHIQ_IOC_MAGIC, 12);
ioctl_none!(release_service, VCHIQ_IOC_MAGIC, 13);
ioctl_write_ptr!(set_service_option, VCHIQ_IOC_MAGIC, 14, SetServiceOption);
ioctl_write_ptr!(dump_phys_mem, VCHIQ_IOC_MAGIC, 15, DumpPhysMem);
ioctl_write_int!(lib_version, VCHIQ_IOC_MAGIC, 16);
ioctl_none!(close_delivered, VCHIQ_IOC_MAGIC, 17);

pub type ServiceHandle = usize;

#[repr(i8)]
pub enum Status {
    Error = -1,
    Success = 0,
    Retry = 1,
}

#[repr(u8)]
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
pub enum BulkMode {
    Callback,
    Blocking,
    Nocallback,
}

#[repr(u8)]
pub enum ServiceOption {
    Autoclose,
    SlotQuota,
    MessageQuota,
    Synchronous,
    Trace,
}

#[repr(C)]
pub struct Header {
    msgid: i32,    /* The message identifier - opaque to applications. */
    size: u32,     /* Size of message data. */
    data: *mut i8, /* message */
}

pub type Callback = fn(Reason, &Header, ServiceHandle, *mut c_void) -> Status;

#[repr(C)]
pub struct ServiceParams {
    pub fourcc: i32,
    pub callback: Option<Callback>,
    pub userdata: *mut c_void,
    pub version: i16,     /* Increment for non-trivial changes */
    pub version_min: i16, /* Update for incompatible changes */
}

#[repr(C)]
pub struct CreateService {
    pub service_params: ServiceParams,
    pub is_open: i32,
    pub is_vchi: i32,
    pub handle: u32, /* OUT */
}

#[repr(C)]
pub struct Element {
    pub data: *const c_void,
    pub size: i32,
}

#[repr(C)]
pub struct QueueMessage {
    pub handle: ServiceHandle,
    pub count: u32,
    pub elements: *const Element,
}

#[repr(C)]
pub struct QueueBulkTransfer {
    pub handle: ServiceHandle,
    pub data: *mut c_void,
    pub size: u32,
    pub userdata: *mut c_void,
    pub mode: BulkMode,
}

#[repr(C)]
pub struct CompletionData {
    pub reason: Reason,
    pub header: *mut Header,
    pub service_userdata: *mut c_void,
    pub bulk_userdata: *mut c_void,
}

#[repr(C)]
pub struct AwaitCompletion {
    pub count: u32,
    pub buf: *mut CompletionData,
    pub msgbufsize: u32,
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
#[derive(Default)]
pub struct Config {
    pub max_msg_size: i32,
    pub bulk_threshold: i32, /* The message size above which it is better to use
                             a bulk transfer (<= max_msg_size) */
    pub max_outstanding_bulks: i32,
    pub max_services: i32,
    pub version: i16,     /* The version of VCHIQ */
    pub version_min: i16, /* The minimum compatible version of VCHIQ */
}

#[repr(C)]
pub struct GetConfig {
    pub config_size: usize,
    pub pconfig: *mut Config,
}

#[repr(C)]
pub struct SetServiceOption {
    pub handle: ServiceHandle,
    pub option: ServiceOption,
    pub value: i32,
}

#[repr(C)]
pub struct DumpPhysMem {
    pub virt_addr: *mut c_void,
    pub num_bytes: u32,
}
