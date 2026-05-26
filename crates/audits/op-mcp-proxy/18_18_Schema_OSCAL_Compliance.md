# Production Security and Quality Audit Report

This report presents a security and quality audit of the `op-mcp-proxy` crate in the `OP-DBUS` workspace. The audit specifically focuses on compliance with the **Schema-as-Code** discipline (requiring versioned schemas such as Protocol Buffers and machine-readable data contracts instead of ad-hoc structs and strings) and **OSCAL Compliance** (machine-readable representation of security controls, system boundaries, and policies).

---

## 1. Schema-as-Code Audit

The following table documents violations of the Schema-as-Code discipline found in `crates/op-mcp-proxy`. Ad-hoc structs, raw byte-offset slicing, and manual serialization mappings bypass formal validation, versioning, and backward-compatibility guarantees.

| Item | Type | file:line | Has .proto? | Gap Description |
| :--- | :--- | :--- | :---: | :--- |
| **IdentitySled Layout Parsing** | Binary `#[repr(C)]` memory layout | `crates/op-mcp-proxy/src/sled.rs:31-50` | No | Reads raw shared memory bytes from `/dev/shm/plugin_schema.dat` using hardcoded byte offsets (`bytes[0..32]`, `bytes[32..40]`, etc.). Bypasses any schema-driven serialization like Protocol Buffers, creating fragility if `op-identity` changes its structure. |
| **JSON-RPC MCP untyped JSON** | Untyped `simd_json::OwnedValue` | `crates/op-mcp-proxy/src/main.rs:124-150` | No | Enforces JSON-RPC and MCP data contracts dynamically via ad-hoc nested string keys (e.g. `req["method"]`, `req["params"]["model"]`) rather than serializing into typed, versioned Rust structures generated from a schema. |
| **OpenAI Compatibility Structures** | Ad-hoc Rust Structs | `crates/op-mcp-proxy/src/http_server.rs:53-111` | No | Defines OpenAI-compatible HTTP request and response models (such as `ChatCompletionRequest` and `ChatCompletionResponse`) manually with standard Serde macros instead of deriving them from versioned OpenAPI schemas or Protocol Buffers. |
| **SQLite Session Database Schema** | Inline raw SQL statements | `crates/op-mcp-proxy/src/session.rs:52-70` | No | Defines database tables (`sessions`, `wireguard_users`) and constraints as raw multi-line strings inside code. Bypasses versioned migration files or schema-as-code declarative synchronization tools. |

---

## 2. OSCAL Coverage Audit

The following table lists security controls, system boundaries, policies, and service interfaces implemented within the code but lacking machine-readable mapping definitions in OSCAL artifacts (such as Component Definitions, System Security Plans (SSP), and Assessment Plans).

| Control Area | Implemented at file:line | OSCAL Artifact | Gap Description |
| :--- | :--- | :--- | :--- |
| **IA-2: Identification and Authentication** | `crates/op-mcp-proxy/src/session.rs:141` | Component Definition | WireGuard public-key session association is implemented to identify peers, but this authentication boundary is not documented in any machine-readable OSCAL component definition. |
| **IA-5: Authenticator Management** | `crates/op-mcp-proxy/src/gcloud_auth.rs:114` | System Security Plan (SSP) | Retrieval of GCP OAuth tokens from local home directories and VSCode caches is coded directly but lacks control mapping describing credential ingestion boundaries. |
| **SC-8: Transmission Confidentiality** | `crates/op-mcp-proxy/src/vertex_grpc.rs:37` | System Security Plan (SSP) | TLS configuration with webpki roots is established programmatically for Vertex AI interactions, but there is no OSCAL representation for transit encryption boundaries. |
| **AU-2: Event Logging & AU-12** | `crates/op-mcp-proxy/src/http_server.rs:259` | System Security Plan (SSP) | Request logging middleware tracks and categorizes HTTP statuses but has no associated metadata tracing to OSCAL AU controls. |
| **SC-5: Denial of Service Protection** | `crates/op-mcp-proxy/src/http_server.rs:136` | Component Definition | Throttling limits (default 200 RPM) are configured directly in code without formal documentation as machine-readable boundary protection controls. |

---

## 3. Major and Critical Security Findings

### Finding 1: Local Denial of Service (DoS) via Truncated Identity Sled (Critical)
* **Citation**: `crates/op-mcp-proxy/src/sled.rs:31-45`
* **Vulnerability Type**: Insecure Memory Mapping / Unhandled `SIGBUS` Signal
* **Exploitability**: Directly exploitable by any unprivileged local user.
* **Description**:
  The application reads host identity information by memory-mapping the file `/dev/shm/plugin_schema.dat` (which lies in the world-writable Linux shared memory directory `/dev/shm`).
  ```rust
  let file = File::open(SLED_PATH).ok()?;
  let mmap = unsafe { MmapOptions::new().len(SLED_SIZE).map(&file).ok()? };
  if mmap.len() < SLED_SIZE { return None; }

  let bytes = &mmap[..SLED_SIZE];
  let wg_pubkey     = &bytes[0..32];
  let mutation_index = u64::from_le_bytes(bytes[32..40].try_into().ok()?);
  ```
  The code attempts to guard against short reads with the check `if mmap.len() < SLED_SIZE`. However, because `.len(SLED_SIZE)` is explicitly set on the `MmapOptions` builder, the returned `Mmap` length is guaranteed to be exactly `SLED_SIZE` (208 bytes) if the map call succeeds.
  Under Linux, mapping a file beyond its actual physical size is permitted by the kernel up to the memory page boundary (typically 4096 bytes). However, any subsequent read access to memory offsets that lie beyond the physical file size raises a **`SIGBUS` (Bus Error)** signal.
  If an unprivileged attacker creates an empty (0-byte) file or truncates `/dev/shm/plugin_schema.dat`, the mapping operation succeeds, but accessing `bytes[0..32]` or parsing `mutation_index` triggers a `SIGBUS` signal. Because `op-mcp-proxy` runs without a custom signal handler, the kernel immediately terminates the process. Since this operation is executed on startup (in `main.rs:31`), any local user can permanently prevent the proxy from running.
* **Remediation**:
  Verify the physical metadata size of the file descriptor prior to performing the memory mapping:
  ```rust
  let file = File::open(SLED_PATH).ok()?;
  let metadata = file.metadata().ok()?;
  if metadata.len() < SLED_SIZE as u64 {
      warn!("Sled file is truncated or empty; ignoring mapping to prevent SIGBUS.");
      return None;
  }
  ```

---

### Finding 2: Plaintext Storage of Active OAuth Tokens & World-Readable Database Permissions (Major)
* **Citation**: `crates/op-mcp-proxy/src/session.rs:50-58`
* **Vulnerability Type**: Cryptographic Credential Exposure & Insecure File Permissions
* **Exploitability**: Directly exploitable by local unauthorized users.
* **Description**:
  The session manager initializes a local SQLite database at `~/.local/share/mcp-proxy/sessions.db` to record WireGuard authentication context. The database stores active Google Cloud OAuth tokens (`oauth_token TEXT` at line 56) in plaintext.
  When creating the database directory and opening the connection, the code does not enforce restrictive filesystem permissions:
  ```rust
  if let Some(parent) = db_path.parent() {
      std::fs::create_dir_all(parent)?;
  }
  let conn = Connection::open(&db_path)?;
  ```
  Consequently, both the `mcp-proxy` directory and the `sessions.db` database are created utilizing the system's default `umask` (commonly `0022` or `0002`). This leaves the SQLite database world-readable or group-readable. Any local user or process with access to the filesystem can open the SQLite file and extract active Google Cloud OAuth credentials.
* **Remediation**:
  1. Ensure that the database file is restricted to the owner (`0600` permissions) by configuring permissions programmatically on Unix targets before opening the connection:
     ```rust
     #[cfg(unix)]
     {
         use std::os::unix::fs::DirBuilderExt;
         if let Some(parent) = db_path.parent() {
             let mut builder = std::fs::DirBuilder::new();
             builder.recursive(true).mode(0o700);
             builder.create(parent)?;
         }
     }
     ```
  2. Encrypt the database credentials at rest or utilize standard platform-specific secret vaults instead of raw SQLite tables.

---

### Finding 3: Division-by-Zero Panic in HTTP Rate Limiter (Major)
* **Citation**: `crates/op-mcp-proxy/src/http_server.rs:43-50`
* **Vulnerability Type**: Integer Division / Floating Point Panic (DoS)
* **Exploitability**: Triggerable through configuration or environment variables.
* **Description**:
  The OpenAI-compatible endpoint uses a token bucket rate limiter to protect the system. Throttling wait duration is calculated inside the `try_consume` method:
  ```rust
  let wait_secs = (1.0 - self.tokens) * 60.0 / self.capacity;
  Err(std::time::Duration::from_secs_f64(wait_secs))
  ```
  If `VERTEX_RATE_LIMIT_RPM` is misconfigured or explicitly configured to `0` (which is a common pattern for "disabling" a rate-limit constraint), the `capacity` field becomes `0.0`.
  Calculating `wait_secs` with a `capacity` of `0.0` results in division by zero, yielding `f64::INFINITY`.
  In Rust, calling `std::time::Duration::from_secs_f64` with `f64::INFINITY` or `f64::NAN` triggers an immediate panic. A single incoming HTTP request on `/v1/chat/completions` will panic the server, causing a denial of service.
* **Remediation**:
  Explicitly reject or handle cases where rate limit capacity is set to `0`, either by treating it as a total block or by bypassing the rate-limiting logic:
  ```rust
  if self.capacity <= 0.0 {
      // Either return an error or disable rate limiting entirely
      return Ok(());
  }
  ```