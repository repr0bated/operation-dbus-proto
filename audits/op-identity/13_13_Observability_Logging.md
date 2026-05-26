# Production Quality & Observability Audit: op-identity

## 1. Observability Summary & Counts

### 1.1 Tracing Macros vs. `println!`
Across the audited codebase, logging is handled exclusively by the `tracing` ecosystem. There are **zero** instances of standard `println!` macros. 

#### Macro Usage Breakdown:
*   **`tracing::info!`**: 10 occurrences
*   **`tracing::debug!`**: 11 occurrences
*   **`tracing::warn!`**: 9 occurrences
*   **`tracing::error!`**: 0 occurrences
*   **`println!`**: 0 occurrences
*   **Total Logging Events**: 30

#### Breakdown by File:
*   **`crates/op-identity/src/gcloud_auth.rs`**
    *   `debug!`: 4 (Lines 66, 102, 207, 221)
    *   `info!`: 4 (Lines 81, 87, 93, 99)
    *   `warn!`: 3 (Lines 137, 147, 160)
*   **`crates/op-identity/src/session.rs`**
    *   `debug!`: 1 (Line 101)
    *   `info!`: 4 (Lines 110, 233, 261, 276)
    *   `warn!`: 1 (Line 121)
*   **`crates/op-identity/src/wg.rs`**
    *   `debug!`: 2 (Lines 51, 57)
    *   `warn!`: 1 (Line 27)
*   **`crates/op-identity/src/wireguard.rs`**
    *   `debug!`: 4 (Lines 22, 34, 41, 44)
    *   `warn!`: 1 (Line 51)
*   **`crates/op-identity/src/schema_bridge.rs`**
    *   `tracing::info!`: 2 (Lines 496, 557)
    *   `tracing::warn!`: 3 (Lines 394, 499, 520)

---

## 2. Silent & Swallowed Errors

Multiple system errors are ignored, caught using `.ok()`, or converted directly to empty collection structures without being logged or propagated.

*   **`crates/op-identity/src/gcloud_auth.rs:50`**: Uses `std::fs::read_dir(&dir).ok()?` to discover cached tokens. If the `.antigravity-server` directory lacks read permissions, or does not exist, the error is silently swallowed without logging.
*   **`crates/op-identity/src/gcloud_auth.rs:126`**: `std::fs::read_to_string(path).ok()?` silently ignores reading failures of the cache token file, leaving developers blind to authorization state transitions.
*   **`crates/op-identity/src/gcloud_auth.rs:173` and `Line 188`**: Silently discards system errors when spawning the `gcloud` command-line process via `.output().ok()?`.
*   **`crates/op-identity/src/token.rs:78`**: Uses `let _ = self.write_to_keyring(&ct).await;` to write fresh OAuth credentials. Errors returned by the system keyring (e.g., `dbus` daemon unavailable) are discarded without logging or fallback feedback.
*   **`crates/op-identity/src/wireguard.rs:98`**: Swallows standard command failures:
    ```rust
    if !output.status.success() {
        return Ok(Vec::new());
    }
    ```
    If `wg show wg0 latest-handshakes` fails with a non-zero exit code (e.g., interface missing), it returns an empty vector silently instead of capturing the `stderr` stream or emitting a `warn!` event.
*   **`crates/op-identity/src/wireguard.rs:126`**: Swallows non-zero exits for `wg show wg0 allowed-ips` in the exact same manner, masking configurations issues.
*   **`crates/op-identity/src/schema_bridge.rs:475`**:
    ```rust
    let Ok(out) = Command::new("incus")
        .args(["exec", "wg-xray", "--", "wg", "show", &iface, "latest-handshakes"])
        .output()
    else { continue };
    ```
    Silently skips errors when the `incus` CLI fails to execute or is absent from the host.
*   **`crates/op-identity/src/schema_bridge.rs:480`**: Swallows standard command execution failures for `incus exec` via `if !out.status.success() { continue }`, ignoring container execution failures during live handshake tracing.

---

## 3. PII & Secret Exposure in Logs

*   **`crates/op-identity/src/session.rs:233`**: Logs raw cleartext user email addresses (Personally Identifiable Information) during the registration stage:
    ```rust
    info!("Registered WireGuard user: {} -> {}", pubkey, user_email);
    ```
*   **`crates/op-identity/src/session.rs:20`**: The `Session` struct derives `Debug`:
    ```rust
    #[derive(Debug, Clone)]
    pub struct Session {
        pub session_id: String,
        pub pubkey: String,
        pub user_email: Option<String>,
        pub oauth_token: Option<String>,
        pub token_expires_at: Option<DateTime<Utc>>,
        pub created_at: DateTime<Utc>,
        pub last_seen_at: DateTime<Utc>,
    }
    ```
    Because `oauth_token` is preserved as a plain `Option<String>` inside a `#[derive(Debug)]` struct, any developer logging the session context (e.g., `debug!("Current session: {:?}", session)`) will dump raw, high-privilege Google Cloud OAuth tokens directly to system journals or diagnostic endpoints.

---

## 4. Metrics Instrumentation

*   **No Active Metrics Instrumentation**: The `op-identity` crate possesses **zero metrics instrumentation**. No prometheus registry, opentelemetry counters, or `metrics` crate dependencies are imported or called in any of the audited source files.

---

## 5. Security & Quality Findings

### CRITICAL: Memory Mapping Out-of-Bounds Read (Undefined Behavior / Crash Loop)
*   **File**: `crates/op-identity/src/anna_scribe.rs`
*   **Lines**: 58-65
*   **Vulnerability**: 
    The function `notarize_arrival` maps `/dev/shm/plugin_schema.dat` into memory using `MmapOptions::new().map(&file)` without specifying an explicit target size or verifying the file size on disk beforehand:
    ```rust
    let mmap = unsafe {
        MmapOptions::new()
            .map(&file)
            .map_err(|_| "Memory map failed".to_string())?
    };
    let schema_ptr = mmap.as_ptr() as *const PluginSchema;
    let is_valid = unsafe { (*schema_ptr).is_valid };
    ```
    If `/dev/shm/plugin_schema.dat` is empty (0 bytes) or truncated (e.g., after a system restart, crash, or an interrupted swap operation), `map()` maps 0 bytes or a truncated slice. Casting this pointer directly to a `*const PluginSchema` (which requires at least 80 bytes) and dereferencing fields like `is_valid` (offset 40) or `mutation_index` (offset 32) triggers an **out-of-bounds memory read**, leading to an immediate **SIGBUS/SIGSEGV crash**. Because this is run during connection arrival, an attacker triggering WireGuard connections while the shared memory file is empty can force the notary arbitrator into a persistent crash loop.
*   **Remediation**: Explicitly verify that the file size matches `std::mem::size_of::<PluginSchema>()` before calling `map()`, or use `.len(std::mem::size_of::<PluginSchema>())` on `MmapOptions` to force length validation at mapping boundaries (as is correctly done in `crates/op-identity/src/schema_bridge.rs:219`).

---

### HIGH: Weak Cryptographic Hash Function (MD5)
*   **File**: `crates/op-identity/src/anna_scribe.rs`
*   **Lines**: 74-76
*   **Vulnerability**:
    ```rust
    let payload = format!("{}:{}", wg_pubkey, current_mutation);
    let genesis_hash = format!("{:x}", md5::compute(payload.as_bytes()));
    ```
    The cryptographic binding between the WireGuard identity and the current mutation index relies on **MD5**. Because MD5 is vulnerable to collision attacks, this is a weak mechanism for establishing trace IDs and session ledger footprints, exposing the system to replay or footprint-collision injection attacks.
*   **Remediation**: Transition the genesis footprint hash to SHA-256 to align with the higher cryptographic standards used in the rest of the workspace.

---

### Schema-as-Code Compliance Deviations
The system asserts a "schema-as-code discipline using Protocol Buffers and OSCAL." However, multiple instances express core identity and configuration contracts using ad-hoc constructs, binary structures, or raw string interpolations:

*   **Ad-hoc Shared Memory Layouts**: Both `PluginSchema` (`crates/op-identity/src/anna_scribe.rs:18`) and `IdentitySled` (`crates/op-identity/src/schema_bridge.rs:120`) define ad-hoc, packed raw C binary layouts mapped directly to shared memory paths rather than compiled versioned ProtoBuf schemas.
*   **Ad-hoc Session Tracking**: `SessionLedger` (`crates/op-identity/src/anna_scribe.rs:28`) is represented as an ad-hoc Rust struct, bypassing structured schema definitions.
*   **JSON Config Generation via Format String**: `crates/op-identity/src/schema_bridge.rs:253` constructs the entire `xray-ghostbridge.json` configuration file via dynamic string interpolation (`format!`) of a template block instead of using a versioned serialized schema struct through `serde`:
    ```rust
    let config = format!(
        r#"{{
      "log": {{ "loglevel": "warning" }},
      "dns": {{ ...
    "#
    ```
    This string-based schema construction easily leads to parsing failures, escaping issues, or malformed JSON payloads if components like `footprint`, `trace_id`, or `uuid` contain unexpected characters.