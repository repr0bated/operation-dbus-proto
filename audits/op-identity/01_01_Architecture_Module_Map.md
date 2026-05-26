# Production Security & Quality Audit: `op-identity`

---

## 1. Architecture & Module Map

### Overview
The `op-identity` crate acts as the native identity control plane for the system. It ties cryptographic WireGuard transport handshakes directly to active sessions and schema mutations, bypassing standard disk-bound database layers (such as SQLite) and Btrfs-backed mutations to preserve high-performance transport I/O entirely in memory (`tmpfs`/`shm`).

### Module Tree
The library module hierarchy is declared in `crates/op-identity/src/lib.rs` as follows:

```
crates/op-identity/src/lib.rs (Entry Point)
 ├── anna_scribe.rs (Identity notary arbitrator; memory-mapped PluginSchema)
 ├── gcloud_auth.rs (Google Cloud OAuth credential negotiation)
 ├── registration.rs (WireGuard keypair & token generators)
 ├── schema_bridge.rs (Identity Sled layout, Shuttle runner, Xray config generator)
 ├── session.rs (In-memory session manager utilizing DashMaps)
 ├── token.rs (OAuth system keyring integration)
 └── wireguard.rs (WireGuard interface detection and peer allowed-IP resolution)
```

### Entry Points
*   **Library Entry Point:** `crates/op-identity/src/lib.rs`
*   **Orphaned/Shadow File:** `crates/op-identity/src/wg.rs` is physically present in the source tree but is **not** declared via `mod wg;` in `lib.rs`, nor is it integrated into the crate's compilation unit.

### Architectural Notes
*   **Zero-Copy Design:** The architecture relies on mapping `#[repr(C)]` structs directly over memory-mapped files located in the virtual filesystem `/dev/shm/` (e.g., `plugin_schema.dat`).
*   **State Persistence:** Traditional relational storage is avoided. State maps are managed inside concurrent `DashMap` instances (`crates/op-identity/src/session.rs:41`) to prevent Btrfs write loops and high-write wear.

---

## 2. Executive Summary

This security and quality audit of the `op-identity` crate identified severe architectural and implementation vulnerabilities that compromise the system's memory safety, identity authenticity, and process reliability. 

### Key Findings
1.  **Memory Safety Violation (Critical):** The API for reading shared memory returns an unbound raw pointer alongside its owning memory-mapped allocation guard. This design easily induces **Use-After-Free (UAF)** conditions via compiler-driven immediate drops of anonymous variables.
2.  **Denial of Service via Memory Slicing (Critical):** Direct casting of raw mapped memory to structures occurs without validating that the mapped buffer size is equal to or greater than the target struct size. Truncated or empty files in `/dev/shm` trigger immediate `SIGBUS` crashes.
3.  **Authentication Bypass via Fallback (High):** When system tools fail to retrieve a peer's public key, the WireGuard identity engine silently falls back to a guessable string based on the local hostname, allowing spoofing of cryptographic identities.
4.  **JSON Injection (High):** Configuration files for the network gateway (Xray) are built using ad-hoc string formatting, presenting a high risk of config corruption and injection attacks.

---

## 3. Critical Findings

### Finding 1: Use-After-Free / Memory Corruption due to Dangling Pointer API Design
*   **File:** `crates/op-identity/src/schema_bridge.rs`
*   **Line(s):** 209–214
*   **Impact:** Memory Corruption / Exploitability of state tracking memory.

#### Technical Description
The function `read_sled` maps the shared memory space into the process address space and returns a raw pointer along with the owning `memmap2::Mmap` allocation guard:

```rust
pub fn read_sled() -> std::io::Result<(*const IdentitySled, memmap2::Mmap)> {
    let file = File::open(SHM_SLED_PATH)?;
    let mmap = unsafe { MmapOptions::new().len(IdentitySled::SIZE).map(&file)? };
    let ptr = mmap.as_ptr() as *const IdentitySled;
    Ok((ptr, mmap))
}
```

Because the raw pointer `*const IdentitySled` does not carry a lifetime bound to the `Mmap` wrapper, the compiler cannot enforce borrow checker rules on it. If a developer consumes this tuple using an anonymous placeholder or immediately drops the second element:

```rust
let (ptr, _) = read_sled()?; // Mmap is dropped immediately here
let sled = unsafe { &*ptr }; // Dereferencing unmapped memory -> Crash / UAF
```

The underlying memory segment is unmapped immediately. Any subsequent read of `ptr` constitutes a **Use-After-Free (UAF)** memory safety violation. This is highly exploitable to cause arbitrary segmentation faults or read garbage state if the virtual memory region is recycled.

#### Remediation
Redesign `read_sled` to return a safe, lifetime-bound RAII wrapper or a smart pointer that implements `Deref<Target = IdentitySled>` to guarantee that the mapping remains valid for the duration of the reference's lifetime:

```rust
pub struct MappedSled {
    _mmap: memmap2::Mmap,
}

impl std::ops::Deref for MappedSled {
    type Target = IdentitySled;
    fn deref(&self) -> &Self::Target {
        unsafe { &*(self._mmap.as_ptr() as *const IdentitySled) }
    }
}

pub fn read_sled_safe() -> std::io::Result<MappedSled> {
    let file = File::open(SHM_SLED_PATH)?;
    let mmap = unsafe { MmapOptions::new().len(IdentitySled::SIZE).map(&file)? };
    Ok(MappedSled { _mmap: mmap })
}
```

---

### Finding 2: Out-of-Bounds Memory Dereference & denial of Service (SIGBUS) in `AnnaScribe`
*   **File:** `crates/op-identity/src/anna_scribe.rs`
*   **Line(s):** 68–76
*   **Impact:** Process Denial of Service / Memory Leaks / Potential exploitation.

#### Technical Description
In `AnnaScribe::notarize_arrival`, the shared memory layout is loaded and directly dereferenced:

```rust
let file = File::open("/dev/shm/plugin_schema.dat")
    .map_err(|_| "A.N.N.A. Scribe: Missing Schema. Connection Rejected.".to_string())?;

// Zero-copy cast into the absolute base schema
let mmap = unsafe {
    MmapOptions::new()
        .map(&file)
        .map_err(|_| "Memory map failed".to_string())?
};
let schema_ptr = mmap.as_ptr() as *const PluginSchema;

let is_valid = unsafe { (*schema_ptr).is_valid };
```

There is **zero validation** of the size of `plugin_schema.dat` before casting and reading. If `/dev/shm/plugin_schema.dat` has been truncated, is empty (0 bytes), or is smaller than `std::mem::size_of::<PluginSchema>()` (which is approximately 80 bytes), dereferencing `schema_ptr` will perform an out-of-bounds read. On Unix systems, attempting to read mapped pages beyond the end of the file triggers a `SIGBUS` signal, instantly terminating the service. 

Since `/dev/shm` is typically a shared memory space accessible by multiple local processes, any local unprivileged process could truncate this file to cause a persistent Denial of Service of the identity control plane.

#### Remediation
Always query and validate the file size or the mapped buffer length before performing any unsafe pointer casts:

```rust
let metadata = file.metadata().map_err(|_| "Failed to query metadata")?;
if metadata.len() < std::mem::size_of::<PluginSchema>() as u64 {
    return Err("A.N.N.A. Scribe: Schema file is truncated or corrupted.".to_string());
}
```

---

## 4. High/Medium Risk Findings

### Finding 3: Cryptographic Bypass via Hostname Fallback in Identity Detection
*   **File:** `crates/op-identity/src/wireguard.rs`
*   **Line(s):** 48–52
*   **Impact:** High — Critical logic bypass of cryptographic identity.

#### Technical Description
The cryptographic identity provider is designed to use the local WireGuard public key as a secure, unforgeable login token. However, in `get_local_pubkey`, if the command execution fails or the interface is not configured, the system falls back to a deterministic, guessable string:

```rust
// Fallback: generate a deterministic ID from hostname
let hostname = hostname::get()
    .map(|h| h.to_string_lossy().to_string())
    .unwrap_or_else(|_| "unknown".to_string());

warn!("Could not get WireGuard pubkey, using hostname-based ID");
Ok(format!("local:{}", hostname))
```

This fallback identity string (`local:<hostname>`) is returned as a successful `Ok(...)` result. When `SessionManager::get_or_create_session_from_wireguard` calls this function (`crates/op-identity/src/session.rs:77`), it will accept the non-cryptographic identifier. 

If the host environment's WireGuard client binary is missing, temporarily unresponsive, or unprivileged, any local user or peer that can predict the hostname can impersonate the node's control plane session, completely bypassing the WireGuard public-key-as-identity security model.

#### Remediation
Remove non-cryptographic fallbacks in security-critical code paths. If cryptographic identity cannot be asserted, return a hard error rather than a soft fallback string:

```rust
match output {
    Ok(out) if out.status.success() => {
        let pubkey = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !pubkey.is_empty() {
            return Ok(pubkey);
        }
    }
    _ => {}
}
anyhow::bail!("WireGuard identity is unavailable: Cryptographic assertion failed.");
```

---

### Finding 4: Insecure String Formatting and JSON Injection in Gateway Configs
*   **File:** `crates/op-identity/src/schema_bridge.rs`
*   **Line(s):** 288–305 (and lines 242–261)
*   **Impact:** High — Corruption of routing plane / Arbitrary tunnel execution.

#### Technical Description
The function `write_xray_config_with_sockets` generates a complex routing configuration for the Xray proxy by concatenating and interpolating format strings:

```rust
    let config = format!(
        r#"{{
  "log": {{ "loglevel": "warning" }},
  ...
          "serviceName": "Ghostbridge.StateSync",
          "multiMode": true,
          "metadata": {{
            "X-Ghostbridge-Footprint": "{footprint}",
            "X-Ghostbridge-Trace-ID": "{trace_id}"
          }}
  ...
"#,
        profile = nextdns_profile,
        footprint = footprint,
        trace_id = trace_id,
        ...
```

No character escaping or structure verification is performed. If an environment variable or memory sled string (such as the `NEXTDNS_PROFILE_ID`, `XRAY_UUID`, or `trace_id`) contains a double-quote character (`"`), a trailing comma, or structural JSON characters, the resulting formatted string will be invalid JSON. 

Because this configuration is written to `/dev/shm/xray-ghostbridge.json` and executed with system privileges, structural manipulation of this JSON could allow malicious actors to rewrite proxy rules, inject arbitrary endpoints, or alter DNS endpoints.

#### Remediation
Serialize a strongly-typed Rust struct using `serde` and `serde_json` rather than performing ad-hoc string formatting:

```rust
#[derive(Serialize)]
struct XrayConfig {
    log: LogSettings,
    dns: DnsSettings,
    inbounds: Vec<Inbound>,
    outbounds: Vec<Outbound>,
    routing: RoutingSettings,
}
// Serialize config using serde_json::to_writer_pretty()
```

---

### Finding 5: Weak Hashing Algorithm (MD5) Used to Notarize Connections
*   **File:** `crates/op-identity/src/anna_scribe.rs`
*   **Line(s):** 86–87
*   **Impact:** Medium — Cryptographic session collision risk.

#### Technical Description
The `AnnaScribe` notary notarizes arrivals by mapping a WireGuard key and a mutation index into an identity string:

```rust
let payload = format!("{}:{}", wg_pubkey, current_mutation);
let genesis_hash = format!("{:x}", md5::compute(payload.as_bytes()));
```

MD5 is cryptographically broken and prone to collision attacks. If this hash is used to authorize access or to distinguish between active sessions, an attacker could potentially compute input variations that result in collision states. This could lead to duplicate trace IDs, allowing session hijacking or cross-session data leaks.

#### Remediation
Replace MD5 with a secure cryptographic hashing algorithm (e.g., SHA-256 or SHA-3) to assert session and identity footprints:

```rust
let mut hasher = Sha256::new();
hasher.update(payload.as_bytes());
let genesis_hash = hex::encode(hasher.finalize());
```

---

### Finding 6: Local File Disclosure / Cleartext Token Leak in `try_antigravity_token`
*   **File:** `crates/op-identity/src/gcloud_auth.rs`
*   **Line(s):** 115–124
*   **Impact:** Medium — Exposure of highly privileged GCP tokens.

#### Technical Description
The credential manager reads Google Cloud companion OAuth tokens from `.antigravity-server/*.token` on the local file system:

```rust
async fn try_antigravity_token(&self) -> Option<String> {
    let path = self.antigravity_token_path.as_ref()?;

    let content = std::fs::read_to_string(path).ok()?;
    let token = content.trim().to_string();
```

OAuth tokens of the `ya29.` family have direct administrative access to Google Cloud projects. The implementation reads these tokens as plain text from standard directories without verifying the file owner, POSIX file permissions (e.g., verifying that permissions are restricted to `0600`), or verifying whether the path is a symbolic link pointing to a sensitive resource.

#### Remediation
1. Verify that the file permissions of the token file are strictly owner-only (`0600`) before opening.
2. Ensure the path is not a dangling or malicious symbolic link.

---

## 5. Schema-as-Code & Quality Compliance

The project enforces a Schema-as-Code discipline. The codebase is audited below against these principles.

### Violations of Schema-as-Code
1.  **Ad-Hoc Struct Memory Casting:** The `PluginSchema` structure defined in `crates/op-identity/src/anna_scribe.rs:18` represents a direct, hand-rolled memory map definition mapping bytes from `plugin_schema.dat`. Instead of a compiled and version-controlled Protobuf schema or an OSCAL-derived model file, it uses an unversioned raw Rust C-repr struct. This lacks automatic code generation or cross-language schema enforcement.
2.  **Ad-Hoc String Parsers for Subid Taxonomy:** The subid taxonomy segments (e.g., `sch.network.plugin-schema.resolve@v1`) are defined and parsed through raw string manipulation and manual substring split loops (`crates/op-identity/src/schema_bridge.rs:88`):
    ```rust
    let mut parts = body.splitn(5, '.');
    ```
    This ad-hoc parsing should be driven by versioned declarative schemas generated directly from formal API or protocol definitions, rather than best-effort string parsing libraries within the identity library.
3.  **Monolithic Hand-Rolled Memory alignment:** The `IdentitySled` struct (`crates/op-identity/src/schema_bridge.rs:136`) mixes identity, OSCAL references, and network routing configurations inside a monolithic, raw array byte buffer structure. Any modification of fields requires manual calculations of offsets and padding arrays (e.g., `pub _pad: [u8; 7]` and `pub _pad2: [u8; 7]`), which is brittle and highly prone to schema-alignment drifts during updates.

### Quality Issues: Duplicate & Orphaned Code
*   **File:** `crates/op-identity/src/wg.rs`
*   **Audit Observation:** This entire file is an orphaned compilation unit. It implements duplicate logic for `get_peer_pubkey` and `get_local_pubkey` that mirrors `crates/op-identity/src/wireguard.rs`. Because `wg.rs` is not declared as a module in `lib.rs`, it represents dead, unmaintained code in the source repository that could confuse developers or lead to maintenance drifts.

---

## 6. Audit Conclusion & Action Plan

The following table summarizes the identified vulnerabilities categorized by severity:

| ID | Finding | Severity | File |
|---|---|---|---|
| **1** | Use-After-Free in `read_sled` | **Critical** | `schema_bridge.rs` |
| **2** | denial of Service / Out-of-bounds Read in `AnnaScribe` | **Critical** | `anna_scribe.rs` |
| **3** | Cryptographic Bypass via Hostname Fallback | **High** | `wireguard.rs` |
| **4** | JSON Injection in Xray Configuration Generation | **High** | `schema_bridge.rs` |
| **5** | Weak Hashing (MD5) for State Synchronization | **Medium** | `anna_scribe.rs` |
| **6** | Unchecked Local File Token Exposure | **Medium** | `gcloud_auth.rs` |

### Immediate Actions
1.  **Refactor Memory Mapping API:** Replace the tuple-return of `read_sled` with the safe `MappedSled` wrapper to prevent immediate drop patterns and dangling raw pointer usage.
2.  **Integrate File Size Validation:** Enforce strict byte-size comparisons before dereferencing file mapping pointers.
3.  **Abolish Hostname Identity Fallbacks:** Ensure that any failure to authenticate via cryptographic keys results in a connection rejection.
4.  **Replace String Formatting with Struct Serialization:** Convert the Xray configuration template into a structured Rust struct, serializing it safely via `serde_json`.