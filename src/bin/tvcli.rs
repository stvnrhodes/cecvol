use cecvol::lgip;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Keycode for pairing the LG tv with the server.
    #[arg(long, env = "LG_KEYCODE")]
    keycode: String,

    /// TV MAC address for WoL.
    #[arg(long, env = "LG_MAC_ADDR")]
    mac_addr: String,

    commands: Vec<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let mut mac_addr = [0u8; 6];
    for (i, s) in args.mac_addr.split(":").enumerate() {
        mac_addr[i] = u8::from_str_radix(s, 16)?;
    }
    let tv = lgip::LGTV::new("LGWebOSTV.local".to_string(), mac_addr, &args.keycode);
    let mut cmd = args.commands.join(" ");
    cmd.push_str("\r");
    println!("{}", cmd);
    println!("{}", tv.send_command(&cmd)?);
    Ok(())
}
