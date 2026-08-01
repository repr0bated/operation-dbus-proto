# LICENSE AUDIT

### 1. Workspace License
- **License Field:** `Apache-2.0` (defined in the main workspace package configuration at `Cargo.toml:39`).

### 2. Crate License Check
- **Crate:** `op-identity`
- **License Field:** **MISSING** (No `license` or `license.workspace = true` key is present in `crates/op-identity/Cargo.toml:1-6`).

### 3. Incompatible Licenses Scan (`Cargo.lock`)
- **Copyleft (GPL/AGPL/SSPL) Crates:** None found.
- **Copyleft-Adjacent Crates:** `cozo` version `0.7.6` is licensed under MPL-2.0 (Mozilla Public License 2.0), which is a weak copyleft license. It is compatible with the workspace's `Apache-2.0` license provided that Cozo remains unmodified or any modifications to it are distributed under MPL-2.0 rules.

---

# PRODUCTION QUALITY AND SECURITY AUDIT

## CRITICAL SEVERITY FINDINGS

### 1. Keyring Deserialization Memory Corruption / Undefined Behavior
- **Citation:** `crates/op-identity/src/token.rs:75-76`
- **Vulnerability:** Unsafe deserialization with `simd-json` on unpadded buffers.
- **Impact:** 
  The function `read_from_keyring` retrieves a JSON string from the system keyring via `entry.get_password()?` and immediately invokes `unsafe { simd_json::from_str(&mut json) }`. 
  `simd-json` requires that the input string buffer is padded with at least `simd_json::SIMDJSON_PADDING` (typically 32 bytes) of extra capacity beyond the string's logical length, and that it is properly aligned. A standard `String` returned from `keyring` does not guarantee this padding or SIMD-compatible alignment. Passing a raw, unpadded `&mut String` to `simd_json::from_str` causes out-of-bounds reads and writes by SIMD instructions, leading to heap corruption, memory leaks, or segmentation faults.

### 2. WireGuard IP Matching Logic Flaw (Authorization Bypass)
- **Citation:** `crates/op-identity/src/wireguard.rs:85-87`
- **Vulnerability:** Substring check on IP address strings.
- **Impact:**
  The `get_pubkey_for_ip` function parses `wg show allowed-ips` output and matches the peer's public key using `ips.contains(peer_ip)`. Because `contains` performs a substring match rather than a strict network/CIDR equality comparison:
  - An attacker operating from IP `10.0.0.10` will match a registered allowed IP block of `10.0.0.1/32` or `10.0.0.100/32` (since `"10.0.0.10"`.contains(`"10.0.0.1"`) or vice versa is true).
  - This allows a malicious peer with a trailing digit or matching prefix to hijack the session identity of another legitimate peer, fully bypassing identity cryptographic enforcement.

### 3. Out-Of-Bounds Read and Data Race via Unsynchronized Memory Maps
- **Citation:** `crates/op-identity/src/anna_scribe.rs:56-62` and `crates/op-identity/src/schema_bridge.rs:191-196`
- **Vulnerability:** Unchecked raw pointer dereferencing on memory-mapped files without synchronization.
- **Impact:**
  Both `AnnaScribe::notarize_arrival` and `read_sled` memory-map `/dev/shm/plugin_schema.dat` and cast the raw pointer to `*const PluginSchema` and `*const IdentitySled`, respectively, before dereferencing them.
  - **No File Size Validation:** There is no check to verify that the file size is at least equal to `std::mem::size_of::<PluginSchema>()` or `IdentitySled::SIZE`. If `/dev/shm/plugin_schema.dat` is empty or truncated, dereferencing the pointer triggers an immediate out-of-bounds read and causes a `SIGBUS` or `SIGSEGV` crash.
  - **Data Race (UB):** The memory mapping is performed without atomic types or memory barriers. If `SchemaEngine` is concurrently writing to the same shared memory location, reading non-atomic fields (e.g., `is_valid`, `mutation_index`) causes a data race, which constitutes Undefined Behavior under the Rust memory model and can result in reading torn or inconsistent states.

---

## HIGH SEVERITY FINDINGS

### 1. PATH Interception via Bare Subprocess Execution
- **Citation:** `crates/op-identity/src/gcloud_auth.rs:254`, `crates/op-identity/src/token.rs:59`, `crates/op-identity/src/wg.rs:17`, and `crates/op-identity/src/wireguard.rs:31`
- **Vulnerability:** Subprocess invocation searching system `PATH` instead of using absolute executable paths.
- **Impact:**
  The commands `Command::new("gcloud")`, `Command::new("wg")`, `Command::new("incus")`, and `Command::new("xray")` search the default system `PATH` for their respective binaries. If the environment's `PATH` variable is misconfigured or writable by an unprivileged user, an attacker can drop a malicious binary named `gcloud` or `wg` into a directory that takes precedence, resulting in privilege escalation or arbitrary command execution as the service user.

### 2. JSON Injection in Stateless Configuration Generator
- **Citation:** `crates/op-identity/src/schema_bridge.rs:222-384`
- **Vulnerability:** Manual JSON string interpolation using `format!`.
- **Impact:**
  Instead of utilizing `serde_json` to safely serialize a structured config, `write_xray_config_with_sockets` manually constructs JSON using format interpolation of `uuid`, `private_key`, and `short_id`. If any of these values are populated from untrusted environment variables or contain double quotes or backslashes, they will break the JSON structure, leading to parsing errors in Xray, or allowing arbitrary JSON injection to redefine Xray routing rules.

### 3. Insecure Hostname Fallback Identity
- **Citation:** `crates/op-identity/src/wireguard.rs:48-53`
- **Vulnerability:** Fallback to unauthenticated system hostname.
- **Impact:**
  If the `wg` CLI command fails or the local interface is missing, the code falls back to generating a deterministic ID using the system hostname (`format!("local:{}", hostname)`). Hostnames are trivially spoofable, guessable, and lack cryptographic authenticity. Relying on hostnames for user identification breaks the secure-by-default login model of the system.

---

## MEDIUM SEVERITY FINDINGS

### 1. Concurrent Token Refresh Race Condition
- **Citation:** `crates/op-identity/src/session.rs:173-199`
- **Vulnerability:** Unsynchronized external credential refresh.
- **Impact:**
  The `get_valid_token` function retrieves the current session, checks for token expiration, and refreshes the token via `self.gcloud_auth.get_token().await?`. If multiple concurrent async tasks detect an expired token simultaneously, they will unlock the session map and trigger concurrent token refresh requests. This floods the Google Cloud auth endpoints and causes write-race conditions where older tokens overwrite newer ones.

### 2. Insecure Permissions on Cached Tokens
- **Citation:** `crates/op-identity/src/gcloud_auth.rs:49-65`
- **Vulnerability:** Reading cached OAuth tokens without checking file permissions.
- **Impact:**
  The token lookup looks inside the `~/.antigravity-server` directory for `.token` files. The implementation fails to verify that these token files have strict permissions (e.g., owner-read-write only, `0600`). This makes cached OAuth credentials accessible to other local users or compromised processes running on the same machine.

### 3. Cryptographically Broken Hash Algorithm (MD5)
- **Citation:** `crates/op-identity/src/anna_scribe.rs:73`
- **Vulnerability:** Use of MD5 for cryptographic session ledger footprints.
- **Impact:**
  MD5 is vulnerable to collision attacks and is no longer considered secure for generating unique cryptographic footprints. Although used for "continuity with the EventChain system," relying on MD5 to map a WireGuard public key to a session fingerprint is a cryptographically weak practice.

---

## LOW SEVERITY FINDINGS

### 1. Hardcoded Shared Memory Pathing
- **Citation:** `crates/op-identity/src/anna_scribe.rs:52` and `crates/op-identity/src/anna_scribe.rs:110`
- **Vulnerability:** Hardcoded files under `/dev/shm`.
- **Impact:**
  The path `/dev/shm/plugin_schema.dat` and `/dev/shm/snowball_session.log` are hardcoded. This prevents running multiple instances of the service or testing the code cleanly in environments where `/dev/shm` is read-only or restricted.

---

# SCHEMA-AS-CODE DISCIPLINE AUDIT

The codebase implements a schema-as-code discipline, but several locations violate this by expressing data contracts as ad-hoc, manual structs or plain string parsing instead of versioned Protobuf or OSCAL schemas.

### Ad-Hoc Structs & Manual Binary Layouts
- **Citation:** `crates/op-identity/src/anna_scribe.rs:19-25`
  `PluginSchema` is written as a manual, unversioned C-repr struct directly mapped to shared memory.
- **Citation:** `crates/op-identity/src/schema_bridge.rs:135-182`
  The `IdentitySled` struct defines a complex data contract including identity blocks, subid taxonomies, compliance frameworks, and routing parameters. Defining this complex compliance layout as a raw Rust struct mapping bytes rather than generating the memory structure from versioned schemas (like OSCAL or Protocol Buffers) makes it highly fragile and hard to maintain across control plane components.

### Ad-Hoc String Parsers instead of Schemas
- **Citation:** `crates/op-identity/src/schema_bridge.rs:75-121`
  The `SubidTaxonomy::parse` function performs ad-hoc string parsing (`<category>.<component-type>.<subject>.<verb>[.<facet>][@vN]`) with manual string splitting and validation checks. These taxonomies should be derived from versioned schemas (e.g., JSON Schema, Protobuf definitions, or OSCAL components) to ensure structural conformance across all microservices.

---
## ⚠ Citation Warnings
- `crates/op-identity/src/gcloud_auth.rs:254`: file has 244 lines
