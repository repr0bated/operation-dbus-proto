# Public API Surface & Dead Code

### Public API Surface Enumeration
Below is the comprehensive inventory of all `pub` items (modules, re-exports, structs, enums, functions, constants, and fields) declared in the compiled modules of `op-identity`.

* **Modules (`pub mod`):** 7 totals
  * `anna_scribe` (`crates/op-identity/src/lib.rs:4`)
  * `gcloud_auth` (`crates/op-identity/src/lib.rs:5`)
  * `registration` (`crates/op-identity/src/lib.rs:6`)
  * `schema_bridge` (`crates/op-identity/src/lib.rs:7`)
  * `session` (`crates/op-identity/src/lib.rs:8`)
  * `token` (`crates/op-identity/src/lib.rs:9`)
  * `wireguard` (`crates/op-identity/src/lib.rs:10`)
* **Re-exports (`pub use`):** 14 totals
  * `AnnaScribe`, `PluginSchema`, `SessionLedger` (`crates/op-identity/src/lib.rs:12`)
  * `GCloudAuth` (`crates/op-identity/src/lib.rs:13`)
  * `generate_magic_link_token`, `generate_wireguard_keypair`, `WireGuardKeyPair` (`crates/op-identity/src/lib.rs:14`)
  * `read_sled`, `run_schema_shuttle`, `socket_entries_from_env`, `watch_wireguard_handshakes`, `write_sled`, `write_sled_from_wg`, `write_sled_full`, `IdentitySled`, `SHM_SLED_PATH`, `SHM_XRAY_CONFIG`, `SocketEntry`, `SubidCategory`, `SubidTaxonomy` (`crates/op-identity/src/lib.rs:15`)
  * `Session`, `SessionManager` (`crates/op-identity/src/lib.rs:21`)
  * `CachedToken`, `TokenManager` (`crates/op-identity/src/lib.rs:22`)
  * `PeerInfo`, `WireGuardIdentity` (`crates/op-identity/src/lib.rs:23`)
* **Structs (`pub struct`):** 10 totals
  * `PluginSchema` (`crates/op-identity/src/anna_scribe.rs:18`)
  * `SessionLedger` (`crates/op-identity/src/anna_scribe.rs:28`)
  * `AnnaScribe` (`crates/op-identity/src/anna_scribe.rs:36`)
  * `GCloudAuth` (`crates/op-identity/src/gcloud_auth.rs:26`)
  * `WireGuardKeyPair` (`crates/op-identity/src/registration.rs:11`)
  * `Session` (`crates/op-identity/src/session.rs:19`)
  * `UserMapping` (`crates/op-identity/src/session.rs:31`)
  * `SessionManager` (`crates/op-identity/src/session.rs:40`)
  * `CachedToken` (`crates/op-identity/src/token.rs:11`)
  * `TokenManager` (`crates/op-identity/src/token.rs:17`)
  * `WireGuardIdentity` (`crates/op-identity/src/wireguard.rs:7`)
  * `PeerInfo` (`crates/op-identity/src/wireguard.rs:211`)
  * `SubidTaxonomy` (`crates/op-identity/src/schema_bridge.rs:55`)
  * `IdentitySled` (`crates/op-identity/src/schema_bridge.rs:139`)
  * `SocketEntry` (`crates/op-identity/src/schema_bridge.rs:263`)
* **Enums (`pub enum`):** 1 total
  * `SubidCategory` (`crates/op-identity/src/schema_bridge.rs:16`)
* **Constants & Statics (`pub const` / `pub static`):** 3 totals
  * `OAUTH_SCOPES` (`crates/op-identity/src/gcloud_auth.rs:16`)
  * `SHM_SLED_PATH` (`crates/op-identity/src/schema_bridge.rs:26`)
  * `SHM_XRAY_CONFIG` (`crates/op-identity/src/schema_bridge.rs:27`)
* **Functions (`pub fn` outside of impl blocks):** 9 totals
  * `generate_wireguard_keypair` (`crates/op-identity/src/registration.rs:18`)
  * `generate_magic_link_token` (`crates/op-identity/src/registration.rs:29`)
  * `write_sled` (`crates/op-identity/src/schema_bridge.rs:237`)
  * `read_sled` (`crates/op-identity/src/schema_bridge.rs:252`)
  * `socket_entries_from_env` (`crates/op-identity/src/schema_bridge.rs:277`)
  * `write_sled_from_wg` (`crates/op-identity/src/schema_bridge.rs:412`)
  * `write_sled_full` (`crates/op-identity/src/schema_bridge.rs:453`)
  * `watch_wireguard_handshakes` (`crates/op-identity/src/schema_bridge.rs:491`)
  * `run_schema_shuttle` (`crates/op-identity/src/schema_bridge.rs:547`)

### Top 10 Most Impactful Public APIs
| Item | Type | file:line | Impact |
| :--- | :--- | :--- | :--- |
| `SessionManager` | Struct | `crates/op-identity/src/session.rs:40` | High; coordinates active peer VPN sessions and authentication states. |
| `run_schema_shuttle` | Function | `crates/op-identity/src/schema_bridge.rs:547` | Critical; spawns and bootstraps the network data plane routing. |
| `AnnaScribe` | Struct | `crates/op-identity/src/anna_scribe.rs:36` | Critical; top-level gatekeeper that notarizes initial peer connection arrivals. |
| `IdentitySled` | Struct | `crates/op-identity/src/schema_bridge.rs:139` | Critical; the 640-byte zero-copy shared memory protocol data layout. |
| `GCloudAuth` | Struct | `crates/op-identity/src/gcloud_auth.rs:26` | High; credential manager interfacing with GCP OAuth endpoints. |
| `WireGuardIdentity` | Struct | `crates/op-identity/src/wireguard.rs:7` | High; manages local interface identity detection and peer handshakes. |
| `write_sled_full` | Function | `crates/op-identity/src/schema_bridge.rs:453` | High; primary state mutations writer called directly by the SchemaEngine. |
| `generate_wireguard_keypair` | Function | `crates/op-identity/src/registration.rs:18` | Medium; cryptographic key generation for VPN nodes. |
| `SubidTaxonomy` | Struct | `crates/op-identity/src/schema_bridge.rs:55` | Medium; parses compliance components and domain boundaries. |
| `PluginSchema` | Struct | `crates/op-identity/src/anna_scribe.rs:18` | High; absolute base shared memory structure for system node mapping. |

### Glob Re-exports
* **None found.** No `pub use *` statements are present. All re-exports in `crates/op-identity/src/lib.rs` are explicitly listed.

### Public Struct Fields that should be Private
* **`WireGuardKeyPair` Fields:** `private_key` and `public_key` (`crates/op-identity/src/registration.rs:12-13`) are completely public, permitting raw access to key material which should be protected and only readable through accessors.
* **`Session` Fields:** All fields (`crates/op-identity/src/session.rs:20-26`) such as `oauth_token` and `token_expires_at` are exposed publicly, meaning outer modules can bypass safety checks of `SessionManager` and mutate timestamps or overwrite tokens directly.
* **`UserMapping` Fields:** Fields `pubkey`, `user_email`, and `allowed_ip` (`crates/op-identity/src/session.rs:32-35`) are public and lack accessor encapsulation.

---

### Dead Code Analysis
No `#[allow(dead_code)]` annotations exist anywhere within the audited files. However, multiple structural dead code occurrences, including a completely dangling (uncompiled) module file, were uncovered.

#### Uncompiled Dangling Module File
* **`crates/op-identity/src/wg.rs`** is present in the codebase but has **no matching `mod wg;` or `pub mod wg;` declaration** in `lib.rs`. It contains duplicative code competing directly with `wireguard.rs`. Because it is completely unreferenced by the compiler, all of its structures and functions represent dead code.

#### Unused Structures & Functions Table

| Item | Type | file:line | Recommendation |
| :--- | :--- | :--- | :--- |
| `wg.rs` entire file contents | Modules / Code | `crates/op-identity/src/wg.rs:1` | **Remove.** The file is uncompiled, dangling, and duplicates `wireguard.rs`. |
| `TokenManager` | Struct | `crates/op-identity/src/token.rs:17` | **Refactor or Remove.** Compiled but never instantiated or referenced anywhere else in the crate. |
| `CachedToken` | Struct | `crates/op-identity/src/token.rs:11` | **Remove.** Used only by `TokenManager` which itself is dead code. |
| `IdentitySled::subid_taxonomy` | Method | `crates/op-identity/src/schema_bridge.rs:222` | **Remove or Test.** Never called in production flows. |
| `IdentitySled::subid_category_str` | Method | `crates/op-identity/src/schema_bridge.rs:204` | **Remove.** Declared but never used. |
| `IdentitySled::subid_component_type_str` | Method | `crates/op-identity/src/schema_bridge.rs:205` | **Remove.** Declared but never used. |
| `IdentitySled::subid_subject_str` | Method | `crates/op-identity/src/schema_bridge.rs:206` | **Remove.** Declared but never used. |
| `IdentitySled::subid_verb_str` | Method | `crates/op-identity/src/schema_bridge.rs:207` | **Remove.** Declared but never used. |
| `IdentitySled::subid_facet_str` | Method | `crates/op-identity/src/schema_bridge.rs:208` | **Remove.** Declared but never used. |
| `IdentitySled::control_source_str` | Method | `crates/op-identity/src/schema_bridge.rs:209` | **Remove.** Declared but never used. |
| `IdentitySled::control_refs_str` | Method | `crates/op-identity/src/schema_bridge.rs:210` | **Remove.** Declared but never used. |
| `IdentitySled::statement_refs_str` | Method | `crates/op-identity/src/schema_bridge.rs:211` | **Remove.** Declared but never used. |
| `generate_magic_link_token` | Function | `crates/op-identity/src/registration.rs:29` | **Expose or Remove.** Exported but never called by any crate logic (only executed in tests). |

---

# Schema-as-Code Compliance Review

The codebase purports to implement a strict identity-and-state arbitration framework. However, it severely violates the "Schema-as-Code" discipline in several areas:

1. **Ad-Hoc String Interpolation for Critical Configuration Contracts:**
   In `crates/op-identity/src/schema_bridge.rs:290-410`, the VLESS/Xray configuration file layout is generated by interpolating variables into a raw, ad-hoc JSON format string. This entirely bypasses formalized data schemas (such as OSCAL or Protocol Buffers). Any syntax error or unexpected character in variables like keys or UUIDs will generate corrupted JSON files, destabilizing the network engine.
2. **Raw Memory Casting over Schema Versioning:**
   The `PluginSchema` (`crates/op-identity/src/anna_scribe.rs:18`) and `IdentitySled` (`crates/op-identity/src/schema_bridge.rs:139`) are expressed as ad-hoc C-compatible structs mapped directly from raw shared memory (`/dev/shm`). This approach relies on implicit memory offsets and compiler-specific padding rather than formalized, versioned Protobuf or OSCAL schemas. It lacks version negotiation, making binary data migrations extremely fragile.

---

# Security & Quality Findings

### [CRITICAL] Memory Safety Violation: Unbound Lifetime in Memory-Mapped Sled Reader
* **Reference:** `crates/op-identity/src/schema_bridge.rs:252`
* **Vulnerability Type:** Use-After-Free (UAF) / Page Fault
* **Exploitable:** Yes.

#### Description
The function `read_sled` returns a tuple containing an unbound raw pointer and an owning memory-mapped file handle:
```rust
pub fn read_sled() -> std::io::Result<(*const IdentitySled, memmap2::Mmap)> {
    let file = File::open(SHM_SLED_PATH)?;
    let mmap = unsafe { MmapOptions::new().len(IdentitySled::SIZE).map(&file)? };
    let ptr = mmap.as_ptr() as *const IdentitySled;
    Ok((ptr, mmap))
}
```
The raw pointer `*const IdentitySled` does not carry any lifetime association linking it to the second member of the tuple (`memmap2::Mmap`). Consequently, Rust's borrow checker cannot enforce that the `Mmap` remains allocated while the pointer is in use. 

If a caller drops the `Mmap` guard while retaining the raw pointer, the memory mapping is instantly unmapped. Any subsequent dereference of the raw pointer will trigger an immediate Use-After-Free or a kernel Page Fault (SIGSEGV/SIGBUS).

#### Remediation
Redesign `read_sled` to return a safe wrapper struct that encapsulates the `Mmap` and exposes accessors returning references with safe lifetimes tied directly to the lifetime of the wrapper:
```rust
pub struct SledGuard {
    _mmap: memmap2::Mmap,
}

impl SledGuard {
    pub fn as_sled(&self) -> &IdentitySled {
        unsafe { &*(self._mmap.as_ptr() as *const IdentitySled) }
    }
}

pub fn read_sled() -> std::io::Result<SledGuard> {
    let file = File::open(SHM_SLED_PATH)?;
    let mmap = unsafe { MmapOptions::new().len(IdentitySled::SIZE).map(&file)? };
    Ok(SledGuard { _mmap: mmap })
}
```

---

### [CRITICAL] Undefined Behavior: Safe Deref of Unsafe Byte-to-Bool Mappings
* **Reference:** `crates/op-identity/src/anna_scribe.rs:64`
* **Vulnerability Type:** Undefined Behavior (UB)
* **Exploitable:** Yes.

#### Description
The `PluginSchema` struct contains a boolean field `is_valid`:
```rust
#[repr(C)]
pub struct PluginSchema {
    pub wireguard_pubkey: [u8; 32],
    pub mutation_index: u64,
    pub is_valid: bool,
    pub hashed_footprint: [u8; 32],
}
```
In `AnnaScribe::notarize_arrival`, the shared-memory byte array is cast directly to a raw pointer and dereferenced:
```rust
let schema_ptr = mmap.as_ptr() as *const PluginSchema;
let is_valid = unsafe { (*schema_ptr).is_valid };
```
In Rust, a `bool` value must strictly be represented in memory as the byte value `0x00` (`false`) or `0x01` (`true`). If the file `/dev/shm/plugin_schema.dat` contains any other value (such as `0xFF`, or garbage left by uninitialized padding/truncation), dereferencing `is_valid` triggers immediate Undefined Behavior. LLVM optimizations will assume the boolean is either `0` or `1`, which can lead to silent miscompilations, arbitrary branching, or memory access corruption.

#### Remediation
Do not map raw memory-mapped bytes directly to Rust types containing strict invariants like `bool` or `enums`. Instead, read the field as a `u8` and validate it manually:
```rust
#[repr(C)]
pub struct PluginSchema {
    pub wireguard_pubkey: [u8; 32],
    pub mutation_index: u64,
    pub is_valid: u8, // Use u8 instead of bool
    pub hashed_footprint: [u8; 32],
}

// Inside notarize_arrival:
let is_valid = unsafe { (*schema_ptr).is_valid == 1 };
```

---

### [CRITICAL] Memory Safety: Unpadded Keyring Buffer with Unsafe SIMD JSON Parser
* **Reference:** `crates/op-identity/src/token.rs:91`
* **Vulnerability Type:** Out-of-Bounds Memory Overread / Segmentation Fault
* **Exploitable:** Yes.

#### Description
In `read_from_keyring`, a string is retrieved from the system keyring and parsed via the unsafe `simd_json` API:
```rust
async fn read_from_keyring(&self) -> Result<CachedToken> {
    let entry = keyring::Entry::new("mcp-identity", "gcloud-token")?;
    let mut json = entry.get_password()?;
    Ok(unsafe { simd_json::from_str(&mut json) }?)
}
```
The `simd_json::from_str` parser is highly optimized and requires that the mutable string buffer passed to it contains trailing padding bytes of size `simd_json::SIMDJSON_PADDING` (typically 32 or 64 bytes depending on target SIMD architecture). This padding allows the parser's SIMD instructions to safely execute vector reads past the logical end of the string without faulting.

Because `entry.get_password()` returns a standard `std::string::String` without padding, passing a mutable reference directly to `simd_json::from_str` leads to an out-of-bounds memory overread. If the string terminates near a page boundary, this triggers an immediate segmentation fault or reads garbage memory.

#### Remediation
If high performance is not required for keyring reads, use safe standard JSON deserializers (such as `serde_json::from_str`). If `simd-json` must be used, copy the string into a padded buffer using `simd_json::to_padded_bin`:
```rust
async fn read_from_keyring(&self) -> Result<CachedToken> {
    let entry = keyring::Entry::new("mcp-identity", "gcloud-token")?;
    let json_str = entry.get_password()?;
    Ok(serde_json::from_str(&json_str)?) // Safe parsing
}
```

---

### [HIGH] Security Bypass: Authentication Spoofing via Hostname Fallback
* **Reference:** `crates/op-identity/src/wireguard.rs:49`
* **Vulnerability Type:** Identity Impersonation / Authentication Bypass
* **Exploitable:** Yes.

#### Description
The cryptographic security model of `op-identity` relies on the WireGuard handshake acting as the absolute secure login. However, if the interface command fails or has no key, the provider falls back to using the local system hostname:
```rust
// Fallback: generate a deterministic ID from hostname
let hostname = hostname::get()
    .map(|h| h.to_string_lossy().to_string())
    .unwrap_or_else(|_| "unknown".to_string());

warn!("Could not get WireGuard pubkey, using hostname-based ID");
Ok(format!("local:{}", hostname))
```
This fallback invalidates the cryptographic boundary of the architecture. If a local attacker can influence the hostname (for example, in containerized or DHCP environments), they can easily spoof the identity of high-privileged nodes, bypassing WireGuard authorization.

#### Remediation
Remove any non-cryptographic identity fallbacks. If the WireGuard interface cannot provide a valid cryptographic public key, the authentication routine must fail immediately:
```rust
Ok(out) => {
    let stderr = String::from_utf8_lossy(&out.stderr);
    debug!("wg show failed: {}", stderr);
    anyhow::bail!("WireGuard command failed. Fallbacks are disabled.");
}
```

---

### [HIGH] Path Hijacking: Local Privilege Escalation via Insecure Shell PATH Resolution
* **Reference:** `crates/op-identity/src/schema_bridge.rs:445` (also `crates/op-identity/src/wireguard.rs:43`, `crates/op-identity/src/gcloud_auth.rs:194`)
* **Vulnerability Type:** Execution with Untrusted Path / Privilege Escalation
* **Exploitable:** Yes.

#### Description
The application spawns multiple helper binaries (`incus`, `wg`, `gcloud`, `xray`) using relative names without specifying their absolute filesystem paths. For example, in `schema_bridge.rs`:
```rust
let Ok(out) = Command::new("incus")
    .args(["exec", "wg-xray", "--", "wg", "show", &iface, "latest-handshakes"])
    ...
```
Because these are relative command lookups, the system searches directories specified in the caller's environment `PATH` variable to find the binaries. If the control plane runs with root or elevated permissions, a local user who can write to any path prefix inside the environment's `PATH` can place a malicious executable named `incus`, `wg`, or `xray` there. When the control plane executes the command, it will launch the attacker's binary under high privileges, resulting in local privilege escalation.

#### Remediation
Always resolve commands using secure, absolute paths (e.g., `/usr/bin/wg`, `/usr/bin/incus`), or strictly sanitize and override the `PATH` environment variable of the executed commands.

---

### [HIGH] Cryptographic Exposure: Hardcoded Tunnels and Reality Fallback Credentials
* **Reference:** `crates/op-identity/src/schema_bridge.rs:498-500` (also `crates/op-identity/src/schema_bridge.rs:528-533`)
* **Vulnerability Type:** Cryptographic Exposure / Hardcoded Secrets
* **Exploitable:** Yes.

#### Description
If environment variables for the Xray proxy tunnel are not configured, the bridge silently falls back to hardcoded cryptographic credentials:
```rust
let uuid    = env::var("XRAY_UUID").unwrap_or_else(|_| "40813c05-4a7c-4d5b-b027-33912551287f".to_string());
let privkey = env::var("XRAY_PRIVATE_KEY").unwrap_or_else(|_| "-MULA7gIbk_58CKa4TNHovpYNt192NUkPlQF7f3caWo".to_string());
let short   = env::var("XRAY_SHORT_ID").unwrap_or_else(|_| "2a32c53278372687".to_string());
```
These fallback secrets are identical across all deployments of the software. Anyone with read access to the source code can easily intercept, decrypt, or connect to target client nodes.

#### Remediation
Do not provide static fallbacks for cryptographic credentials. Fail the process initialization immediately if secure parameters are not explicitly passed via verified configuration channels:
```rust
let xray_uuid = env::var("XRAY_UUID")
    .map_err(|_| anyhow::anyhow!("XRAY_UUID environment variable must be configured"))?;
```

---

### [HIGH] Security Quality: Uncontrolled World-Writable Shared Memory Paths
* **Reference:** `crates/op-identity/src/schema_bridge.rs:26-27`
* **Vulnerability Type:** Race Condition / Permission Denial of Service (DoS)
* **Exploitable:** Yes.

#### Description
The application reads and writes directly to predictable locations in `/dev/shm` (shared memory tmpfs):
```rust
pub const SHM_SLED_PATH: &str = "/dev/shm/plugin_schema.dat";
pub const SHM_XRAY_CONFIG: &str = "/dev/shm/xray-ghostbridge.json";
```
Because `/dev/shm` is typically a world-writable directory, any local unprivileged process can pre-create these absolute paths or place a symlink at `/dev/shm/plugin_schema.dat` pointing to a critical system file. If the high-privileged control plane writes to that path, it can overwrite arbitrary system configuration files or get blocked from writing, leading to local privilege escalation or complete Denial of Service.

#### Remediation
Store shared files inside an explicitly secure subdirectory owned exclusively by the runtime user of the control plane (e.g., `/run/op-identity/`), with strict directory permissions (`0700`).

---

### [MEDIUM] Cryptographic Continuity: Use of Broken MD5 Hash in Accountability Loop
* **Reference:** `crates/op-identity/src/anna_scribe.rs:82`
* **Vulnerability Type:** Cryptographic Weakness
* **Exploitable:** No.

#### Description
`AnnaScribe::notarize_arrival` utilizes the broken MD5 hashing algorithm to generate transaction hashes and trace IDs:
```rust
let payload = format!("{}:{}", wg_pubkey, current_mutation);
let genesis_hash = format!("{:x}", md5::compute(payload.as_bytes()));
```
While MD5 is specified here for compatibility with the Btrfs `EventChain` system, it is highly prone to collision attacks. Tying cryptographically secure trace IDs and session genesis events to MD5 means an attacker with sufficient computational capabilities can craft colliding inputs to tamper with historical session logs.

#### Remediation
Migrate both the Btrfs `EventChain` and the `AnnaScribe` notarization algorithms to a cryptographically secure hash standard such as SHA-256 or BLAKE3.

---
## ⚠ Citation Warnings
- `crates/op-identity/src/wireguard.rs:211`: file has 165 lines
