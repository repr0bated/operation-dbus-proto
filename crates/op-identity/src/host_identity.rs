//! Canonical host identity for 3tched — your WireGuard peer on the decoy mesh.
//!
//! The container **is** the sled **is** the identity: `session_id` == Incus name.
//! These constants are the real host pubkey and mesh anchor; a fixed fixture PSK
//! is included only so CI and local provisioning can derive a stable session_id.
//! Production signup still generates a fresh PSK per account.

use anyhow::Result;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};

use crate::{derive_session_id_from_psk, session_proof};

/// 3tched hub peer on the decoy VPS mesh (`10.0.0.2`).
pub const HOST_PEER_PUBKEY: &str = "iNqgBk7pBC/iXUmF+v4PNvvdAxjP5qWjxzMYYRE8+hw=";

/// Decoy VPS WireGuard hub (identity ingress), not this host's local iface.
pub const DECOY_HUB_PUBKEY: &str = "6mx4ycJeDMEDUknDY+sVlus1PQOEGG9/XrGFBuB1GFY=";

/// Mesh IP stamped on the host at decoy ingress.
pub const HOST_PEER_IP: &str = "10.0.0.2/32";

/// Fixed 32-byte PSK (base64) for deterministic session derivation in dev/CI.
/// Not the production signup PSK — only anchors a reproducible session_id.
pub const FIXTURE_PSK_B64: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";

/// Resolved host identity: pubkey + session_id == container name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostIdentity {
    pub wg_pubkey: String,
    pub psk_b64: String,
    pub session_id: String,
    pub container_name: String,
    pub session_proof: String,
    pub peer_ip: String,
}

/// Materialize the canonical host identity record.
pub fn host_identity() -> Result<HostIdentity> {
    let psk = BASE64.decode(FIXTURE_PSK_B64)?;
    let session_id = derive_session_id_from_psk(HOST_PEER_PUBKEY, &psk)?;
    let session_proof = session_proof(&session_id);
    Ok(HostIdentity {
        wg_pubkey: HOST_PEER_PUBKEY.to_string(),
        psk_b64: FIXTURE_PSK_B64.to_string(),
        session_id: session_id.clone(),
        container_name: session_id,
        session_proof,
        peer_ip: HOST_PEER_IP.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verify_session_proof;

    #[test]
    fn host_session_id_is_stable() {
        let a = host_identity().expect("host identity");
        let b = host_identity().expect("host identity");
        assert_eq!(a.session_id, b.session_id);
        assert_eq!(a.container_name, a.session_id);
        assert_eq!(a.wg_pubkey, HOST_PEER_PUBKEY);
        assert!(verify_session_proof(&a.session_id, &a.session_proof));
    }

    #[test]
    fn container_name_equals_session_id() {
        let id = host_identity().unwrap();
        assert_eq!(id.container_name, id.session_id);
    }
}
