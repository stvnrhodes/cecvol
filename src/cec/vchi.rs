// VideoCore Hardware Interface
//
// Inspired by the following files:
//
// https://github.com/raspberrypi/userland/blob/master/interface/vchiq_arm/vchiq_lib.c
// https://github.com/raspberrypi/userland/blob/master/interface/vmcs_host/vc_vchi_cecservice.c

use crate::cec::vchiq_ioctl;
use crate::cec::vchiq_ioctl::{ServiceHandle, VersionNum};
use crate::cec::{CECCommand, CECConnection, CECError, CECMessage, LogicalAddress};
use array_init::array_init;
use core::ffi::c_void;
use lazy_static::lazy_static;
use log::{debug, info, warn};
use nix::errno::Errno;
use num_enum::TryFromPrimitive;
use std::convert::{TryFrom, TryInto};
use std::fs::{File, OpenOptions};
use std::mem::{size_of, zeroed};
use std::os::raw::c_int;
use std::os::unix::io::{AsRawFd, RawFd};
use std::ptr;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

const DEV_VCHIQ: &str = "/dev/vchiq";
const VCHIQ_SERVICE_HANDLE_INVALID: ServiceHandle = 0;
const NOTIFY_BUFFER_SIZE: usize = 1024;
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

struct VchiqIoctls {
    vchiq: File,
}
impl VchiqIoctls {
    fn fd(&self) -> RawFd {
        self.vchiq.as_raw_fd()
    }

    pub fn get_config(&self) -> Result<vchiq_ioctl::Config, nix::Error> {
        let mut config: vchiq_ioctl::Config = Default::default();
        let mut arg = vchiq_ioctl::GetConfig {
            config_size: size_of::<vchiq_ioctl::Config>(),
            pconfig: &mut config,
        };
        retry(|| unsafe { vchiq_ioctl::get_config(self.fd(), &mut arg) })?;
        Ok(config)
    }

    pub fn dequeue_message(
        &mut self,
        handle: ServiceHandle,
        buffer: &mut [u8],
    ) -> Result<usize, nix::Error> {
        let mut dequeue = vchiq_ioctl::DequeueMessage {
            handle: handle,
            blocking: 0,
            bufsize: buffer.len() as u32,
            buf: buffer.as_mut_ptr() as *mut c_void,
        };
        retry(|| unsafe { vchiq_ioctl::dequeue_message(self.fd(), &mut dequeue) })
            .map(|n| n as usize)
    }

    pub fn create_service(
        &mut self,
        client: FourCC,
        signal: Arc<Signal>,
        vc_version: VersionNum,
    ) -> Result<ServiceHandle, nix::Error> {
        let userdata = Box::new(ServiceUserdata {
            signal: &signal,
            handle: 0,
        });
        let mut service = vchiq_ioctl::CreateService {
            service_params: vchiq_ioctl::ServiceParams {
                fourcc: client.into(),
                callback: None,
                userdata: Box::into_raw(userdata) as *mut c_void,
                version: vc_version,
                version_min: vc_version,
            },
            is_open: 1,
            is_vchi: 1,
            handle: VCHIQ_SERVICE_HANDLE_INVALID, /* OUT */
        };

        retry(|| unsafe { vchiq_ioctl::create_service(self.fd(), &mut service) })?;
        let mut recovered_userdata = service.service_params.userdata as *mut ServiceUserdata;
        unsafe { (*recovered_userdata).handle = service.handle };
        retry(|| unsafe { vchiq_ioctl::release_service(self.fd(), service.handle as usize) })?;
        Ok(service.handle)
    }

    pub fn lib_version(&mut self, version: VersionNum) -> Result<(), nix::Error> {
        retry(|| unsafe { vchiq_ioctl::lib_version(self.fd(), version as usize) }).map(|_| ())
    }

    pub fn connect(&mut self) -> Result<(), nix::Error> {
        retry(|| unsafe { vchiq_ioctl::connect(self.fd(), 0) }).map(|_| ())
    }

    pub fn queue_message(
        &mut self,
        msg: vchiq_ioctl::QueueMessage,
    ) -> Result<vchiq_ioctl::Status, nix::Error> {
        let code = retry(|| unsafe { vchiq_ioctl::queue_message(self.fd(), &msg) })?;
        Ok(vchiq_ioctl::Status::try_from(code as i8).unwrap_or(vchiq_ioctl::Status::Error))
    }

    pub fn using_service<F>(&mut self, handle: ServiceHandle, func: F) -> Result<(), nix::Error>
    where
        F: FnOnce(&mut Self) -> Result<(), nix::Error>,
    {
        retry(|| unsafe { vchiq_ioctl::use_service(self.fd(), handle as usize) })?;
        func(self)?;
        retry(|| unsafe { vchiq_ioctl::release_service(self.fd(), handle as usize) })?;
        Ok(())
    }
    // TODO(stvn): Implement await_completion through returning a closure?
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
#[repr(u16)]
#[derive(Copy, Clone, Debug, PartialEq, TryFromPrimitive)]
#[allow(dead_code)]
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
#[repr(u16)]
#[derive(Copy, Clone, Debug, PartialEq, TryFromPrimitive)]
#[allow(dead_code)]
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

// CEC service commands
#[repr(u32)]
#[allow(dead_code)]
enum CECServiceCommand {
    RegisterCmd = 0,
    RegisterALl,
    DeregisterCmd,
    DeregisterAll,
    SendMsg,
    GetLogicalAddr,
    AllocLogicalAddr,
    ReleaseLogicalAddr,
    GetTopology,
    SetVendorId,
    SetOSDName,
    GetPhysicalAddr,
    GetVendorId,
    //The following 3 commands are used when CEC middleware is
    //running in passive mode (i.e. it does not allocate
    //logical address automatically)
    PollAddr,
    SetLogicalAddr,
    AddDevice,
    SetPassive,
}

#[derive(Debug)]
struct Signal(Mutex<bool>, Condvar);
impl Signal {
    fn new() -> Arc<Signal> {
        Arc::new(Signal(Mutex::new(false), Condvar::new()))
    }
    fn notify_one(&self) {
        let Signal(lock, cvar) = self;
        let mut data_available = lock.lock().unwrap();
        *data_available = true;
        cvar.notify_one();
    }
    fn wait_for_event(&self) {
        let Signal(lock, cvar) = self;
        let mut data_available = lock.lock().unwrap();
        while !*data_available {
            data_available = cvar.wait(data_available).unwrap();
        }
        *data_available = false;
    }
}

struct MsgbufArray([*mut c_void; 8]);
impl MsgbufArray {
    fn new() -> MsgbufArray {
        MsgbufArray(array_init(|_: usize| ptr::null_mut()))
    }
    fn replenish(&mut self, remaining_available: usize) -> usize {
        if remaining_available < self.len() {
            debug!("buffers at {}, allocating more", remaining_available);
            for i in remaining_available..self.len() {
                let MsgbufArray(arr) = self;
                arr[i] = unsafe { libc::malloc(MSGBUF_SIZE) };
            }
        }
        self.len()
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
            if !ptr.is_null() {
                unsafe { libc::free(*ptr) }
            }
        }
    }
}

#[allow(dead_code)]
pub struct HardwareInterface {
    // File for directly interfacing with hardware.
    vchiq: Arc<Mutex<VchiqIoctls>>,

    // Handles for all registered services.
    tvservice_client_handle: ServiceHandle,
    tvservice_notify_handle: ServiceHandle,
    cec_client_handle: ServiceHandle,
    cec_notify_handle: ServiceHandle,
    // Signals to use for confirming message send.
    tvservice_client_signal: Arc<Signal>,
    cec_client_signal: Arc<Signal>,

    // Threads to use for handling incoming messages.
    tvservice_notify_thread: thread::JoinHandle<()>,
    cec_notify_thread: thread::JoinHandle<()>,
    completion_thread: thread::JoinHandle<()>,
}

#[derive(thiserror::Error, Debug)]
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

#[repr(C)]
struct SendMsgParam {
    follower: u32,
    length: u32,
    payload: [u8; 16], //max. 15 bytes padded to 16
    is_reply: u32,
}
impl SendMsgParam {
    pub fn new(follower: LogicalAddress, payload: &[u8], is_reply: bool) -> SendMsgParam {
        let mut internal_payload = [0; 16];
        internal_payload[0..payload.len()].copy_from_slice(payload);

        SendMsgParam {
            follower: (follower as u32).to_be(),
            length: (payload.len() as u32).to_be(),
            payload: internal_payload,
            is_reply: (is_reply as u32).to_be(),
        }
    }
}

#[derive(Debug)]
struct ServiceUserdata<'a> {
    signal: &'a Signal,
    handle: ServiceHandle,
}

impl HardwareInterface {
    // Initialise the CEC service for use.
    pub fn init() -> Result<HardwareInterface, CreationError> {
        // Ensure that only a single HardwareInterface exists.
        let mut already_initialized = INITIALIZED.lock().unwrap();
        if *already_initialized {
            return Err(CreationError::AlreadyInitialized);
        }
        *already_initialized = true;

        // Open the /dev/vchiq file and set up the correct library version
        let vchiq = Arc::new(Mutex::new(VchiqIoctls {
            vchiq: OpenOptions::new().read(true).write(true).open(DEV_VCHIQ)?,
        }));
        let config = vchiq.lock().unwrap().get_config()?;
        if config.version < VCHIQ_VERSION_MIN || config.version_min > VCHIQ_VERSION {
            return Err(CreationError::CouldNotRetrieveDriverVersion);
        }
        debug!("vchiq config: {:?}", config);
        if config.version >= VCHIQ_VERSION_LIB_VERSION {
            vchiq.lock().unwrap().lib_version(VCHIQ_VERSION)?;
        }
        let use_close_delivered = config.version >= VCHIQ_VERSION_CLOSE_DELIVERED;

        // Connect and spin up a thread
        vchiq.lock().unwrap().connect()?;
        let fd = vchiq.lock().unwrap().fd();
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
                    msgbufcount: 0,
                    msgbufs: msgbufs.as_mut_ptr(),
                };

                loop {
                    // Fill up completion_data with allocated memory.
                    // This could potentionally leak memory.
                    args.msgbufcount = msgbufs.replenish(args.msgbufcount);

                    // Continually await completion until no messages are received.
                    // We intentionally avoid grabbing a mutex.
                    let cnt = retry(|| unsafe { vchiq_ioctl::await_completion(fd, &mut args) })
                        .unwrap() as usize;

                    if cnt <= 0 {
                        warn!("await_completion returned 0, no longer watching...");
                        break;
                    }
                    for completion in completion_data[..cnt].iter() {
                        // TODO(stvn): Change based on reason
                        let userdata =
                            unsafe { &mut *(completion.service_userdata as *mut ServiceUserdata) };
                        userdata.signal.notify_one();
                        if completion.reason == vchiq_ioctl::Reason::ServiceClosed
                            && use_close_delivered
                        {
                            retry(|| unsafe { vchiq_ioctl::close_delivered(fd, userdata.handle) })
                                .unwrap();
                        }
                    }
                }
            })?;

        // Initialize all the clients we intend on using.
        let tvservice_client_signal = Signal::new();
        let tvservice_client_handle = vchiq.lock().unwrap().create_service(
            TVSERVICE_CLIENT_NAME,
            tvservice_client_signal.clone(),
            VC_TVSERVICE_VER,
        )?;
        let tvservice_notify_signal = Signal::new();
        let tvservice_notify_handle = vchiq.lock().unwrap().create_service(
            TVSERVICE_NOTIFY_NAME,
            tvservice_notify_signal.clone(),
            VC_TVSERVICE_VER,
        )?;

        let cec_client_signal = Signal::new();
        let cec_client_handle = vchiq.lock().unwrap().create_service(
            CECSERVICE_CLIENT_NAME,
            cec_client_signal.clone(),
            VC_CECSERVICE_VER,
        )?;
        let cec_notify_signal = Signal::new();
        let cec_notify_handle = vchiq.lock().unwrap().create_service(
            CECSERVICE_NOTIFY_NAME,
            cec_notify_signal.clone(),
            VC_CECSERVICE_VER,
        )?;

        // Spawn notification threads now that we have the handles
        let tvservice_vchiq = Arc::clone(&vchiq);
        let tvservice_notify_thread = thread::Builder::new()
            .name("TVService Notify".into())
            .spawn(move || {
                loop {
                    // Wait for data
                    tvservice_notify_signal.wait_for_event();

                    // Grab all available data
                    loop {
                        let mut notify_buffer = [0; NOTIFY_BUFFER_SIZE];
                        let num_bytes = tvservice_vchiq
                            .lock()
                            .unwrap()
                            .dequeue_message(tvservice_notify_handle, &mut notify_buffer)
                            .unwrap();

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
                        let reason = HDMIReason::try_from(u16::from_le_bytes(
                            notify_buffer[0..2].try_into().unwrap(),
                        ));
                        let params = &notify_buffer[4..12];
                        debug!("tv_notification {:?} {:02x?}", reason, params);

                        // TODO(stvn): Add callbackS
                        if num_bytes == TVSERVICE_NOTIFY_SIZE {
                            break;
                        }
                    }
                }
            })?;
        let cec_vchiq = Arc::clone(&vchiq);
        let cec_notify_thread =
            thread::Builder::new()
                .name("CEC Notify".into())
                .spawn(move || {
                    loop {
                        // Wait for data
                        cec_notify_signal.wait_for_event();

                        // Grab all available data
                        loop {
                            let mut notify_buffer = [0; NOTIFY_BUFFER_SIZE];
                            let num_bytes = cec_vchiq
                                .lock()
                                .unwrap()
                                .dequeue_message(cec_notify_handle, &mut notify_buffer)
                                .unwrap();
                            if num_bytes < CEC_NOTIFY_SIZE {
                                warn!(
                                    "cec returned too few bytes ({}), skipping message...",
                                    num_bytes
                                );
                                break;
                            }
                            let reason_num =
                                u16::from_le_bytes(notify_buffer[0..2].try_into().unwrap());
                            let reason = CECReason::try_from(reason_num).unwrap_or(CECReason::None);
                            let params = &notify_buffer[4..20];

                            // https://github.com/raspberrypi/userland/blob/master/interface/vmcs_host/vc_cec.h#L409
                            match reason {
                                CECReason::LogicalAddr => {
                                    let logical = LogicalAddress::try_from(params[0])
                                        .unwrap_or(LogicalAddress::Unknown);
                                    let physical =
                                        u16::from_be_bytes(params[4..6].try_into().unwrap());
                                    info!("logical: {:?}, physical: {:x?}", logical, physical);
                                }
                                CECReason::Rx => {
                                    let cmd = CECCommand::from_raw(params);
                                    match cmd {
                                        Ok(c) => info!("{:?}", c),
                                        Err(_) => {
                                            info!("{:?} {:02x?}", reason, params);
                                        }
                                    }
                                }
                                _ => {
                                    info!("{:?} {:02x?}", reason, params);
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
            vchiq: vchiq,
            tvservice_client_handle: tvservice_client_handle,
            tvservice_notify_handle: tvservice_notify_handle,
            cec_client_handle: cec_client_handle,
            cec_notify_handle: cec_notify_handle,
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
}
impl CECConnection for HardwareInterface {
    fn transmit(&self, cmd: CECCommand) -> Result<(), CECError> {
        let payload = if cmd.message == CECMessage::None {
            vec![]
        } else {
            let mut p = vec![cmd.message.get_opcode() as u8];
            p.extend(cmd.message.get_parameters());
            p
        };
        let elements = [
            vchiq_ioctl::Element::new(&CECServiceCommand::SendMsg),
            vchiq_ioctl::Element::new(&SendMsgParam::new(cmd.destination, &payload, true)),
        ];
        let msg = vchiq_ioctl::QueueMessage::new(self.cec_client_handle, &elements);

        let mut vchiq = self.vchiq.lock().unwrap();
        vchiq
            .using_service(self.cec_client_handle, |vchiq| {
                // Send the command.
                let status = vchiq.queue_message(msg)?;
                match status {
                    vchiq_ioctl::Status::Success => {}
                    vchiq_ioctl::Status::Error | vchiq_ioctl::Status::Retry => {
                        warn!("failed to send command {:?}, error: {:?}", cmd, status);
                        // TODO(stvn): Return error
                        return Err(nix::Error::UnsupportedOperation);
                    }
                }

                // Wait for the command to be acknowledged. The acknowledgement
                // message is filled with zeroes.
                self.cec_client_signal.wait_for_event();
                let mut notify_buffer = [0; NOTIFY_BUFFER_SIZE];
                vchiq.dequeue_message(self.cec_client_handle, &mut notify_buffer)?;

                Ok(())
            })
            .unwrap();
        Ok(())
    }
}
