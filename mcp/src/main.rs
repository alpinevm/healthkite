use std::process;
use healthkite_mcp::backend;
use healthkite_mcp::config::Config;
use healthkite_mcp::mcp;

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
