use cecvol::action;
use cecvol::auth;
use cecvol::cec;
use cecvol::lgip;
use cecvol::tv;
use cecvol::wol;

use action::devices::{
    ErrorCodes, Execution, FulfillmentRequest, FulfillmentResponse, RequestPayload,
};

use clap::Parser;
use log::info;
use rouille::router;
use rouille::Request;
use rouille::Response;
use rouille::ResponseBody;
use serde_json::json;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Mutex;

const DEVICE_ID: &str = "1";

fn index() -> Response {
    Response::html(include_str!("../index.html"))
}
fn manifest() -> Response {
    Response {
        status_code: 200,
        headers: vec![(
            "Content-Type".into(),
            "application/json; charset=utf-8".into(),
        )],
        data: ResponseBody::from_data(include_str!("../manifest.json")),
        upgrade: None,
    }
}

fn fulfillment(app_state: AppState, request: &Request) -> Response {
    let cec = &app_state.cec;
    let req: FulfillmentRequest = match rouille::input::json_input(request) {
        Ok(r) => r,
        Err(e) => return Response::text(e.to_string()).with_status_code(400),
    };

    let request_id = req.request_id.clone();
    for input in &req.inputs {
        match input {
            RequestPayload::Sync => {
                return Response::json(&FulfillmentResponse {
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
                });
            }
            RequestPayload::Query { devices: _ } => {
                // let mut device_data = HashMap::new();
                return Response::json(&FulfillmentResponse {
                    request_id: request_id,
                    payload: json!({
                        // TODO
                        // "devices": device_data,
                    }),
                });
            }
            RequestPayload::Execute { commands } => {
                let mut cec = cec.lock().unwrap();
                for c in commands {
                    for e in &c.execution {
                        match e {
                            Execution::VolumeRelative { relative_steps } => {
                                if let Err(e) = cec.volume_change(*relative_steps) {
                                    return e.into();
                                }
                            }
                            Execution::Mute { mute } => {
                                if let Err(e) = cec.mute(*mute) {
                                    return e.into();
                                }
                            }
                            Execution::OnOff { on } => {
                                if let Err(e) = cec.on_off(*on) {
                                    return e.into();
                                }
                            }
                            Execution::WakeOnLan => {
                                if let Err(e) = wol::wake(app_state.server_mac_addr) {
                                    return Response::text(e.to_string()).with_status_code(500);
                                }
                            }
                            Execution::SetInput { new_input } => {
                                let input = match new_input.as_str() {
                                    "1" | "HDMI 1" => tv::Input::HDMI1,
                                    "2" | "HDMI 2" => tv::Input::HDMI2,
                                    "3" | "HDMI 3" => tv::Input::HDMI3,
                                    "4" | "HDMI 4" => tv::Input::HDMI4,
                                    _ => {
                                        return Response::json(&FulfillmentResponse {
                                            request_id: request_id,
                                            payload: json!({
                                                "errorCode": ErrorCodes::NotSupported,
                                                "debugString": "unsupported input",
                                            }),
                                        })
                                    }
                                };
                                if let Err(e) = cec.set_input(input) {
                                    return e.into();
                                }
                            }
                            _ => {
                                return Response::json(&FulfillmentResponse {
                                    request_id: request_id,
                                    payload: json!({
                                        "errorCode": ErrorCodes::NotSupported,
                                        "debugString": "unknown command",
                                    }),
                                })
                            }
                        }
                        // TODO(stvn): Do all executions in the array, improve error handling
                        return Response::json(&FulfillmentResponse {
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
                        });
                    }
                }
            }
            RequestPayload::Disconnect => println!("Disconnect"),
        }
    }

    Response::json(&FulfillmentResponse {
        request_id: request_id,
        payload: json!({
            "errorCode": ErrorCodes::NotSupported,
            "debugString": "no inputs provided",
        }),
    })
}

fn varz() -> Response {
    let metrics = prometheus::gather();
    let encoder = prometheus::TextEncoder::new();
    match encoder.encode_to_string(&metrics) {
        Ok(encoded) => Response::text(encoded),
        Err(err) => Response::text(err.to_string()).with_status_code(500),
    }
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

    /// Newline-separated tokens acceptable for Authorization header
    #[arg(long, env = "AUTH_TOKENS")]
    auth_tokens: Option<String>,

    /// Permitted emails for login
    #[arg(long, env = "ALLOWED_EMAILS")]
    allowed_emails: Vec<String>,

    /// Client id for OIDC login
    #[arg(long, env = "OIDC_CLIENT_ID")]
    oidc_client_id: Option<String>,

    /// Client secret for OIDC login
    #[arg(long, env = "OIDC_CLIENT_SECRET")]
    oidc_client_secret: Option<String>,
}

#[derive(Clone)]
struct AppState {
    server_mac_addr: [u8; 6],
    cec: Arc<Mutex<Box<dyn tv::TVConnection + Sync + Send>>>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
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

    let mut auth_tokens = HashSet::new();
    if let Some(tokens) = args.auth_tokens {
        for line in tokens.lines() {
            if line.trim() != "" && !line.starts_with("#") {
                auth_tokens.insert(line.trim().to_string());
            }
        }
    }
    let mut allowed_emails = HashSet::new();
    for e in args.allowed_emails {
        allowed_emails.insert(e);
    }

    let authorizer = match (args.oidc_client_id, args.oidc_client_secret) {
        (Some(oidc_client_id), Some(oidc_client_secret)) => {
            info!("enforcing login");
            Some(auth::Authorizer::new(
                auth_tokens,
                allowed_emails,
                oidc_client_id,
                oidc_client_secret,
            ))
        }
        _ => {
            info!("not enforcing login");
            None
        }
    };

    let app_state = AppState {
        cec: conn,
        server_mac_addr,
    };

    info!("Starting server...");

    rouille::start_server(&args.http_addr, move |request| {
        info!(
            "{method} {url}",
            method = request.method(),
            url = request.raw_url(),
        );
        let route = |req: &Request| {
            router!(req,
                (GET) ["/"] => {index()},
                (GET) ["/manifest.json"] => {manifest()},
                (GET) ["/varz"] => {varz()},
                (POST) ["/fulfillment"] => {fulfillment(app_state.clone(), req)},
                _ => rouille::Response::empty_404()
            )
        };
        let resp = match &authorizer {
            Some(a) => a.ensure_authorized(request, route),
            None => route(request),
        };
        info!("... {status}", status = resp.status_code,);
        resp
    });
}
