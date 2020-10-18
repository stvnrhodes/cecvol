// VideoCore Hardware Interface
//
// Inspired by the following files:
//
// https://github.com/raspberrypi/userland/blob/master/interface/vchiq_arm/vchiq_lib.c
// https://github.com/raspberrypi/userland/blob/master/interface/vmcs_host/vc_vchi_cecservice.c

use crate::cec::vchiq_ioctl;
use crate::cec::vchiq_ioctl::{Element, ServiceHandle, VersionNum};
use crate::cec::{
    CECCommand, CECConnection, CECError, DeviceType, LogicalAddress, PhysicalAddress,
};
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
const OSD_NAME_LENGTH: usize = 14;

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

    pub fn use_service(&mut self, handle: ServiceHandle) -> Result<(), nix::Error> {
        retry(|| unsafe { vchiq_ioctl::use_service(self.fd(), handle as usize) }).map(|_| ())
    }

    pub fn release_service(&mut self, handle: ServiceHandle) -> Result<(), nix::Error> {
        retry(|| unsafe { vchiq_ioctl::release_service(self.fd(), handle as usize) }).map(|_| ())
    }

    pub fn close_delivered(&mut self, handle: ServiceHandle) -> Result<(), nix::Error> {
        retry(|| unsafe { vchiq_ioctl::close_delivered(self.fd(), handle as usize) }).map(|_| ())
    }

    pub fn using_service<F, E>(&mut self, handle: ServiceHandle, func: F) -> Result<(), E>
    where
        F: FnOnce(&mut Self) -> Result<(), E>,
        E: std::convert::From<nix::Error>,
    {
        self.use_service(handle)?;
        func(self)?;
        self.release_service(handle)?;
        Ok(())
    }

    pub fn await_completion_fn(
        &self,
    ) -> impl Fn(&mut vchiq_ioctl::AwaitCompletion) -> Result<usize, nix::Error> {
        let fd = self.fd();
        Box::new(move |args: &mut vchiq_ioctl::AwaitCompletion| {
            Ok(retry(|| unsafe { vchiq_ioctl::await_completion(fd, args) })? as usize)
        })
    }
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
    LogicalAddrLost = 1 << 15, /*<Only for passive mode, if the logical address is lost for whatever reason, this will be triggered */
}

// CEC service commands
#[repr(u32)]
#[allow(dead_code)]
enum CECServiceCommand {
    RegisterCmd = 0,
    RegisterAll,
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

#[derive(thiserror::Error, Debug)]
pub enum ServiceError {
    #[error("No acknowledgement")]
    NoAck,
    #[error("In the process of shutting down")]
    Shutdown,
    #[error("Block is busy")]
    Busy,
    #[error("No logical address")]
    NoLogicalAddr,
    #[error("No physical address")]
    NoPhysicalAddr,
    #[error("NoTopology")]
    NoTopology,
    #[error("Invalid follower")]
    InvalidFollower,
    #[error("Invalid arguments")]
    InvalidArgument,
    #[error("No status returned")]
    MissingStatus,
    #[error("Error when queuing message")]
    VchiqError,
    #[error("Retriable error when queuing message")]
    RetryError,
    #[error("Invalid logical address")]
    LogicalAddr(#[from] num_enum::TryFromPrimitiveError<LogicalAddress>),
    #[error("Bad slice size")]
    BadSliceSize(#[from] std::array::TryFromSliceError),
    #[error("Bad ioctl call")]
    NixError(#[from] nix::Error),
}
impl ServiceError {
    fn from_ioctl_return_value(val: u8) -> Result<(), Self> {
        match val {
            1 => Err(Self::NoAck),
            2 => Err(Self::Shutdown),
            3 => Err(Self::Busy),
            4 => Err(Self::NoLogicalAddr),
            5 => Err(Self::NoPhysicalAddr),
            6 => Err(Self::NoTopology),
            7 => Err(Self::InvalidFollower),
            8 => Err(Self::InvalidArgument),
            _ => Ok(()),
        }
    }
    fn from_vchiq_status(status: vchiq_ioctl::Status) -> Result<(), Self> {
        match status {
            vchiq_ioctl::Status::Error => Err(Self::VchiqError),
            vchiq_ioctl::Status::Retry => Err(Self::RetryError),
            vchiq_ioctl::Status::Success => Ok(()),
        }
    }
}
impl Into<CECError> for ServiceError {
    fn into(self) -> CECError {
        CECError::Other(Box::new(self))
    }
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
            follower: (follower as u32).to_le(),
            length: (payload.len() as u32).to_le(),
            payload: internal_payload,
            is_reply: (is_reply as u32).to_le(),
        }
    }
}

type MessageCallback = Arc<Mutex<Option<Box<dyn FnMut(&CECCommand) + Send>>>>;

#[derive(Debug)]
struct ServiceUserdata<'a> {
    signal: &'a Signal,
    handle: ServiceHandle,
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

    // Callbacks to use for responding to incoming messages
    cec_rx_callback: MessageCallback,
    cec_tx_callback: MessageCallback,
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
        let vchiq_completion = vchiq.clone();
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

                let await_completion = vchiq_completion.lock().unwrap().await_completion_fn();
                loop {
                    // Fill up message buffer with allocated memory.
                    // This could potentionally leak memory.
                    args.msgbufcount = msgbufs.replenish(args.msgbufcount);
                    let size = await_completion(&mut args).unwrap();

                    for completion in completion_data[..size].iter() {
                        match completion.reason {
                            vchiq_ioctl::Reason::MessageAvailable
                            | vchiq_ioctl::Reason::ServiceClosed => {
                                let userdata = unsafe {
                                    &mut *(completion.service_userdata as *mut ServiceUserdata)
                                };
                                userdata.signal.notify_one();
                                if completion.reason == vchiq_ioctl::Reason::ServiceClosed
                                    && use_close_delivered
                                {
                                    vchiq_completion
                                        .lock()
                                        .unwrap()
                                        .close_delivered(userdata.handle)
                                        .unwrap();
                                }
                            }
                            _ => {
                                debug!("{:?}", completion.reason);
                            }
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
        let tvservice_vchiq = vchiq.clone();
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

                        // TODO(stvn): Add callbacks
                        if num_bytes == TVSERVICE_NOTIFY_SIZE {
                            break;
                        }
                    }
                }
            })?;
        let cec_vchiq = vchiq.clone();
        let cec_rx_callback: MessageCallback = Arc::new(Mutex::new(None));
        let cec_rx_callback_copy = cec_rx_callback.clone();
        let cec_tx_callback: MessageCallback = Arc::new(Mutex::new(None));
        let cec_tx_callback_copy = cec_tx_callback.clone();
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
                            let msg_size = notify_buffer[2] as usize;
                            let params = &notify_buffer[4..4 + msg_size];

                            match reason {
                                CECReason::LogicalAddr => {
                                    let logical = LogicalAddress::try_from(params[0])
                                        .unwrap_or(LogicalAddress::Broadcast);
                                    let physical =
                                        u16::from_be_bytes(params[4..6].try_into().unwrap());
                                    info!("logical: {:?}, physical: {:x?}", logical, physical);
                                }
                                CECReason::Rx => match CECCommand::from_raw(params) {
                                    Ok(cmd) => match &mut *cec_rx_callback.lock().unwrap() {
                                        Some(func) => func(&cmd),
                                        None => {
                                            debug!("{:?} {:x?}", reason, cmd);
                                        }
                                    },
                                    Err(_) => {
                                        info!("{:?} {:02x?}", reason, params);
                                    }
                                },
                                CECReason::Tx => match CECCommand::from_raw(params) {
                                    Ok(cmd) => match &mut *cec_tx_callback.lock().unwrap() {
                                        Some(func) => func(&cmd),
                                        None => {
                                            debug!("{:?} {:x?}", reason, cmd);
                                        }
                                    },
                                    Err(_) => {
                                        info!("{:?} {:02x?}", reason, params);
                                    }
                                },
                                CECReason::ButtonPressed
                                | CECReason::ButtonReleased
                                | CECReason::RemotePressed
                                | CECReason::RemoteReleased => match CECCommand::from_raw(params) {
                                    Ok(c) => info!("{:?} {:x?}", reason, c),
                                    Err(_) => {
                                        info!("{:?} {:02x?}", reason, params);
                                    }
                                },
                                CECReason::Topology => {
                                    info!("devices present: {:02x?}", &params[0..2])
                                }
                                CECReason::LogicalAddrLost => {
                                    let logical = LogicalAddress::try_from(params[0])
                                        .unwrap_or(LogicalAddress::Broadcast);
                                    let physical =
                                        u16::from_be_bytes(params[4..6].try_into().unwrap());
                                    info!(
                                        "lost addr, last logical: {:?}, physical: {:x?}",
                                        logical, physical
                                    );
                                }
                                CECReason::None => {
                                    warn!(
                                        "unknown cec notification: {:02x?}",
                                        &notify_buffer[..20]
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
            cec_rx_callback: cec_rx_callback_copy,
            cec_tx_callback: cec_tx_callback_copy,
        })
    }

    fn send_cec_command_with_reply(&self, elements: &[Element]) -> Result<Vec<u8>, ServiceError> {
        let mut vec = vec![];
        self.vchiq
            .lock()
            .unwrap()
            .using_service(self.cec_client_handle, |vchiq| {
                // Send the command.
                let msg = vchiq_ioctl::QueueMessage::new(self.cec_client_handle, elements);
                ServiceError::from_vchiq_status(vchiq.queue_message(msg)?)?;

                // Wait for the command to be acknowledged.
                self.cec_client_signal.wait_for_event();
                let mut notify_buffer = [0; NOTIFY_BUFFER_SIZE];
                let num_bytes =
                    vchiq.dequeue_message(self.cec_client_handle, &mut notify_buffer)?;
                if num_bytes < 1 {
                    Err(ServiceError::MissingStatus)
                } else {
                    vec = notify_buffer[0..num_bytes].to_vec();
                    Ok(())
                }
            })
            .map(|_| vec)
    }

    fn send_cec_command(&self, elements: &[Element]) -> Result<(), ServiceError> {
        match self.send_cec_command_with_reply(elements) {
            Ok(s) => ServiceError::from_ioctl_return_value(s[0]),
            Err(e) => Err(e),
        }
    }

    fn send_cec_command_without_reply(&self, elements: &[Element]) -> Result<(), ServiceError> {
        self.vchiq
            .lock()
            .unwrap()
            .using_service(self.cec_client_handle, |vchiq| {
                // Send the command. We don't expect any acknowledgement.
                let msg = vchiq_ioctl::QueueMessage::new(self.cec_client_handle, elements);
                ServiceError::from_vchiq_status(vchiq.queue_message(msg)?)?;
                Ok(())
            })
    }

    pub fn get_logical_addr(&self) -> Result<LogicalAddress, ServiceError> {
        let elems = &[Element::new(&CECServiceCommand::GetLogicalAddr)];
        let resp = self.send_cec_command_with_reply(elems)?;
        LogicalAddress::try_from(resp[0] & 0xf).map_err(|e| e.into())
    }

    pub fn get_physical_addr(&self) -> Result<PhysicalAddress, ServiceError> {
        let elems = &[Element::new(&CECServiceCommand::GetPhysicalAddr)];
        let resp = self.send_cec_command_with_reply(elems)?;
        Ok(u16::from_le_bytes(resp[0..2].try_into()?))
    }

    pub fn alloc_logical_addr(&self) -> Result<(), ServiceError> {
        self.send_cec_command_without_reply(&[Element::new(&CECServiceCommand::AllocLogicalAddr)])
    }

    #[allow(dead_code)]
    pub fn release_logical_address(&self) -> Result<(), ServiceError> {
        self.send_cec_command_without_reply(&[Element::new(&CECServiceCommand::ReleaseLogicalAddr)])
    }

    pub fn set_vendor_id(&self, vendor_id: u32) -> Result<(), ServiceError> {
        let id_bytes = vendor_id.to_le_bytes();
        self.send_cec_command_without_reply(&[
            Element::new(&CECServiceCommand::SetVendorId),
            Element::new(&id_bytes),
        ])
    }

    pub fn set_osd_name(&self, osd_name: &str) -> Result<(), ServiceError> {
        let mut osd_bytes = [0; OSD_NAME_LENGTH];
        osd_bytes[..osd_name.len()].copy_from_slice(osd_name.as_bytes());
        self.send_cec_command_without_reply(&[
            Element::new(&CECServiceCommand::SetOSDName),
            Element::new(&osd_bytes),
        ])
    }

    #[allow(dead_code)]
    pub fn get_vendor_id(&self, addr: LogicalAddress) -> Result<u32, ServiceError> {
        let addr_bytes = (addr as u32).to_le_bytes();
        let resp = self.send_cec_command_with_reply(&[
            Element::new(&CECServiceCommand::GetVendorId),
            Element::new(&addr_bytes),
        ])?;
        Ok(u32::from_le_bytes(resp[..4].try_into()?))
    }

    /// Sets and polls a particular address to find out its availability in the
    /// CEC network.
    ///
    /// Only available when CEC is running in passive mode. The host can
    /// only call this function during logical address allocation stage.
    /// address is free if error code is VC_CEC_ERROR_NO_ACK
    #[allow(dead_code)]
    pub fn poll_address(&self, addr: LogicalAddress) -> Result<(), ServiceError> {
        let addr_bytes = (addr as u32).to_le_bytes();
        self.send_cec_command(&[
            Element::new(&CECServiceCommand::PollAddr),
            Element::new(&addr_bytes),
        ])
    }

    /// Sets the logical address, device type and vendor ID to be in use.
    ///
    /// Only available when CEC is running in passive mode. It is the
    /// responsibility of the host to make sure the logical address
    /// is actually free (see vc_cec_poll_address). Physical address used
    /// will be what is read from EDID and cannot be set.
    #[allow(dead_code)]
    pub fn set_logical_address(
        &self,
        addr: LogicalAddress,
        device: DeviceType,
        vendor_id: u32,
    ) -> Result<(), ServiceError> {
        let params = [
            (addr as u32).to_le(),
            (device as u32).to_le(),
            vendor_id.to_le(),
        ];
        self.send_cec_command(&[
            Element::new(&CECServiceCommand::SetLogicalAddr),
            Element::new(&params),
        ])
    }

    #[allow(dead_code)]
    pub fn set_passive(&self, enabled: bool) -> Result<(), ServiceError> {
        let param = (enabled as u32).to_le();
        self.send_cec_command(&[
            Element::new(&CECServiceCommand::SetLogicalAddr),
            Element::new(&param),
        ])
    }
}
impl CECConnection for HardwareInterface {
    fn transmit(&self, cmd: CECCommand) -> Result<(), CECError> {
        self.send_cec_command(&[
            Element::new(&CECServiceCommand::SendMsg),
            Element::new(&SendMsgParam::new(
                cmd.destination,
                &cmd.message.payload(),
                true,
            )),
        ])
        .map_err(|e| e.into())
    }

    fn get_logical_address(&self) -> Result<LogicalAddress, CECError> {
        self.get_logical_addr().map_err(|e| e.into())
    }

    fn get_physical_address(&self) -> Result<PhysicalAddress, CECError> {
        self.get_physical_addr().map_err(|e| e.into())
    }

    fn set_rx_callback(&self, func: Box<dyn FnMut(&CECCommand) + Send>) {
        *self.cec_rx_callback.lock().unwrap() = Some(func)
    }
    fn set_tx_callback(&self, func: Box<dyn FnMut(&CECCommand) + Send>) {
        *self.cec_tx_callback.lock().unwrap() = Some(func)
    }
}
