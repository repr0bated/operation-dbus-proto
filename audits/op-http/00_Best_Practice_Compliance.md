| Pattern | File:Line | Crate Approach | Corpus Best Practice | Gap | Rating |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `format_json_manual` | `crates/op-http/src/health.rs:177` | Return `"healthy"` as a raw string and manually format the message string. | Use versioned Protocol Buffer schemas or structured enums to express data contracts. | **Schema-as-Code Violation**: Expresses API health contracts using ad-hoc raw strings and untyped messages rather than versioned schemas. | Major Gap |
| `format_json_manual` | `crates/op-http/src/health.rs:182` | Return `"unhealthy"` as a raw string and manually format the error status. | Use versioned Protocol Buffer schemas or structured enums to express data contracts. | **Schema-as-Code Violation**: Expresses API health contracts using ad-hoc raw strings and untyped messages rather than versioned schemas. | Major Gap |
| `format_json_manual` | `crates/op-http/src/health.rs:187` | Return `"unhealthy"` as a raw string and format the connection error. | Use versioned Protocol Buffer schemas or structured enums to express data contracts. | **Schema-as-Code Violation**: Expresses API health contracts using ad-hoc raw strings and untyped messages rather than versioned schemas. | Major Gap |
| `format_json_manual` | `crates/op-http/src/health.rs:194` | Return `"error"` as a raw string and format client creation failures. | Use versioned Protocol Buffer schemas or structured enums to express data contracts. | **Schema-as-Code Violation**: Expresses API health contracts using ad-hoc raw strings and untyped messages rather than versioned schemas. | Major Gap |
| `format_json_manual` | `crates/op-http/src/health.rs:226` | Return `"unhealthy"` status with manual string formatting for missing paths. | Use versioned Protocol Buffer schemas or structured enums to express data contracts. | **Schema-as-Code Violation**: Expresses API health contracts using ad-hoc raw strings and untyped messages rather than versioned schemas. | Major Gap |
| `unwrap_expect` | `crates/op-http/src/health.rs:66` | `.duration_since(UNIX_EPOCH).unwrap()` | Use `.unwrap_or_default()` or safely handle system clock regression. | Thread panic risk if NTP adjustments cause system time to run backward. | Major Gap |
| `unwrap_expect` | `crates/op-http/src/health.rs:75` | `.duration_since(UNIX_EPOCH).unwrap()` | Use `.unwrap_or_default()` or safely handle system clock regression. | Thread panic risk if NTP adjustments cause system time to run backward. | Major Gap |
| `unwrap_expect` | `crates/op-http/src/health.rs:80` | `.duration_since(UNIX_EPOCH).unwrap()` | Use `.unwrap_or_default()` or safely handle system clock regression. | Thread panic risk if NTP adjustments cause system time to run backward. | Major Gap |
| `unwrap_expect` | `crates/op-http/src/health.rs:169` | `.duration_since(UNIX_EPOCH).unwrap()` | Use `.unwrap_or_default()` or safely handle system clock regression. | Thread panic risk if NTP adjustments cause system time to run backward. | Major Gap |
| `unwrap_expect` | `crates/op-http/src/health.rs:204` | `.duration_since(UNIX_EPOCH).unwrap()` | Use `.unwrap_or_default()` or safely handle system clock regression. | Thread panic risk if NTP adjustments cause system time to run backward. | Major Gap |
| `command_new` | `crates/op-http/src/tls.rs:262` | Fork `openssl` binary to retrieve certificate modulus. | Parse TLS certificate details in-process using Rust libraries. | Reliance on external system binaries; performance and runtime failure risks. | Major Gap |
| `command_new` | `crates/op-http/src/tls.rs:267` | Fork `openssl` binary to retrieve private key modulus. | Parse private key details in-process using Rust libraries. | Reliance on external system binaries; performance and runtime failure risks. | Major Gap |
| `command_new` | `crates/op-http/src/tls.rs:279` | Fork `openssl` binary to check certificate end date. | Parse certificate metadata in-process using Rust libraries. | Reliance on external system binaries; performance and runtime failure risks. | Major Gap |
| `command_new` | `crates/op-http/src/tls.rs:296` | Fork `openssl` binary to extract certificate issuer. | Parse certificate metadata in-process using Rust libraries. | Reliance on external system binaries; performance and runtime failure risks. | Major Gap |
| `std_fs_in_async` | `crates/op-http/src/tls.rs:9` | Import and use synchronous `std::fs::File` and `std::io::BufReader`. | Use async runtime file utilities (e.g. `tokio::fs::File`). | Blocking synchronous I/O operations will starve threads in an async executor. | Major Gap |

---

### Actionable Recommendations

#### 1. Enforce Schema-as-Code for Health Payloads
*   **File**: `crates/op-http/src/health.rs` (Lines 177, 182, 187, 194, 226)
*   **Action**: Eliminate ad-hoc raw status strings (`"healthy"`, `"unhealthy"`, `"error"`) and manually formatted unstructured error strings. Define a versioned Protocol Buffer schema (e.g., `health.proto`) that represents the health check response model:
    ```protobuf
    syntax = "proto3";
    package op.http.v1;

    enum HealthStatus {
      HEALTH_STATUS_UNSPECIFIED = 0;
      HEALTH_STATUS_HEALTHY = 1;
      HEALTH_STATUS_UNHEALTHY = 2;
      HEALTH_STATUS_ERROR = 3;
    }

    message HealthCheckResponse {
      HealthStatus status = 1;
      string message = 2;
      uint64 last_check = 3;
    }
    ```
    Generate the Rust structures from this schema and serialize the model to JSON/Protobuf using standard serialization attributes.

#### 2. Remediate Monotonic / Safe Timing Panics
*   **File**: `crates/op-http/src/health.rs` (Lines 66, 75, 80, 169, 204)
*   **Action**: `SystemTime::duration_since` returns a `Result` that will yield an `Err` if the system clock drifts backward (e.g., during NTP corrections). Avoid calling `.unwrap()` directly on this result. Implement a safe helper that defaults to `0` or uses relative monotonic timers for measurement:
    ```rust
    let start_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| std::time::Duration::from_secs(0))
        .as_secs();
    ```

#### 3. Eliminate External Process Execution for Certificate Inspection
*   **File**: `crates/op-http/src/tls.rs` (Lines 262, 267, 279, 296)
*   **Action**: Refactor the code to remove calls to the external `openssl` binary via `Command::new`. Instead, parse the certificates and keys directly in Rust memory using programmatic, secure parsers such as `x509-parser` or `ring`:
    *   For certificate properties (issuer, end date): Use `x509-parser::pem::parse_x509_pem`.
    *   For modulus comparison: Read PEM structures directly via `rustls-pemfile` or `openssl` FFI bindings to extract and compare public key components in-process.

#### 4. Resolve Blocking I/O in Async Contexts
*   **File**: `crates/op-http/src/tls.rs` (Line 9)
*   **Action**: Replace imports of `std::fs::File` and `std::io::BufReader` with their asynchronous equivalents from `tokio::fs::File` and `tokio::io::BufReader`. Alternatively, spawn blocking calls on a dedicated thread pool to avoid blocking the primary async executor thread:
    ```rust
    let content = tokio::task::spawn_blocking(move || {
        std::fs::read(path)
    }).await??;
    ```