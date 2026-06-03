use std::process;
use std::sync::Arc;
use wirebody_mcp::config::Config;
use wirebody_mcp::discovery;
use wirebody_mcp::http::{PskMaterial, RawHttpTransport};
use wirebody_mcp::mcp;
use wirebody_mcp::wirebody::WirebodyClient;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env()?;
    let endpoint = discovery::discover_wirebody_endpoint(
        &config.service_type,
        &config.keys,
        config.discovery_timeout,
    )?;
    let psk = PskMaterial::new(
        config.keys.psk_identity().to_vec(),
        config.keys.psk().to_vec(),
    );
    let transport = RawHttpTransport::new(endpoint.clone(), psk);
    let client = WirebodyClient::new(endpoint.to_string(), Arc::new(transport));
    mcp::run_stdio(client)?;
    Ok(())
}
