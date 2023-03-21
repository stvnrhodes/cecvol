use cecvol::lgip;
use std::{
    env,
    net::{IpAddr, Ipv4Addr},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    let addr = IpAddr::V4(Ipv4Addr::new(192, 168, 86, 39));
    let tv = lgip::LGTV::new(addr, [0x64, 0x95, 0x6c, 0x06, 0x84, 0x98], "0J8FOLOW");
    let mut cmd = args[1..].join(" ");
    cmd.push_str("\r");
    println!("{}", tv.send_command(&cmd)?);
    Ok(())
}
