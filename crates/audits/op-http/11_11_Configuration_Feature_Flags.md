# 1. Environment Variable Reads

Below is the complete list of runtime environment variables read via `std::env::var` within the `op-http` crate:

| Environment Variable | File & Line | Context |
| :--- | :--- | :--- |
| `SSL_CERT_PATH` | `crates/op-http/src/tls.rs:207` | Used to specify the file path of the SSL certificate for HTTPS configuration. |
| `SSL_KEY_PATH` | `crates/op-http/src/tls.rs:208` | Used to specify the file path of the private key for HTTPS configuration. |

### Environment Variables with No Defaults & No Error Handling
* **`SSL_CERT_PATH` and `SSL_KEY_PATH`**: 
  * **Default Values**: No default strings or fallback paths are assigned directly to these environment variables if they fail to resolve.
  * **Error Handling**: The error handling is robust against application crashes. If the variables are missing or cannot be read, the program does not panic. Instead, the `if let (Ok(cert), Ok(key))` check at `crates/op-http/src/tls.rs:206` evaluates to `false` and falls through gracefully to subsequent priority checks (such as Cloudflare, Nginx, and Let's Encrypt default file locations).

---

# 2. Cargo Features & Additive Behavior

Based on the provided configuration files:
* **`crates/op-http/Cargo.toml`**: No explicit `[features]` section is defined for the `op-http` crate. It relies purely on dependencies configured through the workspace.
* **Workspace `Cargo.toml`**:
  * `default = ["grpc"]`
  * `grpc = []`
* **Additive Behavior**: In Cargo, features are strictly **additive**. Activating a feature can only add dependencies or enable conditional compilation blocks (e.g., via `#[cfg(feature = "...")]`); it cannot subtract functionality or configuration.

---

# 3. Hardcoded Paths, Ports, and Addresses

The following table documents all hardcoded local system paths, domain names, network ports, and binding addresses discovered in the source:

### Network Bindings and Ports
| Hardcoded Value | File & Line | Context / Purpose |
| :--- | :--- | :--- |
| `8080` (Port) | `crates/op-http/src/server.rs:43` | Default HTTP port in `ServerConfig::default()`. |
| `8443` (Port) | `crates/op-http/src/server.rs:44` | Default HTTPS port in `ServerConfig::default()`. |
| `"0.0.0.0"` (IP) | `crates/op-http/src/server.rs:45` | Default bind host interface in `ServerConfig::default()`. |
| `8080` (Port) | `crates/op-http/src/server.rs:189` | Default HTTP port in `HttpServerBuilder::new()`. |
| `8443` (Port) | `crates/op-http/src/server.rs:190` | Default HTTPS port in `HttpServerBuilder::new()`. |
| `"0.0.0.0"` (IP) | `crates/op-http/src/server.rs:188` | Default bind host interface in `HttpServerBuilder::new()`. |
| `":443"` (Port) | `crates/op-http/src/request_filters.rs:25` | Used to inspect host headers to decide whether to append Strict-Transport-Security (HSTS). |

### System Paths and Domains (TLS Auto-Detection Stack)
| Hardcoded Path / Domain | File & Line | Context / Severity |
| :--- | :--- | :--- |
| `/etc/ssl/cloudflare/origin.pem` | `crates/op-http/src/tls.rs:218` | Cloudflare origin certificate location. |
| `/etc/ssl/cloudflare/origin.key` | `crates/op-http/src/tls.rs:219` | Cloudflare origin private key location. |
| `/etc/ssl/cloudflare/cert.pem` | `crates/op-http/src/tls.rs:221` | Cloudflare origin cert alternative location. |
| `/etc/ssl/cloudflare/key.pem` | `crates/op-http/src/tls.rs:222` | Cloudflare origin key alternative location. |
| `/etc/cloudflare/origin.pem` | `crates/op-http/src/tls.rs:224` | Alternative Cloudflare directory cert. |
| `/etc/cloudflare/origin.key` | `crates/op-http/src/tls.rs:225` | Alternative Cloudflare directory key. |
| `/etc/cloudflare/cert.pem` | `crates/op-http/src/tls.rs:226` | Alternative Cloudflare cert. |
| `/etc/cloudflare/key.pem` | `crates/op-http/src/tls.rs:227` | Alternative Cloudflare key. |
| `/etc/ssl/cloudflare/ghostbridge.tech/cert.pem` | `crates/op-http/src/tls.rs:230` | Domain-specific Cloudflare cert. |
| `/etc/ssl/cloudflare/ghostbridge.tech/key.pem` | `crates/op-http/src/tls.rs:231` | Domain-specific Cloudflare key. |
| `/home/jeremy/certs/cloudflare_origin.pem` | `crates/op-http/src/tls.rs:234` | **Security Risk**: Hardcoded path to a specific developer's home directory. |
| `/home/jeremy/certs/cloudflare_origin.key` | `crates/op-http/src/tls.rs:235` | **Security Risk**: Hardcoded path to a specific developer's private key. |
| `/etc/nginx/ssl/ghostbridge.crt` | `crates/op-http/src/tls.rs:250` | Nginx SSL certificate location. |
| `/etc/nginx/ssl/ghostbridge.key` | `crates/op-http/src/tls.rs:251` | Nginx SSL private key location. |
| `/etc/nginx/ssl/proxmox.crt` | `crates/op-http/src/tls.rs:253` | Proxmox SSL certificate. |
| `/etc/nginx/ssl/proxmox.key` | `crates/op-http/src/tls.rs:253` | Proxmox SSL private key. |
| `/etc/nginx/ssl/server.crt` | `crates/op-http/src/tls.rs:254` | Generic server certificate. |
| `/etc/nginx/ssl/server.key` | `crates/op-http/src/tls.rs:254` | Generic server private key. |
| `/etc/nginx/ssl/cloudflare.crt` | `crates/op-http/src/tls.rs:255` | Cloudflare server certificate. |
| `/etc/nginx/ssl/cloudflare.key` | `crates/op-http/src/tls.rs:256` | Cloudflare server private key. |
| `"ghostbridge.tech"` | `crates/op-http/src/tls.rs:266` | Let's Encrypt target domain. |
| `"proxmox.ghostbridge.tech"` | `crates/op-http/src/tls.rs:267` | Let's Encrypt target domain. |
| `"op-web.ghostbridge.tech"` | `crates/op-http/src/tls.rs:268` | Let's Encrypt target domain. |
| `/etc/letsencrypt/live/{}/fullchain.pem` | `crates/op-http/src/tls.rs:277` | Let's Encrypt template path. |
| `/etc/letsencrypt/live/{}/privkey.pem` | `crates/op-http/src/tls.rs:278` | Let's Encrypt template path. |
| `/etc/pve/nodes/{}/pve-ssl.pem` | `crates/op-http/src/tls.rs:286` | Proxmox Virtual Environment node template. |
| `/etc/pve/nodes/{}/pve-ssl.key` | `crates/op-http/src/tls.rs:287` | Proxmox Virtual Environment node template. |
| `/etc/ssl/certs/ssl-cert-snakeoil.pem` | `crates/op-http/src/tls.rs:296` | System fallback snakeoil certificate. |
| `/etc/ssl/private/ssl-cert-snakeoil.key` | `crates/op-http/src/tls.rs:297` | System fallback snakeoil private key. |
| `/etc/ssl/certs/localhost.pem` | `crates/op-http/src/tls.rs:299` | Localhost fallback certificate. |
| `/etc/ssl/private/localhost.key` | `crates/op-http/src/tls.rs:300` | Localhost fallback private key. |

---

# 4. Schema-as-Code Violations

The codebase enforces a strict schema-as-code discipline using Protocol Buffers and OSCAL schemas. However, several data contracts in `op-http` are expressed as ad-hoc, unversioned Rust structs or dynamically built strings:

### 1. Ad-hoc Health Check Response Contracts
* **File & Line**: `crates/op-http/src/health.rs:12-28`
* **Violation**: The structures `HealthResponse` and `ServiceHealth` are designed for external serialization to JSON over HTTP. Rather than referencing a shared, versioned schema, they are declared as ad-hoc Serde-serializable structs. Any alteration to these fields risks breaking external monitoring integrations.

### 2. Dynamically Assembled JSON Metrics
* **File & Line**: `crates/op-http/src/metrics.rs:188-204`
* **Violation**: The `json_metrics` endpoint manually constructs response objects on-the-fly using the `simd_json::json!` macro. There is no typed schema or static layout definition matching these JSON contracts, making client integration highly fragile and prone to runtime typing errors.

---

# 5. Security & Quality Findings

### [CRITICAL] Protocol Corruption and Denial of Service in Compression Middleware
* **File & Line**: `crates/op-http/src/request_filters.rs:118-128`
* **Vulnerability Type**: Protocol Mismatch / Denial of Service
* **Description**: The native `compression` middleware intercepts the response, inserts the `Content-Encoding: gzip` header, and passes the payload directly to the client *without actually compressing* the response body.
* **Exploitation Impact**: Clients (browsers, command-line utilities, or API gateways) receiving this response will attempt to decompress the uncompressed text/JSON stream using the gzip algorithm. This will throw immediate decompression errors (e.g., invalid gzip header), causing the client to reject the payload and resulting in a total Denial of Service for any routes protected by this filter.

```rust
pub async fn compression(
    request: Request,
    next: Next,
) -> Response {
    // Add compression headers
    let mut response = next.run(request).await;

    let headers = response.headers_mut();
    headers.insert("Content-Encoding", "gzip".parse().unwrap()); // BUG: Header is added, but no compression is performed on body!

    response
}
```

### [HIGH] Security Bypass in API Authentication and Rate Limiting
* **File & Line**: 
  * `crates/op-http/src/request_filters.rs:77-101` (`api_key_auth`)
  * `crates/op-http/src/request_filters.rs:105-115` (`rate_limit`)
* **Vulnerability Type**: Missing Functional Implementation (CWE-276)
* **Description**: The security filter functions parse client API keys, Authorization headers, and real IPs, but then fail to enforce any security bounds. They contain hardcoded comments: `"For now, allow all requests (authentication is optional)"` and `"For now, just log and allow all requests"`.
* **Exploitation Impact**: Developers integrating these filters into their routing tables may falsely believe their endpoints are protected against unauthorized access and brute-force traffic. Since the middleware always permits request propagation, endpoints remain completely exposed.

### [MEDIUM] Information Leak and Potential Privilege Hijack via Developer Paths
* **File & Line**: `crates/op-http/src/tls.rs:234-235`
* **Vulnerability Type**: Weak Hardcoded Configuration Reference
* **Description**: The automatic SSL certificate auto-detection stack checks `/home/jeremy/certs/cloudflare_origin.pem` and `/home/jeremy/certs/cloudflare_origin.key`.
* **Exploitation Impact**: This exposes developer usernames and machine directory structure to anyone auditing the codebase. Furthermore, if the production daemon is run on a shared multi-user environment where a local user can pre-create or write to `/home/jeremy`, they can supply rogue certificates or trap private keys to intercept TLS connections.

### [MEDIUM] Clock Manipulation Panics via Blind Unwrapping
* **File & Line**: `crates/op-http/src/health.rs:66`, `76`, `81`
* **Vulnerability Type**: Denial of Service via Panics (CWE-248)
* **Description**: The health check metrics call `.duration_since(UNIX_EPOCH).unwrap()`.
* **Exploitation Impact**: If the system clock starts uninitialized (e.g., in a bare-metal container before NTP syncs) or is intentionally altered to a time prior to the UNIX epoch (January 1, 1970), calling any health routing endpoint will trigger an immediate panic, crashing the entire service thread.