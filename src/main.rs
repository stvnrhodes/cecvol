mod action;
mod auth;
mod cec;
mod lgip;
mod tv;
mod wol;

use action::devices::{
    DeviceState, ErrorCodes, Execution, FulfillmentRequest, FulfillmentResponse, InputKey,
    InputNames, RequestPayload,
};
use axum::extract;
use axum::http::Request;
use axum::http::StatusCode;
use axum::middleware;
use axum::response;
use axum::response::IntoResponse;
use axum::response::Response;
use axum::routing;
use axum::Router;
use clap::Parser;
use log::{error, info};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

const DEVICE_ID: &str = "1";

async fn index() -> impl IntoResponse {
    response::Html(include_str!("index.html"))
}

fn device_state(cec: &cec::CEC) -> DeviceState {
    DeviceState {
        online: Some(true),
        current_volume: None, //Some(cec.current_volume()),
        is_muted: None,       //Some(cec.is_muted()),
        on: Some(cec.is_on()),
        current_input: Some(format!("{:x}", cec.current_input())),
    }
}

async fn fulfillment(
    cec: extract::State<Arc<Mutex<cec::CEC>>>,
    req: extract::Json<FulfillmentRequest>,
) -> response::Result<response::Json<FulfillmentResponse>> {
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
                return Ok(response::Json(FulfillmentResponse {
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
                return Ok(response::Json(FulfillmentResponse {
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
                            // Execution::SetVolume { volume_level } => {
                            //     cec.set_volume_level(*volume_level)?;
                            // }
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
                                wol::wake([0x24, 0x4b, 0xfe, 0x55, 0x78, 0x94])
                                    .map_err(|_| StatusCode::IM_A_TEAPOT)?;
                            }
                            Execution::SetInput { new_input } => {
                                cec.set_input(new_input)?;
                            }
                            _ => {
                                return Ok(response::Json(FulfillmentResponse {
                                    request_id: request_id,
                                    payload: json!({
                                        "errorCode": ErrorCodes::NotSupported,
                                        "debugString": "unknown command",
                                    }),
                                }))
                            }
                        }
                        // TODO(stvn): Do all executions in the array, improve error handling
                        return Ok(response::Json(FulfillmentResponse {
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

    Ok(response::Json(FulfillmentResponse {
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

async fn cecexec(
    cec: extract::State<Arc<Mutex<cec::CEC>>>,
    req: extract::Json<ExecRequest>,
) -> response::Result<response::Json<ExecResponse>> {
    let cmd: Vec<u8> = req
        .cmd
        .split(":")
        .map(|s| u8::from_str_radix(s, 16).unwrap_or(0))
        .collect();
    cec.lock().unwrap().transmit_raw(&cmd)?;
    Ok(response::Json(ExecResponse {}))
}

async fn varz() -> response::Result<impl IntoResponse> {
    let metrics = prometheus::gather();
    let encoder = prometheus::TextEncoder::new();
    encoder
        .encode_to_string(&metrics)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("{}", e)).into())
}

async fn add_observability<B>(
    req: Request<B>,
    next: middleware::Next<B>,
) -> response::Result<Response> {
    let path = format!("{:?}", req.uri().path_and_query().unwrap());
    let resp = next.run(req).await;
    // /fulfillment 200 {"content-type": "application/json", "content-length": "170"}
    info!(
        "{request} {status}",
        request = path,
        status = resp.status().as_str(),
    );
    Ok(resp)
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("INFO"))
        .format_timestamp(Some(env_logger::fmt::TimestampPrecision::Millis))
        .init();

    info!("Creating CEC connection...");
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

    let conn = Arc::new(Mutex::new(cec_conn));

    let thread_conn = conn.clone();
    thread::spawn(move || {
        match thread_conn.lock().unwrap().poll_all() {
            Ok(()) => {}
            Err(e) => error!("{}", e),
        }
        thread::sleep(Duration::from_secs(100));
    });

    let app = Router::new()
        .route("/", routing::get(index))
        .route("/varz", routing::get(varz))
        .route("/cecexec", routing::post(cecexec))
        .route("/fulfillment", routing::post(fulfillment))
        .route("/auth", routing::get(auth::auth))
        .route_layer(middleware::from_fn(auth::has_valid_auth))
        .route("/login", routing::get(auth::login_page).post(auth::login))
        .route("/token", routing::post(auth::token))
        .route_layer(middleware::from_fn(add_observability))
        .with_state(conn);

    info!("Starting server...");
    axum::Server::bind(&args.http_addr.parse().unwrap())
        .serve(app.into_make_service())
        .await?;
    Ok(())
}
