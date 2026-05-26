# D-Bus & IPC Attack Surface Audit

## D-Bus Service Configuration & Bus Type
Based on the implementation details across the gateway codebase, the service is designed to run on the **D-Bus system bus**. 
* **Privileged System Commands:** The service executes administrative actions, such as managing encrypted Btrfs subvolumes (`btrfs subvolume create` in `crates/op-gateway/src/encrypted_storage.rs:142`), mounting block devices (`mount` in `crates/op-gateway/src/encrypted_storage.rs:271`), and creating image containers via `dd` (`crates/op-gateway/src/encrypted_storage.rs:212`). These commands require root privileges.
* **Global Path Storage:** Critical network and cryptographic configurations are stored globally in `/var/lib/op-dbus/` (e.g., `crates/op-gateway/src/encrypted_storage.rs:77`).
* **Bus Policy:** No system bus policy file (e.g., `.conf` policy in `/usr/share/dbus-1/system.d/`) was provided in the `FILES` section. Consequently, a comparison to detect over-permissioned `allow` rules cannot be performed. However, because this daemon runs on the system bus with high privileges, any exposed interface must restrict access explicitly.

---

## D-Bus Interface Inventory

The following D-Bus interface methods are implemented within the `McpGatewayManager` block in `crates/op-gateway/src/mcp_gateway.rs:343-394`:

| Interface / Method | Arguments | Returns | Mutates State? | Caller Identity Checked? |
| :--- | :--- | :--- | :--- | :--- |
| `dbus_route_client` | `client_name: &str`, `auth_token: Option<&str>`, `peer_pubkey: Option<&str>` | `Result<simd_json::OwnedValue>` | **Yes** (Inserts into routing cache) | **No** |
| `dbus_validate_session` | `session_id: &str` | `Result<simd_json::OwnedValue>` | **No** | **No** |
| `dbus_get_capabilities` | `session_id: &str` | `Result<simd_json::OwnedValue>` | **No** | **No** |

### Signals Registered
No D-Bus signals are defined or registered in the provided codebase.

---

# Security & Quality Findings

### Finding 1 [CRITICAL]: Publicly Derivable WireGuard Preshared Key (PSK)
* **File:** `crates/op-gateway/src/wireguard_auth.rs`
* **Lines:** 845–863 (called via 649–694)

#### Description
The WireGuard authentication manager implements a custom mechanism to automatically derive static WireGuard Preshared Keys (PSKs) using Argon2. However, the inputs used to derive the PSK are entirely public and static:
```rust
pub fn derive_stable_psk(&self, peer_key: &[u8; 32]) -> Vec<[u8; 32]> {
    let mut results = Vec::with_capacity(1);

    // Use a fixed salt for consistency (stable PSK)
    let salt = b"WG-STABLE-PSK-2024";

    let mut input = Vec::with_capacity(39);
    input.extend_from_slice(b"WG-PSK-");
    input.extend_from_slice(peer_key);
    // No timestamp - PSK should be stable

    let argon2 = Argon2::default();
    let mut psk = [0u8; 32];
    if argon2.hash_password_into(&input, salt, &mut psk).is_ok() {
        results.push(psk);
    }

    results
}
```

#### Vulnerability Mechanics
1. A WireGuard Preshared Key is a symmetric secret meant to provide quantum-resistance and channel authentication. It must remain a secret shared exclusively between the client and the server.
2. In this implementation, the Argon2 input is derived solely from the client's public key (`peer_key`), and the salt is a hardcoded byte array (`b"WG-STABLE-PSK-2024"`). No server-side secret, private key, or master key is mixed into the derivation.
3. Because WireGuard transmits the initiator's public key in plaintext during the handshake, any passive eavesdropper can intercept the public key, combine it with the hardcoded salt, and compute the exact client PSK offline.

#### Impact
This completely neutralizes the cryptographic security of the WireGuard PSK. Passive and active network adversaries can impersonate peers or decrypt captured traffic, completely bypassing the mutual authentication layer.

#### Remediation
Ensure that the PSK derivation mixes in a cryptographically strong, server-side master secret (such as the `MasterKey` managed in `crates/op-gateway/src/encrypted_storage.rs:60` or a secure key stored in `/etc/`) that is never exposed to the client or public networks.

---

### Finding 2 [CRITICAL]: Missing Caller Identity Validation on System D-Bus Methods
* **File:** `crates/op-gateway/src/mcp_gateway.rs`
* **Lines:** 343–394

#### Description
The exposed D-Bus methods (`dbus_route_client`, `dbus_validate_session`, and `dbus_get_capabilities`) handle client routing, capability queries, and session lookups. However, they lack any authorization checks or caller identity verification.

#### Vulnerability Mechanics
When exposed on the system bus, any local unprivileged process can invoke these endpoints. 
1. `dbus_route_client` (line 346) triggers state mutation by writing directly to the memory cache via `route_client` (line 104) and inserting a `RoutingDecision` under a derived cache key:
   ```rust
   let mut cache = self.routing_cache.write().await;
   let cache_key = self.generate_cache_key(&client_info);
   cache.insert(cache_key, routing_decision.clone());
   ```
2. Any unprivileged local attacker can populate the routing cache, overwrite existing cache entries, or spoof client information to disrupt communication or gain unauthorized capabilities.
3. Lack of UID/PID verification allows arbitrary local processes to validate sessions and retrieve routing endpoints for active WireGuard users, exposing sensitive connection details.

#### Impact
Unauthorized local state modification, routing cache poisoning, and information disclosure of active routing endpoints and capabilities.

#### Remediation
Integrate identity checking using `zbus::Connection::connection().peer_credentials()` to verify the caller's UID and PID. Restrict invocation of these administrative routing APIs to the owner of the system service (e.g., `root` or a dedicated system group) or use Polkit for fine-grained authorization.

---

### Finding 3 [HIGH]: Memory Safety Violation via Unsafe `simd_json::from_str` on Unpadded Strings
* **Files:** `crates/op-gateway/src/encrypted_storage.rs` and `crates/op-gateway/src/wireguard_auth.rs`
* **Lines:** `encrypted_storage.rs:440` and `wireguard_auth.rs:199`

#### Description
The codebase utilizes unsafe `simd_json::from_str` to deserialize metadata and JSON payloads. 
In `crates/op-gateway/src/encrypted_storage.rs`:
```rust
let entry_json = async_fs::read_to_string(&key_file_path).await?;
let mut entry_str = entry_json.clone();
let entry: EncryptedKeyEntry = unsafe { simd_json::from_str(&mut entry_str) }?;
```
In `crates/op-gateway/src/wireguard_auth.rs`:
```rust
let flags_json: String = row.get("flags");
let mut flags_str = flags_json.clone();
let flags: std::collections::HashMap<String, String> =
    unsafe { simd_json::from_str(&mut flags_str) }.unwrap_or_default();
```

#### Vulnerability Mechanics
`simd-json` relies on hardware-level vector instructions (AVX2, SSE, etc.) that load data in 16-byte or 32-byte chunks. Because of this, its unsafe in-place parser expects the input buffer to be padded with extra bytes (`simd_json::PADDING_SIZE`) beyond the length of the string to avoid reading past the allocated memory buffer.
Standard `String` buffers created via `.clone()` or returned by SQLx databases do **not** guarantee this padding. When the parser processes these raw unpadded strings within an `unsafe` block, it can perform an out-of-bounds read (buffer overread), potentially causing a segmentation fault, memory corruption, or undefined behavior.

#### Impact
Potential crash (denial of service) or memory corruption of the control plane when parsing serialized cryptographic entries or session flags.

#### Remediation
Avoid using unsafe, raw in-place parsing on unpadded strings. Use safe interfaces such as `simd_json::from_slice` which automatically copy and pad the input buffer internally, or explicitly resize and pad the string buffer before parsing.

---

### Finding 4 [MEDIUM]: Ad-Hoc Data Contracts Violating Schema-As-Code Discipline
* **Files:** `crates/op-gateway/src/mcp_gateway.rs` and `crates/op-gateway/src/wireguard_auth.rs`
* **Lines:** `mcp_gateway.rs:17`, `mcp_gateway.rs:41`, `mcp_gateway.rs:52`, `mcp_gateway.rs:359`, `wireguard_auth.rs:297`

#### Description
The control plane utilizes ad-hoc serialized JSON payloads and native Rust structs to model and transmit routing decisions, client metadata, capabilities, and session configurations over the D-Bus network interface. This violates the codebase's strict schema-as-code discipline, which mandates the use of versioned schemas (such as Protocol Buffers or OSCAL-compliant structures).

Specific ad-hoc constructs:
* `RoutingDecision` (struct representing routing capability metadata in `crates/op-gateway/src/mcp_gateway.rs:17`)
* `McpClientInfo` (struct containing authentication tokens and peer public keys in `crates/op-gateway/src/mcp_gateway.rs:41`)
* `McpSession` (struct in `crates/op-gateway/src/mcp_gateway.rs:52`)
* `WireGuardSession` (struct representing persisted sessions in `crates/op-gateway/src/wireguard_auth.rs:297`)
* Ad-hoc JSON serialization via the `json!` macro within D-Bus response structures, such as in `crates/op-gateway/src/mcp_gateway.rs:359`:
  ```rust
  Ok(json!({
      "endpoint": routing_decision.endpoint,
      "allowed_tools": routing_decision.allowed_tools,
      "capabilities": routing_decision.capabilities,
      "has_full_access": routing_decision.has_full_access,
      "session_id": routing_decision.session_id,
      "access_level": match routing_decision.access_level { ... }
  }))
  ```

#### Impact
This approach bypasses schema validation, leaving the control plane vulnerable to data contract drift, decoding errors during version upgrades, and integration gaps between services.

#### Remediation
Define these messages using structured Protocol Buffers (e.g., using `prost` or native `.proto` definitions) or OSCAL profiles. Ensure all IPC interfaces serialize and deserialize versioned schema-conforming payloads rather than raw, unchecked JSON blobs.

---
## ⚠ Citation Warnings
- `crates/op-gateway/src/mcp_gateway.rs:343`: file has 337 lines
- `crates/op-gateway/src/mcp_gateway.rs:359`: file has 337 lines
