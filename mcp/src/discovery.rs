use crate::crypto::WirebodyKeys;
use crate::http::Endpoint;
use mdns_sd::{ServiceDaemon, ServiceEvent};
use std::time::{Duration, Instant};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DiscoveryError {
    #[error("mDNS discovery failed: {0}")]
    Mdns(String),
    #[error(
        "Cannot discover Wirebody instance {instance} on {service_type} within {timeout_ms}ms"
    )]
    Timeout {
        instance: String,
        service_type: String,
        timeout_ms: u128,
    },
    #[error("discovered Wirebody service has no IPv4 address")]
    MissingAddress,
}

pub fn discover_wirebody_endpoint(
    service_type: &str,
    keys: &WirebodyKeys,
    timeout: Duration,
) -> Result<Endpoint, DiscoveryError> {
    let mdns = ServiceDaemon::new().map_err(|error| DiscoveryError::Mdns(error.to_string()))?;
    let receiver = mdns
        .browse(service_type)
        .map_err(|error| DiscoveryError::Mdns(error.to_string()))?;
    let deadline = Instant::now() + timeout;
    let expected = keys.instance_label();

    while Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match receiver.recv_timeout(remaining.min(Duration::from_millis(250))) {
            Ok(ServiceEvent::ServiceResolved(service)) => {
                if !fullname_matches(service.get_fullname(), expected, service_type) {
                    continue;
                }
                let Some(address) = service.get_addresses_v4().into_iter().next() else {
                    let _ = mdns.shutdown();
                    return Err(DiscoveryError::MissingAddress);
                };
                let endpoint =
                    Endpoint::from_parts("https", address.to_string(), service.get_port())
                        .map_err(|error| DiscoveryError::Mdns(error.to_string()))?;
                let _ = mdns.shutdown();
                return Ok(endpoint);
            }
            Ok(_) => {}
            Err(_) => {}
        }
    }

    let _ = mdns.shutdown();
    Err(DiscoveryError::Timeout {
        instance: expected.to_string(),
        service_type: service_type.to_string(),
        timeout_ms: timeout.as_millis(),
    })
}

fn fullname_matches(fullname: &str, instance_label: &str, service_type: &str) -> bool {
    let mut expected = String::with_capacity(instance_label.len() + service_type.len() + 1);
    expected.push_str(instance_label);
    expected.push('.');
    expected.push_str(service_type.trim_start_matches('.'));
    fullname.eq_ignore_ascii_case(&expected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_dns_sd_fullname_case_insensitively() {
        assert!(fullname_matches(
            "abcd._wirebody._tcp.local.",
            "ABCD",
            "_wirebody._tcp.local."
        ));
        assert!(!fullname_matches(
            "other._wirebody._tcp.local.",
            "abcd",
            "_wirebody._tcp.local."
        ));
    }
}
