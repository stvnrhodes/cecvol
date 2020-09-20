// use actix::prelude::*;

// use cec_rs::{
//     CecCommand, CecConnection, CecConnectionCfgBuilder, CecDeviceType, CecDeviceTypeVec,
//     CecKeypress,
// };

// struct VolumeState {
//     current_volume: i32,
//     is_muted: bool,
// }

// #[derive(Message)]
// #[rtype(result = "Result<bool, std::io::Error>")]
// struct Mute {
//     mute: bool,
// }

// #[derive(Message)]
// #[rtype(result = "Result<bool, std::io::Error>")]
// struct SetVolume {
//     volume_level: i32,
// }

// #[derive(Message)]
// #[rtype(result = "Result<bool, std::io::Error>")]
// struct VolumeRelative {
//     relative_steps: i32,
// }

// // Messages
// //
// /// Define message
// #[derive(Message)]
// #[rtype(result = "Result<bool, std::io::Error>")]
// struct Ping;

// // Define actor
// pub struct CECActor {
//     conn: CecConnection,
// }

// // Provide Actor implementation for our actor
// impl Actor for CECActor {
//     type Context = Context<Self>;

//     fn started(&mut self, ctx: &mut Context<Self>) {
//         let cfg = CecConnectionCfgBuilder::default()
//             .port("RPI".into())
//             .device_name("Pi".into())
//             .activate_source(false) /* Don't auto-turn on the TV */
//             .device_types(CecDeviceTypeVec::new(CecDeviceType::RecordingDevice))
//             .key_press_callback(Box::new(on_key_press))
//             .command_received_callback(Box::new(on_command_received))
//             .build()
//             .unwrap();
//         let conn = cfg.open().unwrap();
//         println!("CECActor is alive");
//     }

//     fn stopped(&mut self, ctx: &mut Context<Self>) {
//         println!("Actor is stopped");
//     }
// }

// /// Define handler for `Ping` message
// impl Handler<Ping> for CECActor {
//     type Result = Result<bool, std::io::Error>;

//     fn handle(&mut self, msg: Ping, ctx: &mut Context<Self>) -> Self::Result {
//         println!("Ping received");

//         Ok(true)
//     }
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
