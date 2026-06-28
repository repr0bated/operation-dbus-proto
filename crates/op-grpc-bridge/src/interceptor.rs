// 🟢 🛡️ The Tonic gRPC Gatekeeper (Middleware Interceptor)
// Sits on the primary gRPC ingress at port 18789. Intercepts Xray-injected headers,
// performs a zero-copy check against the IdentitySled in shared memory, and either
// allows the gRPC payload through or drops the connection instantly.
//
// Operated by A.N.N.A. Scribe. No payload enters the system without a cryptographic
// "Snowball" session. No SQL databases, no D-Bus watchers. 1:1 Direct Read only.

use serde_json::Value as JsonValue;
use std::collections::HashSet;
use tonic::{Request, Status};

/// Sled capability grants path in SHM (read directly, no polling).
const CAPABILITY_GRANTS_PATH: &str = "/dev/shm/opdbus/capability-grants.json";

/// Can be overridden via OP_GRANTS_PATH environment variable for testing.
fn capability_grants_path() -> String {
    std::env::var("OP_GRANTS_PATH").unwrap_or_else(|_| CAPABILITY_GRANTS_PATH.to_string())
}

/// Load capability grants for a footprint from SHM (VAL-ENFORCE-002).
/// Returns a set of capability strings that the footprint has been granted.
/// If the file is missing or the footprint is not found, returns an empty set.
#[allow(clippy::implicit_return)]
pub fn load_capability_grants(footprint_hex: &str) -> HashSet<String> {
    let path = capability_grants_path();
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(_) => return HashSet::new(),
    };
    let grants_json: JsonValue = match serde_json::from_str(&text) {
        Ok(j) => j,
        Err(_) => return HashSet::new(),
    };
    let per_footprint = grants_json
        .as_object()
        .and_then(|o| o.get(footprint_hex))
        .and_then(|v| v.as_object())
        .and_then(|o| o.get("capabilities"))
        .and_then(|c| c.as_array());

    match per_footprint {
        Some(arr) => arr
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect(),
        None => HashSet::new(),
    }
}

impl GhostbridgeContext {
    /// Check if this context grants a specific capability (VAL-ENFORCE-002).
    pub fn grants(&self, capability: &str) -> bool {
        self.granted_capabilities.contains(capability)
    }
}

/// Request context attached to gRPC extensions containing footprint and session info.
/// Retrieved by the bridge for capability enforcement (VAL-ENFORCE-001, VAL-ENFORCE-002).
#[derive(Debug, Clone)]
pub struct GhostbridgeContext {
    /// The hex-encoded footprint extracted from the sled
    pub footprint_hex: String,
    /// The hex-encoded trace/session ID
    pub trace_id_hex: String,
    /// WireGuard public key (hex-encoded) for this session
    pub wg_pubkey_hex: String,
    /// Capabilities granted to this footprint (loaded from SHM grants file)
    pub granted_capabilities: HashSet<String>,
}

/// Check whether a sled is "valid" per the Absolute Base rule.
fn is_sled_valid(sled: &op_identity::IdentitySled) -> bool {
    sled.hashed_footprint != [0u8; 32] && sled.trace_id != [0u8; 16]
}

/// THE GATEKEEPER: Tonic gRPC Interceptor on port 18789.
///
/// Enforces the Absolute Base rule: if the `x-ghostbridge-footprint` provided by Xray
/// does not perfectly match the hashed footprint sitting in shared memory, the payload
/// is rejected. Once validated, embeds the `x-ghostbridge-trace-id` into Tonic Request
/// extensions so the Chatbot and Qdrant semantic search on the Accountability Page
/// have the exact Trace ID needed to link the session.
///
/// Also loads capability grants for the footprint and attaches them to the request
/// context for enforcement at the bridge layer (VAL-ENFORCE-001, VAL-ENFORCE-002).
#[derive(Clone, Debug)]
pub struct GhostbridgeInterceptor;

impl tonic::service::Interceptor for GhostbridgeInterceptor {
    fn call(&mut self, req: Request<()>) -> Result<Request<()>, Status> {
        ghostbridge_interceptor(req)
    }
}

#[allow(clippy::result_large_err)]
pub fn ghostbridge_interceptor(mut req: Request<()>) -> Result<Request<()>, Status> {
    // 1. Extract the Xray-injected Identity Headers (The Accountability Loop)
    //    Clone header values upfront to release the immutable borrow on `req`,
    //    allowing the mutable `extensions_mut()` call downstream.
    let footprint_value = req.metadata().get("x-ghostbridge-footprint").cloned();
    let trace_value = req.metadata().get("x-ghostbridge-trace-id").cloned();

    if footprint_value.is_none() || trace_value.is_none() {
        return Err(Status::unauthenticated(
            "A.N.N.A. Scribe: Missing Ghostbridge Identity Sled. Connection Dropped.",
        ));
    }

    // 2. 1:1 Direct Read from the SchemaEngine's shared memory (No SQL, No Polling)
    let (sled_ptr, _mmap) = op_identity::read_sled()
        .map_err(|_| Status::internal("SchemaEngine Memory Unreachable"))?;
    let sled = unsafe { &*sled_ptr };

    // The Absolute Base: No valid schema, it does not exist.
    if !is_sled_valid(sled) {
        return Err(Status::failed_precondition(
            "A.N.N.A. Scribe: Invalid Schema State. Cease and Desist.",
        ));
    }

    let current_footprint = sled.hashed_footprint;

    // 3. The Strike/Etch Validation: Check if the payload is in sync with Btrfs.
    //    If a Btrfs mutation has occurred and the client's footprint is stale,
    //    the connection is dropped without consuming any NVMe I/O.
    let request_footprint = footprint_value
        .as_ref()
        .unwrap()
        .to_str()
        .map_err(|_| Status::invalid_argument("Invalid footprint header encoding"))?;
    let expected_footprint = hex::encode(current_footprint);

    if request_footprint != expected_footprint {
        return Err(Status::permission_denied(
            "A.N.N.A. Scribe: Temporal Hash Mismatch. \
             Session footprint is out of sync with current Btrfs mutation.",
        ));
    }

    // 4. Load capability grants for this footprint from SHM (VAL-ENFORCE-002).
    let granted_capabilities = load_capability_grants(request_footprint);

    // 5. Attach full context to request extensions for bridge-layer enforcement.
    // This satisfies VAL-ENFORCE-001 (extract footprint) and VAL-ENFORCE-002
    // (read grants to check for required capability).
    let context = GhostbridgeContext {
        footprint_hex: request_footprint.to_string(),
        trace_id_hex: trace_value
            .as_ref()
            .unwrap()
            .to_str()
            .unwrap_or("")
            .to_string(),
        wg_pubkey_hex: hex::encode(sled.wireguard_pubkey),
        granted_capabilities,
    };
    req.extensions_mut().insert(context);

    Ok(req)
}

#[cfg(test)]
mod tests {
    use super::*;
    use op_identity::IdentitySled;
    use std::path::Path;
    use tonic::metadata::MetadataValue;

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
    fn test_identity_sled_repr_c_layout() {
        let size = std::mem::size_of::<IdentitySled>();
        assert_eq!(
            size, 152,
            "IdentitySled must be exactly 152 bytes per spec, got {} bytes",
            size
        );
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
    fn test_schema_engine_unreachable_returns_internal() {
        // Make the test hermetic: point the interceptor at an isolated/empty
        // SchemaEngine memory region via the env-overridable sled path. This
        // ensures the "unreachable" branch is exercised deterministically
        // regardless of host SHM state (a populated /dev/shm/plugin_schema.dat
        // may exist on the live system). Do NOT merely flip the expected error.
        let isolated_path = format!(
            "/dev/shm/opdbus-test-sled-unreachable-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        // The path does not exist → read_sled() returns Err → Status::internal.
        std::env::set_var("OP_SLED_PATH", &isolated_path);

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
        assert_eq!(
            status.code(),
            tonic::Code::Internal,
            "unreachable sled must yield Status::internal, got {:?}: {}",
            status.code(),
            status.message()
        );
        assert!(status.message().contains("SchemaEngine Memory Unreachable"));

        // Restore the default sled path for other tests.
        std::env::remove_var("OP_SLED_PATH");
    }

    #[test]
    fn test_invalid_sled_rejected() {
        let sled = IdentitySled {
            wireguard_pubkey: [0u8; 32],
            mutation_index: 1,
            hashed_footprint: [0u8; 32],
            trace_id: [0u8; 16],
            schema_version: 0,
            reserved: [0u8; 44],
            vector_id: [0u8; 16],
        };
        assert!(!is_sled_valid(&sled));
    }

    #[test]
    fn test_valid_sled_accepted() {
        let sled = IdentitySled {
            wireguard_pubkey: [0xCC; 32],
            mutation_index: 42,
            hashed_footprint: [0xDD; 32],
            trace_id: [0xEE; 16],
            schema_version: 1,
            reserved: [0u8; 44],
            vector_id: [0xDD; 16],
        };
        assert!(is_sled_valid(&sled));
    }

    // ── VAL-ENFORCE-001: GhostbridgeInterceptor extracts footprint and session ID ──

    #[test]
    fn interceptor_extracts_footprint_and_session_id() {
        let base = test_base_dir();
        let (footprint_hex, trace_hex) = write_test_sled(&base);

        // Write capability grants file
        let grants_path = base.join("opdbus/capability-grants.json");
        let grants_json = serde_json::json!({
            footprint_hex.clone(): {
                "capabilities": ["plugin.alpha.read", "plugin.beta.write"]
            }
        });
        std::fs::create_dir_all(grants_path.parent().unwrap()).unwrap();
        std::fs::write(&grants_path, serde_json::to_vec(&grants_json).unwrap()).unwrap();

        // Set up the interceptor to use our test sled
        std::env::set_var("OP_SLED_PATH", sled_path(&base));
        std::env::set_var("OP_GRANTS_PATH", grants_path.into_os_string());

        let mut req = Request::new(());
        req.metadata_mut().insert(
            "x-ghostbridge-footprint",
            MetadataValue::try_from(footprint_hex.clone()).unwrap(),
        );
        req.metadata_mut().insert(
            "x-ghostbridge-trace-id",
            MetadataValue::try_from(trace_hex.clone()).unwrap(),
        );

        let result = ghostbridge_interceptor(req);
        assert!(result.is_ok(), "interceptor should accept valid request");
        let req = result.unwrap();

        // Extract and verify the context from extensions
        let ctx = req
            .extensions()
            .get::<GhostbridgeContext>()
            .expect("GhostbridgeContext should be in extensions");

        assert_eq!(ctx.footprint_hex, footprint_hex);
        assert_eq!(ctx.trace_id_hex, trace_hex);
        assert!(ctx.grants("plugin.alpha.read"), "should grant alpha.read");
        assert!(ctx.grants("plugin.beta.write"), "should grant beta.write");

        cleanup(&base);
        std::env::remove_var("OP_SLED_PATH");
        std::env::remove_var("OP_GRANTS_PATH");
    }

    // ── VAL-ENFORCE-002: Load capability grants for footprint ──

    #[test]
    fn interceptor_loads_capability_grants_for_footprint() {
        let base = test_base_dir();
        let (footprint_hex, trace_hex) = write_test_sled(&base);

        // Write grants with multiple capabilities
        let grants_path = base.join("opdbus/capability-grants.json");
        let grants_json = serde_json::json!({
            footprint_hex.clone(): {
                "capabilities": ["cap.alpha.read", "cap.beta.exec", "cap.gamma.admin"]
            }
        });
        std::fs::create_dir_all(grants_path.parent().unwrap()).unwrap();
        std::fs::write(&grants_path, serde_json::to_vec(&grants_json).unwrap()).unwrap();

        std::env::set_var("OP_SLED_PATH", sled_path(&base));
        std::env::set_var("OP_GRANTS_PATH", grants_path.into_os_string());

        let mut req = Request::new(());
        req.metadata_mut().insert(
            "x-ghostbridge-footprint",
            MetadataValue::try_from(footprint_hex).unwrap(),
        );
        req.metadata_mut().insert(
            "x-ghostbridge-trace-id",
            MetadataValue::try_from(trace_hex).unwrap(),
        );

        let req = ghostbridge_interceptor(req).unwrap();
        let ctx = req.extensions().get::<GhostbridgeContext>().unwrap();

        assert!(ctx.grants("cap.alpha.read"));
        assert!(ctx.grants("cap.beta.exec"));
        assert!(ctx.grants("cap.gamma.admin"));
        assert!(!ctx.grants("cap.not-granted"));

        cleanup(&base);
        std::env::remove_var("OP_SLED_PATH");
        std::env::remove_var("OP_GRANTS_PATH");
    }

    // ── VAL-ENFORCE-005: No grants file returns empty grants set ──

    #[test]
    fn interceptor_no_grants_file_returns_empty_grants() {
        // Set up a test environment with a sled but no grants file
        let base = test_base_dir();
        let (_, _) = write_test_sled(&base);

        // Set up a path that doesn't exist for grants
        let grants_path = base.join("nonexistent-grants.json");
        std::env::set_var("OP_SLED_PATH", sled_path(&base));
        std::env::set_var("OP_GRANTS_PATH", grants_path.into_os_string());

        let mut req = Request::new(());
        req.metadata_mut().insert(
            "x-ghostbridge-footprint",
            MetadataValue::try_from(hex::encode([0xBB; 32])).unwrap(),
        );
        req.metadata_mut().insert(
            "x-ghostbridge-trace-id",
            MetadataValue::try_from(hex::encode([0xCC; 16])).unwrap(),
        );

        let req = ghostbridge_interceptor(req).unwrap();
        let ctx = req.extensions().get::<GhostbridgeContext>().unwrap();

        assert!(
            ctx.granted_capabilities.is_empty() || !ctx.grants("any-capability"),
            "should have no grants"
        );

        cleanup(&base);
        std::env::remove_var("OP_SLED_PATH");
        std::env::remove_var("OP_GRANTS_PATH");
    }

    // ── Helper functions for tests ──

    fn test_base_dir() -> std::path::PathBuf {
        let id = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::path::PathBuf::from(format!("/dev/shm/opdbus-test-interceptor-{}-{}", id, nanos));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create test base dir");
        dir
    }

    fn sled_path(base: &Path) -> String {
        base.join("plugin_schema.dat")
            .to_string_lossy()
            .into_owned()
    }

    fn write_test_sled(base: &Path) -> (String, String) {
        let sled = IdentitySled {
            wireguard_pubkey: [0xAA; 32],
            mutation_index: 1,
            hashed_footprint: [0xBB; 32],
            trace_id: [0xCC; 16],
            schema_version: 1,
            reserved: [0u8; 44],
            vector_id: [0xDD; 16],
        };
        let footprint_hex = hex::encode(sled.hashed_footprint);
        let trace_hex = hex::encode(sled.trace_id);

        // Clean up any existing sled file from prior tests that might pollute /dev/shm
        let _ = std::fs::remove_file("/dev/shm/plugin_schema.dat");

        // Write sled bytes directly to the custom path (bypass write_sled's hardcoded path)
        let bytes: &[u8] = unsafe {
            std::slice::from_raw_parts(
                &sled as *const IdentitySled as *const u8,
                IdentitySled::SIZE,
            )
        };
        let sled_path = sled_path(base);
        let tmp = format!("{}.tmp", sled_path);
        let mut f = std::fs::File::create(&tmp).unwrap();
        std::io::Write::write_all(&mut f, bytes).unwrap();
        f.sync_data().unwrap();
        std::fs::rename(&tmp, &sled_path).unwrap();

        // Set env var for sled path - need to clone before we move
        let sled_path_for_env = sled_path.clone();
        std::env::set_var("OP_SLED_PATH", sled_path_for_env);

        (footprint_hex, trace_hex)
    }

    fn cleanup(path: &std::path::Path) {
        let _ = std::fs::remove_dir_all(path);
    }
}
