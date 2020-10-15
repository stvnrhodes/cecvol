// ioctl functions for VideoCore Hardware Interface Queue
//
// Inspired by the following files:
//
// https://github.com/raspberrypi/userland/blob/master/interface/vchi/vchi_common.h
// https://github.com/raspberrypi/userland/blob/master/interface/vchiq_arm/vchiq_if.h
// https://github.com/raspberrypi/userland/blob/master/interface/vchiq_arm/vchiq_ioctl.h

// ioctl(6, _IOC(_IOC_READ|_IOC_WRITE, 0xc4, 0xa, 0x8), 0x7e9d7bc0) = 0
// ioctl(6, _IOC(_IOC_NONE, 0xc4, 0x10, 0), 0x8) = 0
// ioctl(6, _IOC(_IOC_NONE, 0xc4, 0, 0), 0) = 0
// [2020-10-14T04:42:17.543Z DEBUG cecvol::cec::vchi] initializing clients
// ioctl(6, _IOC(_IOC_READ|_IOC_WRITE, 0xc4, 0x2, 0x1c), 0x7ea0bba0) = 0
// ioctl(6, _IOC(_IOC_READ|_IOC_WRITE, 0xc4, 0x2, 0x1c), 0x7ea0bbc0) = 0
// ioctl(6, _IOC(_IOC_READ|_IOC_WRITE, 0xc4, 0x2, 0x1c), 0x7ea0bbe0) = 0
// ioctl(6, _IOC(_IOC_READ|_IOC_WRITE, 0xc4, 0x2, 0x1c), 0x7ea0bc00) = 0
// strace: Process 18067 attached
// strace: Process 18068 attached
// strace: Process 18069 attached
// [2020-10-14T04:42:17.552Z DEBUG cecvol] Starting server...
// [pid 18069] ioctl(6, _IOC(_IOC_READ|_IOC_WRITE, 0xc4, 0x7, 0x10), 0x767feb40) = -1 ENOTTY (Inappropriate ioctl for device)
// thread 'VCHIQ completion' panicked at 'called `Result::unwrap()` on an `Err` value: Sys(ENOTTY)', src/cec/vchi.rs:488:26

// ioctl(6, _IOC(_IOC_READ|_IOC_WRITE, 0xc4, 0xa, 0x8), 0x7efbbc00) = 0
// [2020-10-14T21:59:43.541Z DEBUG cecvol::cec::vchi] vchiq config: Config { max_msg_size: 4088, bulk_threshold: 4088, max_outstanding_bulks: 4, max_services: 4096, version: 8, version_min: 3 }
// ioctl(6, _IOC(_IOC_NONE, 0xc4, 0x10, 0), 0x8) = 0
// ioctl(6, _IOC(_IOC_NONE, 0xc4, 0, 0), 0) = 0
// strace: Process 28929 attached
// [pid 28928] ioctl(6, _IOC(_IOC_READ|_IOC_WRITE, 0xc4, 0x2, 0x1c), 0x7efbbbe0) = 0
// [pid 28928] ioctl(6, _IOC(_IOC_READ|_IOC_WRITE, 0xc4, 0x2, 0x1c), 0x7efbbc00) = 0
// [2020-10-14T21:59:43.547Z DEBUG cecvol::cec::vchi] buffers at 0, allocating more
// [pid 28929] ioctl(6, _IOC(_IOC_READ|_IOC_WRITE, 0xc4, 0x7, 0x14) <unfinished ...>
// [pid 28928] ioctl(6, _IOC(_IOC_NONE, 0xc4, 0xd, 0), 0x112007) = 0
// [pid 28928] ioctl(6, _IOC(_IOC_NONE, 0xc4, 0xd, 0), 0x113008) = 0
// [pid 28928] ioctl(6, _IOC(_IOC_READ|_IOC_WRITE, 0xc4, 0x2, 0x1c), 0x7efbbc20) = 0
// [pid 28928] ioctl(6, _IOC(_IOC_READ|_IOC_WRITE, 0xc4, 0x2, 0x1c), 0x7efbbc40) = 0
// [pid 28928] ioctl(6, _IOC(_IOC_NONE, 0xc4, 0xd, 0), 0x114009) = 0
// [pid 28928] ioctl(6, _IOC(_IOC_NONE, 0xc4, 0xd, 0), 0x11500a) = 0

// get config
// library version
// connect
// create service
// release service
//  new thread await completion
// create service 2x
// release service 2x
// create service 2x
// release service 2x
// use service
// queue message
//  new thread with completion

// strace: Process 18127 attached
// [pid 18126] ioctl(6, _IOC(_IOC_READ|_IOC_WRITE, 0xc4, 0xa, 0x8), 0x7e9dfec4) = 0
// [pid 18126] ioctl(6, _IOC(_IOC_NONE, 0xc4, 0x10, 0), 0x8) = 0
// [pid 18126] ioctl(6, _IOC(_IOC_NONE, 0xc4, 0, 0), 0) = 0
// strace: Process 18128 attached
// [pid 18126] ioctl(6, _IOC(_IOC_READ|_IOC_WRITE, 0xc4, 0x2, 0x1c), 0x7e9dfe44) = 0
// [pid 18126] ioctl(6, _IOC(_IOC_NONE, 0xc4, 0xd, 0), 0x9d007) = 0
// [pid 18128] ioctl(6, _IOC(_IOC_READ|_IOC_WRITE, 0xc4, 0x7, 0x14) <unfinished ...>
// [pid 18126] ioctl(6, _IOC(_IOC_READ|_IOC_WRITE, 0xc4, 0x2, 0x1c), 0x7e9dfde4) = 0
// [pid 18126] ioctl(6, _IOC(_IOC_READ|_IOC_WRITE, 0xc4, 0x2, 0x1c), 0x7e9dfde4) = 0
// strace: Process 18129 attached
// [pid 18126] ioctl(6, _IOC(_IOC_NONE, 0xc4, 0xd, 0), 0x9e008) = 0
// [pid 18126] ioctl(6, _IOC(_IOC_NONE, 0xc4, 0xd, 0), 0x9f009) = 0
// [pid 18126] ioctl(6, _IOC(_IOC_READ|_IOC_WRITE, 0xc4, 0x2, 0x1c), 0x7e9dfe04) = 0
// [pid 18126] ioctl(6, _IOC(_IOC_READ|_IOC_WRITE, 0xc4, 0x2, 0x1c), 0x7e9dfe04) = 0
// [pid 18126] ioctl(6, _IOC(_IOC_NONE, 0xc4, 0xd, 0), 0xa000a) = 0
// [pid 18126] ioctl(6, _IOC(_IOC_NONE, 0xc4, 0xd, 0), 0xa100b) = 0
// strace: Process 18130 attached
// [pid 18126] ioctl(6, _IOC(_IOC_NONE, 0xc4, 0xc, 0), 0xa000a) = 0
// [pid 18126] ioctl(6, _IOC(_IOC_WRITE, 0xc4, 0x4, 0xc), 0x7e9dfe14) = 0
// [pid 18128] <... ioctl resumed> , 0x75efeaf4) = 1                         -
// [pid 18128] ioctl(6, _IOC(_IOC_READ|_IOC_WRITE, 0xc4, 0x7, 0x14) <unfinished ...>
// [pid 18126] ioctl(6, _IOC(_IOC_READ|_IOC_WRITE, 0xc4, 0x8, 0x10), 0x7e9dfdc8) = 20
// [pid 18126] ioctl(6, _IOC(_IOC_NONE, 0xc4, 0xd, 0), 0xa000a) = 0
// [pid 18126] ioctl(6, _IOC(_IOC_READ|_IOC_WRITE, 0xc4, 0x2, 0x1c) <unfinished ...>
// [pid 18130] ioctl(6, _IOC(_IOC_NONE, 0xc4, 0xc, 0) <unfinished ...>
// [pid 18126] <... ioctl resumed> , 0x7e9dfe04) = 0
// [pid 18130] <... ioctl resumed> , 0xa000a) = 0
// [pid 18130] ioctl(6, _IOC(_IOC_WRITE, 0xc4, 0x4, 0xc), 0x74cfdae4) = 0
// [pid 18130] ioctl(6, _IOC(_IOC_READ|_IOC_WRITE, 0xc4, 0x8, 0x10), 0x74cfda98) = -1 EAGAIN (Resource temporarily unavailable)
// [pid 18126] ioctl(6, _IOC(_IOC_READ|_IOC_WRITE, 0xc4, 0x2, 0x1c) <unfinished ...>
// [pid 18128] <... ioctl resumed> , 0x75efeaf4) = 1
// [pid 18128] ioctl(6, _IOC(_IOC_READ|_IOC_WRITE, 0xc4, 0x7, 0x14) <unfinished ...>
// [pid 18130] ioctl(6, _IOC(_IOC_READ|_IOC_WRITE, 0xc4, 0x8, 0x10) <unfinished ...>
// [pid 18126] <... ioctl resumed> , 0x7e9dfe04) = 0
// [pid 18130] <... ioctl resumed> , 0x74cfda98) = 52
// [pid 18130] ioctl(6, _IOC(_IOC_NONE, 0xc4, 0xd, 0), 0xa000a) = 0
// [pid 18130] ioctl(6, _IOC(_IOC_NONE, 0xc4, 0xc, 0), 0xa100b) = 0
// [pid 18126] ioctl(6, _IOC(_IOC_NONE, 0xc4, 0xd, 0), 0xa200c) = 0
// [pid 18126] ioctl(6, _IOC(_IOC_NONE, 0xc4, 0xd, 0), 0xa300d) = 0
// strace: Process 18131 attached
// [pid 18126] ioctl(6, _IOC(_IOC_NONE, 0xc4, 0xc, 0), 0x9d007) = 0
// [pid 18126] ioctl(6, _IOC(_IOC_NONE, 0xc4, 0xc, 0), 0x9d007) = 0
// [pid 18126] ioctl(6, _IOC(_IOC_WRITE, 0xc4, 0x4, 0xc), 0x7e9dfea4) = 0
// [pid 18126] ioctl(6, _IOC(_IOC_NONE, 0xc4, 0xd, 0) <unfinished ...>
// [pid 18128] <... ioctl resumed> , 0x75efeaf4) = 1
// [pid 18126] <... ioctl resumed> , 0x9d007) = 0
// [pid 18128] ioctl(6, _IOC(_IOC_READ|_IOC_WRITE, 0xc4, 0x7, 0x14) <unfinished ...>
// [pid 18126] ioctl(6, _IOC(_IOC_NONE, 0xc4, 0xc, 0), 0x9d007) = 0
// [pid 18126] ioctl(6, _IOC(_IOC_READ|_IOC_WRITE, 0xc4, 0x8, 0x10), 0x7e9dfe80) = 13
// [pid 18126] ioctl(6, _IOC(_IOC_NONE, 0xc4, 0xd, 0), 0x9d007) = 0
// [pid 18126] ioctl(6, _IOC(_IOC_NONE, 0xc4, 0xd, 0), 0x9d007) = 0
// [pid 18126] ioctl(6, _IOC(_IOC_NONE, 0xc4, 0xc, 0), 0xa200c) = 0
// [pid 18126] ioctl(6, _IOC(_IOC_WRITE, 0xc4, 0x4, 0xc), 0x7e9dff4c) = 0
// [pid 18126] ioctl(6, _IOC(_IOC_READ|_IOC_WRITE, 0xc4, 0x8, 0x10) <unfinished ...>
// [pid 18128] <... ioctl resumed> , 0x75efeaf4) = 1
// [pid 18126] <... ioctl resumed> , 0x7e9dff08) = 4

use core::ffi::c_void;
use core::mem::size_of;
use log::debug;
use nix::{ioctl_none, ioctl_readwrite, ioctl_write_ptr};
use num_enum::TryFromPrimitive;

// The ioctls for vchiq incorrectly take an input when specifying ioctl_none,
// so we create our own ioctl_none function that's equivalent to ioctl_write_int.
macro_rules! ioctl_none_write_int {
    ($(#[$attr:meta])* $name:ident, $ioty:expr, $nr:expr) => (
        $(#[$attr])*
        pub unsafe fn $name(fd: libc::c_int,
                            data:usize)
                            -> nix::Result<libc::c_int> {
            nix::convert_ioctl_res!(libc::ioctl(fd, nix::request_code_none!($ioty, $nr) as nix::sys::ioctl::ioctl_num_type, data))
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
ioctl_none!(close_delivered, VCHIQ_IOC_MAGIC, 17);

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
        debug!("{:?} {} {:?}", reason, handle, data);
        let closure: &mut F = unsafe { &mut *(data as *mut F) };
        (*closure)(reason, unsafe { header.as_ref() }, handle)
    }

    (trampoline::<F>, closure as *mut F as *mut c_void)
}

#[repr(C)]
#[derive(Debug)]
pub struct ServiceParams {
    pub fourcc: u32,
    pub callback: Option<Callback>,
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
#[derive(Debug)]
pub struct CompletionData {
    pub reason: Reason,
    pub header: *mut Header,
    pub service_userdata: *mut c_void,
    pub bulk_userdata: *mut c_void,
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
