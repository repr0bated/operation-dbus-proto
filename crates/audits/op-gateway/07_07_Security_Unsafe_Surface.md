# Production Security & Quality Audit: op-gateway

---

## 1. Unsafe Blocks & Safety Analysis

Two `unsafe` blocks are utilized in the codebase. Both lack the mandatory `// SAFETY:` comments explaining why the operations are safe.

### Finding 1: Missing Safety Comment for Zero-Copy Deserialization
*   **File & Location**: `crates/op-gateway/src/encrypted_storage.rs` (around line 366)
*   **Context**:
    ```rust
    let entry_json = async_fs::read_to_string(&key_file_path).await?;
    let mut entry_str = entry_json.clone();
    let entry: EncryptedKeyEntry = unsafe { simd_json::from_str(&mut entry_str) }?;
    ```
*   **Analysis**: `simd_json::from_str` performs in-place mutation of the input string to parse JSON tokens. This is `unsafe` because concurrent modification or lifetime mismatch can trigger undefined behavior. While mutating a freshly cloned local string (`entry_str`) prevents external concurrent mutations, there is no documented safety proof explaining this design choice.

### Finding 2: Missing Safety Comment for Session Flag Parsing
*   **File & Location**: `crates/op-gateway/src/wireguard_auth.rs` (around line 173)
*   **Context**:
    ```rust
    let flags_json: String = row.get("flags");
    let mut flags_str = flags_json.clone();
    let flags: std::collections::HashMap<String, String> =
        unsafe { simd_json::from_str(&mut flags_str) }.unwrap_or_default();
    ```
*   **Analysis**: Similar to the first finding, `simd_json::from_str` is used to mutate an owned local copy of a SQLite string field. While locally safe, it violates the Rust compiler safety contract documentation requirements by omitting the `// SAFETY:` comment.

---

## 2. Command Executions (`Command::new()`)

The crate `op-gateway` contains exactly **5** instances of `Command::new()`.

| # | File:Line | Command | Arguments | Control / Validation State |
|---|---|---|---|---|
| 1 | `crates/op-gateway/src/encrypted_storage.rs:115` | `btrfs` | `["subvolume", "create", "-e", self.storage_path.to_str().unwrap()]` | Partially user-controlled via config (`storage_path` derived from `base_path` and `subvolume_name`). No input sanitization is performed. |
| 2 | `crates/op-gateway/src/encrypted_storage.rs:153` | `dd` | `["if=/dev/zero", &format!("of={}", container_path.display()), "bs=1M", "count=100"]` | Partially user-controlled via config (`container_path` derived from `base_path`). |
| 3 | `crates/op-gateway/src/encrypted_storage.rs:186` | `btrfs` | `["subvolume", "create", self.storage_path.to_str().unwrap()]` | Partially user-controlled via config (`storage_path`). |
| 4 | `crates/op-gateway/src/encrypted_storage.rs:203` | `mount` | `[device_path, self.storage_path.to_str().unwrap()]` | `device_path` comes directly from `config.luks_device_name`. `storage_path` comes from `config.subvolume_name`. Both are unsanitized. |
| 5 | `crates/op-gateway/src/encrypted_storage.rs:454` | `df` | `["-T", self.storage_path.to_str().unwrap()]` | Uses config-derived `storage_path`. Unsanitized. |

### Forbidden Commands Check
A strict search for forbidden commands (`ovs-*` utilities, raw OpenFlow tools, system shells like `sh`/`bash`, and exfiltration binaries like `curl`/`wget`/`nc`) yielded **0 hits**. No forbidden binaries are invoked by `Command::new()`.

---

## 3. Hardcoded Secrets, Default IPs, and Passphrases

Several default values and sensitive parameters are hardcoded directly into the gateway configuration and cryptographic modules:

*   **Default Storage Directory**: `crates/op-gateway/src/encrypted_storage.rs:80`
    *   `base_path: PathBuf::from("/var/lib/op-dbus/encrypted")`
    *   *Risk*: If this directory is not pre-secured on target systems, files could be exposed to other local processes.
*   **Fallback SQLite Database Path**: `crates/op-gateway/src/wireguard_auth.rs:41`
    *   `sqlite:///var/lib/op-dbus/wireguard.db`
*   **Test/Hardcoded Passphrase Warning**: `crates/op-gateway/src/encrypted_storage.rs:172`
    *   `warn!("LUKS setup requires manual intervention - using test passphrase");`
*   **Fixed Salt for Stable PSK Derivation**: `crates/op-gateway/src/wireguard_auth.rs:608`
    *   `let salt = b"WG-STABLE-PSK-2024";`
*   **Fixed Salt for Session Key Derivation**: `crates/op-gateway/src/wireguard_auth.rs:634`
    *   `let salt = b"WG-SESSION-KEY-2024";`

---

## 4. D-Bus Interface Exposure

The codebase implements several D-Bus-compatible interface functions inside `McpGatewayManager` (`crates/op-gateway/src/mcp_gateway.rs`). These methods are exposed to system-bus peers:

1.  **`dbus_route_client`** (accepts `client_name: &str`, `auth_token: Option<&str>`, `peer_pubkey: Option<&str>`)
    *   *Risk*: Allows system bus peers to probe the routing decisions and discover the underlying gRPC service endpoints (`grpc://localhost:50051` or `grpc://localhost:50052`).
2.  **`dbus_validate_session`** (accepts `session_id: &str`)
    *   *Risk*: Allows system bus peers to check the validity of any session identifier.
3.  **`dbus_get_capabilities`** (accepts `session_id: &str`)
    *   *Risk*: Exposes the capabilities (e.g., `["tools", "resources", "full_access"]`) of active sessions to local peers.

*Threat Assessment*: Because the system D-Bus is accessible by local unprivileged users, any peer on the system bus can execute these methods. If no external access-control layer (such as policy files or PolicyKit checks) is implemented, unprivileged users can map active sessions, validate stolen session tokens, and leak private gateway infrastructure endpoints.

---

## 5. Schema-As-Code Discipline Violations

This codebase uses ad-hoc serialization structs instead of standardized, versioned schemas (such as Protocol Buffers or OSCAL standard configurations) to declare data contracts:

*   **Gateway Configurations & Stats**:
    *   `EncryptedStorageConfig` and `KdfParams` (`crates/op-gateway/src/encrypted_storage.rs:20-37`)
    *   `StorageStats` (`crates/op-gateway/src/encrypted_storage.rs:480-491`)
    *   `WireGuardStats` (`crates/op-gateway/src/wireguard_auth.rs:219-231`)
*   **Routing Decisions & Client Details**:
    *   `RoutingDecision` (`crates/op-gateway/src/mcp_gateway.rs:13-21`)
    *   `AccessLevel` (`crates/op-gateway/src/mcp_gateway.rs:24-31`)
    *   `McpClientInfo` (`crates/op-gateway/src/mcp_gateway.rs:35-42`)
    *   `McpSession` (`crates/op-gateway/src/mcp_gateway.rs:46-54`)
*   **D-Bus Ad-Hoc Payload Responses**:
    *   `dbus_route_client` returns an ad-hoc constructed `simd_json::json!` object (`crates/op-gateway/src/mcp_gateway.rs:289-300`) with fields like `"endpoint"`, `"allowed_tools"`, `"capabilities"`, etc.
    *   `dbus_validate_session` returns an ad-hoc JSON structure (`crates/op-gateway/src/mcp_gateway.rs:306-309`).
    *   `dbus_get_capabilities` returns an ad-hoc JSON structure (`crates/op-gateway/src/mcp_gateway.rs:315-318`).

### Recommendation
Port these data structures to versioned Protocol Buffers (using `.proto` definitions compiled with `prost`) or generate compliant OSCAL assessment definitions using `op-compliance` to avoid data contract drift and structural validation bypasses across service restarts.

---

## 6. High & Medium Risks

### Finding 3: Path Traversal Vulnerability in Key Retrieval
*   **Severity**: High
*   **File & Location**: `crates/op-gateway/src/encrypted_storage.rs` (lines 350-365)
*   **Context**:
    ```rust
    pub async fn retrieve_key(&self, key_id: &str) -> anyhow::Result<Vec<u8>> {
        ...
        let key_file_path = self.storage_path.join(format!("{}.key", key_id));
        if !key_file_path.exists() {
            return Err(anyhow::anyhow!("Key not found: {}", key_id));
        }
    ```
*   **Analysis**: The `key_id` argument is accepted as a raw string and combined directly with the encrypted `storage_path` using `Path::join` without sanitization. If the `key_id` contains path traversal sequences like `../../`, a caller with control over `key_id` can break out of the btrfs subvolume context and verify the existence of or read arbitrary files on the local filesystem.
*   **Remediation**: Restrict the character set of `key_id` (e.g., allow only alphanumeric or hex characters) and assert that the resolved `key_file_path` is a child of `self.storage_path`.

### Finding 4: Insecure Panic Vector on Non-UTF-8 Path Conversions
*   **Severity**: Medium
*   **File & Location**: `crates/op-gateway/src/encrypted_storage.rs` (lines 125, 186, 203, 454)
*   **Context**:
    ```rust
    self.storage_path.to_str().unwrap()
    ```
*   **Analysis**: Calling `.unwrap()` on `.to_str()` will trigger a thread panic if the filesystem path contains invalid UTF-8 characters. This represents a Denial-of-Service (DoS) vector if directory structures are manipulated locally by low-privilege actors.
*   **Remediation**: Pass the `Path` or `OsStr` reference directly to `Command::arg`, as `Command::new` natively supports non-UTF-8 OS paths.