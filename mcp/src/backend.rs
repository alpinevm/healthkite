use crate::config::Config;
use crate::discovery;
use crate::http::{HttpError, HttpResponse, HttpTransport, PskHttpConnection, PskMaterial};
use crate::healthkite::HealthKiteClient;
use parking_lot::Mutex;
use std::sync::Arc;

pub fn client_from_config(config: Config) -> HealthKiteClient {
    let description = format!(
        "Bonjour service {}.{}",
        config.keys.instance_label(),
        config.service_type.trim_start_matches('.')
    );
    HealthKiteClient::new(description, Arc::new(DiscoveringTransport::new(config)))
}

struct DiscoveringTransport {
    config: Config,
    psk: PskMaterial,
    connection: Mutex<Option<PskHttpConnection>>,
}

impl DiscoveringTransport {
    fn new(config: Config) -> Self {
        let psk = PskMaterial::new(config.keys.psk_identity().to_vec(), config.keys.psk().to_vec());
        Self {
            config,
            psk,
            connection: Mutex::new(None),
        }
    }
}

impl HttpTransport for DiscoveringTransport {
    fn get(&self, path_and_query: &str) -> Result<HttpResponse, HttpError> {
        let mut connection = self.connection.lock();

        if let Some(cached) = connection.as_mut() {
            match cached.get(path_and_query) {
                Ok(response) => return Ok(response),
                Err(error) => {
                    eprintln!(
                        "healthkite-mcp: cached HealthKite MCP connection failed; reconnecting: {error}"
                    );
                    *connection = None;
                }
            }
        }

        let endpoint = discovery::discover_healthkite_endpoint(
            &self.config.service_type,
            &self.config.keys,
            self.config.discovery_timeout,
        )
        .map_err(|error| HttpError::Connect(error.to_string()))?;

        let mut fresh = PskHttpConnection::connect(endpoint, &self.psk)?;
        let response = fresh.get(path_and_query)?;
        *connection = Some(fresh);
        Ok(response)
    }
}
