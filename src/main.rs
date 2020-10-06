use actix_http::Response;
use actix_web::{get, middleware, post, web, App, HttpServer, Responder};
use env_logger;
use serde::{Deserialize, Serialize};

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
//https://developers.google.com/assistant/smarthome/develop/process-intents#sync-response
// /fulfillment
// Content-Type: application/json
// Authorization: Bearer ACCESS_TOKEN
// {
//     "requestId": "ff36a3cc-ec34-11e6-b1a0-64510650abcf",
//     "inputs": [{
//       "intent": "action.devices.SYNC"
//     }]
// }

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FulfillmentRequest {
    request_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FulfillmentResponse {
    request_id: String,
}

#[post("/fulfillment")]
async fn fulfillment(req: web::Json<FulfillmentRequest>) -> web::Json<FulfillmentResponse> {
    // https://developers.google.com/assistant/smarthome/develop/implement-oauth#implement_oauth_account_linking
    // Todo: verify client id and redirect uri is correct
    // check that user is signed in
    // generate auth code, expire after 10m
    //redirect browser to redirect_uri, include code and state
    web::Json(FulfillmentResponse {
        request_id: req.request_id.clone(),
    })
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // let cecActor = CECActor::start();
    env_logger::init();
    HttpServer::new(|| {
        App::new()
            // .app_data(cecActor)
            .wrap(middleware::Logger::default())
            .wrap(middleware::Compress::default())
            .service(fulfillment)
            .service(auth)
            .route("/", web::get().to(index))
        // .route("/{name}", web::get().to(greet))
    })
    .bind("0.0.0.0:8080")?
    .run()
    .await
}

// todo
// read vol
// need to read up on google home graph
// allow mute
// for now, curl to test out each?
//https://developers.google.com/assistant/`/develop/create#setup-server
// use anyhow::Result;
// use cec_rs::{CecCommand, CecConnectionCfgBuilder, CecDeviceType, CecDeviceTypeVec, CecKeypress};
// use std::io::{stdin, stdout, Write};
// use termion::event::Key;
// use termion::input::TermRead;
// use termion::raw::IntoRawMode;

// fn main() -> Result<()> {
//     // Set up terminal to accept keypresses
//     let stdin = stdin();
//     let mut stdout = stdout().into_raw_mode()?;
//     stdout.flush()?;

//     let cfg = CecConnectionCfgBuilder::default()
//         .port("RPI".into())
//         .device_name("Pi".into())
//         .activate_source(false) /* Don't auto-turn on the TV */
//         .device_types(CecDeviceTypeVec::new(CecDeviceType::RecordingDevice))
//         .key_press_callback(Box::new(on_key_press))
//         .command_received_callback(Box::new(on_command_received))
//         .build()
//         .unwrap();
//     let conn = cfg.open().unwrap();

//     for c in stdin.keys() {
//         print!("{}", termion::clear::CurrentLine);

//         match c.unwrap() {
//             Key::Char('q') => break,
//             Key::Char('m') => {
//                 conn.audio_toggle_mute().unwrap();
//                 print!("m\r\n");
//             }
//             Key::Char(c) => {
//                 print!("{}\r\n", c);
//             }
//             Key::Up => {
//                 print!("u");
//                 conn.volume_up(true).unwrap();
//                 print!("p\r\n");
//             }
//             Key::Down => {
//                 print!("d");
//                 conn.volume_down(true).unwrap();
//                 print!("own\r\n");
//             }
//             _ => {}
//         };
//     }

//     Ok(())
// }

// fn on_key_press(keypress: CecKeypress) {
//     println!(
//         "onKeyPress: {:?}, keycode: {:?}, duration: {:?}",
//         keypress, keypress.keycode, keypress.duration
//     );
// }

// fn on_command_received(command: CecCommand) {
//     println!(
//         "onCommandReceived:  opcode: {:?}, initiator: {:?}",
//         command.opcode, command.initiator
//     );
// }
