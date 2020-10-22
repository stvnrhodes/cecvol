mod action;
mod auth;
mod cec;

use action::devices::{
    CommandErrors, CommandResults, CommandStatus, Device, DeviceAttributes, DeviceInfo,
    DeviceNames, DeviceState, DeviceTrait, DeviceType, ErrorCodes, ExecuteResponsePayload,
    Execution, FulfillmentRequest, FulfillmentResponse, InputKey, InputNames,
    InputSelectorAttributes, OnOffAttributes, QueryResponsePayload, RequestPayload, ResponseErrors,
    ResponsePayload, SyncResponsePayload, VolumeAttributes,
};
use actix_http::Response;
use actix_web::{middleware, post, web, App, HttpServer, Responder};
use log::{debug, error};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

const DEVICE_ID: &str = "1";

async fn index() -> impl Responder {
    let resp: &'static [u8] = include_bytes!("index.html");
    //.set_header(http::header::CONTENT_TYPE, "text/html; charset=UTF-8")
    Response::Ok().content_type("text/html").body(resp)
}

fn device_state(cec: &cec::CEC) -> DeviceState {
    DeviceState {
        online: Some(true),
        current_volume: Some(cec.current_volume()),
        is_muted: Some(cec.is_muted()),
        on: Some(cec.is_on()),
        current_input: Some(format!("{:x}", cec.current_input())),
    }
}

#[post("/fulfillment")]
async fn fulfillment(
    req: web::Json<FulfillmentRequest>,
    cec: web::Data<Mutex<cec::CEC>>,
) -> Result<web::Json<FulfillmentResponse>, actix_web::Error> {
    let request_id = req.request_id.clone();
    for input in &req.inputs {
        match input {
            RequestPayload::Sync => {
                let inputs: Vec<InputKey> = cec
                    .lock()
                    .unwrap()
                    .names_by_addr()
                    .iter()
                    .map(|(addr, names)| InputKey {
                        key: format!("{:x}", addr),
                        names: vec![InputNames {
                            lang: "en".into(),
                            name_synonym: names.to_vec(),
                        }],
                    })
                    .collect();
                return Ok(web::Json(FulfillmentResponse {
                    request_id: request_id,
                    payload: ResponsePayload::Sync(SyncResponsePayload {
                        // TODO(stvn): Switch to oauth identity
                        agent_user_id: "cecvol-stvn-user".into(),
                        devices: vec![Device {
                            id: DEVICE_ID.to_string(),
                            device_type: DeviceType::RemoteControl,
                            traits: vec![
                                DeviceTrait::InputSelector,
                                DeviceTrait::OnOff,
                                DeviceTrait::Volume,
                            ],
                            name: DeviceNames {
                                name: "cecvol".into(),
                                nicknames: vec!["pi".into(), "raspberry pi".into()],
                                default_names: vec!["Raspberry Pi 3 Model B+".into()],
                            },
                            will_report_state: false,
                            room_hint: None,
                            device_info: Some(DeviceInfo {
                                manufacturer: Some("Raspberry Pi Foundation".into()),
                                model: Some("PI3P".into()),
                                hw_version: None,
                                sw_version: None,
                            }),
                            attributes: DeviceAttributes {
                                input_selector_attributes: Some(InputSelectorAttributes {
                                    command_only_input_selector: false,
                                    ordered_inputs: false,
                                    available_inputs: inputs,
                                }),
                                on_off_attributes: Some(OnOffAttributes {
                                    command_only_on_off: false,
                                    query_only_on_off: false,
                                }),
                                volume_attributes: Some(VolumeAttributes {
                                    volume_max_level: 100,
                                    volume_can_mute_and_unmute: true,
                                    volume_default_percentage: 15,
                                    level_step_size: 2,
                                    command_only_volume: true,
                                }),
                            },
                        }],
                        errors: None,
                    }),
                }));
            }
            RequestPayload::Query { devices } => {
                let mut device_data = HashMap::new();
                for device in devices {
                    if device.id == DEVICE_ID {
                        device_data
                            .insert(DEVICE_ID.to_string(), device_state(&cec.lock().unwrap()));
                    }
                }
                return Ok(web::Json(FulfillmentResponse {
                    request_id: request_id,
                    payload: ResponsePayload::Query(QueryResponsePayload {
                        devices: device_data,
                        errors: None,
                    }),
                }));
            }
            RequestPayload::Execute { commands } => {
                let mut cec = cec.lock().unwrap();
                for c in commands {
                    for e in &c.execution {
                        match e {
                            Execution::SetVolume { volume_level } => {
                                cec.set_volume_level(*volume_level)?;
                            }
                            Execution::VolumeRelative { relative_steps } => {
                                cec.volume_change(*relative_steps)?;
                            }
                            Execution::Mute { mute } => {
                                cec.mute(*mute)?;
                            }
                            Execution::OnOff { on } => {
                                cec.on_off(*on)?;
                            }
                            Execution::SetInput { new_input } => {
                                if !cec.is_on() {
                                    cec.on_off(true)?;
                                    tokio::time::delay_for(Duration::from_millis(4000)).await;
                                }
                                cec.set_input(new_input)?;
                            }
                            _ => {
                                return Ok(web::Json(FulfillmentResponse {
                                    request_id: request_id,
                                    payload: ResponsePayload::Error(ResponseErrors {
                                        error_code: ErrorCodes::NotSupported,
                                        debug_string: "unknown command".into(),
                                    }),
                                }))
                            }
                        }
                        // TODO(stvn): Do all executions in the array, improve error handling
                        return Ok(web::Json(FulfillmentResponse {
                            request_id: request_id,
                            payload: ResponsePayload::Execute(ExecuteResponsePayload {
                                commands: vec![CommandResults {
                                    ids: c.devices.iter().map(|d| d.id.clone()).collect(),
                                    status: CommandStatus::Success,
                                    error_code: CommandErrors::None,
                                    states: Some(device_state(&cec)),
                                }],
                                errors: None,
                            }),
                        }));
                    }
                }
            }
            RequestPayload::Disconnect => println!("Disconnect"),
        }
    }

    Ok(web::Json(FulfillmentResponse {
        request_id: request_id,
        payload: ResponsePayload::Error(ResponseErrors {
            error_code: ErrorCodes::NotSupported,
            debug_string: "no inputs provided".into(),
        }),
    }))
}

#[derive(Deserialize)]
pub struct ExecRequest {
    cmd: String,
}
#[derive(Serialize)]
pub struct ExecResponse {}

#[post("/cecexec")]
async fn cecexec(
    req: web::Json<ExecRequest>,
    cec: web::Data<Mutex<cec::CEC>>,
) -> Result<web::Json<ExecResponse>, actix_web::Error> {
    let cmd: Vec<u8> = req
        .cmd
        .split(":")
        .map(|s| u8::from_str_radix(s, 16).unwrap_or(0))
        .collect();
    cec.lock().unwrap().transmit_raw(&cmd)?;
    Ok(web::Json(ExecResponse {}))
}

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    env_logger::from_env(env_logger::Env::default().default_filter_or("debug"))
        .format_timestamp(Some(env_logger::fmt::TimestampPrecision::Millis))
        .init();

    debug!("Creating CEC connection...");
    let vchi = cec::vchi::HardwareInterface::init()?;
    let osd_name = "cecvol";
    // LG's vendor code seems to be required for UserControl commands to work.
    let vendor_id = 0x00e091;
    vchi.set_osd_name(osd_name)?;
    vchi.set_vendor_id(vendor_id)?;

    if vchi.get_logical_addr()? == cec::LogicalAddress::Broadcast
        && vchi.get_physical_addr()? != 0xffff
    {
        vchi.alloc_logical_addr()?;
    }
    let cec_conn = cec::CEC::new(
        Arc::new(vchi),
        osd_name,
        vendor_id,
        &[
            ("HDMI 1", 0x1000),
            ("HDMI 2", 0x2000),
            ("HDMI 3", 0x3000),
            ("HDMI 4", 0x4000),
            ("1", 0x1000),
            ("2", 0x2000),
            ("3", 0x3000),
            ("4", 0x4000),
            ("NintendoSwitch", 0x2000),
            ("PC", 0x4000),
            ("Serpens", 0x4000),
        ],
    )?;
    let conn = web::Data::new(Mutex::new(cec_conn));

    let thread_conn = conn.clone();
    thread::spawn(move || {
        match thread_conn.lock().unwrap().poll_all() {
            Ok(()) => {}
            Err(e) => error!("{}", e),
        }
        thread::sleep(Duration::from_secs(100));
    });
    debug!("Starting server...");
    HttpServer::new(move || {
        App::new()
            .app_data(conn.clone())
            .wrap(middleware::Logger::default())
            .wrap(middleware::Compress::default())
            .service(fulfillment)
            .service(cecexec)
            .service(auth::auth)
            .route("/", web::get().to(index))
    })
    .bind("0.0.0.0:8080")?
    .run()
    .await?;
    Ok(())
}

// TODO
// make screen show state
// - oauth
// read in .env
// custom port #
// - RoutingChange SetStreamPath set the input
