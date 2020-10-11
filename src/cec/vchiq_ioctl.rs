// ioctl functions for VideoCore Hardware Interface Queue
//
// Inspired by the following files:
//
// https://github.com/raspberrypi/userland/blob/master/interface/vchi/vchi_common.h
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
ioctl_write_int!(close_delivered, VCHIQ_IOC_MAGIC, 17);

pub type ServiceHandle = usize;
pub type VersionNum = i16;

#[repr(i8)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Status {
    Error = -1,
    Success = 0,
    Retry = 1,
}

// Callback reasons when an event occurs on a service
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq)]
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
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum CallbackReason {
    Min,
    //This indicates that there is data available
    //handle is the msg id that was transmitted with the data
    //    When a message is received and there was no FULL message available previously, send callback
    //    Tasks get kicked by the callback, reset their event and try and read from the fifo until it fails
    MsgAvailable,
    MsgSent,
    MsgSpaceAvailable, // XXX not yet implemented
    // This indicates that a transfer from the other side has completed
    BulkReceived,
    //This indicates that data queued up to be sent has now gone
    //handle is the msg id that was used when sending the data
    BulkSent,
    BulkRXSpaceAvailable, // XXX not yet implemented
    BulkTXSpaceAvailable, // XXX not yet implemented
    ServiceClosed,
    // this side has sent XOFF to peer due to lack of data consumption by service
    // (suggests the service may need to take some recovery action if it has
    // been deliberately holding off consuming data)
    SentXOff,
    SentXOn,
    // indicates that a bulk transfer has finished reading the source buffer
    DataRead,
    // power notification events (currently host side only)
    PeerOff,
    PeerSuspended,
    PeerOn,
    PeerResumed,
    ForcedPowerOff,

    // some extra notifications provided by vchiq_arm
    ServiceOpened,
    BulkReceiveAborted,
    BulkTransmitAborted,

    ReasonMax,
}

impl From<Reason> for CallbackReason {
    fn from(r: Reason) -> CallbackReason {
        match r {
            Reason::ServiceOpened => CallbackReason::ServiceOpened,
            Reason::ServiceClosed => CallbackReason::ServiceClosed,
            Reason::MessageAvailable => CallbackReason::MsgAvailable,
            Reason::BulkTransmitDone => CallbackReason::BulkSent,
            Reason::BulkReceiveDone => CallbackReason::BulkReceived,
            Reason::BulkTransmitAborted => CallbackReason::BulkTransmitAborted,
            Reason::BulkReceiveAborted => CallbackReason::BulkReceiveAborted,
        }
    }
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
#[derive(Debug)]
pub struct Header {
    msgid: i32,    /* The message identifier - opaque to applications. */
    size: u32,     /* Size of message data. */
    data: *mut i8, /* message */
}

pub type Callback = extern "C" fn(Reason, *const Header, ServiceHandle, *mut c_void) -> Status;

/// Unpack a Rust closure, extracting a `void*` pointer to the data and a
/// trampoline function which can be used to invoke it.
///
/// # Safety
///
/// It is the user's responsibility to ensure the closure outlives the returned
/// `void*` pointer.
///
/// Calling the trampoline function with anything except the `void*` pointer
/// will result in *Undefined Behaviour*.
///
/// The closure should guarantee that it never panics, seeing as panicking
/// across the FFI barrier is *Undefined Behaviour*. You may find
/// `std::panic::catch_unwind()` useful.
pub fn callback_closure<F>(closure: &mut F) -> (Callback, *mut c_void)
where
    F: FnMut(Reason, Option<&Header>, ServiceHandle) -> Status,
{
    extern "C" fn trampoline<F>(
        reason: Reason,
        header: *const Header,
        handle: ServiceHandle,
        data: *mut c_void,
    ) -> Status
    where
        F: FnMut(Reason, Option<&Header>, ServiceHandle) -> Status,
    {
        let closure: &mut F = unsafe { &mut *(data as *mut F) };
        (*closure)(reason, unsafe { header.as_ref() }, handle)
    }

    (trampoline::<F>, closure as *mut F as *mut c_void)
}

#[repr(C)]
pub struct ServiceParams {
    pub fourcc: u32,
    pub callback: Option<Callback>,
    pub userdata: *const c_void,
    pub version: VersionNum,     /* Increment for non-trivial changes */
    pub version_min: VersionNum, /* Update for incompatible changes */
}

#[repr(C)]
pub struct CreateService {
    pub service_params: ServiceParams,
    pub is_open: i32,
    pub is_vchi: i32,          // True if callback is non-nil
    pub handle: ServiceHandle, /* OUT */
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
#[derive(Debug)]
pub struct CompletionData {
    pub reason: Reason,
    pub header: Option<Header>,
    pub service_userdata: *mut c_void,
    pub bulk_userdata: *mut c_void,
}

#[repr(C)]
pub struct AwaitCompletion {
    pub count: usize,
    pub buf: *mut CompletionData,
    pub msgbufsize: usize,
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
    pub version: VersionNum,     /* The version of VCHIQ */
    pub version_min: VersionNum, /* The minimum compatible version of VCHIQ */
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
