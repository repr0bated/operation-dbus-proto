### 1. Schema-as-Code Table

| Item | Type | file:line | Has .proto? | Gap |
| :--- | :--- | :--- | :--- | :--- |
| `DesiredState` | Struct | `crates/op-state/src/plugin.rs:18` | No | Defined as an ad-hoc Rust struct with Serde serialization. No counterpart `.proto` definition exists to enforce contract compliance across system components. |
| `EncryptedState` | Struct | `crates/op-state/src/crypto.rs:19` | No | Expresses state encryption metadata using an ad-hoc Rust structure with base64-encoded string fields rather than a structured schema. |
| `PluginMetadata` | Struct | `crates/op-state/src/plugin.rs:98` | No | Uses manual hash mapping and nested untyped `simd_json::OwnedValue` objects instead of a strongly-typed schema contract. |
| `StateManagerDBus::apply_openflow_state` | RPC | `crates/op-state/src/dbus_server.rs:92` | No | Consumes arbitrary payload inputs via a raw `state_json: String` parameter, bypassing static API contract validation. |
| `StateManagerDBus::apply_contract_mutation` | RPC | `crates/op-state/src/dbus_server.rs:118` | No | Processes mutation queries using a string-serialized, untyped JSON payload (`request_json: String`). |
| `PluginDbusHost::get_state` | RPC | `crates/op-state/src/dbus_server.rs:160` | No | Serializes local state directly into raw JSON strings for external transport over D-Bus without schema enforcement. |
| `UseCaseTemplate` | Struct | `crates/op-state/src/schema_validator.rs:12` | No | Hardcodes validation configurations and constraints inside structural fields, bypassing versioned validation definitions. |

---

### 2. OSCAL Coverage Table

| Control Area | Implemented at file:line | OSCAL Artifact | Gap |
| :--- | :--- | :--- | :--- |
| **System Configuration & Least Functionality (NIST SP 800-53 CM-6 / CM-7)** | `crates/op-state/src/authority.rs:12` | None | Disables critical system services (`NetworkManager` and `systemd-networkd`) directly via shell invocation with no security control mapping or capability documentation in an OSCAL component definition. |
| **Information at Rest Protection (NIST SP 800-53 SC-28)** | `crates/op-state/src/crypto.rs:43` | None | Implements key derivation (Argon2) and encryption (AES-256-GCM) logic for state storage without linking the cryptoprocessor components to an OSCAL SSP control. |
| **Access Enforcement & Authorization (NIST SP 800-53 AC-3)** | `crates/op-state/src/dbus_server.rs:92` | None | Exposes system-modifying D-Bus endpoints (`apply_openflow_state`, `apply_contract_mutation`) that run with elevated privileges (implied by service teardowns in `authority.rs`), but lacks caller identity validation or OSCAL control registration. |
| **Information Integrity (NIST SP 800-53 SI-7)** | `crates/op-state/src/plugin.rs:29` | None | Implements state hashing for verification using an outdated hashing algorithm (MD5), missing alignment with federal cryptoprocessor validation guidelines. |
| **Security Plan / Policy Enforcement (NIST SP 800-53 PL-2 / CM-3)** | `crates/op-state/src/schema_validator.rs:213` | None | Restricts system configurations using use case templates hardcoded in Rust instead of externalized machine-readable profiles. |

---

### 3. Recommendations

#### CRITICAL: Broken Password-Based Decryption (Salt Loss)
*   **Location:** `crates/op-state/src/crypto.rs:43`
*   **Impact:** Any state encrypted with a password-derived key cannot be decrypted on subsequent runs or in different execution contexts. When `StateEncryption::from_password` is called, it generates a fresh, random salt on the fly:
    ```rust
    let salt = SaltString::generate(&mut OsRng);
    ```
    However, the generated salt is never stored inside the `StateEncryption` context, nor is it returned to the caller. Furthermore, during encryption, the salt field in `EncryptedState` is hardcoded to `None` at `crates/op-state/src/crypto.rs:129`:
    ```rust
    Ok(EncryptedState {
        nonce: BASE64.encode(nonce_bytes),
        salt: None, // salt is permanently discarded here
        ciphertext: BASE64.encode(ciphertext),
        version: 1,
    })
    ```
    When a new process attempts to reconstruct the key using `from_password` with the same password, it generates a different random salt, causing key derivation to yield a completely different AES key, leading to absolute decryption failure.
*   **Remediation:** 
    1. Redesign `StateEncryption::from_password` to accept an optional salt: `pub fn from_password(password: &str, salt: Option<&SaltString>)`.
    2. Store the salt string within the `StateEncryption` struct.
    3. Modify `encrypt` to populate `EncryptedState::salt` with the base64-encoded representation of the KDF salt.
    4. Ensure that `decrypt` reconstructs the key using the salt stored inside the decrypted `EncryptedState` structure before running AES-GCM decryption.

#### CRITICAL: Missing D-Bus Authorization & Privilege Escalation Vector
*   **Location:** `crates/op-state/src/dbus_server.rs:92` and `crates/op-state/src/dbus_server.rs:118`
*   **Impact:** The `StateManagerDBus` methods `apply_openflow_state` and `apply_contract_mutation` mutate critical network layouts and execute modifications with administrative system authority. However, these methods contain no authorization checks. Because this service interacts with system services like `systemd-networkd` (as seen in `crates/op-state/src/authority.rs`), the daemon likely runs as root. Any unprivileged process with access to the system D-Bus can send messages to these endpoints to execute arbitrary network changes or write unvalidated state mutations, resulting in complete privilege escalation to root.
*   **Remediation:**
    1. Integrate credential checking into the D-Bus handlers. Retrieve the peer's connection credentials using the `zbus::Connection::peer_credentials` API.
    2. Validate that the peer's effective UID matches `0` (root) or a configured administrative group before executing any state changes.
    3. Use PolicyKit (polkit) integration to authenticate and authorize user-space actions requesting state mutations.

#### MAJOR: Use of Cryptographically Broken MD5 Hashing for State Validation
*   **Location:** `crates/op-state/src/plugin.rs:29`
*   **Impact:** The integrity hash for `DesiredState` is computed using MD5:
    ```rust
    let hash = format!(
        "{:x}",
        md5::compute(simd_json::to_string(&state).unwrap_or_default())
    );
    ```
    MD5 is vulnerable to collision attacks. Malicious actors could generate structurally distinct system states that yield identical MD5 hashes, bypassing integrity checks or cache key validations, leading to state confusion and configuration injection.
*   **Remediation:** Replace `md5::compute` with a cryptographically secure hash function such as SHA-256 (which is already imported via the `sha2` crate in `Cargo.toml`).

#### MAJOR: Lack of Schema-as-Code Discipline (Ad-Hoc JSON Payloads)
*   **Location:** `crates/op-state/src/dbus_server.rs:92` and throughout the crate
*   **Impact:** Extensive reliance on `simd_json::OwnedValue` and raw JSON strings (`String`) for RPC inputs and outputs makes the application vulnerable to deserialization errors and API version drift.
*   **Remediation:** 
    1. Express all core structures (`DesiredState`, `StateChange`, `EncryptedState`) in `.proto` files using Protocol Buffers syntax (`proto3`).
    2. Use `tonic-build` or `prost-build` within a `build.rs` script to generate robust, typed Rust models.
    3. Employ `protovalidate` for declarative field constraint enforcement.

#### MAJOR: Hardcoded Policy Validation Templates
*   **Location:** `crates/op-state/src/schema_validator.rs:213`
*   **Impact:** Validated use cases (such as the `privacy_router` definition) are hardcoded inside Rust functions (`load_default_use_cases`). This prevents security compliance officers from auditing, adapting, or replacing security policies dynamically without recompiling the binary.
*   **Remediation:** Externalize all validation constraints and schemas into machine-readable JSON or YAML documents conformant with an OSCAL Component Definition. Load these policies dynamically at startup and validate them using an external engine.