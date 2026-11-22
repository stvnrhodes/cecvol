use cecvol::action::fulfillment::FulfillmentHandler;
use cecvol::cec;
use cecvol::lgip;
use cecvol::oauth;
use cecvol::server::{self, AppState};
use cecvol::tv;

use clap::Parser;
use log::{info, warn};
use std::sync::{Arc, Mutex};

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

    /// OAuth 2.0 client ID for Google Home Cloud-to-Cloud account linking.
    #[arg(long, env = "OAUTH_CLIENT_ID")]
    oauth_client_id: Option<String>,

    /// OAuth 2.0 client secret for Google Home Cloud-to-Cloud account linking (min 32 bytes).
    #[arg(long, env = "OAUTH_CLIENT_SECRET")]
    oauth_client_secret: Option<String>,

    /// Separate HMAC signing key for OAuth tokens (min 32 bytes; rotate to revoke all tokens).
    #[arg(long, env = "OAUTH_SIGNING_KEY")]
    oauth_signing_key: Option<String>,

    /// Google Home Developer Console project ID (automatically allows Google's prod + sandbox OAuth redirect URIs).
    #[arg(long, env = "GOOGLE_PROJECT_ID")]
    google_project_id: Option<String>,

    /// Additional allowed OAuth redirect URIs (comma-separated).
    #[arg(long, env = "OAUTH_REDIRECT_URIS", value_delimiter = ',')]
    oauth_redirect_uri: Vec<String>,

    /// Default relative volume step size advertised to Google Home.
    #[arg(long, env = "VOLUME_STEP_SIZE", default_value_t = 3)]
    volume_step_size: i32,
}

fn parse_mac(s: &str) -> Result<[u8; 6], Box<dyn std::error::Error>> {
    let mut mac = [0u8; 6];
    for (i, part) in s.split(':').enumerate() {
        mac[i] = u8::from_str_radix(part, 16)?;
    }
    Ok(mac)
}

fn build_oauth_server(args: &Args) -> Result<Option<Arc<oauth::OAuthServer>>, String> {
    match (
        &args.oauth_client_id,
        &args.oauth_client_secret,
        &args.oauth_signing_key,
    ) {
        (None, None, None) => Ok(None),
        (Some(client_id), Some(client_secret), Some(signing_key)) => {
            let mut redirect_uris = args.oauth_redirect_uri.clone();
            if let Some(project_id) = &args.google_project_id {
                if !project_id.is_empty() {
                    redirect_uris.extend(oauth::google_redirect_uris(project_id));
                }
            }
            if redirect_uris.is_empty() {
                warn!("OAuth client configured without --google-project-id or --oauth-redirect-uri; /oauth/authorize will reject all redirect URIs");
            }
            let server = oauth::OAuthServer::new(
                client_id.clone(),
                client_secret.clone(),
                signing_key.clone(),
                redirect_uris.clone(),
            )
            .map_err(|e| e.to_string())?;
            info!(
                "Google Home OAuth 2.0 enabled ({} allowed redirect URI(s))",
                redirect_uris.len()
            );
            Ok(Some(Arc::new(server)))
        }
        _ => Err(
            "OAUTH_CLIENT_ID, OAUTH_CLIENT_SECRET, and OAUTH_SIGNING_KEY must all be set to enable Google Home OAuth 2.0"
                .to_string(),
        ),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("INFO"))
        .format_timestamp(Some(env_logger::fmt::TimestampPrecision::Millis))
        .init();

    let tv: Box<dyn tv::TVConnection + Sync + Send> = if args.use_lg_ip_control {
        let tv_mac_addr = parse_mac(args.lg_mac_addr.as_deref().unwrap())?;
        Box::new(lgip::LGTV::new(
            "LGWebOSTV.local".to_string(),
            tv_mac_addr,
            args.lg_keycode.as_deref().unwrap(),
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

    let app_state = AppState {
        fulfillment: FulfillmentHandler {
            cec: Arc::new(Mutex::new(tv)),
            server_mac_addr: parse_mac(&args.server_mac_addr)?,
            volume_step_size: args.volume_step_size,
        },
        oauth: build_oauth_server(&args)?,
    };

    server::run(&args.http_addr, app_state)
}
