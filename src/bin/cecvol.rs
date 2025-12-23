use cecvol::action;
use cecvol::cec;
use cecvol::lgip;
use cecvol::tv;
use cecvol::wol;

use action::devices::{
    ErrorCodes, Execution, FulfillmentRequest, FulfillmentResponse, RequestPayload,
};

use clap::Parser;
use log::info;
use serde::Serialize;
use serde_json::json;
use std::io::Cursor;

use std::sync::Arc;
use std::sync::Mutex;
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

const DEVICE_ID: &str = "1";

fn index() -> Response<Cursor<Vec<u8>>> {
    Response::from_string(include_str!("../index.html"))
        .with_header(Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap())
}

fn manifest() -> Response<Cursor<Vec<u8>>> {
    Response::from_data(include_str!("../manifest.json").as_bytes().to_vec())
        .with_header(Header::from_bytes("Content-Type", "application/json; charset=utf-8").unwrap())
}

fn json_response<T: Serialize>(data: &T) -> Response<Cursor<Vec<u8>>> {
    let s = serde_json::to_string(data).unwrap();
    Response::from_string(s)
        .with_header(Header::from_bytes("Content-Type", "application/json").unwrap())
}

fn fulfillment(app_state: AppState, request: &mut Request) -> Response<Cursor<Vec<u8>>> {
    let cec = &app_state.cec;

    let mut content = String::new();
    if let Err(e) = request.as_reader().read_to_string(&mut content) {
        return Response::from_string(e.to_string()).with_status_code(StatusCode(400));
    }

    let req: FulfillmentRequest = match serde_json::from_str(&content) {
        Ok(r) => r,
        Err(e) => return Response::from_string(e.to_string()).with_status_code(StatusCode(400)),
    };

    let request_id = req.request_id.clone();
    for input in &req.inputs {
        match input {
            RequestPayload::Sync => {
                return json_response(&FulfillmentResponse {
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
                return json_response(&FulfillmentResponse {
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
                                    return Response::from_string(e.to_string())
                                        .with_status_code(StatusCode(500));
                                }
                            }
                            Execution::Mute { mute } => {
                                if let Err(e) = cec.mute(*mute) {
                                    return Response::from_string(e.to_string())
                                        .with_status_code(StatusCode(500));
                                }
                            }
                            Execution::OnOff { on } => {
                                if let Err(e) = cec.on_off(*on) {
                                    return Response::from_string(e.to_string())
                                        .with_status_code(StatusCode(500));
                                }
                            }
                            Execution::WakeOnLan => {
                                if let Err(e) = wol::wake(app_state.server_mac_addr) {
                                    return Response::from_string(e.to_string())
                                        .with_status_code(StatusCode(500));
                                }
                            }
                            Execution::SetInput { new_input } => {
                                let input = match new_input.as_str() {
                                    "1" | "HDMI 1" => tv::Input::HDMI1,
                                    "2" | "HDMI 2" => tv::Input::HDMI2,
                                    "3" | "HDMI 3" => tv::Input::HDMI3,
                                    "4" | "HDMI 4" => tv::Input::HDMI4,
                                    _ => {
                                        return json_response(&FulfillmentResponse {
                                            request_id: request_id,
                                            payload: json!({
                                                "errorCode": ErrorCodes::NotSupported,
                                                "debugString": "unsupported input",
                                            }),
                                        })
                                    }
                                };
                                if let Err(e) = cec.set_input(input) {
                                    return Response::from_string(e.to_string())
                                        .with_status_code(StatusCode(500));
                                }
                            }
                            _ => {
                                return json_response(&FulfillmentResponse {
                                    request_id: request_id,
                                    payload: json!({
                                        "errorCode": ErrorCodes::NotSupported,
                                        "debugString": "unknown command",
                                    }),
                                })
                            }
                        }
                        // TODO(stvn): Do all executions in the array, improve error handling
                        return json_response(&FulfillmentResponse {
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

    json_response(&FulfillmentResponse {
        request_id: request_id,
        payload: json!({
            "errorCode": ErrorCodes::NotSupported,
            "debugString": "no inputs provided",
        }),
    })
}

fn varz() -> Response<Cursor<Vec<u8>>> {
    let metrics = prometheus::gather();
    let encoder = prometheus::TextEncoder::new();
    match encoder.encode_to_string(&metrics) {
        Ok(encoded) => Response::from_string(encoded),
        Err(err) => Response::from_string(err.to_string()).with_status_code(StatusCode(500)),
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

    let app_state = AppState {
        cec: conn,
        server_mac_addr,
    };

    info!("Starting server...");
    let server = Server::http(&args.http_addr).unwrap();

    for mut request in server.incoming_requests() {
        info!(
            "{method} {url}",
            method = request.method(),
            url = request.url(),
        );

        let app_state = app_state.clone();

        std::thread::spawn(move || {
            let route = |req: &mut Request| -> Response<Cursor<Vec<u8>>> {
                let url = req.url().to_string();
                let path = if let Some(idx) = url.find('?') {
                    &url[..idx]
                } else {
                    &url
                };

                match (req.method(), path) {
                    (Method::Get, "/") => index(),
                    (Method::Get, "/manifest.json") => manifest(),
                    (Method::Get, "/varz") => varz(),
                    (Method::Post, "/fulfillment") => fulfillment(app_state, req),
                    _ => Response::from_data(Vec::new()).with_status_code(StatusCode(404)),
                }
            };

            let resp = route(&mut request);

            info!("... {status}", status = resp.status_code().0);
            let _ = request.respond(resp);
        });
    }

    Ok(())
}
