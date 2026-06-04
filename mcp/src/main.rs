use std::process;
use wirebody_mcp::backend;
use wirebody_mcp::config::Config;
use wirebody_mcp::mcp;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env()?;
    let client = backend::client_from_config(config);
    mcp::run_stdio(client)?;
    Ok(())
}
