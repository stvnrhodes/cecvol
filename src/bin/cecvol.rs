use cecvol::action;
use cecvol::cec;
use cecvol::lgip;
use cecvol::tv;
use cecvol::wol;

use action::devices::{
    ErrorCodes, Execution, FulfillmentRequest, FulfillmentResponse, RequestPayload,
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
use log::info;
use serde_json::json;
use std::sync::Arc;
use std::sync::Mutex;

const DEVICE_ID: &str = "1";

async fn index() -> impl IntoResponse {
    response::Html(include_str!("../index.html"))
}

async fn fulfillment(
    app_state: extract::State<AppState>,
    req: extract::Json<FulfillmentRequest>,
) -> response::Result<response::Json<FulfillmentResponse>> {
    let cec = &app_state.cec;
    let request_id = req.request_id.clone();
    for input in &req.inputs {
        match input {
            RequestPayload::Sync => {
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
                                    // TODO
                                    // "availableInputs": inputs,
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
            RequestPayload::Query { devices: _ } => {
                // let mut device_data = HashMap::new();
                return Ok(response::Json(FulfillmentResponse {
                    request_id: request_id,
                    payload: json!({
                        // TODO
                        // "devices": device_data,
                    }),
                }));
            }
            RequestPayload::Execute { commands } => {
                let mut cec = cec.lock().unwrap();
                for c in commands {
                    for e in &c.execution {
                        match e {
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
                                wol::wake(app_state.server_mac_addr)
                                    .map_err(|_| StatusCode::IM_A_TEAPOT)?;
                            }
                            Execution::SetInput { new_input } => {
                                let input = match new_input.as_str() {
                                    "1" | "HDMI 1" => tv::Input::HDMI1,
                                    "2" | "HDMI 2" => tv::Input::HDMI2,
                                    "3" | "HDMI 3" => tv::Input::HDMI3,
                                    "4" | "HDMI 4" => tv::Input::HDMI4,
                                    _ => {
                                        return Ok(response::Json(FulfillmentResponse {
                                            request_id: request_id,
                                            payload: json!({
                                                "errorCode": ErrorCodes::NotSupported,
                                                "debugString": "unsupported input",
                                            }),
                                        }))
                                    }
                                };
                                cec.set_input(input)?;
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
                                        // "states": device_state(&cec)
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

    /// If true, control over ip.
    #[arg(long)]
    use_lg_ip_control: bool,

    /// Keycode for pairing the LG tv with the server.
    #[arg(long, env = "LG_KEYCODE")]
    lg_keycode: Option<String>,

    /// TV MAC address for WoL, in xx:xx:xx:xx:xx:xx form.
    #[arg(long, env = "LG_MAC_ADDR")]
    lg_mac_addr: Option<String>,

    /// Server MAC address for WoL, in xx:xx:xx:xx:xx:xx form.
    #[arg(long, env = "SERVER_MAC_ADDR")]
    server_mac_addr: String,
}

#[derive(Clone)]
struct AppState {
    server_mac_addr: [u8; 6],
    cec: Arc<Mutex<Box<dyn tv::TVConnection + Sync + Send>>>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("INFO"))
        .format_timestamp(Some(env_logger::fmt::TimestampPrecision::Millis))
        .init();

    let tv: Box<dyn tv::TVConnection + Sync + Send> = if args.use_lg_ip_control {
        let mut tv_mac_addr = [0u8; 6];
        for (i, s) in args.lg_mac_addr.unwrap().split(":").enumerate() {
            tv_mac_addr[i] = u8::from_str_radix(s, 16)?;
        }
        Box::new(lgip::LGTV::new(
            "LGWebOSTV.local".to_string(),
            tv_mac_addr,
            &args.lg_keycode.unwrap(),
        ))
    } else {
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
        let cec_conn = cec::CEC::new(vchi, osd_name, vendor_id)?;
        cec_conn.poll_all()?;
        Box::new(cec_conn)
    };

    let conn: Arc<Mutex<Box<dyn tv::TVConnection + Sync + Send>>> = Arc::new(Mutex::new(tv));

    let mut server_mac_addr = [0u8; 6];
    for (i, s) in args.server_mac_addr.split(":").enumerate() {
        server_mac_addr[i] = u8::from_str_radix(s, 16)?;
    }
    let app_state = AppState {
        cec: conn,
        server_mac_addr,
    };

    let app = Router::new()
        .route("/", routing::get(index))
        .route("/varz", routing::get(varz))
        .route("/fulfillment", routing::post(fulfillment))
        .route_layer(middleware::from_fn(add_observability))
        .with_state(app_state);

    info!("Starting server...");
    axum::Server::bind(&args.http_addr.parse().unwrap())
        .serve(app.into_make_service())
        .await?;
    Ok(())
}
