use crate::config::Config;
use crate::discovery;
use crate::http::{HttpError, HttpResponse, HttpTransport, PskMaterial, RawHttpTransport};
use crate::wirebody::WirebodyClient;
use std::sync::Arc;

pub fn client_from_config(config: Config) -> WirebodyClient {
    let description = format!(
        "Bonjour service {}.{}",
        config.keys.instance_label(),
        config.service_type.trim_start_matches('.')
    );
    WirebodyClient::new(description, Arc::new(DiscoveringTransport::new(config)))
}

struct DiscoveringTransport {
    config: Config,
    psk: PskMaterial,
}

impl DiscoveringTransport {
    fn new(config: Config) -> Self {
        let psk = PskMaterial::new(config.keys.psk_identity().to_vec(), config.keys.psk().to_vec());
        Self { config, psk }
    }
}

impl HttpTransport for DiscoveringTransport {
    fn get(&self, path_and_query: &str) -> Result<HttpResponse, HttpError> {
        let endpoint = discovery::discover_wirebody_endpoint(
            &self.config.service_type,
            &self.config.keys,
            self.config.discovery_timeout,
        )
        .map_err(|error| HttpError::Connect(error.to_string()))?;

        RawHttpTransport::new(endpoint, self.psk.clone()).get(path_and_query)
    }
}
