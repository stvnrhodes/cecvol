use cecvol::lgip;
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    let tv = lgip::LGTV::new("LGWebOSTV.local".to_string(), [0x64, 0x95, 0x6c, 0x06, 0x84, 0x98], "0J8FOLOW");
    let mut cmd = args[1..].join(" ");
    cmd.push_str("\r");
    println!("{}", tv.send_command(&cmd)?);
    Ok(())
}
