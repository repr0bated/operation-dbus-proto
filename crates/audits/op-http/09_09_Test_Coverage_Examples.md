# Test Coverage Audit

### Test Metrics
* **Total Test Functions**: 0
* **Property-Based Tests**: None found
* **Fuzzing Configurations**: None found

**No tests found**  
* **Risk Level**: **High Risk**
* **Justification**: The `op-http` crate serves as the central HTTP/TLS gateway for the entire `op-dbus` workspace, acting as the single source of truth for routing, TLS termination, metrics, and security middleware. The complete absence of unit, integration, property, or fuzz tests means that critical routing logic, TLS parsing, and custom authentication/rate-limiting middleware are entirely unverified in CI, risking silent regression or security bypasses.

---

# Schema-as-Code Discipline Audit

The codebase exhibits several departures from a strict "schema-as-code" discipline, opting for ad-hoc structs and unstructured, raw JSON construction rather than versioned Protocol Buffers or structured OSCAL documents.

### Ad-hoc JSON Serialization for Health Monitoring
* **Location**: `crates/op-http/src/health.rs:11-26`
* **Details**: The health monitoring contracts are defined using ad-hoc, unversioned Rust structs (`HealthResponse` and `ServiceHealth`) with Serde attributes. These data structures are consumed by external monitoring tooling and load balancers, but they lack a formal, versioned protocol schema (such as Protobuf) to guarantee backward compatibility during control-plane upgrades.

### Raw JSON Metrics Construct with `simd_json::json!`
* **Location**: `crates/op-http/src/metrics.rs:211-224`
* **Details**: The `json_metrics` endpoint constructs its response dynamically using the `simd_json::json!` macro. Defining metrics contracts dynamically as inline string-keyed objects bypasses type safety and makes integration brittle, as there is no versioned schema defining the metrics output format.

---

# Security & Quality Findings

## [Medium] Reliance on Shell Execution for TLS Certificate Operations
* **Location**: `crates/op-http/src/tls.rs:314-323`, `crates/op-http/src/tls.rs:328-330`, and `crates/op-http/src/tls.rs:344-346`
* **Description**: The functions `validate_cert_key_match`, `get_cert_expiry`, and `is_cloudflare_cert` invoke the system `openssl` binary using `std::process::Command`. 
* **Impact**: 
  1. **Deployment Fragility / Denial of Service**: If the `openssl` binary is missing from the target host's `PATH`, these helper functions will fail, potentially crashing or degrading startup/validation procedures.
  2. **Injection Potential**: If certificate paths can be influenced by users (e.g., via dynamically provisioned certificates or configuration files), passing unsanitized arguments directly to `Command` can lead to unexpected command execution or file disclosure depending on the system `openssl` parser.
* **Remediation**: Use a native Rust cryptographic library (such as the `x509-parser` crate) to parse PEM certificates, compute moduli, check expiration dates, and verify issuer fields entirely in memory without spawning child processes.

---

## [Medium] Permissive Wildcard CORS Enabled by Default
* **Location**: `crates/op-http/src/request_filters.rs:124-129` and `crates/op-http/src/middleware.rs:132-145`
* **Description**: The default CORS layer configuration allows any origin (`Any`), any method (`Any`), and any header (`Any`).
* **Impact**: Under production scenarios, allowing wildcard origins permits malicious websites to make cross-origin requests to this daemon control plane. If this endpoint is exposed on localhost or a local network without strict authentication tokens, it facilitates Cross-Site Request Forgery (CSRF) or cross-origin data extraction.
* **Remediation**: Restrict the default CORS policy to only permit trusted local or loopback domains, and require explicit configuration values to authorize additional origins. Avoid setting `allow_origin(Any)` in production default profiles.

---

## [Low] Authentication Middleware Fails Open (Authentication is Optional)
* **Location**: `crates/op-http/src/request_filters.rs:81-103`
* **Description**: The `api_key_auth` middleware extracts authentication keys from headers (`x-api-key`, `authorization`, `x-password`), prints debug messages, but then unconditionally calls `next.run(request).await`.
* **Impact**: The middleware performs no actual validation of credentials, serving only as a logging mechanism. If developers integrate this middleware under the assumption that it secures pathways, it will silently fail open, leading to an authentication bypass.
* **Remediation**: Enforce strict validation within the authentication middleware. If authentication is indeed optional, rename the middleware to `optional_api_key_logger` to prevent security-by-obscurity assumptions, and implement a dedicated, hard-failing `require_api_key` middleware for restricted routes.

---

## [Low] Unchecked System Time Subtract Operations Can Panic
* **Location**: `crates/op-http/src/health.rs:68-69`, `crates/op-http/src/health.rs:78-80`, `crates/op-http/src/health.rs:83-85`, and `crates/op-http/src/health.rs:159-161`
* **Description**: The health check and service check calculations perform `SystemTime::now().duration_since(UNIX_EPOCH).unwrap()`.
* **Impact**: If the host system clock undergoes a significant backward synchronization (e.g., via NTP adjustment or manual clock reset to a time before the UNIX epoch or before the application's startup time), `duration_since` will return an `Err`. Calling `unwrap()` on this result will cause a thread panic, potentially terminating active HTTP connections or crashing the health endpoint.
* **Remediation**: Replace `unwrap()` with defensive error handling, or use `Instant::now()` for relative durations (like uptime) as it is guaranteed to be monotonic and monotonic clocks do not tick backwards.

---
## ⚠ Citation Warnings
- `crates/op-http/src/tls.rs:314`: file has 303 lines
- `crates/op-http/src/tls.rs:328`: file has 303 lines
- `crates/op-http/src/tls.rs:344`: file has 303 lines
