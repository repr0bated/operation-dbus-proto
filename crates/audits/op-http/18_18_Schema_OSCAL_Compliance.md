# Schema-as-Code Audit

The following table identifies data contracts, serialization mappings, and state endpoints that are implemented as ad-hoc Rust structures rather than versioned Protocol Buffer (`.proto`) schemas.

| Item | Type | file:line | Has .proto? | Gap |
| :--- | :--- | :--- | :--- | :--- |
| `HealthResponse` | Rust Struct | `crates/op-http/src/health.rs:11-18` | No | Defined as an ad-hoc Serde struct. Monitoring status and platform state should be exported via a versioned Protobuf definition to ensure deterministic system introspection. |
| `ServiceHealth` | Rust Struct | `crates/op-http/src/health.rs:21-26` | No | Defined as an ad-hoc Rust struct with manual Serde serialization. Inhibits cross-language schema compliance. |
| `json_metrics` response | Ad-hoc JSON | `crates/op-http/src/metrics.rs:224-245` | No | Uses `simd_json::json!` to build dynamic untyped objects on the fly. This violates schema-as-code principles for observability data contracts. |

---

# OSCAL Coverage Audit

The following table evaluates security-relevant controls implemented in code (such as cryptographic path detection, authorization, and rate limiting) against machine-readable Open Security Controls Assessment Language (OSCAL) artifacts or NIST SP 800-53 compliance alignments.

| Control Area | Implemented at file:line | OSCAL Artifact | Gap |
| :--- | :--- | :--- | :--- |
| **Identification and Authentication (IA-2 / IA-8)** | `crates/op-http/src/request_filters.rs:81-101` | None | The `api_key_auth` filter contains a hardcoded bypass ("For now, allow all requests (authentication is optional)") and lacks mapping to an OSCAL Component Definition specifying required authenticators. |
| **Rate Limiting / DoS Protection (SC-5)** | `crates/op-http/src/request_filters.rs:104-121` | None | The `rate_limit` filter is a mock implementation that only logs and allows all requests ("For now, just log and allow all requests"). No machine-readable threshold policy is enforced. |
| **Cryptographic Protection (SC-13)** | `crates/op-http/src/tls.rs:149-247` | None | Auto-detection of certificates scans hardcoded filesystem locations, including developer-specific personal directories (`/home/jeremy/certs/...`) and system paths, rather than validating against OSCAL-mapped deployment boundaries. |
| **Cryptographic Key / Certificate Validation (SC-12)** | `crates/op-http/src/tls.rs:250-294` | None | Spawns external CLI processes (`Command::new("openssl")`) to parse moduli and expiries, lacking explicit control assertions and error mapping in system authorization policies. |

---

# Recommendations

### Major Gap 1: Insecure Command Spawning for Certificate Validation
* **Location:** `crates/op-http/src/tls.rs:250-294`
* **Vulnerability Analysis:** Spawning external processes using `Command::new("openssl")` is highly inefficient and creates an insecure dependency on system-level binaries. If an attacker can control the path inputs (via the environment variables `SSL_CERT_PATH` or `SSL_KEY_PATH`), they may manipulate parameters passed to `openssl` or execute unexpected command behaviors.
* **Remediation:** Replace external process spawning with native library calls. Use the `openssl` or `rustls` Rust bindings to extract the public modulus, check expiration details, and validate certificate subjects programmatically without invoking external CLI shells.

### Major Gap 2: Personal Local Development Paths in Production Code
* **Location:** `crates/op-http/src/tls.rs:181-184`
* **Vulnerability Analysis:** Hardcoding user-specific paths such as `/home/jeremy/certs/cloudflare_origin.pem` introduces information disclosure (revealing local developer names) and causes runtime fragile failures or privilege boundary crossings in shared multi-tenant execution environments.
* **Remediation:** Remove all personal or environment-specific paths from the auto-detection array. Restrict certificate paths to standard configuration variables or uniform system-wide directory standards (e.g., `/etc/ssl/certs/`).

### Major Gap 3: Bypassed Authentication and Rate Limiting
* **Location:** `crates/op-http/src/request_filters.rs:81-121`
* **Vulnerability Analysis:** The middleware filters are configured as permissive bypasses that log but do not block unauthorized traffic or excessive request volumes. Spawning these services onto network-facing interfaces allows complete access to control-plane endpoints.
* **Remediation:** Turn on strict validation by default inside the `api_key_auth` and `rate_limit` filters. If a key is missing or invalid, reject the connection with `StatusCode::UNAUTHORIZED`. Integrate a functional token-bucket or sliding-window rate limiter utilizing `governor` (which is already present in the workspace dependencies).

### Major Gap 4: Missing Schema-as-Code and OSCAL Controls Mapping
* **Location:** `crates/op-http/src/health.rs:11-26`, `crates/op-http/src/metrics.rs:224-245`
* **Vulnerability Analysis:** Relying on ad-hoc Serde structs and dynamic untyped `simd_json` responses inhibits policy enforcement and versioned API evolution across independent client-server platforms.
* **Remediation:** codify all API, health, and metric schema contracts in Protocol Buffer (`.proto`) files. Map HTTP endpoints to OSCAL System Security Plan (SSP) control parameters, defining machine-readable access policy boundaries for authentication requirements, rate-limiting thresholds, and permitted TLS suites.