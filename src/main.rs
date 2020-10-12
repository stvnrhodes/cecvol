#![feature(backtrace)]
mod action;
mod cec;

use action::devices::{
    CommandErrors, CommandResults, CommandStatus, DeviceState, ErrorCodes, ExecuteResponsePayload,
    Execution, FulfillmentRequest, FulfillmentResponse, QueryResponsePayload, RequestPayload,
    ResponseErrors, ResponsePayload, SyncResponsePayload,
};
use actix_http::Response;
use actix_web::{get, middleware, post, web, App, HttpServer, Responder};
use cec_rs::{CecCommand, CecConnectionCfgBuilder, CecDeviceType, CecDeviceTypeVec, CecKeypress};
use log::{debug, info};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Deserialize)]
struct AuthInfo {
    // client_id: String,
// redirect_uri: String,
// state: String,
// scope: Option<String>,
// response_type: String, // always code
// user_locale: String,
}

async fn index() -> impl Responder {
    let resp: &'static [u8] = include_bytes!("index.html");
    //.set_header(http::header::CONTENT_TYPE, "text/html; charset=UTF-8")
    Response::Ok().content_type("text/html").body(resp)
}

#[get("/auth")]
async fn auth(_info: web::Query<AuthInfo>) -> impl Responder {
    // https://developers.google.com/assistant/smarthome/develop/implement-oauth#implement_oauth_account_linking
    // Todo: verify client id and redirect uri is correct
    // check that user is signed in
    // generate auth code, expire after 10m
    //redirect browser to redirect_uri, include code and state
    "Unimplemented"
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
                return Ok(web::Json(FulfillmentResponse {
                    request_id: request_id,
                    payload: ResponsePayload::Sync(SyncResponsePayload {
                        // TODO(stvn): Switch to oauth identity
                        agent_user_id: "cecvol-stvn-user".into(),
                        // TODO(stvn): Add devices
                        devices: vec![],
                        errors: None,
                    }),
                }));
            }
            RequestPayload::Query { devices: _ } => {
                return Ok(web::Json(FulfillmentResponse {
                    request_id: request_id,
                    payload: ResponsePayload::Query(QueryResponsePayload {
                        devices: HashMap::new(),
                        errors: None,
                    }),
                }));
            }
            RequestPayload::Execute { commands } => {
                for c in commands {
                    for e in &c.execution {
                        match e {
                            Execution::SetVolume { volume_level } => println!("{}", volume_level),
                            Execution::VolumeRelative { relative_steps } => {
                                info!("changing volume of {:?} by {}", c.devices, relative_steps);
                                cec.lock().unwrap().volume_change(*relative_steps)?;
                                let result = CommandResults {
                                    ids: c.devices.iter().map(|d| d.id.clone()).collect(),
                                    status: CommandStatus::SUCCESS,
                                    error_code: CommandErrors::None,
                                    states: DeviceState {
                                        online: Some(true),
                                        current_volume: Some(5),
                                        ..Default::default()
                                    },
                                };
                                return Ok(web::Json(FulfillmentResponse {
                                    request_id: request_id,
                                    payload: ResponsePayload::Execute(ExecuteResponsePayload {
                                        commands: vec![result],
                                        errors: None,
                                    }),
                                }));
                            }
                            Execution::Mute { mute } => {
                                cec.lock().unwrap().mute(*mute)?;
                                return Ok(web::Json(FulfillmentResponse {
                                    request_id: request_id,
                                    payload: ResponsePayload::Execute(ExecuteResponsePayload {
                                        commands: vec![],
                                        errors: None,
                                    }),
                                }));
                            }
                            Execution::OnOff { on } => {
                                cec.lock().unwrap().on_off(*on)?;
                                return Ok(web::Json(FulfillmentResponse {
                                    request_id: request_id,
                                    payload: ResponsePayload::Execute(ExecuteResponsePayload {
                                        commands: vec![],
                                        errors: None,
                                    }),
                                }));
                            }
                            Execution::SetInput { new_input } => {
                                cec.lock().unwrap().set_input(new_input.clone())?;
                                return Ok(web::Json(FulfillmentResponse {
                                    request_id: request_id,
                                    payload: ResponsePayload::Execute(ExecuteResponsePayload {
                                        commands: vec![],
                                        errors: None,
                                    }),
                                }));
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

// #[actix_web::main]
// async fn main() -> anyhow::Result<()> {
fn main() -> anyhow::Result<()> {
    env_logger::from_env(env_logger::Env::default().default_filter_or("debug"))
        .format_timestamp(Some(env_logger::fmt::TimestampPrecision::Millis))
        .init();

    debug!("Creating CEC connection...");
    cec::vchi::HardwareInterface::init()?;
    // let cfg = CecConnectionCfgBuilder::default()
    //     .port("RPI".into())
    //     .device_name("Pi".into())
    //     .activate_source(false) /* Don't auto-turn on the TV */
    //     .device_types(CecDeviceTypeVec::new(CecDeviceType::RecordingDevice))
    //     .key_press_callback(Box::new(on_key_press))
    //     .command_received_callback(Box::new(on_command_received))
    //     .build()
    //     .unwrap();
    // let conn = web::Data::new(Mutex::new(cec::CEC::new(cfg.open().unwrap())));

    // debug!("Starting server...");
    // HttpServer::new(move || {
    //     App::new()
    //         .app_data(conn.clone())
    //         .wrap(middleware::Logger::default())
    //         .wrap(middleware::Compress::default())
    //         .service(fulfillment)
    //         .service(auth)
    //         .route("/", web::get().to(index))
    // })
    // .bind("0.0.0.0:8080")?
    // .run()
    // .await?;
    Ok(())
}

fn on_key_press(keypress: CecKeypress) {
    info!(
        "onKeyPress: {:?}, keycode: {:?}, duration: {:?}",
        keypress, keypress.keycode, keypress.duration
    );
}

fn on_command_received(command: CecCommand) {
    info!(
        "onCommandReceived:  opcode: {:?}, initiator: {:?}",
        command.opcode, command.initiator
    );
}
