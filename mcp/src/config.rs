use crate::crypto::{HealthKiteKeys, SERVICE_TYPE};
use std::env;
use std::time::Duration;
use thiserror::Error;

const DEFAULT_DISCOVERY_TIMEOUT_MS: u64 = 3_000;

#[derive(Clone, Debug)]
pub struct Config {
    pub service_type: String,
    pub discovery_timeout: Duration,
    pub keys: HealthKiteKeys,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error(
        "HEALTHKITE_TOKEN or HEALTHKITE_ROOT is required for mDNS discovery and TLS-PSK authentication"
    )]
    MissingRootSecret,
    #[error(
        "HEALTHKITE_URL is no longer supported; HealthKite MCP now requires mDNS discovery and TLS-PSK"
    )]
    ManualUrlUnsupported,
    #[error("{0}")]
    InvalidValue(String),
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_env_map(env::vars())
    }

    pub fn from_env_map<I, K, V>(vars: I) -> Result<Self, ConfigError>
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        let env: std::collections::HashMap<String, String> = vars
            .into_iter()
            .map(|(key, value)| (key.into(), value.into()))
            .collect();

        if env
            .get("HEALTHKITE_URL")
            .is_some_and(|value| !value.trim().is_empty())
        {
            return Err(ConfigError::ManualUrlUnsupported);
        }

        let root = env
            .get("HEALTHKITE_ROOT")
            .or_else(|| env.get("HEALTHKITE_TOKEN"))
            .filter(|value| !value.trim().is_empty())
            .ok_or(ConfigError::MissingRootSecret)?;

        let keys = HealthKiteKeys::derive(root.as_bytes())
            .map_err(|_| ConfigError::InvalidValue("failed to derive HealthKite MCP keys".to_string()))?;

        let service_type = env
            .get("HEALTHKITE_SERVICE_TYPE")
            .filter(|value| !value.trim().is_empty())
            .cloned()
            .unwrap_or_else(|| SERVICE_TYPE.to_string());

        let discovery_timeout = env
            .get("HEALTHKITE_DISCOVERY_TIMEOUT_MS")
            .filter(|value| !value.trim().is_empty())
            .map(|value| {
                value
                    .parse::<u64>()
                    .map(Duration::from_millis)
                    .map_err(|_| {
                        ConfigError::InvalidValue(
                            "HEALTHKITE_DISCOVERY_TIMEOUT_MS must be an integer".to_string(),
                        )
                    })
            })
            .transpose()?
            .unwrap_or_else(|| Duration::from_millis(DEFAULT_DISCOVERY_TIMEOUT_MS));

        Ok(Self {
            service_type,
            discovery_timeout,
            keys,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requires_token_for_discovery() {
        let error = Config::from_env_map(std::iter::empty::<(&str, &str)>()).unwrap_err();
        assert!(matches!(error, ConfigError::MissingRootSecret));
    }

    #[test]
    fn rejects_manual_url() {
        let error = Config::from_env_map([
            ("HEALTHKITE_URL", "http://phone.local:5606"),
            ("HEALTHKITE_TOKEN", "0123456789abcdef"),
        ])
        .unwrap_err();
        assert!(matches!(error, ConfigError::ManualUrlUnsupported));
    }

    #[test]
    fn derives_keys_from_healthkite_token() {
        let cfg = Config::from_env_map([("HEALTHKITE_TOKEN", "0123456789abcdef")]).unwrap();
        assert_eq!(cfg.keys.psk().len(), 32);
    }
}
