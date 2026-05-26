### Dependencies & Feature Inventory

#### Direct Dependencies from `crates/op-identity/Cargo.toml`

| Dependency | Version | Explicitly Enabled Features | Pulled in by Default / Workspace | Security / Quality Notes |
| :--- | :--- | :--- | :--- | :--- |
| `anyhow` | `1` | None | Yes | General error handling wrapper. |
| `tokio` | `1` | `["full"]` | Yes | Over-privileged feature set (`full`) increases compile time and binary size. |
| `serde` | `1` | `["derive"]` | Yes | Standard serialization framework. |
| `simd-json`| Workspace | `["serde", "serde_impl"]` | Yes (from workspace) | **Soundness/CVE Adjacent**: Uses `unsafe` parsing; requires 16–32 byte buffer padding to avoid out-of-bounds reads. |
| `zbus` | `5.12` | `["tokio"]` | Yes | D-Bus communication framework. |
| `chrono` | `0.4` | `["serde"]` | Yes | Time utilities. |
| `sha2` | Workspace | None | Yes (from workspace) | Cryptographic hash functions. |
| `uuid` | `1.6` | `["v4", "serde"]` | Yes | UUID generation. |
| `tracing` | `0.1` | None | Yes | Diagnostics and log instrumentation. |
| `keyring` | `2` | None | Yes | Secure credential management. |
| `op-core` | Path | None | N/A (Internal) | Internal core dependency. |
| `op-compliance`| Path| None | N/A (Internal) | Internal compliance dependency. |
| `dashmap` | Workspace | None | Yes (from workspace) | Concurrent hash map. |
| `dirs` | `5` | None | Yes | User home directory lookup. |
| `hostname` | `0.4` | None | Yes | Hostname lookup. |
| `rand` | Workspace | None | Yes (from workspace) | Random number generator. |
| `base64` | Workspace | None | Yes (from workspace) | Base64 encoding. |
| `hex` | Workspace | None | Yes (from workspace) | Hex encoding. |
| `memmap2` | Workspace | None | Yes (from workspace) | Memory mapping utility. |
| `md5` | Workspace | None | Yes (from workspace) | **Security Risk**: Cryptographically broken hash algorithm. Used for session identity generation. |
| `x25519-dalek` | `2` | `["static_secrets"]` | Yes | Diffie-Hellman key exchange. |

#### Crate Features Section (`crates/op-identity/Cargo.toml`)
* **None defined** in the package-specific `Cargo.toml`.

#### Schema-as-Code Dependencies
* **Missing Protobuf / Versioned Schema Enforcement**: While the workspace contains `prost`, `tonic`, and `jsonschema` dependencies, the `op-identity` crate defines its critical data contracts (`PluginSchema` and `IdentitySled`) as raw, ad-hoc `#[repr(C)]` memory layouts. There is **no** versioned schema validation (`prost-build`, `schemars`, `oscal-rs`) protecting these physical memory-mapped structures.

---

### Storage Backend Check

The `op-identity` crate intentionally avoids writing data backends to Btrfs-backed block storage to prevent mutation loops. It enforces a purely volatile, in-memory, and OS-level credential storage architecture.

#### Storage Backend Inventory (Provided Files)

| Backend | Found at File:Line | Role (KV / Graph / Cache / Queue) | Architectural/Quality Violation |
| :--- | :--- | :--- | :--- |
| `DashMap` | `crates/op-identity/src/session.rs:36-37` | Volatile In-Memory Session & User Mapping KV | None. Used as a high-performance, lock-free volatile store. |
| Shared Memory (`/dev/shm/plugin_schema.dat`) | `crates/op-identity/src/schema_bridge.rs:25` | Zero-copy IPC State Bridge | **High**: Prone to unsynchronized concurrent writes and alignment mismatches. |
| `keyring` (`org.freedesktop.secrets`) | `crates/op-identity/src/token.rs:73` | Secure OS-level Token Cache | None. |

* **CozoDB / Sled Absence**: The workspace lists `cozo` and `sled`, but `op-identity` bypasses persistent database engines completely, operating strictly out of ephemeral `tmpfs` and system keyring caches.

---

### Audit Findings

#### CRITICAL: JSON/Protocol Injection in Xray Configuration Generator
* **File:Line**: `crates/op-identity/src/schema_bridge.rs:214-300`
* **Vulnerability Type**: Input Sanitization / Configuration Injection
* **Description**: The function `write_xray_config_with_sockets` populates the Xray configuration JSON using raw string formatting (`format!`) instead of `serde_json` serialization:
  ```rust
  "servers": [ "https://dns.nextdns.io/{profile}/Ghostbridge-Incus" ]
  ```
  The `{profile}` placeholder is populated by `nextdns_profile_str()`, which reads directly from the zero-copy shared memory block (`IdentitySled::nextdns_profile`). An attacker capable of mutating shared memory (`/dev/shm/plugin_schema.dat`) can inject double quotes (`"`), commas, or control characters into the 16-byte `nextdns_profile` buffer. 
* **Exploit Scenario**: 
  1. An attacker writes the following payload into `/dev/shm/plugin_schema.dat` under `nextdns_profile`: `a/../xyz","dns-out`
  2. The Shuttle daemon reads the sled and runs `write_xray_config`.
  3. The resulting formatted JSON is broken/injected with malicious DNS routes, hijacking DNS resolution inside the Incus network namespace.
* **Remediation**: Completely abandon manual string formatting for JSON generation. Represent the Xray config as a strongly-typed Rust struct and serialize it safely using `serde_json::to_string`.

---

#### HIGH: Unsound Pointer Cast & Missing Bounds Checks on Memory-Mapped File
* **File:Line**: `crates/op-identity/src/anna_scribe.rs:56-61` and `crates/op-identity/src/schema_bridge.rs:252-257`
* **Vulnerability Type**: Memory Safety / Soundness / Undefined Behavior
* **Description**: A.N.N.A Scribe reads from `/dev/shm/plugin_schema.dat` by memory-mapping it and immediately casting the raw buffer pointer to a structured reference without checking the physical size of the file:
  ```rust
  let mmap = unsafe {
      MmapOptions::new()
          .map(&file)
          .map_err(|_| "Memory map failed".to_string())?
  };
  let schema_ptr = mmap.as_ptr() as *const PluginSchema;
  let is_valid = unsafe { (*schema_ptr).is_valid };
  ```
  If `/dev/shm/plugin_schema.dat` is empty or truncated to a size smaller than `std::mem::size_of::<PluginSchema>()` (at least 73 bytes), this cast leads to an out-of-bounds memory read, triggering an immediate `SIGBUS` or `SIGSEGV` crash of the daemon. Furthermore, because the memory map is un-synchronized, concurrent writers can alter the file in-place, leading to data races and Undefined Behavior.
* **Remediation**: Check that the mapped file size is exactly equal to `std::mem::size_of::<PluginSchema>()` (or `IdentitySled::SIZE`) before casting the pointer. Guard memory accesses with appropriate file-locking primitives (e.g., `fs2::FileExt::lock_shared`).

---

#### HIGH: Use of Cryptographically Broken Hash (MD5) for Identity Notarization
* **File:Line**: `crates/op-identity/src/anna_scribe.rs:77-79`
* **Vulnerability Type**: Weak Cryptographic Algorithm
* **Description**: A.N.N.A Scribe constructs the deterministic session "Snowball" genesis hash using MD5:
  ```rust
  let payload = format!("{}:{}", wg_pubkey, current_mutation);
  let genesis_hash = format!("{:x}", md5::compute(payload.as_bytes()));
  ```
  MD5 is completely broken under collision and preimage attacks. If an attacker can craft a WireGuard public key that collides with an existing identity's MD5 fingerprint under a target mutation index, they can spoof trace identifiers and poison the accountability loop.
* **Remediation**: Upgrade the genesis hashing algorithm to SHA-256 or BLAKE3 to guarantee cryptographic resistance to collisions and preimages.

---

#### HIGH: Undefined Behavior Risk via `simd_json::from_str` on Inadequately Padded String
* **File:Line**: `crates/op-identity/src/token.rs:74-75`
* **Vulnerability Type**: Soundness / Memory Safety
* **Description**: The codebase invokes `simd_json::from_str` on a standard mutable `String` retrieved from the system keyring:
  ```rust
  let mut json = entry.get_password()?;
  Ok(unsafe { simd_json::from_str(&mut json) }?)
  ```
  `simd-json` requires the target input buffer to have an allocation padding of at least `simd_json::SIMDJSON_PADDING` (typically 16 or 32 bytes) at the end of the string. Standard Rust `String` allocations do not guarantee this padding. Running `simd_json::from_str` on unpadded strings can lead to out-of-bounds reads during vector register operations, causing crashes or memory leakage.
* **Remediation**: Use `simd_json::to_padded_bin` or fallback to standard, safe `serde_json::from_str` for values coming from unpadded external sources like system keyrings.

---

#### HIGH: Identity Spoofing via Predictable Hostname Fallback
* **File:Line**: `crates/op-identity/src/wireguard.rs:46-52`
* **Vulnerability Type**: Authentication Bypass / Identity Verification Failure
* **Description**: If the WireGuard interface command fails or the public key cannot be queried, the `get_local_pubkey()` method silently downgrades the system's cryptographically secure identity verification to a hostname-based string:
  ```rust
  warn!("Could not get WireGuard pubkey, using hostname-based ID");
  Ok(format!("local:{}", hostname))
  ```
  Hostnames are highly predictable, completely unverified, and easily forgeable by any unprivileged local process. An attacker can hijack session identities on a shared local plane by matching their hostname to the victim's.
* **Remediation**: If cryptographic identity detection fails, fail loudly and refuse to authenticate the session. Under no circumstances should the system fallback to unauthenticated, predictable identifiers.

---

#### MEDIUM: Schema-as-Code Quality Gap — Ad-Hoc Data Contracts
* **File:Line**: `crates/op-identity/src/anna_scribe.rs:18` and `crates/op-identity/src/schema_bridge.rs:125`
* **Vulnerability Type**: Design Quality / Interoperability Risk
* **Description**: The crate defines `PluginSchema` and `IdentitySled` as ad-hoc, manual C-compatible structs mapped to raw byte structures. This bypasses the schema-as-code discipline defined in the project manifest. Any mismatch in memory alignment, endianness, padding, or struct versioning between the writer (SchemaEngine) and the reader (A.N.N.A. Scribe / Shuttle) will result in silent memory corruption or parsing of garbage values.
* **Remediation**: Replace raw `#[repr(C)]` shared memory structures with serialized, versioned contracts (e.g., Protocol Buffers / `prost` or JSON Schema validated payloads) stored in shared memory.

---

#### MEDIUM: Hardcoded Cryptographic Private Keys and UUIDs in Fallback Paths
* **File:Line**: `crates/op-identity/src/schema_bridge.rs:400-402`
* **Vulnerability Type**: Hardcoded Secrets
* **Description**: The Xray configuration re-baking loop contains hardcoded fallback credentials:
  ```rust
  let uuid    = env::var("XRAY_UUID").unwrap_or_else(|_| "40813c05-4a7c-4d5b-b027-33912551287f".to_string());
  let privkey = env::var("XRAY_PRIVATE_KEY").unwrap_or_else(|_| "-MULA7gIbk_58CKa4TNHovpYNt192NUkPlQF7f3caWo".to_string());
  let short   = env::var("XRAY_SHORT_ID").unwrap_or_else(|_| "2a32c53278372687".to_string());
  ```
  If environment variables are misconfigured in production, the system silently degrades to using a globally shared, static private key and UUID. This allows any passive network observer to intercept and decrypt Xray-Ghostbridge network traffic.
* **Remediation**: Remove the default string fallback. If mandatory environment variables are missing, abort execution and throw a clear configuration error.