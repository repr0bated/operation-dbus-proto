### Async & Concurrency Analysis

#### 1. Reactor/Executor Metrics

* **`async fn` count:** 40
* **`tokio::spawn` count:** 0
* **`spawn_blocking` count:** 0

#### 2. Reactor Blocking Hazards
* **Blocking Commands in Synchronous Contexts:**
  In `crates/op-state/src/authority.rs:14` and `crates/op-state/src/authority.rs:37`, `std::process::Command::output()` is invoked synchronously to stop/disable and query `NetworkManager` and `systemd-networkd`. While these functions themselves are synchronous, they are called inside the larger system control plane context. If invoked from within an asynchronous task thread without offloading to `spawn_blocking`, they will block the Tokio reactor thread.
* **Blocking File I/O in Synchronous Cryptographic Operations:**
  In `crates/op-state/src/crypto.rs:79`, `crates/op-state/src/crypto.rs:141`, and `crates/op-state/src/crypto.rs:152`, blocking filesystem calls (`std::fs::read`, `std::fs::write`, `std::fs::read_to_string`) are utilized. While these are synchronous functions, if they are executed on async worker threads during state transactions, they will degrade reactor performance.

#### 3. Send/Sync Bounds on Public Async Traits
* **Native Async Traits Lacking Send Future Bounds:**
  In `crates/op-state/src/dbus_plugin_base.rs:24`, the trait `DbusStatePluginBase` uses native `async fn` syntax under `#![allow(async_fn_in_trait)]`. Unlike `StatePlugin` (which uses `#[async_trait]`), native async trait functions in Rust do not automatically enforce that the returned `Future` implements `Send`. When this trait is implemented and its methods are invoked within a multi-threaded Tokio runtime (which requires `Send` bounds on spawned tasks), it can cause compile-time failures depending on the implementer's internal state.

---

### Security & Quality Audit Findings

#### [Critical] Complete Decryption Failure / Persistent State Loss via Discarded Salt
* **File:** `crates/op-state/src/crypto.rs:52`
* **Vulnerability Type:** Cryptographic Design Defect / High-Impact Bug
* **Description:** 
  The password-based key derivation function generates a cryptographically secure random salt on the fly, uses it to derive an AES-256 key via Argon2, and then immediately discards the salt:
  ```rust
  pub fn from_password(password: &str) -> Result<Self> {
      let salt = SaltString::generate(&mut OsRng); // Salt generated here
      let argon2 = Argon2::default();

      let mut key_bytes = [0u8; KEY_SIZE];
      argon2
          .hash_password_into(
              password.as_bytes(),
              salt.as_str().as_bytes(),
              &mut key_bytes,
          )
          .map_err(|e| anyhow::anyhow!("Failed to derive key: {}", e))?;

      let key = *Key::<Aes256Gcm>::from_slice(&key_bytes);
      Ok(Self { key }) // Salt is discarded and never stored or returned
  }
  ```
  Additionally, when encrypting data, the `salt` field of the returned `EncryptedState` is explicitly set to `None`:
  ```rust
  Ok(EncryptedState {
      nonce: BASE64.encode(nonce_bytes),
      salt: None, // Discarded salt is not persisted with ciphertext
      ciphertext: BASE64.encode(ciphertext),
      version: 1,
  })
  ```
  Consequently, every invocation of `StateEncryption::from_password` with the same password generates a different key. Any state persisted to disk using a password-derived key becomes permanently undecryptable upon application restart, leading to catastrophic data loss and persistent state corruption.
* **Remediation:** 
  Modify `from_password` to accept the salt as an argument, or modify `EncryptedState` to persist the derived salt so that it can be loaded and passed to Argon2 during decryption.

---

#### [High] File Creation Permission Race Condition (CWE-377 / CWE-732)
* **File:** `crates/op-state/src/crypto.rs:79`
* **Vulnerability Type:** Privileged Key Exposure
* **Description:** 
  In `StateEncryption::from_key_file`, when a new key file is generated, the file is written to disk first using default system permissions (which are subject to the process's umask and can be world- or group-readable):
  ```rust
  // Save key with restricted permissions
  std::fs::write(path, encryption.key.as_slice()).context("Failed to write key file")?;

  // Set permissions to 600 (owner read/write only)
  #[cfg(unix)]
  {
      use std::os::unix::fs::PermissionsExt;
      let mut perms = std::fs::metadata(path)?.permissions();
      perms.set_mode(0o600);
      std::fs::set_permissions(path, perms)
          .context("Failed to set key file permissions")?;
  }
  ```
  There is a critical time-of-check to time-of-use (TOCTOU) window between `std::fs::write` and `std::fs::set_permissions`. During this window, an unprivileged local attacker or a malicious process on a multi-user system could read the raw 256-bit symmetric encryption key.
* **Remediation:** 
  Use Unix-specific `OpenOptionsExt` to create the file with `0o600` permissions atomically at the time of creation, or apply a restrictive umask before writing:
  ```rust
  use std::fs::OpenOptions;
  use std::os::unix::fs::OpenOptionsExt;

  let mut options = OpenOptions::new();
  options.write(true).create(true).truncate(true);
  #[cfg(unix)]
  options.mode(0o600);

  let mut file = options.open(path)?;
  file.write_all(encryption.key.as_slice())?;
  ```

---

#### [High] Undefined Behavior via `unsafe { simd_json::from_str }` on Mutated Strings
* **Files:** `crates/op-state/src/crypto.rs:167`, `crates/op-state/src/crypto.rs:177`, `crates/op-state/src/crypto.rs:197`, and `crates/op-state/src/dbus_server.rs:114`
* **Vulnerability Type:** Memory Safety / Undefined Behavior
* **Description:** 
  `simd-json` is an in-place parser. When parsing via `simd_json::from_str`, the input string is modified in-place to perform tasks like unescaping characters. Using `unsafe { simd_json::from_str(&mut contents) }` is highly dangerous because if parsing fails or mutates the string buffer to contain invalid UTF-8, subsequent access, dropping, or reuse of the `String` violates Rust's fundamental safety invariant that all `str`/`String` types must contain valid UTF-8. This leads to immediate Undefined Behavior.
  For example, in `is_encrypted`:
  ```rust
  pub fn is_encrypted(path: &Path) -> Result<bool> {
      let contents = std::fs::read_to_string(path).context("Failed to read state file")?;

      // Try to parse as encrypted state
      let mut c1 = contents.clone();
      if unsafe { simd_json::from_str::<EncryptedState>(&mut c1) }.is_ok() {
          return Ok(true);
      }

      // Try to parse as plain state
      let mut c2 = contents;
      if unsafe { simd_json::from_str::<State>(&mut c2) }.is_ok() {
          return Ok(false);
      }
      ...
  ```
  If `simd_json::from_str` on `c1` fails, `c1` is left in an arbitrary, potentially invalid UTF-8 state when it goes out of scope and is dropped.
* **Remediation:** 
  Avoid reading the files as strings. Read files as raw byte buffers (`Vec<u8>`) and parse them using `simd_json::from_slice`, which does not carry UTF-8 invariants for its input buffer and is entirely safe.

---

#### [Medium] Use of Cryptographically Broken Hash Algorithm (MD5)
* **File:** `crates/op-state/src/plugin.rs:24`
* **Vulnerability Type:** Cryptographic Weakness
* **Description:** 
  The `DesiredState::new` constructor generates state footprint hashes using MD5:
  ```rust
  pub fn new(state: Value) -> Self {
      let hash = format!(
          "{:x}",
          md5::compute(simd_json::to_string(&state).unwrap_or_default())
      );
      ...
  ```
  MD5 is vulnerable to collision attacks. If state changes are tracked, verified, or validated using this MD5 hash, a malicious plugin or user could construct different state payloads that yield identical hashes, bypassing state transition validation and footprint auditing.
* **Remediation:** 
  Replace the MD5 computation with SHA-256 (which is already imported and used elsewhere in the crate, such as in `dbus_plugin_base.rs`).

---

#### [Medium] Silent Network Authority Enforcement Failure
* **File:** `crates/op-state/src/authority.rs:14`
* **Vulnerability Type:** Error-Handling / Logic Defect
* **Description:** 
  `NetworkAuthority::enforce_authority` issues shell commands to stop and disable competing network services, but explicitly discards command execution results:
  ```rust
  pub fn enforce_authority() -> Result<()> {
      // Disable NetworkManager if running
      let _ = Command::new("systemctl")
          .args(["stop", "NetworkManager"])
          .output();
      ...
      log::info!("Network authority enforced - plugin system is sole controller");
      Ok(())
  }
  ```
  If this application is run by an unprivileged user, systemctl commands will fail silently. The system will log `"Network authority enforced"` despite failing to stop competing services. This creates a split-brain networking scenario where both `NetworkManager` and the plugin system attempt to configure interfaces simultaneously.
* **Remediation:** 
  Check the exit status of each `Command`. Raise an error if enforcement commands fail, or log appropriate warnings.

---

#### [Low] Non-Atomic File Renames on Permission Regression
* **File:** `crates/op-state/src/crypto.rs:141`
* **Vulnerability Type:** Security Regression
* **Description:** 
  `save_encrypted` writes the encrypted payload to a temporary file (`.tmp`) and renames it to ensure atomicity:
  ```rust
  let tmp_path = path.with_extension("tmp");
  std::fs::write(&tmp_path, json).context("Failed to write encrypted state")?;
  std::fs::rename(&tmp_path, path).context("Failed to rename state file")?;
  ```
  `std::fs::write` creates the temporary file using default umask permissions. When it is renamed over `path`, the target file loses its original restricted file permissions (e.g., `0o600`) and takes on the broader permissions of the `.tmp` file.
* **Remediation:** 
  Explicitly apply the target file's existing permissions to `tmp_path` before performing the rename.

---

### Schema-as-Code Compliance Audit

The `op-state` crate exhibits a split discipline, departing from versioned schemas in favor of ad-hoc structs and unstructured dynamic types:

1. **Ad-Hoc Structs for Core Cryptographic Assertions:**
   In `crates/op-state/src/crypto.rs:18`, the `EncryptedState` structural contract is defined as an ad-hoc Rust struct and serialized as generic JSON rather than being defined as a versioned Protobuf schema or an OSCAL-compliant security control record.
2. **Hardcoded Schema Definitions and Logic-Driven Validation:**
   In `crates/op-state/src/schema_validator.rs:11` and `crates/op-state/src/schema_validator.rs:136`, schemas are modeled via ad-hoc structs (`UseCaseTemplate`, `FieldCombination`, `Constraint`) and hardcoded as nested maps inside the application logic. 
3. **Unstructured Dynamic JSON Fields:**
   In `crates/op-state/src/plugin.rs:107`, the plugin metadata holds `feature_schemas` and `object_schemas` as raw `simd_json::OwnedValue` values. Passing schemas as unstructured, dynamic JSON payloads prevents compile-time contract enforcement and decouples the state validation from a central versioned schema registry.