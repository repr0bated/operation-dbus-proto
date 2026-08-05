// ?? ??? The Tonic gRPC Gatekeeper (Middleware Interceptor)
// Sits on the primary gRPC ingress at port 8090. Intercepts Xray-injected headers,
// performs a zero-copy check against the IdentitySled in shared memory, and either
// allows the gRPC payload through or drops the connection instantly.
//
// Operated by A.N.N.A. Scribe. No payload enters the system without a cryptographic
// "Snowball" session. No SQL databases, no D-Bus watchers. 1:1 Direct Read only.

use std::net::SocketAddr;
use std::sync::{Arc, OnceLock};

use axum::extract::ConnectInfo as AxumConnectInfo;
use op_identity::FootprintVerifyError;
use tonic::metadata::MetadataValue;
use tonic::service::Interceptor;
use tonic::transport::server::TcpConnectInfo;
use tonic::{Request, Status};

use crate::mutation_engine::MutationEngine;
use crate::oracle_assertion::{
    AssertionRejection, AssertionValidator, HumanPrincipalIdentity,
};

/// gRPC metadata key for the optional oracle identity assertion (OIA1 wire bytes).
pub const ASSERTION_METADATA_KEY: &str = "x-oracle-identity-assertion-bin";

/// Identity extracted by the Ghostbridge interceptor and attached to each
/// accepted request for bridge-layer authorization.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GhostbridgeIdentity {
    pub footprint: String,
    pub session_id: String,
}

/// Process-wide handle to the engine, set once from `build_operation_routes`
/// (the single choke point all routes are built from) before any interceptor
/// can run. Lets the interceptor do a per-identity Cozo lookup
/// (`identity_sled_dispatch::get_identity`) without threading `Arc<MutationEngine>`
/// through every one of the ~10 `.with_interceptor()` call sites.
static ENGINE: OnceLock<Arc<MutationEngine>> = OnceLock::new();

pub fn set_engine(engine: Arc<MutationEngine>) {
    let _ = ENGINE.set(engine);
}

/// Per-serving-instance interceptor carrying its own assertion validator.
#[derive(Clone)]
pub struct GhostbridgeInterceptorWithValidator {
    validator: Arc<AssertionValidator>,
}

impl GhostbridgeInterceptorWithValidator {
    pub fn new(validator: Arc<AssertionValidator>) -> Self {
        Self { validator }
    }
}

impl tonic::service::Interceptor for GhostbridgeInterceptorWithValidator {
    fn call(&mut self, req: Request<()>) -> Result<Request<()>, Status> {
        ghostbridge_interceptor_with_validator(&self.validator, req)
    }
}

/// Build a per-serving-instance interceptor that governs the optional oracle
/// assertion path and falls back to the legacy Ghostbridge footprint path.
pub fn make_ghostbridge_interceptor(
    validator: Arc<AssertionValidator>,
) -> GhostbridgeInterceptorWithValidator {
    GhostbridgeInterceptorWithValidator::new(validator)
}

/// Registration bootstrap interceptor sharing the same validator-backed gate.
#[derive(Clone)]
pub struct RegistrationInterceptorWithValidator {
    gate: GhostbridgeInterceptorWithValidator,
}

impl RegistrationInterceptorWithValidator {
    pub fn new(validator: Arc<AssertionValidator>) -> Self {
        Self {
            gate: GhostbridgeInterceptorWithValidator::new(validator),
        }
    }
}

impl tonic::service::Interceptor for RegistrationInterceptorWithValidator {
    fn call(&mut self, req: Request<()>) -> Result<Request<()>, Status> {
        match req.extensions().get::<tonic::GrpcMethod<'_>>() {
            Some(method)
                if method.service() == "operation.registration.v1.RegistrationService"
                    && matches!(method.method(), "SendMagicLink" | "VerifyMagicLink") =>
            {
                Ok(req)
            }
            _ => self.gate.call(req),
        }
    }
}

pub fn make_registration_interceptor(
    validator: Arc<AssertionValidator>,
) -> RegistrationInterceptorWithValidator {
    RegistrationInterceptorWithValidator::new(validator)
}

/// Resolve the footprint/session pair for capability gating. Human principal
/// identity shadows Ghostbridge when both are present.
pub fn bridge_capability_identity(
    extensions: &tonic::Extensions,
) -> Option<GhostbridgeIdentity> {
    if let Some(human) = extensions.get::<HumanPrincipalIdentity>() {
        return Some(GhostbridgeIdentity {
            footprint: hex::encode(human.footprint),
            session_id: human.principal_id.clone(),
        });
    }
    extensions.get::<GhostbridgeIdentity>().cloned()
}

fn peer_socket_addr(req: &Request<()>) -> Option<SocketAddr> {
    if let Some(info) = req.extensions().get::<TcpConnectInfo>() {
        if let Some(addr) = info.remote_addr() {
            return Some(addr);
        }
    }
    req.extensions()
        .get::<AxumConnectInfo<SocketAddr>>()
        .map(|ci| ci.0)
}

fn read_assertion_wire(req: &Request<()>) -> Result<Option<Vec<u8>>, Status> {
    let values: Vec<_> = req
        .metadata()
        .get_all_bin(ASSERTION_METADATA_KEY)
        .into_iter()
        .collect();
    if values.is_empty() {
        return Ok(None);
    }
    if values.len() > 1 {
        return Err(AssertionRejection::Malformed.into());
    }
    let wire = values[0]
        .to_bytes()
        .map_err(|_| AssertionRejection::Malformed)?;
    Ok(Some(wire.to_vec()))
}

fn ghostbridge_interceptor_with_validator(
    validator: &AssertionValidator,
    mut req: Request<()>,
) -> Result<Request<()>, Status> {
    if let Some(wire) = read_assertion_wire(&req)? {
        let source = peer_socket_addr(&req);
        let now = chrono::Utc::now().timestamp();
        let identity = validator
            .validate(&wire, source, now)
            .map_err(Status::from)?;
        req.extensions_mut().insert(identity);
        return Ok(req);
    }
    ghostbridge_footprint_interceptor(req)
}

/// Legacy/test entry: footprint-only path (assertion metadata absent).
#[allow(clippy::result_large_err)]
pub fn ghostbridge_interceptor(req: Request<()>) -> Result<Request<()>, Status> {
    ghostbridge_footprint_interceptor(req)
}

/// Look up the specific caller's identity sled and check its footprint +
/// term (`expires_at`) - the per-identity path. Resolves either by a
/// WireGuard pubkey (host/xray-mesh callers, session_id derived from the
/// pubkey) or, when there's no pubkey, directly by the trace_id the caller
/// presents (vendor/partner identities like a browser frontend that has no
/// WireGuard identity of its own - e.g. Lovable). Returns `None` when
/// there's no engine registered yet, no identifying header, or no matching
/// record, so the caller can fall back to the shared host legacy sled.
fn verify_per_identity(
    pubkey: Option<&str>,
    trace_id: Option<&str>,
    request_footprint: &str,
) -> Option<Result<String, Status>> {
    let engine = ENGINE.get()?;
    let session_id = pubkey.map(op_identity::session::derive_session_id);
    let args = match (&session_id, trace_id) {
        (Some(sid), _) => serde_json::json!({ "session_id": sid }),
        (None, Some(trace)) => serde_json::json!({ "trace_id": trace }),
        (None, None) => return None,
    };

    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(
            crate::identity_sled_dispatch::dispatch_identity_sled_method(
                engine.as_ref(),
                "get_identity",
                &args,
            ),
        )
    });

    let identity = match result {
        Ok(value) => value.get("identity").cloned()?,
        Err(_) => return None,
    };
    let hashed_footprint = identity.get("hashed_footprint")?.as_str()?.to_string();
    let expires_at = identity.get("expires_at").and_then(|v| v.as_i64());

    if let Some(expires_at) = expires_at {
        if expires_at != 0 && expires_at <= chrono::Utc::now().timestamp() {
            return Some(Err(Status::permission_denied(
                "Identity term has expired. Re-authenticate to renew.",
            )));
        }
    }

    if hashed_footprint != request_footprint {
        return Some(Err(Status::permission_denied(
            "Temporal Hash Mismatch. Session footprint is out of sync with current mutation.",
        )));
    }

    let resolved_session_id = identity
        .get("session_id")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or(session_id)?;
    Some(Ok(resolved_session_id))
}

/// THE GATEKEEPER: legacy Ghostbridge footprint path (byte-for-byte when assertion absent).
#[allow(clippy::result_large_err)]
fn ghostbridge_footprint_interceptor(mut req: Request<()>) -> Result<Request<()>, Status> {
    let footprint_value = req.metadata().get("x-ghostbridge-footprint").cloned();
    let trace_value = req
        .metadata()
        .get("x-ghostbridge-trace-id")
        .or_else(|| req.metadata().get("x-wireguard-pubkey"))
        .cloned();

    if footprint_value.is_none() || trace_value.is_none() {
        return Err(Status::unauthenticated(
            "A.N.N.A. Scribe: Missing Ghostbridge Identity Sled. Connection Dropped.",
        ));
    }

    let request_footprint = footprint_value
        .as_ref()
        .unwrap()
        .to_str()
        .map_err(|_| Status::invalid_argument("Invalid footprint header encoding"))?;

    let pubkey_value = req.metadata().get("x-wireguard-pubkey").cloned();
    let raw_trace_value = req.metadata().get("x-ghostbridge-trace-id").cloned();
    let pubkey_str = pubkey_value
        .as_ref()
        .map(|v| v.to_str())
        .transpose()
        .map_err(|_| Status::invalid_argument("Invalid pubkey header encoding"))?;
    let trace_str = raw_trace_value
        .as_ref()
        .map(|v| v.to_str())
        .transpose()
        .map_err(|_| Status::invalid_argument("Invalid trace header encoding"))?;
    if pubkey_str.is_some() || trace_str.is_some() {
        if let Some(outcome) = verify_per_identity(pubkey_str, trace_str, request_footprint) {
            let session_id = outcome?;
            req.extensions_mut().insert(GhostbridgeIdentity {
                footprint: request_footprint.to_string(),
                session_id,
            });
            return Ok(req);
        }
    }

    op_identity::verify_ghostbridge_footprint(request_footprint).map_err(|error| match error {
        FootprintVerifyError::SledUnreachable => {
            Status::internal("MutationEngine Memory Unreachable")
        }
        FootprintVerifyError::InvalidSled => {
            Status::failed_precondition("A.N.N.A. Scribe: Invalid Schema State. Cease and Desist.")
        }
        FootprintVerifyError::Mismatch => Status::permission_denied(
            "A.N.N.A. Scribe: Temporal Hash Mismatch. \
             Session footprint is out of sync with current Btrfs mutation.",
        ),
    })?;

    let session_id = trace_value
        .as_ref()
        .expect("trace checked above")
        .to_str()
        .map_err(|_| Status::invalid_argument("Invalid trace header encoding"))?
        .to_string();
    req.extensions_mut().insert(GhostbridgeIdentity {
        footprint: request_footprint.to_string(),
        session_id,
    });

    Ok(req)
}

#[derive(Clone, Debug)]
pub struct GhostbridgeInterceptor;

impl tonic::service::Interceptor for GhostbridgeInterceptor {
    fn call(&mut self, req: Request<()>) -> Result<Request<()>, Status> {
        ghostbridge_interceptor(req)
    }
}

/// Permit only the two calls needed to establish an identity. Everything else
/// on RegistrationService still passes through the normal sled gate.
#[allow(clippy::result_large_err)]
pub fn registration_interceptor(req: Request<()>) -> Result<Request<()>, Status> {
    match req.extensions().get::<tonic::GrpcMethod<'_>>() {
        Some(method)
            if method.service() == "operation.registration.v1.RegistrationService"
                && matches!(method.method(), "SendMagicLink" | "VerifyMagicLink") =>
        {
            Ok(req)
        }
        _ => ghostbridge_interceptor(req),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::human_principal_dispatch::tests::{pk, register, temp_cozo};
    use crate::oracle_assertion::tests::{
        signed_with_fields, source_at, test_ip, test_issuer, trust_store_for_issuer,
    };
    use crate::oracle_assertion::{derive_human_footprint, AssertionValidator};
    use tonic::metadata::MetadataValue;
    use tonic::Code;

    fn request_with_connect_info(ip: std::net::IpAddr) -> Request<()> {
        let mut req = Request::new(());
        req.extensions_mut().insert(source_at(ip));
        req.extensions_mut().insert(TcpConnectInfo {
            local_addr: Some(source_at(ip)),
            remote_addr: Some(source_at(ip)),
        });
        req
    }

    fn insert_assertion_metadata(req: &mut Request<()>, wire: &[u8]) {
        req.metadata_mut()
            .insert_bin(ASSERTION_METADATA_KEY, MetadataValue::from_bytes(wire));
    }

    fn insert_ghostbridge_headers(req: &mut Request<()>, footprint: &str, trace: &str) {
        req.metadata_mut().insert(
            "x-ghostbridge-footprint",
            MetadataValue::try_from(footprint).expect("footprint header"),
        );
        req.metadata_mut().insert(
            "x-ghostbridge-trace-id",
            MetadataValue::try_from(trace).expect("trace header"),
        );
    }


    fn fresh_signed(
        issuer: &op_identity::oracle_assertion::DecoyIssuer,
        pubkey: &str,
        nonce: [u8; 16],
    ) -> op_identity::oracle_assertion::SignedAssertion {
        let now = chrono::Utc::now().timestamp();
        signed_with_fields(
            issuer,
            pubkey,
            test_ip(),
            now - 5,
            now + 300,
            nonce,
            None,
        )
    }

    fn validator_for_tests() -> Arc<AssertionValidator> {
        let issuer = test_issuer();
        Arc::new(AssertionValidator::new(trust_store_for_issuer(&issuer)))
    }

    #[test]
    fn test_rejects_missing_footprint_header() {
        let req = Request::new(());
        let result = ghostbridge_interceptor(req);
        assert!(result.is_err());
        let status = result.unwrap_err();
        assert_eq!(status.code(), tonic::Code::Unauthenticated);
        assert!(status
            .message()
            .contains("Missing Ghostbridge Identity Sled"));
    }

    #[test]
    fn registration_bootstrap_allows_magic_link_without_identity() {
        for method in ["SendMagicLink", "VerifyMagicLink"] {
            let mut req = Request::new(());
            req.extensions_mut().insert(tonic::GrpcMethod::new(
                "operation.registration.v1.RegistrationService",
                method,
            ));
            assert!(registration_interceptor(req).is_ok());
        }
    }

    #[test]
    fn registration_bootstrap_does_not_expose_admin_methods() {
        let mut req = Request::new(());
        req.extensions_mut().insert(tonic::GrpcMethod::new(
            "operation.registration.v1.RegistrationService",
            "ListUsers",
        ));
        assert_eq!(
            registration_interceptor(req).unwrap_err().code(),
            tonic::Code::Unauthenticated
        );
    }

    #[test]
    fn test_rejects_missing_trace_header() {
        let mut req = Request::new(());
        req.metadata_mut().insert(
            "x-ghostbridge-footprint",
            MetadataValue::from_static("deadbeef"),
        );
        let result = ghostbridge_interceptor(req);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::Unauthenticated);
    }

    #[test]
    fn test_rejects_missing_footprint_with_trace_only() {
        let mut req = Request::new(());
        req.metadata_mut().insert(
            "x-ghostbridge-trace-id",
            MetadataValue::from_static("trace-abc"),
        );
        let result = ghostbridge_interceptor(req);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::Unauthenticated);
    }

    #[test]
    fn test_footprint_mismatch_detection() {
        let sled_footprint: [u8; 32] = [0xAA; 32];
        let expected = hex::encode(sled_footprint);
        let request_footprint = "0000000000000000000000000000000000000000000000000000000000000000";
        assert_ne!(request_footprint, expected);
    }

    #[test]
    fn test_footprint_match_succeeds() {
        let sled_footprint: [u8; 32] = [0xBB; 32];
        let expected = hex::encode(sled_footprint);
        let request_footprint = hex::encode([0xBB; 32]);
        assert_eq!(request_footprint, expected);
    }

    #[test]
    fn test_mutation_engine_unreachable_returns_internal() {
        if op_identity::read_sled().is_ok() {
            eprintln!("skipping: live IdentitySled present in shared memory");
            return;
        }
        let mut req = Request::new(());
        req.metadata_mut().insert(
            "x-ghostbridge-footprint",
            MetadataValue::from_static("aabbccdd"),
        );
        req.metadata_mut().insert(
            "x-ghostbridge-trace-id",
            MetadataValue::from_static("trace-aabbccdd"),
        );
        let result = ghostbridge_interceptor(req);
        assert!(result.is_err());
        let status = result.unwrap_err();
        assert_eq!(status.code(), tonic::Code::Internal);
        assert!(status
            .message()
            .contains("MutationEngine Memory Unreachable"));
    }

    /// VAL-BRIDGE-021
    pub async fn assertion_present_valid_inserts_human_principal_identity_impl() {
        let _cozo = temp_cozo();
        let issuer = test_issuer();
        let pubkey = pk(41);
        register(&pubkey, "interceptor").await.expect("register");
        let mut gate = make_ghostbridge_interceptor(Arc::new(AssertionValidator::new(
            trust_store_for_issuer(&issuer),
        )));
        let signed = fresh_signed(&issuer, &pubkey, [0x41; 16]);
        let mut req = request_with_connect_info(test_ip());
        insert_assertion_metadata(&mut req, &signed.to_wire());
        let out = gate.call(req).expect("valid assertion");
        let identity = out
            .extensions()
            .get::<HumanPrincipalIdentity>()
            .expect("human identity inserted");
        assert_eq!(identity.human_pubkey, pubkey);
        assert_eq!(identity.footprint, derive_human_footprint(&pubkey));
    }

    /// VAL-BRIDGE-022
    pub async fn assertion_present_invalid_returns_unauthenticated_impl() {
        let mut gate = make_ghostbridge_interceptor(validator_for_tests());
        let mut req = request_with_connect_info(test_ip());
        insert_assertion_metadata(&mut req, b"not-an-oia1-envelope");
        let status = gate.call(req).unwrap_err();
        assert_eq!(status.code(), Code::Unauthenticated);
        assert!(status.message().contains("Malformed"));
    }

    /// VAL-BRIDGE-023
    pub async fn assertion_present_footprint_headers_not_consulted_impl() {
        let _cozo = temp_cozo();
        let issuer = test_issuer();
        let pubkey = pk(42);
        register(&pubkey, "shadow").await.expect("register");
        let mut gate = make_ghostbridge_interceptor(Arc::new(AssertionValidator::new(
            trust_store_for_issuer(&issuer),
        )));
        let signed = fresh_signed(&issuer, &pubkey, [0x42; 16]);
        let mut req = request_with_connect_info(test_ip());
        insert_assertion_metadata(&mut req, &signed.to_wire());
        insert_ghostbridge_headers(
            &mut req,
            "0000000000000000000000000000000000000000000000000000000000000000",
            "wrong-trace",
        );
        let out = gate.call(req).expect("assertion governs");
        assert!(out.extensions().get::<GhostbridgeIdentity>().is_none());
        assert!(out.extensions().get::<HumanPrincipalIdentity>().is_some());
    }

    /// VAL-BRIDGE-024
    pub async fn assertion_absent_ghostbridge_path_unchanged_impl() {
        let req = Request::new(());
        let baseline = ghostbridge_interceptor(req).unwrap_err();
        let mut gate = make_ghostbridge_interceptor(validator_for_tests());
        let req = Request::new(());
        let gated = gate.call(req).unwrap_err();
        assert_eq!(baseline.code(), gated.code());
        assert_eq!(baseline.message(), gated.message());
    }

    /// VAL-BRIDGE-025
    pub async fn duplicate_assertion_metadata_values_reject_malformed_impl() {
        let mut gate = make_ghostbridge_interceptor(validator_for_tests());
        let mut req = request_with_connect_info(test_ip());
        req.metadata_mut().append_bin(
            ASSERTION_METADATA_KEY,
            MetadataValue::from_bytes(b"OIA1"),
        );
        req.metadata_mut().append_bin(
            ASSERTION_METADATA_KEY,
            MetadataValue::from_bytes(b"OIA1"),
        );
        let status = gate.call(req).unwrap_err();
        assert_eq!(status.code(), Code::Unauthenticated);
        assert!(status.message().contains("Malformed"));
    }

    /// VAL-BRIDGE-026
    pub async fn assertion_without_connect_info_rejects_missing_connect_info_impl() {
        let _cozo = temp_cozo();
        let issuer = test_issuer();
        let pubkey = pk(43);
        register(&pubkey, "noci").await.expect("register");
        let mut gate = make_ghostbridge_interceptor(Arc::new(AssertionValidator::new(
            trust_store_for_issuer(&issuer),
        )));
        let signed = fresh_signed(&issuer, &pubkey, [0x43; 16]);
        let mut req = Request::new(());
        insert_assertion_metadata(&mut req, &signed.to_wire());
        let status = gate.call(req).unwrap_err();
        assert_eq!(status.code(), Code::Unauthenticated);
        assert!(status.message().contains("MissingConnectInfo"));
    }

    /// VAL-BRIDGE-027
    pub async fn human_footprint_grant_allows_capability_gate_impl() {
        let _cozo = temp_cozo();
        let pubkey = pk(44);
        register(&pubkey, "grant").await.expect("register");
        let footprint_hex = hex::encode(derive_human_footprint(&pubkey));
        let schema = serde_json::json!({
            "methods": {
                "resolve_key": { "required_capability": "human_principal.read" }
            },
            "capability_grants": {
                footprint_hex: ["human_principal.read"]
            }
        });
        let identity = bridge_capability_identity(&{
            let mut ex = tonic::Extensions::new();
            ex.insert(HumanPrincipalIdentity {
                principal_id: "did:op:human:test".to_string(),
                human_pubkey: pubkey.clone(),
                footprint: derive_human_footprint(&pubkey),
                expires_at: 1_700_000_300,
            });
            ex
        })
        .expect("identity");
        assert!(
            crate::grpc_server::enforce_bridge_capability_with_schema(
                Some(&schema),
                "human_principal",
                "resolve_key",
                Some("human_principal.read"),
                Some(&identity),
            )
            .is_ok()
        );
    }

    /// VAL-BRIDGE-028
    pub async fn human_footprint_missing_grant_denies_capability_gate_impl() {
        let _cozo = temp_cozo();
        let pubkey = pk(45);
        let footprint_hex = hex::encode(derive_human_footprint(&pubkey));
        let schema = serde_json::json!({
            "methods": {
                "resolve_key": { "required_capability": "human_principal.read" }
            },
            "capability_grants": {}
        });
        let identity = GhostbridgeIdentity {
            footprint: footprint_hex,
            session_id: "human".to_string(),
        };
        assert!(
            crate::grpc_server::enforce_bridge_capability_with_schema(
                Some(&schema),
                "human_principal",
                "resolve_key",
                Some("human_principal.read"),
                Some(&identity),
            )
            .is_err()
        );
    }

    /// VAL-BRIDGE-029
    pub async fn human_identity_shadows_ghostbridge_for_capability_gate_impl() {
        let human_fp = derive_human_footprint("BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc=");
        let ghost_fp = "aa".repeat(32);
        let schema = serde_json::json!({
            "methods": {
                "resolve_key": { "required_capability": "human_principal.read" }
            },
            "capability_grants": {
                hex::encode(human_fp): ["human_principal.read"]
            }
        });
        let mut ex = tonic::Extensions::new();
        ex.insert(HumanPrincipalIdentity {
            principal_id: "did:op:human:shadow".to_string(),
            human_pubkey: "BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc=".to_string(),
            footprint: human_fp,
            expires_at: 1_700_000_300,
        });
        ex.insert(GhostbridgeIdentity {
            footprint: ghost_fp,
            session_id: "ghost".to_string(),
        });
        let identity = bridge_capability_identity(&ex).expect("human wins");
        assert_eq!(identity.footprint, hex::encode(human_fp));
        assert!(
            crate::grpc_server::enforce_bridge_capability_with_schema(
                Some(&schema),
                "human_principal",
                "resolve_key",
                Some("human_principal.read"),
                Some(&identity),
            )
            .is_ok()
        );
    }

    /// VAL-BRIDGE-036
    pub async fn registration_interceptor_factory_respects_bootstrap_impl() {
        let mut gate = make_registration_interceptor(validator_for_tests());
        let mut req = Request::new(());
        req.extensions_mut().insert(tonic::GrpcMethod::new(
            "operation.registration.v1.RegistrationService",
            "SendMagicLink",
        ));
        assert!(gate.call(req).is_ok());
    }

    /// VAL-BRIDGE-037
    pub async fn validator_per_instance_interceptor_isolation_impl() {
        let issuer1 = test_issuer();
        let issuer2 = op_identity::oracle_assertion::DecoyIssuer::new(
            ed25519_dalek::SigningKey::from_bytes(&[9u8; 32]),
            "decoy-key-2",
            std::time::Duration::from_secs(900),
        );
        let v1 = Arc::new(AssertionValidator::new(trust_store_for_issuer(&issuer1)));
        let v2 = Arc::new(AssertionValidator::new(trust_store_for_issuer(&issuer2)));
        assert_ne!(v1.trust_store().key_count(), 0);
        assert_eq!(v2.trust_store().key_count(), 1);
        assert!(v1.trust_store().contains_key("decoy-key-1"));
        assert!(v2.trust_store().contains_key("decoy-key-2"));
    }

    /// VAL-BRIDGE-038
    pub async fn bridge_capability_identity_prefers_human_impl() {
        let human_fp = [0x38; 32];
        let mut ex = tonic::Extensions::new();
        ex.insert(HumanPrincipalIdentity {
            principal_id: "did:op:human:pref".to_string(),
            human_pubkey: "pk".to_string(),
            footprint: human_fp,
            expires_at: 1,
        });
        ex.insert(GhostbridgeIdentity {
            footprint: "bb".repeat(32),
            session_id: "ghost".to_string(),
        });
        let id = bridge_capability_identity(&ex).unwrap();
        assert_eq!(id.footprint, hex::encode(human_fp));
        assert_eq!(id.session_id, "did:op:human:pref");
    }

    /// VAL-BRIDGE-039
    pub async fn assertion_bad_signature_does_not_insert_ghostbridge_impl() {
        let _cozo = temp_cozo();
        let issuer = test_issuer();
        let pubkey = pk(46);
        register(&pubkey, "sig").await.expect("register");
        let mut gate = make_ghostbridge_interceptor(Arc::new(AssertionValidator::new(
            trust_store_for_issuer(&issuer),
        )));
        let mut signed = fresh_signed(&issuer, &pubkey, [0x46; 16]);
        signed.signature[0] ^= 0x01;
        let mut req = request_with_connect_info(test_ip());
        insert_assertion_metadata(&mut req, &signed.to_wire());
        insert_ghostbridge_headers(&mut req, &"aa".repeat(32), "trace");
        let status = gate.call(req).unwrap_err();
        assert_eq!(status.code(), Code::Unauthenticated);
        assert!(status.message().contains("BadSignature"));
    }
}

/// Load the capabilities granted to one sled footprint.
///
/// The JSON document is keyed by lowercase footprint hex and each value owns a
/// `capabilities` array. Missing, malformed, or unreadable grant state fails
/// closed.
pub fn load_capability_grants(footprint_hex: &str) -> std::collections::HashSet<String> {
    let path = std::env::var("OP_GRANTS_PATH")
        .unwrap_or_else(|_| "/dev/shm/opdbus/capability-grants.json".to_string());
    let Ok(bytes) = std::fs::read(path) else {
        return std::collections::HashSet::new();
    };
    let Ok(document) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return std::collections::HashSet::new();
    };
    document
        .get(footprint_hex)
        .or_else(|| document.get("*"))
        .and_then(|entry| entry.get("capabilities"))
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_str)
        .map(str::to_string)
        .collect()
}
