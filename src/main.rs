mod action;
mod cec;

use action::devices::{
    CommandErrors, CommandResults, CommandStatus, DeviceState, ErrorCodes, ExecuteResponsePayload,
    Execution, FulfillmentRequest, FulfillmentResponse, QueryResponsePayload, RequestPayload,
    ResponseErrors, ResponsePayload, SyncResponsePayload,
};
use actix_http::Response;
use actix_web::{get, middleware, post, web, App, HttpServer, Responder};
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

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
    let cec_conn = cec::CEC::new(Arc::new(vchi), osd_name, vendor_id);
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
            .service(auth)
            .route("/", web::get().to(index))
    })
    .bind("0.0.0.0:8080")?
    .run()
    .await?;
    Ok(())
}

// TODO
// - internally record device status
// - maybe poll on devices present
// - oauth
