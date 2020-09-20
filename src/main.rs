// use actix_web::{middleware, web, App, HttpRequest, HttpServer, Responder};

// mod cec;
// use cec::CECActor;
// async fn greet(req: HttpRequest) -> impl Responder {
//     let name = req.match_info().get("name").unwrap_or("World");
//     format!("Hello {}!", &name)
// }

// #[actix_web::main]
// async fn main() -> std::io::Result<()> {
//     let cecActor = CECActor::start();

//     HttpServer::new(|| {
//         App::new()
//             .data(cecActor)
//             .wrap(middleware::Logger::default())
//             .route("/", web::get().to(greet))
//             .route("/{name}", web::get().to(greet))
//     })
//     .bind("127.0.0.1:8080")?
//     .run()
//     .await

// todo
// read vol
// need to read up on google home graph
// allow mute
// for now, curl to test out each?
//https://developers.google.com/assistant/`/develop/create#setup-server
use anyhow::Result;
use cec_rs::{CecCommand, CecConnectionCfgBuilder, CecDeviceType, CecDeviceTypeVec, CecKeypress};
use std::io::{stdin, stdout, Write};
use termion::event::Key;
use termion::input::TermRead;
use termion::raw::IntoRawMode;

fn main() -> Result<()> {
    // Set up terminal to accept keypresses
    let stdin = stdin();
    let mut stdout = stdout().into_raw_mode()?;
    stdout.flush()?;

    let cfg = CecConnectionCfgBuilder::default()
        .port("RPI".into())
        .device_name("Pi".into())
        .activate_source(false) /* Don't auto-turn on the TV */
        .device_types(CecDeviceTypeVec::new(CecDeviceType::RecordingDevice))
        .key_press_callback(Box::new(on_key_press))
        .command_received_callback(Box::new(on_command_received))
        .build()
        .unwrap();
    let conn = cfg.open().unwrap();

    for c in stdin.keys() {
        print!("{}", termion::clear::CurrentLine);

        match c.unwrap() {
            Key::Char('q') => break,
            Key::Char('m') => {
                conn.audio_toggle_mute().unwrap();
                print!("m\r\n");
            }
            Key::Char(c) => {
                print!("{}\r\n", c);
            }
            Key::Up => {
                print!("u");
                conn.volume_up(true).unwrap();
                print!("p\r\n");
            }
            Key::Down => {
                print!("d");
                conn.volume_down(true).unwrap();
                print!("own\r\n");
            }
            _ => {}
        };
    }

    Ok(())
}

fn on_key_press(keypress: CecKeypress) {
    println!(
        "onKeyPress: {:?}, keycode: {:?}, duration: {:?}",
        keypress, keypress.keycode, keypress.duration
    );
}

fn on_command_received(command: CecCommand) {
    println!(
        "onCommandReceived:  opcode: {:?}, initiator: {:?}",
        command.opcode, command.initiator
    );
}
