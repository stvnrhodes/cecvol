// VideoCore Hardware Interface
//
// Inspired by the following files:
//
// https://github.com/raspberrypi/userland/blob/master/interface/vchiq_arm/vchiq_lib.c
// https://github.com/raspberrypi/userland/blob/master/interface/vmcs_host/vc_vchi_cecservice.c

use crate::cec::enums::LogicalAddress;
use crate::cec::vchiq_ioctl;
use crate::cec::vchiq_ioctl::{ServiceHandle, VersionNum};
use crate::cec::{CECCommand, CECConnection, CECError};
use array_init::array_init;
use core::ffi::c_void;
use lazy_static::lazy_static;
use log::{debug, info, warn};
use nix::errno::Errno;
use num_enum::TryFromPrimitive;
use std::backtrace::Backtrace;
use std::convert::TryFrom;
use std::fs::File;
use std::mem::{size_of, zeroed};
use std::os::raw::c_int;
use std::os::unix::io::AsRawFd;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use thiserror::Error;

const DEV_VCHIQ: &str = "/dev/vchiq";
const VCHIQ_MAX_INSTANCE_SERVICES: usize = 32;
const VCHIQ_SERVICE_HANDLE_INVALID: ServiceHandle = 0;
const NOTIFY_BUFFER_SIZE: usize = 1024 / size_of::<u32>();
const SLOT_SIZE: usize = 4096;
const MAX_MSG_SIZE: usize = SLOT_SIZE - size_of::<vchiq_ioctl::Header>();
const MSGBUF_SIZE: usize = MAX_MSG_SIZE + size_of::<vchiq_ioctl::Header>();
const TVSERVICE_CLIENT_NAME: FourCC = FourCC::from_str("TVSV");
const TVSERVICE_NOTIFY_NAME: FourCC = FourCC::from_str("TVNT");
const CECSERVICE_CLIENT_NAME: FourCC = FourCC::from_str("CECS");
const CECSERVICE_NOTIFY_NAME: FourCC = FourCC::from_str("CECN");
const TVSERVICE_NOTIFY_SIZE: usize = size_of::<u32>() * 3;
const CEC_NOTIFY_SIZE: usize = size_of::<u32>() * 5;

struct FourCC([char; 4]);
impl FourCC {
    const fn from_str(s: &str) -> FourCC {
        let bytes = s.as_bytes();
        FourCC([
            bytes[0] as char,
            bytes[1] as char,
            bytes[2] as char,
            bytes[3] as char,
        ])
    }
}
impl From<u32> for FourCC {
    fn from(x: u32) -> FourCC {
        FourCC([
            ((x >> 24) & 0xFF) as u8 as char,
            ((x >> 16) & 0xFF) as u8 as char,
            ((x >> 8) & 0xFF) as u8 as char,
            (x & 0xFF) as u8 as char,
        ])
    }
}
impl Into<u32> for FourCC {
    fn into(self) -> u32 {
        let FourCC(x) = self;
        ((x[0] as u32) << 24) | ((x[1] as u32) << 16) | ((x[2] as u32) << 8) | (x[3] as u32)
    }
}

/* The version of VCHIQ - change with any non-trivial change */
const VCHIQ_VERSION: VersionNum = 8;
/* The minimum compatible version - update to match VCHIQ_VERSION with any
** incompatible change */
const VCHIQ_VERSION_MIN: VersionNum = 3;
/* The version that introduced the VCHIQ_IOC_LIB_VERSION ioctl */
const VCHIQ_VERSION_LIB_VERSION: VersionNum = 7;
/* The version that introduced the VCHIQ_IOC_CLOSE_DELIVERED ioctl */
const VCHIQ_VERSION_CLOSE_DELIVERED: VersionNum = 7;

const VC_TVSERVICE_VER: VersionNum = 1;
const VC_CECSERVICE_VER: VersionNum = 1;

lazy_static! {
    static ref INITIALIZED: Mutex<bool> = Mutex::new(false);
}

// https://rust-lang.github.io/unsafe-code-guidelines/layout/function-pointers.html

fn get_config(dev_vchiq: &File) -> Result<vchiq_ioctl::Config, nix::Error> {
    let mut config: vchiq_ioctl::Config = Default::default();
    let mut arg = vchiq_ioctl::GetConfig {
        config_size: size_of::<vchiq_ioctl::Config>(),
        pconfig: &mut config,
    };
    let fd = dev_vchiq.as_raw_fd();
    unsafe {
        retry(|| vchiq_ioctl::get_config(fd, &mut arg))?;
    }
    Ok(config)
}

fn retry<F>(mut func: F) -> nix::Result<c_int>
where
    F: FnMut() -> nix::Result<c_int>,
{
    let r = func();
    match r {
        Err(nix::Error::Sys(Errno::EINTR)) => retry(func),
        _ => r,
    }
}

/**
 * HDMI notifications (defined as a bit mask to be conveniently returned as a state),
 * make sure this does not clash with the values in vc_sdtv.h
 * SDTV notifications start at bit 16.
 * These values are returned by the TV service in a callback.
 */
#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq, TryFromPrimitive)]
enum HDMIReason {
    Unknown,
    Unplugged = 1 << 0,       /*<HDMI cable is detached */
    Attached = 1 << 1,        /*<HDMI cable is attached but not powered on */
    DVI = 1 << 2,             /*<HDMI is on but in DVI mode (no audio) */
    HDMI = 1 << 3,            /*<HDMI is on and HDMI mode is active */
    HDCPUnauth = 1 << 4,      /*<HDCP authentication is broken (e.g. Ri mismatched) or not active */
    HDCPAuth = 1 << 5,        /*<HDCP is active */
    HDCPKeyDownload = 1 << 6, /*<HDCP key download successful/fail */
    HDCPSRMDownload = 1 << 7, /*<HDCP revocation list download successful/fail */
    ChangingMode = 1 << 8,    /*<HDMI is starting to change mode, clock has not yet been set */
}

/**
 * CEC related notification
 */
#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq, TryFromPrimitive)]
enum CECReason {
    None = 0,                  //Reserved - NOT TO BE USED
    Tx = 1 << 0,               /*<A message has been transmitted */
    Rx = 1 << 1,               /*<A message has arrived (only for registered commands) */
    ButtonPressed = 1 << 2,    /*<<User Control Pressed> */
    ButtonReleased = 1 << 3,   /*<<User Control Release> */
    RemotePressed = 1 << 4,    /*<<Vendor Remote Button Down> */
    RemoteReleased = 1 << 5,   /*<<Vendor Remote Button Up> */
    LogicalAddr = 1 << 6,      /*<New logical address allocated or released */
    Topology = 1 << 7,         /*<Topology is available */
    LogicalAddrList = 1 << 15, /*<Only for passive mode, if the logical address is lost for whatever reason, this will be triggered */
}

type Signal = Arc<(Mutex<bool>, Condvar)>;

struct MsgbufArray([*mut c_void; 8]);
impl MsgbufArray {
    fn new() -> MsgbufArray {
        MsgbufArray(array_init(|_: usize| unsafe { libc::malloc(MSGBUF_SIZE) }))
    }
    fn as_mut_ptr(&mut self) -> *mut *mut c_void {
        let MsgbufArray(arr) = self;
        arr.as_mut_ptr()
    }
    fn len(&self) -> usize {
        let MsgbufArray(arr) = self;
        arr.len()
    }
}
impl Drop for MsgbufArray {
    fn drop(&mut self) {
        let MsgbufArray(arr) = self;
        for ptr in arr.iter() {
            unsafe { libc::free(*ptr) }
        }
    }
}

pub struct HardwareInterface {
    // File for directly interfacing with hardware.
    dev_vchiq: Arc<Mutex<File>>,

    // Handles for all registered services.
    tvservice_client_handle: ServiceHandle,
    tvservice_notify_handle: ServiceHandle,
    cec_client_handle: ServiceHandle,
    cec_notify_handle: ServiceHandle,
    // Signals to use for confirming message send.
    tvservice_client_signal: Signal,
    cec_client_signal: Signal,

    // Threads to use for handling incoming messages.
    tvservice_notify_thread: thread::JoinHandle<()>,
    cec_notify_thread: thread::JoinHandle<()>,
    completion_thread: thread::JoinHandle<()>,
}

#[derive(Error, Debug)]
pub enum CreationError {
    #[error("Could not retrieve driver version")]
    CouldNotRetrieveDriverVersion,
    #[error("VHCIQ was already initialized")]
    AlreadyInitialized,
    #[error("Could not open vchiq device")]
    IOError {
        #[from]
        source: std::io::Error,
        backtrace: Backtrace,
    },
    #[error("ioctl call failed")]
    IoctlError {
        #[from]
        source: nix::Error,
        backtrace: Backtrace,
    },
}

impl HardwareInterface {
    fn create_service_req(
        client: FourCC,
        signal: Signal,
        vc_version: VersionNum,
    ) -> vchiq_ioctl::CreateService {
        let (callback, userdata) =
            vchiq_ioctl::callback_closure(&mut move |reason, header, handle| {
                debug!("{:?} for {}, {:?}", reason, handle, header);
                if reason == vchiq_ioctl::Reason::MessageAvailable {
                    let (lock, cvar) = &*signal;
                    let mut data_available = lock.lock().unwrap();
                    *data_available = true;
                    cvar.notify_one();
                }
                vchiq_ioctl::Status::Success
            });

        vchiq_ioctl::CreateService {
            service_params: vchiq_ioctl::ServiceParams {
                fourcc: client.into(),
                callback: Some(callback),
                userdata: userdata,
                version: vc_version,
                version_min: vc_version,
            },
            is_open: 1,
            is_vchi: 1,
            handle: VCHIQ_SERVICE_HANDLE_INVALID, /* OUT */
        }
    }

    // Initialise the CEC service for use.
    pub fn init() -> Result<HardwareInterface, CreationError> {
        // Ensure that only a single HardwareInterface exists.
        let mut already_initialized = INITIALIZED.lock().unwrap();
        if *already_initialized {
            return Err(CreationError::AlreadyInitialized);
        }
        *already_initialized = true;

        // Open the /dev/vchiq file and set up the correct library version
        let dev_vchiq = File::open(DEV_VCHIQ)?;
        let fd = dev_vchiq.as_raw_fd();
        let config = get_config(&dev_vchiq)?;
        if config.version < VCHIQ_VERSION_MIN || config.version_min > VCHIQ_VERSION {
            return Err(CreationError::CouldNotRetrieveDriverVersion);
        }
        if config.version >= VCHIQ_VERSION_LIB_VERSION {
            unsafe { retry(|| vchiq_ioctl::lib_version(fd, VCHIQ_VERSION as libc::c_ulong))? };
        }
        let use_close_delivered = config.version >= VCHIQ_VERSION_CLOSE_DELIVERED;

        // Connect and spin up a thread
        unsafe { retry(|| vchiq_ioctl::connect(fd))? };
        let vchiq = Arc::new(Mutex::new(dev_vchiq));
        let completion_thread = thread::Builder::new()
            .name("VCHIQ completion".into())
            .spawn(move || {
                // Set up memory for ioctl output
                let mut completion_data: [vchiq_ioctl::CompletionData; 8] =
                    array_init(|_: usize| unsafe { zeroed() });
                let mut msgbufs = MsgbufArray::new();
                let mut args = vchiq_ioctl::AwaitCompletion {
                    count: completion_data.len(),
                    buf: completion_data.as_mut_ptr(),
                    msgbufsize: msgbufs.len(),
                    msgbufs: msgbufs.as_mut_ptr(),
                };

                loop {
                    // Continually await completion until no messages are received.
                    // We intentionally avoid grabbing a mutex.
                    let cnt = retry(|| unsafe { vchiq_ioctl::await_completion(fd, &mut args) })
                        .unwrap() as usize;

                    if cnt <= 0 {
                        warn!("await_completion returned 0, no longer watching...");
                        break;
                    }
                    for completion in completion_data[..cnt].iter() {
                        debug!("completion: {:?}", completion);
                        if completion.reason == vchiq_ioctl::Reason::ServiceClosed
                            && use_close_delivered
                        {
                            // TODO(stvn): Deliver service handle in opaque data
                            // let file = vchiq_clone.lock().unwrap();
                            // retry(|| unsafe {
                            //     vchiq_ioctl::close_delivered(
                            //         file.as_raw_fd(),
                            //         service.handle as libc::c_ulong,
                            //     )
                            // })
                            // .unwrap();
                        }
                    }
                }
            })?;

        // Initialize all the clients we intend on using.
        let tvservice_client_signal: Signal = Arc::new((Mutex::new(false), Condvar::new()));
        let mut tvservice_client = Self::create_service_req(
            TVSERVICE_CLIENT_NAME,
            tvservice_client_signal.clone(),
            VC_TVSERVICE_VER,
        );
        let tvservice_notify_signal: Signal = Arc::new((Mutex::new(false), Condvar::new()));
        let mut tvservice_notify = Self::create_service_req(
            TVSERVICE_NOTIFY_NAME,
            tvservice_notify_signal.clone(),
            VC_TVSERVICE_VER,
        );

        let cec_client_signal: Signal = Arc::new((Mutex::new(false), Condvar::new()));
        let mut cec_client = Self::create_service_req(
            CECSERVICE_CLIENT_NAME,
            cec_client_signal.clone(),
            VC_CECSERVICE_VER,
        );
        let cec_notify_signal: Signal = Arc::new((Mutex::new(false), Condvar::new()));
        let mut cec_notify = Self::create_service_req(
            CECSERVICE_NOTIFY_NAME,
            cec_notify_signal.clone(),
            VC_CECSERVICE_VER,
        );

        let file = vchiq.lock().unwrap();
        let fd = file.as_raw_fd();
        retry(|| unsafe { vchiq_ioctl::create_service(fd, &mut tvservice_client) })?;
        retry(|| unsafe { vchiq_ioctl::create_service(fd, &mut tvservice_notify) })?;
        retry(|| unsafe { vchiq_ioctl::create_service(fd, &mut cec_client) })?;
        retry(|| unsafe { vchiq_ioctl::create_service(fd, &mut cec_notify) })?;
        drop(file);

        // Spawn notification threads now that we have the handles
        let tvservice_notify_handle = tvservice_notify.handle;
        let tvservice_vchiq = Arc::clone(&vchiq);
        let tvservice_notify_thread = thread::Builder::new()
            .name("TVService Notify".into())
            .spawn(move || {
                let (lock, cvar) = &*tvservice_notify_signal;
                loop {
                    // Wait for data
                    let mut data_available = lock.lock().unwrap();
                    while !*data_available {
                        data_available = cvar.wait(data_available).unwrap();
                    }

                    // Grab all available data
                    loop {
                        let mut notify_buffer: [u32; NOTIFY_BUFFER_SIZE] = [0; NOTIFY_BUFFER_SIZE];
                        let mut dequeue = vchiq_ioctl::DequeueMessage {
                            handle: tvservice_notify_handle,
                            blocking: 0,
                            bufsize: size_of::<[u32; NOTIFY_BUFFER_SIZE]>() as u32,
                            buf: notify_buffer.as_mut_ptr() as *mut c_void,
                        };
                        let file = tvservice_vchiq.lock().unwrap();
                        let num_bytes = retry(|| unsafe {
                            vchiq_ioctl::dequeue_message(file.as_raw_fd(), &mut dequeue)
                        })
                        .unwrap() as usize;
                        drop(file);

                        if num_bytes < TVSERVICE_NOTIFY_SIZE {
                            warn!(
                                "tvservice returned too few bytes ({}), stopping thread...",
                                num_bytes
                            );
                            return ();
                        }

                        // Check what notification it is and update ourselves
                        // accordingly before notifying the host app
                        // All notifications are of format: reason, param1, param2
                        // (all 32-bit unsigned int)
                        let reason = HDMIReason::try_from(u32::from_be(notify_buffer[0]));
                        let param1 = u32::from_be(notify_buffer[1]);
                        let param2 = u32::from_be(notify_buffer[2]);
                        debug!("tv_notification {:?} {} {}", reason, param1, param2);

                        // TODO(stvn): Add callbackS
                        if num_bytes == TVSERVICE_NOTIFY_SIZE {
                            break;
                        }
                    }
                }
            })?;
        let cec_notify_handle = cec_notify.handle;
        let cec_vchiq = Arc::clone(&vchiq);
        let cec_notify_thread =
            thread::Builder::new()
                .name("CEC Notify".into())
                .spawn(move || {
                    let (lock, cvar) = &*cec_notify_signal;
                    loop {
                        // Wait for data
                        let mut data_available = lock.lock().unwrap();
                        while !*data_available {
                            data_available = cvar.wait(data_available).unwrap();
                        }

                        // Grab all available data
                        loop {
                            let mut notify_buffer: [u32; NOTIFY_BUFFER_SIZE] =
                                [0; NOTIFY_BUFFER_SIZE];
                            let mut dequeue = vchiq_ioctl::DequeueMessage {
                                handle: cec_notify_handle,
                                blocking: 0,
                                bufsize: size_of::<[u32; NOTIFY_BUFFER_SIZE]>() as u32,
                                buf: notify_buffer.as_mut_ptr() as *mut c_void,
                            };
                            let file = cec_vchiq.lock().unwrap();
                            let num_bytes = retry(|| unsafe {
                                vchiq_ioctl::dequeue_message(file.as_raw_fd(), &mut dequeue)
                            })
                            .unwrap() as usize;
                            drop(file);
                            if num_bytes < CEC_NOTIFY_SIZE {
                                warn!(
                                    "cec returned too few bytes ({}), skipping message...",
                                    num_bytes
                                );
                                break;
                            }
                            let reason = CECReason::try_from(u32::from_be(notify_buffer[0]))
                                .unwrap_or(CECReason::None);
                            let param1 = u32::from_be(notify_buffer[1]);
                            let param2 = u32::from_be(notify_buffer[2]);
                            let param3 = u32::from_be(notify_buffer[3]);
                            let param4 = u32::from_be(notify_buffer[4]);
                            // https://github.com/raspberrypi/userland/blob/master/interface/vmcs_host/vc_cec.h#L409
                            match reason {
                                CECReason::LogicalAddr => {
                                    let logical = LogicalAddress::try_from(param1 as i8)
                                        .unwrap_or(LogicalAddress::Unknown);
                                    let physical = (param2 & 0xffff) as u16;
                                    info!("logical: {:?}, physical: {:x?}", logical, physical);
                                }
                                _ => {
                                    info!(
                                        "{:?} {:x?}{:x?}{:x?}{:x?}",
                                        reason, param1, param2, param3, param4
                                    );
                                }
                            }
                            // TODO(stvn): Add callbacks
                            if num_bytes == CEC_NOTIFY_SIZE {
                                break;
                            }
                        }
                    }
                })?;

        Ok(HardwareInterface {
            dev_vchiq: vchiq,
            tvservice_client_handle: tvservice_client.handle,
            tvservice_notify_handle: tvservice_notify_handle,
            cec_client_handle: cec_client.handle,
            cec_notify_handle: cec_notify.handle,
            tvservice_client_signal: tvservice_client_signal,
            tvservice_notify_thread: tvservice_notify_thread,
            cec_client_signal: cec_client_signal,
            cec_notify_thread: cec_notify_thread,
            completion_thread: completion_thread,
        })
    }

    // Starts the command service on each connection, causing INIT messages to
    // be pinged back and forth

    // if poll(LA same), vc_cec_release_logical_address and disable callbacks
    // vc_cec_set_logical_address
    // vc_cec_get_physical_address

    // vc_cec_set_passive
    // vc_cec_register_callback
    //vc_tv_register_callback

    //vc_cec_poll_address
    // handle POLL (msg like '11') in a special way - the way it was
    // originally designed by BCM, expected to happen and documented
    // in API docs (/opt/vc/includes)
    // due to often (more than 20% test cases - CEC bus with 8 devices)
    // irregularities on returned status, repeat until we get SAME
    // result twice in a row

    // vc_cec_send_message2
    pub fn send_cec_message(
        _follower: LogicalAddress,
        _payload: &[u8],
        _is_reply: bool,
    ) -> anyhow::Result<()> {
        // check length under CEC_MAX_XMIT_LENGTH
        // change numbers to bigendian
        // shove into CEC_SEND_MSG_PARAM_T
        // send with VC_CEC_SEND_MSG

        // vchi_msg_queuev with vector of comand and VCHI_FLAGS_BLOCK_UNTIL_QUEUED
        // if has_reply, do cecservice_wait_for_reply
        //   vchi_msg_dequeue
        //  vcos_event_wait(&cecservice_message_available_event) until read>0

        Ok(())
    }
    fn msg_queuev(&self) {
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
impl CECConnection for HardwareInterface {
    fn transmit(&self, cmd: CECCommand) -> Result<(), CECError> {
        Ok(())
    }
}
