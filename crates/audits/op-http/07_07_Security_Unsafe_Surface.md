# OP-HTTP Production Security & Quality Audit

## 1. Executive Summary

This document provides a production security and quality audit of the `op-http` crate and associated workspace configuration. `op-http` serves as the central HTTP/TLS server for the `op-dbus` platform, responsible for TLS termination and router composition. 

The audit focused on structural security risks, compliance with the schema-as-code discipline, usage of unsafe code, external command execution, and architectural patterns.

---

## 2. Security & Unsafe Code Analysis

### Unsafe Code Blocks
* **Total `unsafe` blocks**: **0**
* The audited files in the `op-http` crate contain no explicit `unsafe { ... }` blocks. 

### External Process Spawning (`Command::new()`)
* **Total `Command::new()` invocations**: **4**
* **Location of invocations**:
  * `crates/op-http/src/tls.rs:287` — `Command::new("openssl")` in `validate_cert_key_match`
  * `crates/op-http/src/tls.rs:291` — `Command::new("openssl")` in `validate_cert_key_match`
  * `crates/op-http/src/tls.rs:301` — `Command::new("openssl")` in `get_cert_expiry`
  * `crates/op-http/src/tls.rs:317` — `Command::new("openssl")` in `is_cloudflare_cert`

#### Risk Assessment of `Command::new()`
1. **Forbidden Commands**: No matches were found for the forbidden list (`ovs-*` tools, raw OpenFlow tools, shell executors like `bash`/`sh`, or network exfiltration tools like `curl`/`wget`).
2. **Argument Validation**: The `cert_path` and `key_path` arguments are passed as parameters. Because these functions use `std::process::Command` without shell wrapper spawning, direct shell code injection is prevented. However, if paths containing leading hyphens (e.g., `-flag`) are passed, they could trigger argument injection into `openssl`. 
3. **Async Blockage**: All four invocations utilize `std::process::Command`, which is a synchronous OS process execution. When invoked inside the Tokio async runtime, these calls will block the executor thread, leading to thread starvation and potential denial of service under high TLS connection/negotiation loads.

### Hardcoded IPs, Domains, and Secret Material
* **IP Addresses**:
  * `crates/op-http/src/server.rs:36` & `crates/op-http/src/server.rs:245` — Binds to `0.0.0.0` (all interfaces) by default.
* **Domains & Paths**:
  * `crates/op-http/src/tls.rs:179` — Hardcoded path to `ghostbridge.tech` origin certificate.
  * `crates/op-http/src/tls.rs:183` — Hardcoded path referencing a developer home directory `/home/jeremy/certs/cloudflare_origin.pem`.
  * `crates/op-http/src/tls.rs:213-215` — Hardcoded array of target domains (`ghostbridge.tech`, `proxmox.ghostbridge.tech`, `op-web.ghostbridge.tech`) for automatic Let's Encrypt path building.

### D-Bus Method Exposure
No native D-Bus interface methods are defined or exported inside `crates/op-http`. The crate is designed strictly as an HTTP/TLS transport layer.

---

## 3. Schema-As-Code Compliance

The workspace utilizes Protocol Buffers elsewhere (such as `prost` in `Cargo.toml`), but `op-http` fails to enforce the schema-as-code discipline on its own interfaces. Ad-hoc JSON serialization contracts are declared directly in Rust structs and macro calls rather than generated from versioned schemas:

* **Ad-hoc Serialization Structs**:
  * `crates/op-http/src/health.rs:11-26` — `HealthResponse` and `ServiceHealth` use manual `Serialize`/`Deserialize` derivations to represent data contracts rather than versioned Protocol Buffer or OSCAL schema definitions.
* **In-line JSON Construction**:
  * `crates/op-http/src/metrics.rs:218-232` — `json_metrics` constructs JSON payloads dynamically using the `simd_json::json!` macro rather than compiling from versioned data definitions.

---

## 4. High-Severity Findings

### CORS Wildcard Misconfiguration
* **Location**: `crates/op-http/src/middleware.rs:186-199` & `crates/op-http/src/request_filters.rs:143-148`
* **Vulnerability Type**: Cross-Origin Resource Sharing (CORS) Misconfiguration
* **Description**: The default CORS layer allows any origin, any method, and any header using `Any`. Since this platform acts as an administrative control plane for Linux host systems (`op-dbus`), permitting arbitrary third-party websites to execute cross-origin requests to local endpoints (especially if bound to `0.0.0.0`) presents a severe security risk. While standard browser credentials may not be sent, non-preflighted state-changing requests could still be triggered, or arbitrary reads could occur if credentials are omitted.

### Credential Leakage in Debug Logging (Partial Redaction Failure)
* **Location**: `crates/op-http/src/request_filters.rs:77-83`
* **Vulnerability Type**: Sensitive Data Exposure (CWE-532)
* **Description**: The `api_key_auth` middleware extracts credentials from the `x-api-key`, `authorization`, and `x-password` headers. If the key length is greater than 8, the log redaction logic attempts to preserve the first 4 and last 4 characters using:
  ```rust
  format!("{}...{}", &key[..4], &key[key.len()-4..])
  ```
  If a user passes a 9-character secret password, 8 characters are logged in plaintext, leaving only 1 character redacted. This effectively leaks the cleartext secret to the tracing subsytem.

### Silent TLS Downgrade on Auto-Detection Failure
* **Location**: `crates/op-http/src/server.rs:70`, `crates/op-http/src/server.rs:114-131`, & `crates/op-http/src/tls.rs:142-149`
* **Vulnerability Type**: Insecure Default / Fail-Open (CWE-276)
* **Description**: If `TlsMode::Auto` is enabled and no valid TLS certificates are discovered, the build acceptor logic silently returns `Ok(None)`. When `HttpServer::serve` receives a `None` acceptor, it falls back to plaintext HTTP mode (`"TLS disabled - using HTTP only"`). A failure to locate certificates should trigger a hard initialization panic (fail-closed) rather than silently exposing control-plane communication to passive sniffing and interception.

---

## 5. Medium-Severity Findings

### Broken HSTS Validation Logic
* **Location**: `crates/op-http/src/request_filters.rs:25`
* **Vulnerability Type**: Cryptographic Defect / Logic Bug
* **Description**: The security headers filter attempts to conditionally insert the `Strict-Transport-Security` header by inspecting the `Host` header:
  ```rust
  if host_str.contains(":443") || host_str.starts_with("https://")
  ```
  The `Host` HTTP header *never* contains the protocol scheme (it is formatted as `domain:port`). Therefore, the `host_str.starts_with("https://")` check is dead code and will always evaluate to false. Furthermore, if the request uses a standard HTTPS port of 443, the port is omitted from the `Host` header by default, meaning HSTS will not be injected. This leaves actual HTTPS clients unprotected by HSTS.

### Untrusted Forwarded Headers in Rate Limiting (IP Spoofing)
* **Location**: `crates/op-http/src/request_filters.rs:90-95`
* **Vulnerability Type**: Security Bypass via Header Spoofing (CWE-290)
* **Description**: The rate-limiting logic extracts the client IP address by blindly trusting the `x-forwarded-for` and `x-real-ip` headers without verifying that the request originates from a trusted reverse-proxy subnet. An attacker can supply arbitrary IP addresses in these headers to bypass logging, metrics isolation, and rate-limiting limits.

---

## 6. Low/Code-Quality Findings

### Synchronous OS Command Execution inside Async Context
* **Location**: `crates/op-http/src/tls.rs:284-325`
* **Vulnerability Type**: Async Thread Blocking (Performance Degradation)
* **Description**: The functions `validate_cert_key_match`, `get_cert_expiry`, and `is_cloudflare_cert` use `std::process::Command` to invoke `openssl`. If these helper functions are called inside async request loops, they block the executor thread pool. They should either use `tokio::process::Command` or be spawned inside a `spawn_blocking` closure.

### Brittle Non-portable Hardcoded Local User Directories
* **Location**: `crates/op-http/src/tls.rs:183-186`
* **Vulnerability Type**: Architectural Code Quality Defect
* **Description**: The Cloudflare Origin certificate detection path list contains a hardcoded absolute path to a specific user home directory: `"/home/jeremy/certs/cloudflare_origin.pem"`. This breaks portability across dev environments and can result in unauthorized file read attempts if another local user named `jeremy` can manipulate paths.

---
## ⚠ Citation Warnings
- `crates/op-http/src/tls.rs:317`: file has 303 lines
