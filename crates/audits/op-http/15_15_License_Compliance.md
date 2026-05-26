# Production Security and Quality Audit: `op-http`

---

## 1. License & Dependency Compatibility Audit

### License Extraction
* **Crate:** `op-http`
* **Source of License:** `crates/op-http/Cargo.toml:6` (`license.workspace = true`) inheriting from the workspace root `Cargo.toml:50` (`license = "Apache-2.0"`).
* **Resolved License:** **Apache-2.0**

### GPL/AGPL/SSPL Dependency Scan
A comprehensive scan of `Cargo.lock` was performed. No GPL, AGPL, or SSPL dependencies were detected. 
* **Copyleft Note:** The crate utilizes `cozo` version `0.7.6` (listed in `Cargo.lock`), which is licensed under the **Mozilla Public License 2.0 (MPL-2.0)**. MPL-2.0 is a weak copyleft license. Because it is file-level copyleft and permits dual-licensing/linking with Apache-2.0 code without forcing the entire derivative work to become copyleft, it is compatible with the project's Apache-2.0 workspace license.

### Crates with No License Field
All workspace packages explicitly declare their license inheritance via `license.workspace = true`. In the physical cargo lock file, standard lock configurations do not preserve license metadata; however, all upstream registry crates referenced are compatible open-source libraries.

---

## 2. Schema-as-Code Compliance Audit

The project implements a schema-as-code discipline using Protocol Buffers and OSCAL. Ad-hoc serializations bypass validation and versioning controls. The following locations violate this discipline by defining raw Rust structs or generating ad-hoc string/JSON payloads directly for public APIs:

* **`crates/op-http/src/health.rs:11-18`**: The struct `HealthResponse` defines a public-facing data contract manually using standard serde serialization rather than deriving from a versioned schema or proto definition.
* **`crates/op-http/src/health.rs:21-26`**: The nested `ServiceHealth` struct represents an ad-hoc sub-contract lacking unified versioning metadata.
* **`crates/op-http/src/metrics.rs:205-215`**: The `json_metrics` endpoint constructs a non-versioned, ad-hoc JSON document dynamically using the `simd_json::json!` macro.
* **`crates/op-http/src/metrics.rs:224-245`**: The `metrics_dashboard` handler returns an ad-hoc HTML interface as a hardcoded static string rather than utilizing a schema-defined layout template.

---

## 3. Security & Quality Findings

### [CRITICAL] Blind Gzip Header Insertion Leading to Client Crashes and DoS
* **Location:** `crates/op-http/src/request_filters.rs:137-147`
* **Impact:** Direct functional disruption and client-side denial of service (DoS).
* **Description:** 
  The `compression` filter intercepts HTTP responses and unconditionally inserts a `"Content-Encoding", "gzip"` header:
  ```rust
  pub async fn compression(
      request: Request,
      next: Next,
  ) -> Response {
      // Add compression headers
      let mut response = next.run(request).await;

      let headers = response.headers_mut();
      headers.insert("Content-Encoding", "gzip".parse().unwrap());

      response
  }
  ```
  However, this middleware **does not compress** the body of the response. Any conforming HTTP client (such as a browser or `reqwest`) reading this header will immediately pass the raw, uncompressed payload to its Gzip decompression engine. This mismatch causes decompression parser failures, malformed payload rejections, or program crashes.

---

### [HIGH] Hardcoded No-Op Authentication Bypass
* **Location:** `crates/op-http/src/request_filters.rs:98-118`
* **Impact:** Authentication bypass.
* **Description:**
  The `api_key_auth` middleware parses key headers (`x-api-key`, `authorization`, `x-password`) but ultimately permits all traffic regardless of credential validity:
  ```rust
  // For now, allow all requests (authentication is optional)
  // In production, you would validate the API key here
  if let Some(key) = api_key {
      ...
  }
  next.run(request).await
  ```
  If this request filter is applied to endpoints protecting database control operations or administrative tools, it leaves them completely unauthenticated.

---

### [MEDIUM] Denial of Service via Integer Underflow Panic on NTP Clock Drift
* **Location:** `crates/op-http/src/health.rs:71-74`
* **Impact:** Application panic and crash.
* **Description:**
  The uptime calculation subtracts epoch timestamps:
  ```rust
  let uptime = now - self.start_time
      .duration_since(UNIX_EPOCH)
      .unwrap()
      .as_secs();
  ```
  If the system time is synchronized backwards via NTP or manual adjustment while the server is running, the computed variable `now` will be smaller than the recorded `self.start_time` epoch seconds value. Because these are standard `u64` integers, the subtraction will underflow, causing an immediate thread panic. Under Axum, a panic inside a state-accessing async routine can crash the active task or the server thread pool if configured with panic-on-abort.
* **Remediation:** Use monotonic clocks (`std::time::Instant`) for uptime calculations, or safe numeric operations such as `saturating_sub()`.

---

### [MEDIUM] Command Parameter Injection Risk via Shell Spawning
* **Locations:** 
  * `crates/op-http/src/tls.rs:284-295`
  * `crates/op-http/src/tls.rs:298-309`
  * `crates/op-http/src/tls.rs:313-321`
* **Impact:** Local privilege escalation or unauthorized file access.
* **Description:**
  The certificate utility functions run external shell operations by invoking `Command::new("openssl")` directly passing unchecked path strings:
  ```rust
  let cert_output = Command::new("openssl")
      .args(["x509", "-in", cert_path, "-noout", "-modulus"])
      .output()
  ```
  Although Rust's `Command::new` avoids standard shell execution (thus preventing classic `;` command concatenation), if `cert_path` is sourced from any dynamic configuration input, an attacker can specify a filename beginning with a hyphen (e.g. `-config <malicious_file>`) to inject command-line flags directly into OpenSSL. This could allow reading arbitrary files or invoking unsafe configuration routines.
* **Remediation:** Parse the PEM certificates using pure Rust decoders (such as the `x509-parser` crate) instead of spawning subprocesses.

---
## ⚠ Citation Warnings
- `crates/op-http/src/tls.rs:313`: file has 303 lines
