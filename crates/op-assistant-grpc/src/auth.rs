//! WireGuard identity extraction and authentication middleware.
//!
//! Trust is established at the WireGuard network layer; this middleware merely
//! extracts the public-key metadata and attaches it to the request extensions
//! so downstream handlers can attribute the call.

use tonic::{metadata::MetadataMap, Request, Status};

pub const WIREGUARD_PUBKEY_HEADER: &str = "x-wireguard-pubkey";

#[derive(Debug, Clone)]
pub struct WireGuardIdentity {
    pub pubkey: String,
}

#[allow(clippy::result_large_err)]
pub fn extract_wireguard_identity(metadata: &MetadataMap) -> Result<WireGuardIdentity, Status> {
    let raw = metadata
        .get(WIREGUARD_PUBKEY_HEADER)
        .ok_or_else(|| Status::unauthenticated("missing wireguard identity"))?;
    let pubkey = raw
        .to_str()
        .map_err(|_| Status::invalid_argument("invalid wireguard pubkey encoding"))?
        .to_string();

    if pubkey.is_empty() {
        return Err(Status::unauthenticated("empty wireguard pubkey"));
    }
    Ok(WireGuardIdentity { pubkey })
}

/// Tonic interceptor: extracts the WireGuard identity and attaches it to the
/// request extensions. Returns `Unauthenticated` when the header is missing.
#[allow(clippy::result_large_err)]
pub fn wireguard_auth_interceptor(mut req: Request<()>) -> Result<Request<()>, Status> {
    let identity = extract_wireguard_identity(req.metadata())?;
    req.extensions_mut().insert(identity);
    Ok(req)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tonic::metadata::MetadataValue;

    #[test]
    fn rejects_missing_identity() {
        let req = Request::new(());
        let result = wireguard_auth_interceptor(req);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::Unauthenticated);
    }

    #[test]
    fn accepts_valid_identity() {
        let mut req = Request::new(());
        req.metadata_mut().insert(
            WIREGUARD_PUBKEY_HEADER,
            MetadataValue::from_static("abcd1234"),
        );
        let result = wireguard_auth_interceptor(req).unwrap();
        let id = result.extensions().get::<WireGuardIdentity>().unwrap();
        assert_eq!(id.pubkey, "abcd1234");
    }

    #[test]
    fn rejects_empty_identity() {
        let mut req = Request::new(());
        req.metadata_mut()
            .insert(WIREGUARD_PUBKEY_HEADER, MetadataValue::from_static(""));
        let result = wireguard_auth_interceptor(req);
        assert!(result.is_err());
    }
}
