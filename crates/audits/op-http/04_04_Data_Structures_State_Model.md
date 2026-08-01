### 1. Data Structures and State Audit

#### Concurrency and Reference-Counting Counts

| File | `Arc` | `Rc` | `RefCell` | `RwLock` | `Mutex` | `OnceCell` | `.clone()` Calls |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| `crates/op-http/src/health.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 1 |
| `crates/op-http/src/lib.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-http/src/metrics.rs` | 11 | 0 | 0 | 2 | 0 | 0 | 1 |
| `crates/op-http/src/middleware.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 2 |
| `crates/op-http/src/request_filters.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 2 |
| `crates/op-http/src/router.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-http/src/server.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 3 |
| `crates/op-http/src/tls.rs` | 1 | 0 | 0 | 0 | 0 | 0 | 2 |

#### Large Structs (> 5 Public Fields)
* **`MiddlewareConfig`** (`crates/op-http/src/middleware.rs:22`):
  Has 7 public fields: `cors_enabled`, `cors_origins`, `tracing_enabled`, `compression_enabled`, `timeout`, `security_headers`, and `request_logging`.

#### Globally Mutable State
* **`GLOBAL_METRICS`** (`crates/op-http/src/metrics.rs:274-276`):
  Declared inside a `lazy_static!` block as a globally shared `Arc<Metrics>`. It contains a nested `RwLock<HashMap<String, ServiceMetrics>>` to manage dynamically registered services, allowing global concurrent mutation across thread boundaries.

---

### 2. Schema-As-Code Violations

The codebase expresses several key data contracts as ad-hoc, manually derived Serde structs or inline dynamic structures rather than formal, versioned Protocol Buffer or OSCAL schemas:

* **`HealthResponse` & `ServiceHealth`** (`crates/op-http/src/health.rs:11`, `crates/op-http/src/health.rs:21`):
  These structures define the JSON data contracts for health check monitoring endpoints. Instead of being generated from a central, versioned API definition (such as Protobuf), they are implemented as ad-hoc Rust structs.
* **JSON Metrics Response** (`crates/op-http/src/metrics.rs:204-221`):
  The `json_metrics` handler builds an untyped JSON payload dynamically using the `simd_json::json!` macro. This results in an implicit, unversioned API boundary that is fragile and prone to breaking changes without schema enforcement.

---

### 3. Security and Quality Findings

#### High: Authentication Bypass / No-Op Implementation in API Key Middleware
* **Location**: `crates/op-http/src/request_filters.rs:71-92`
* **Description**: The `api_key_auth` middleware parses potential API keys from the incoming request's headers, but ultimately calls `next.run(request).await` regardless of whether the key is missing, empty, or completely invalid.
* **Impact**: Any router or endpoint that adds this middleware to its execution stack will allow unauthenticated access to resources under the false assumption that access control is being enforced.

#### Medium: Sandbox Escape Risk via Hardcoded Home Directory in Certificate Auto-Detection
* **Location**: `crates/op-http/src/tls.rs:163-166`
* **Description**: The `detect_certificates` function attempts to locate Cloudflare Origin certificates. It includes a hardcoded development path: `"/home/jeremy/certs/cloudflare_origin.pem"` and `"/home/jeremy/certs/cloudflare_origin.key"`.
* **Impact**: On a shared UNIX system or a multi-tenant platform, any user who can gain control of or create the `/home/jeremy` path can place their own malicious certificates or private keys there. This path could then be implicitly trusted by the system during auto-detection fallback sequences.

#### Medium: Denial of Service Risk via Command Subprocess Execution for OpenSSL Modulus Matching
* **Location**: `crates/op-http/src/tls.rs:230-244`
* **Description**: The utility function `validate_cert_key_match` invokes the system's external `openssl` binary via `std::process::Command` to compare certificate and key moduli:
  ```rust
  let cert_output = Command::new("openssl")
      .args(["x509", "-in", cert_path, "-noout", "-modulus"])
      .output()
  ```
* **Impact**: Running blocking command-line subprocesses under an async environment blocks the executor threads. If certificate paths are misconfigured, point to named pipes, or are infinitely blocking streams, the calling thread will lock up indefinitely. Additionally, this introduces a hard runtime dependency on the presence of a globally configured `openssl` binary on the host platform.

#### Low: Panic Hazard via System Time Inversion
* **Location**: `crates/op-http/src/health.rs:81-89`, `crates/op-http/src/health.rs:65`
* **Description**: The `check_health` function calculates timestamps using `SystemTime::now().duration_since(UNIX_EPOCH).unwrap()`.
* **Impact**: The use of `.unwrap()` on `duration_since` will result in an immediate runtime panic if the host system's wall clock is synchronized (e.g., via NTP) to a point in time prior to the UNIX Epoch (or prior to server start time, during uptime calculation).

#### Low: Misleading Static Health Reporting (False Positives)
* **Location**: `crates/op-http/src/health.rs:166-178`
* **Description**: The database health monitoring helper `check_database_health` is a pure placeholder that always returns `"healthy"` with message `"Database connection OK"`, without attempting any database communication.
* **Impact**: Upstream monitors, load balancers, or orchestrators will continuously receive healthy statuses even if database instances are down or unreachable.