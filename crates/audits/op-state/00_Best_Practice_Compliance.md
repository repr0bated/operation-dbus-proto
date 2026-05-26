| Pattern | File:Line | Crate Approach | Corpus Best Practice | Gap | Rating |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `command_new` | `crates/op-state/src/authority.rs:14` | Calls synchronous `Command::new("systemctl")` to stop `NetworkManager` and discards results (`let _ = ...`). | Use asynchronous process invocation or programmatic DBus interface to control services; always handle/propagate results. | Silently ignores errors of system-level network state changes; executes blockingly. | Major Gap |
| `command_new` | `crates/op-state/src/authority.rs:18` | Calls synchronous `Command::new("systemctl")` to disable `NetworkManager` and discards results. | Use asynchronous process invocation or programmatic DBus interface to control services; always handle/propagate results. | Silently ignores errors of critical network state changes; executes blockingly. | Major Gap |
| `command_new` | `crates/op-state/src/authority.rs:23` | Calls synchronous `Command::new("systemctl")` to stop `systemd-networkd` and discards results. | Use programmatic DBus interface or handle process exit status. | Silently ignores service termination errors. | Major Gap |
| `command_new` | `crates/op-state/src/authority.rs:27` | Calls synchronous `Command::new("systemctl")` to disable `systemd-networkd` and discards results. | Use programmatic DBus interface or handle process exit status. | Silently ignores service modification errors. | Major Gap |
| `command_new` | `crates/op-state/src/authority.rs:40` | Checks service status synchronously with `Command::new("systemctl")`. | Use async process execution or programmatic DBus querying. | Blocks the async executor thread. | Minor Gap |
| `unsafe_block` | `crates/op-state/src/crypto.rs:207` | Uses `unsafe { simd_json::from_str(&mut contents) }` on file-loaded `String`. | Buffer passed to `simd_json` must be padded with `simd_json::PADDING_SIZE` bytes; use safe parsed formats. | **Buffer Overread Hazard**: Parsing standard, unpadded string buffers can cause SIMD out-of-bounds reads. Ad-hoc JSON parsing violates Schema-as-Code. | Major Gap |
| `unsafe_block` | `crates/op-state/src/crypto.rs:219` | Uses `unsafe { simd_json::from_str::<EncryptedState>(&mut c1) }` on a cloned string. | Ensure buffers are properly padded or parse using safe deserializers. | Buffer overread risk due to unpadded buffer allocation. Violates Schema-as-Code. | Major Gap |
| `unsafe_block` | `crates/op-state/src/crypto.rs:225` | Uses `unsafe { simd_json::from_str::<State>(&mut c2) }` on a mutable string slice. | Ensure buffers are properly padded or parse using safe deserializers. | Buffer overread risk due to unpadded buffer allocation. Violates Schema-as-Code. | Major Gap |
| `unsafe_block` | `crates/op-state/src/crypto.rs:242` | Uses `unsafe { simd_json::from_str(&mut contents) }` on file-loaded `String`. | Ensure buffers are properly padded or parse using safe deserializers. | Buffer overread risk due to unpadded buffer allocation. Violates Schema-as-Code. | Major Gap |
| `simd_json_from_str` | `crates/op-state/src/crypto.rs:207` | Invokes in-place mutable string JSON parsing. | Use safe, non-mutating parsers like `serde_json` or verify SIMD padding. | Memory safety risk under unsafe parsing of unpadded files. | Major Gap |
| `simd_json_from_str` | `crates/op-state/src/crypto.rs:219` | Invokes in-place mutable string JSON parsing. | Use safe, non-mutating parsers like `serde_json` or verify SIMD padding. | Memory safety risk under unsafe parsing of unpadded files. | Major Gap |
| `simd_json_from_str` | `crates/op-state/src/crypto.rs:225` | Invokes in-place mutable string JSON parsing. | Use safe, non-mutating parsers like `serde_json` or verify SIMD padding. | Memory safety risk under unsafe parsing of unpadded files. | Major Gap |
| `simd_json_from_str` | `crates/op-state/src/crypto.rs:242` | Invokes in-place mutable string JSON parsing. | Use safe, non-mutating parsers like `serde_json` or verify SIMD padding. | Memory safety risk under unsafe parsing of unpadded files. | Major Gap |
| `unwrap_expect` | `crates/op-state/src/crypto.rs:264` | `unwrap()` used in a test. | Normal usage of `unwrap()` in test logic. | None (Test logic) | Compliant |
| `unwrap_expect` | `crates/op-state/src/crypto.rs:267` | `unwrap()` used in a test. | Normal usage of `unwrap()` in test logic. | None (Test logic) | Compliant |
| `unwrap_expect` | `crates/op-state/src/crypto.rs:268` | `unwrap()` used in a test. | Normal usage of `unwrap()` in test logic. | None (Test logic) | Compliant |
| `unwrap_expect` | `crates/op-state/src/crypto.rs:277` | `unwrap()` used in a test. | Normal usage of `unwrap()` in test logic. | None (Test logic) | Compliant |
| `unwrap_expect` | `crates/op-state/src/crypto.rs:287` | `unwrap()` used in a test. | Normal usage of `unwrap()` in test logic. | None (Test logic) | Compliant |
| `std_fs_in_async` | `crates/op-state/src/crypto.rs:78` | Synchronously reads a file using `std::fs::read` inside an async function. | Use asynchronous file systems like `tokio::fs` or offload to blocking threads. | Blocks active executor threads synchronously during system I/O. | Major Gap |
| `std_fs_in_async` | `crates/op-state/src/crypto.rs:95` | Synchronously creates folders with `std::fs::create_dir_all`. | Use asynchronous filesystem APIs. | Blocks active executor threads synchronously during system I/O. | Major Gap |
| `std_fs_in_async` | `crates/op-state/src/crypto.rs:99` | Synchronously writes key file with `std::fs::write`. | Use asynchronous filesystem APIs. | Blocks active executor threads synchronously during system I/O. | Major Gap |
| `std_fs_in_async` | `crates/op-state/src/crypto.rs:105` | Synchronously queries metadata permissions. | Use asynchronous filesystem APIs. | Blocks active executor threads synchronously during system I/O. | Major Gap |
| `std_fs_in_async` | `crates/op-state/src/crypto.rs:107` | Synchronously writes permissions with `std::fs::set_permissions`. | Use asynchronous filesystem APIs. | Blocks active executor threads synchronously during system I/O. | Major Gap |
| `unsafe_block` | `crates/op-state/src/dbus_plugin_base.rs:66` | Parses unpadded debug format strings `format!("{:?}", value)` via unsafe `simd_json::from_str`. | Map variant types safely to structured schemas or parse JSON safely without unsafe SIMD overreads. | **Critical Memory Safety/Security Vulnerability**: Debug formats are not valid JSON, and unpadded strings passed to `simd_json` trigger heap out-of-bounds reads. Directly exploitable. | Critical Gap |
| `simd_json_from_str` | `crates/op-state/src/dbus_plugin_base.rs:66` | Passes formatted debug output to `simd_json`. | Parse values strictly based on versioned schemas/converters. | Memory safety hazard and schema-less design violation. | Critical Gap |
| `format_json_manual` | `crates/op-state/src/dbus_plugin_base.rs:62` | Basic dynamic property failure message formatting. | Normal dynamic context logging. | None | Compliant |
| `format_json_manual` | `crates/op-state/src/dbus_plugin_base.rs:65` | Formats zbus dynamic variant to a string via `format!("{:?}", value)`. | Serialization must use strict data contracts/converters (e.g. structured JSON or protobuf). | Ad-hoc serialization via debug string parsing violates Schema-as-Code. | Major Gap |
| `format_json_manual` | `crates/op-state/src/dbus_plugin_base.rs:87` | Dynamic error context formatting. | Normal context formatting. | None | Compliant |
| `format_json_manual` | `crates/op-state/src/dbus_plugin_base.rs:127` | Dynamic error context formatting. | Normal context formatting. | None | Compliant |
| `format_json_manual` | `crates/op-state/src/dbus_plugin_base.rs:153` | Generates hashes on ad-hoc JSON strings `simd_json::to_string`. | Use deterministic serialized schemas (Protobuf bytes/OSCAL) to perform hashing. | State hashing is performed on ad-hoc, unversioned JSON string output. | Minor Gap |

---

### Actionable Recommendations for Major & Critical Gaps

#### 1. Fix Critical Vulnerability in DBus Property Deserialization (`crates/op-state/src/dbus_plugin_base.rs:65-66`)
* **Vulnerability Analysis:** The current implementation uses Rust’s Debug formatter `format!("{:?}", value)` to obtain a string representation of a dynamic `zbus::zvariant::Value`, which is then passed to `unsafe { simd_json::from_str(...) }`. Standard `String` buffers created via `format!` are not padded with the `simd_json::PADDING_SIZE` bytes required by SIMD loading instructions. Since debug representations are not valid JSON, parsing will fail, but the execution of SIMD operations on unpadded buffers will trigger out-of-bounds reads on the heap (potential information disclosure or memory-fault crash).
* **Remediation:** 
  * Avoid string-based serialization for converting DBus properties to JSON. Instead, implement a safe, structured map conversion from `zbus::zvariant::Value` directly to structured types (e.g., using a recursive converter yielding `serde_json::Value`).
  * If serialization is absolutely necessary, use standard, safe deserializers like `serde_json::from_str` or safe APIs from `simd_json` that perform copy-padding (e.g., `simd_json::to_padded_string`).

#### 2. Eliminate Unsafe/Unpadded `simd_json` Parsing in State Loading (`crates/op-state/src/crypto.rs:207, 219, 225, 242`)
* **Vulnerability Analysis:** Passing contents read directly from raw files (or standard cloned Strings) to `simd_json::from_str` within `unsafe` blocks violates memory safety invariants because these strings lack trailing padding. 
* **Remediation:**
  * Convert the file reading pipeline to populate a padded string explicitly using `simd_json::to_padded_string` before parsing.
  * Alternatively, replace unsafe SIMD parsing with a safe, compliant parser like `serde_json` for processing persistent states.
  * To comply with the project's **Schema-as-Code** discipline, transition `State` and `EncryptedState` from ad-hoc Rust/JSON representations to versioned Protocol Buffer definitions to guarantee deterministic and backwards-compatible decoding.

#### 3. Transition from Sync System I/O to Async operations (`crates/op-state/src/crypto.rs:78, 95, 99, 105, 107`)
* **Issue:** Using `std::fs` operations (`read`, `create_dir_all`, `write`, `metadata`, `set_permissions`) inside asynchronous tasks stalls execution threads, starving the async scheduler.
* **Remediation:** 
  * Replace `std::fs` imports with `tokio::fs` equivalent structures.
  * Change file reading/writing code to use asynchronous waits (e.g., `tokio::fs::read(path).await`).

#### 4. Safe Systemd Management and Error Handling (`crates/op-state/src/authority.rs:14, 18, 23, 27`)
* **Issue:** Invoking external commands synchronously block execution threads, and discarding results using `let _ = ...` silences critical system service initialization/destruction failures.
* **Remediation:**
  * Use DBus-based programmatic management interfaces (e.g., via the systemd manager API over `zbus`) to start, stop, or disable services safely without external shell dependency.
  * If process execution is unavoidable, use `tokio::process::Command` to manage execution asynchronously, verify the status code of the spawned process, and return a robust `Result` up the call stack upon failure.