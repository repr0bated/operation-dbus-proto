# Memory Map & Allocation Analysis

## Memory Map Table

| Site | file:line | Type (ro/rw/sled) | Risk |
|:---|:---|:---|:---|
| `AnnaScribe::notarize_arrival` | `crates/op-identity/src/anna_scribe.rs:62` | `ro` | **High**: TOCTOU and symlink hijacking of `/dev/shm/plugin_schema.dat`. Causes a `SIGBUS` if the file size on disk is smaller than `std::mem::size_of::<PluginSchema>()`. |
| `schema_bridge::read_sled` | `crates/op-identity/src/schema_bridge.rs:269` | `ro` | **High**: Maps `/dev/shm/plugin_schema.dat` with `IdentitySled::SIZE`. Susceptible to `SIGBUS` if the underlying file is truncated or empty. |

## Sled & Memory Mapping Detail

1. **Sled Storage Location**: The `IdentitySled` is stored on a `tmpfs` volume (`/dev/shm/plugin_schema.dat`). If `/dev/shm` is mounted with the `noexec` option, it prevents binary execution from the mount but still permits memory-mapping files. However, because `/dev/shm` is world-writable on standard Linux configurations, the files `plugin_schema.dat`, `snowball_session.log`, and `xray-ghostbridge.json` are exposed to symlink hijacking, local tampering, and unauthorized manipulation by other local users.
2. **Missing Flush/msync**: `write_sled` (`crates/op-identity/src/schema_bridge.rs:247`) writes the memory sled to disk using standard synchronous file IO: `f.write_all(bytes)?` followed by `f.sync_data()?`. However, since the readers map the file read-only, there are no writable memory mappings requiring `msync` or `flush` before drop.
3. **Large Heap Allocations**: No heap allocations larger than 1MB are explicitly instantiated in the reviewed crate. All variables utilize fixed-size arrays within the stack-allocated `#[repr(C)]` structure, with dynamic parameters formatted on demand.

---

# Performance & Allocation Audit

## Hot-Path `format!()` Analysis
The crate invokes `format!()` in latency-sensitive paths:
* **Arrival Notarization** (`crates/op-identity/src/anna_scribe.rs:75-81`): Re-formats WireGuard public keys and mutation indices on every new packet arrival.
* **Xray Configuration Generation** (`crates/op-identity/src/schema_bridge.rs:324-411`): A massive, multi-line `format!()` block is used to dynamically construct the Xray JSON configuration. This allocation-heavy string formatting executes on every tracked handshake event.

## Dynamic Collections & Pre-allocation Failures
* **Peers Vector** (`crates/op-identity/src/wireguard.rs:82`): `get_connected_peers` instantiates an un-allocated vector via `Vec::new()` and appends parsed peers within a loop over stdout lines without invoking `Vec::with_capacity()`.
* **Allowed IPs parsing** (`crates/op-identity/src/wireguard.rs:120`): Collects split string segments into a vector without capacity pre-allocation, forcing frequent heap re-allocations as segments are evaluated.

---

# Security & Quality Findings

### [CRITICAL] Undefined Behavior via Unpadded `simd-json` Parsing
* **Location**: `crates/op-identity/src/token.rs:83`
* **Impact**: Directly exploitable memory corruption, local information disclosure, or segmentation fault.
* **Description**: `simd_json::from_str` is invoked inside an `unsafe` block on a standard mutable string retrieved from the system keyring:
  ```rust
  let mut json = entry.get_password()?;
  Ok(unsafe { simd_json::from_str(&mut json) }?)
  ```
  The `simd-json` parser operates on SIMD vector registers (e.g., AVX2, NEON) and strictly requires that input buffers have at least `simd_json::SIMDJSON_PADDING` (typically 32 bytes) of allocated padding memory beyond the end of the string. A standard `String` retrieved from the `keyring` crate is not padded. Passing a non-padded string to `simd_json` results in out-of-bounds reads and memory safety violations.

---

### [HIGH] Local Privilege Escalation via `/dev/shm` Symlink Hijacking
* **Location**: `crates/op-identity/src/anna_scribe.rs:101`, `crates/op-identity/src/schema_bridge.rs:242`
* **Impact**: Local Privilege Escalation (LPE) or arbitrary file overwrite.
* **Description**: The system relies on hardcoded paths in the world-writable `/dev/shm` directory, specifically:
  * `/dev/shm/snowball_session.log`
  * `/dev/shm/xray-ghostbridge.json`
  * `/dev/shm/plugin_schema.dat`

  Because any unprivileged local user can write to `/dev/shm`, an attacker can create a symlink at `/dev/shm/snowball_session.log` pointing to a sensitive system file (e.g., `/etc/shadow` or `/etc/passwd`). When the privileged identity agent runs `append_snowball` or `write_xray_config`, it follows the symlink and overwrites or appends user-controlled data to the target file.

---

### [HIGH] Local Denial of Service via `SIGBUS` in Zero-Copy Memory Maps
* **Location**: `crates/op-identity/src/schema_bridge.rs:269`, `crates/op-identity/src/anna_scribe.rs:62`
* **Impact**: Process crash (Denial of Service).
* **Description**: Both `read_sled` and `notarize_arrival` map `/dev/shm/plugin_schema.dat` using `memmap2` without validating that the file size on disk is equal to or greater than the size of the target struct (`IdentitySled::SIZE` or `std::mem::size_of::<PluginSchema>()`). 
  
  If the file is truncated, empty, or modified to be smaller than the expected layout, accessing the fields on the cast pointer (e.g., `let is_valid = unsafe { (*schema_ptr).is_valid };`) will attempt to read memory pages that do not correspond to physical file backing, immediately triggering a kernel `SIGBUS` signal and terminating the application.

---

### [MEDIUM] Broken Cryptographic Continuity via MD5 Hashes
* **Location**: `crates/op-identity/src/anna_scribe.rs:76-81`
* **Impact**: Weakened collision resistance for state notarization.
* **Description**: `AnnaScribe::notarize_arrival` uses MD5 (`md5::compute`) to generate the initial hashed footprint and session trace ID:
  ```rust
  let payload = format!("{}:{}", wg_pubkey, current_mutation);
  let genesis_hash = format!("{:x}", md5::compute(payload.as_bytes()));
  ```
  Although the code comments claim this is to maintain "cryptographic continuity" with Btrfs event chains, MD5 is cryptographically broken and prone to collision attacks. An attacker could potentially pre-compute collisions to hijack or spoof trace IDs.

---

### [MEDIUM] Subtraction Overflow Panic on Clock Skew
* **Location**: `crates/op-identity/src/wireguard.rs:118`
* **Impact**: Process panic under debug builds, incorrect validation under release builds.
* **Description**: Handshake timestamps are compared to the current system time:
  ```rust
  let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)...;
  if timestamp > 0 && now - timestamp < 180 {
  ```
  If the host's system clock is adjusted backward, or if a peer handshake timestamp is registered slightly in the future due to network latency and clock skew, `now` will be less than `timestamp`. The subtraction `now - timestamp` will overflow, triggering a runtime panic in debug mode or wrapping to a very large positive number in release mode.
* **Remediation**: Use safe arithmetic operators, such as `now.saturating_sub(timestamp)`.

---

### [MEDIUM] Schema-As-Code Compliance Violation
* **Location**: `crates/op-identity/src/schema_bridge.rs:163-195`, `crates/op-identity/src/schema_bridge.rs:115`
* **Impact**: Brittle data contracts, potential memory misalignment, and structural drift.
* **Description**: The codebase violates the schema-as-code discipline. The data contracts for system compliance (`IdentitySled` and `PluginSchema`) are represented as ad-hoc C-style structs packed with hardcoded byte arrays (`[u8; 128]`, `[u8; 64]`) instead of being defined in a formal, versioned Protocol Buffer schema (`.proto`) or structured OSCAL document. 

  Furthermore, the `SubidTaxonomy` is parsed on the fly using ad-hoc string splitting (`splitn(5, '.')`) and custom segment validations (lines 115-144). If the memory layout changes across updates, or if string boundaries are parsed incorrectly, memory layout mismatch or corrupt compliance parameters can occur.