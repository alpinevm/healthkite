use data_encoding::BASE32_NOPAD;
use hkdf::Hkdf;
use sha2::Sha256;

pub const SERVICE_TYPE: &str = "_wirebody._tcp.local.";
const DISCOVERY_INFO: &[u8] = b"wirebody:discovery:v1";
const AUTH_INFO: &[u8] = b"wirebody:auth:v1";
const DISCOVERY_ID_LEN: usize = 16;
const PSK_LEN: usize = 32;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WirebodyKeys {
    discovery_id: [u8; DISCOVERY_ID_LEN],
    psk: [u8; PSK_LEN],
    instance_label: String,
}

impl WirebodyKeys {
    pub fn derive(root: &[u8]) -> Result<Self, hkdf::InvalidLength> {
        let hk = Hkdf::<Sha256>::new(None, root);

        let mut discovery_id = [0_u8; DISCOVERY_ID_LEN];
        hk.expand(DISCOVERY_INFO, &mut discovery_id)?;

        let mut psk = [0_u8; PSK_LEN];
        hk.expand(AUTH_INFO, &mut psk)?;

        let instance_label = BASE32_NOPAD.encode(&discovery_id).to_ascii_lowercase();
        Ok(Self {
            discovery_id,
            psk,
            instance_label,
        })
    }

    pub fn discovery_id(&self) -> &[u8; DISCOVERY_ID_LEN] {
        &self.discovery_id
    }

    pub fn psk(&self) -> &[u8; PSK_LEN] {
        &self.psk
    }

    pub fn instance_label(&self) -> &str {
        &self.instance_label
    }

    pub fn psk_identity(&self) -> &[u8] {
        self.instance_label.as_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_separated_discovery_and_auth_keys() {
        let keys = WirebodyKeys::derive(b"0123456789abcdef").unwrap();
        assert_eq!(keys.discovery_id().len(), 16);
        assert_eq!(keys.psk().len(), 32);
        assert_ne!(&keys.psk()[..16], keys.discovery_id());
        assert_eq!(keys.instance_label().len(), 26);
        assert!(keys
            .instance_label()
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit()));
    }

    #[test]
    fn derivation_is_stable() {
        let first = WirebodyKeys::derive(b"root-token").unwrap();
        let second = WirebodyKeys::derive(b"root-token").unwrap();
        assert_eq!(first, second);
    }
}
