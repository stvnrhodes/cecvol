mod action;
mod auth;
mod cec;
mod wol;

use action::devices::{
    DeviceState, ErrorCodes, Execution, FulfillmentRequest, FulfillmentResponse, InputKey,
    InputNames, RequestPayload,
};
use actix_web::{middleware, post, web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use clap::Parser;
use log::{debug, error};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

const DEVICE_ID: &str = "1";

async fn index(req: HttpRequest) -> impl Responder {
    if !auth::has_valid_auth(&req).unwrap_or(false) {
        return HttpResponse::Found()
            .header("Location", "/login?redirect=%2F")
            .finish();
    }
    let resp: &'static [u8] = include_bytes!("index.html");
    //.set_header(http::header::CONTENT_TYPE, "text/html; charset=UTF-8")
    HttpResponse::Ok().content_type("text/html").body(resp)
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
    full_req: HttpRequest,
    req: web::Json<FulfillmentRequest>,
    cec: web::Data<Mutex<cec::CEC>>,
) -> Result<web::Json<FulfillmentResponse>, actix_web::Error> {
    if !auth::has_valid_auth(&full_req).unwrap_or(false) {
        return Err(HttpResponse::Unauthorized().into());
    }

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
                    payload: json!({
                        // TODO(stvn): Switch to oauth identity
                        "agentUserId": "cecvol-stvn-user",
                        "devices": [
                            {
                                "id": DEVICE_ID.to_string(),
                                "type": "actions.devices.types.RemoteControl",
                                "traits": [
                                    "action.devices.traits.AppSelector",
                                    "action.devices.traits.InputSelector",
                                    "action.devices.traits.MediaState",
                                    "action.devices.traits.OnOff",
                                    "action.devices.traits.TransportControl",
                                    "action.devices.traits.Volume"
                                ],
                                "name": {
                                    "name": "cecvol",
                                    "nicknames": ["pi", "cec"]
                                },
                                "willReportState": false,
                                "roomHint": "living room",
                                "deviceInfo": {
                                    "manufacturer": "Raspberry Pi Foundation",
                                    "model": "PI3P"
                                },
                                "attributes": {
                                    "availableApplications": [],
                                    "commandOnlyInputSelector": true,
                                    "orderedInputs": false,
                                    "availableInputs": inputs,
                                    "supportActivityState": false,
                                    "supportPlaybackState": false,
                                    "commandOnlyOnOff": true,
                                    "queryOnlyOnOff": false,
                                    "transportControlSupportedCommands": [],
                                    "volumeMaxLevel": 100,
                                    "volumeCanMuteAndUnmute": true,
                                    "volumeDefaultPercentage": 12,
                                    "levelStepSize": 1,
                                    "commandOnlyVolume": true
                                }
                            }
                        ]
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
                    payload: json!({
                        "devices": device_data,
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
                            Execution::WakeOnLan => {
                                // TODO(stvn): Don't hard-code
                                wol::wake([0x24, 0x4b, 0xfe, 0x55, 0x78, 0x94])?;
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
                                    payload: json!({
                                        "errorCode": ErrorCodes::NotSupported,
                                        "debugString": "unknown command",
                                    }),
                                }))
                            }
                        }
                        // TODO(stvn): Do all executions in the array, improve error handling
                        return Ok(web::Json(FulfillmentResponse {
                            request_id: request_id,
                            payload: json!({
                                "commands": [
                                    {
                                        "ids":  c.devices.iter().map(|d| d.id.clone()).collect::<Vec<String>>(),
                                        "status": "SUCCESS",
                                        "states": device_state(&cec)
                                    }
                                ],
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
        payload: json!({
            "errorCode": ErrorCodes::NotSupported,
            "debugString": "no inputs provided",
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

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Address to listen on
    #[arg(long, default_value = "0.0.0.0:8080")]
    http_addr: String,

    /// If true, use a fake cec connection instead of directly using the hardware.
    #[arg(long)]
    use_fake_cec_conn: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    env_logger::from_env(env_logger::Env::default().default_filter_or("debug"))
        .format_timestamp(Some(env_logger::fmt::TimestampPrecision::Millis))
        .init();

    debug!("Creating CEC connection...");
    let osd_name = "cecvol";
    // LG's vendor code seems to be required for UserControl commands to work.
    let vendor_id = 0x00e091;
    let vchi: Arc<dyn cec::CECConnection> = if args.use_fake_cec_conn {
        Arc::new(cec::noop::LogOnlyConn {})
    } else {
        let vchi = cec::vchi::HardwareInterface::init()?;
        vchi.set_osd_name(osd_name)?;
        vchi.set_vendor_id(vendor_id)?;

        if vchi.get_logical_addr()? == cec::LogicalAddress::Broadcast
            && vchi.get_physical_addr()? != 0xffff
        {
            vchi.alloc_logical_addr()?;
        }
        Arc::new(vchi)
    };
    let cec_conn = cec::CEC::new(
        vchi,
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
    actix_web::rt::System::new("main").block_on(async move {
        debug!("Starting server...");
        HttpServer::new(move || {
            App::new()
                .app_data(conn.clone())
                .wrap(middleware::Logger::default())
                .wrap(middleware::Compress::default())
                .service(fulfillment)
                .service(cecexec)
                .service(auth::auth)
                .service(auth::login)
                .service(auth::login_page)
                .route("/", web::get().to(index))
        })
        .bind(args.http_addr)?
        .run()
        .await?;
        Ok(())
    })
}

// TODO
// make screen show state
// - oauth
// read in .env
// custom port #
// - RoutingChange SetStreamPath set the input
// maybe send wol packet too?
