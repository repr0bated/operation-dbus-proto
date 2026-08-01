# Production Security and Quality Audit

## Async & Concurrency Metrics

* **`async fn` Count**: 0 (No Rust source files provided in the `FILES` section)
* **`tokio::spawn` Count**: 0 (No Rust source files provided in the `FILES` section)
* **`spawn_blocking` Count**: 0 (No Rust source files provided in the `FILES` section)

---

## Findings

### Divergent D-Bus Interoperability Layers (`zbus` Major Version Mismatch)
* **Severity**: High
* **File**: `Cargo.toml:89`
* **Description**: 
  `Cargo.toml:89` specifies the workspace dependency for `zbus` as version `"5.12"`. However, the compiled dependency graph in `Cargo.lock` reveals that multiple control-plane crates (`op-agents`, `op-chat`, `op-introspection`, `op-grpc-bridge`, `op-dbus-mirror`, `op-projection`, `op-services`, `op-state`, `op-state-store`, `op-tools`, `op-web`, and `op-workflows`) are executing on `zbus 4.4.0` while `op-identity` compiles against `zbus 5.13.2`. 
* **Impact**: 
  Because D-Bus message serialization, traits, and types are fundamentally incompatible between major versions (`zbus 4` and `zbus 5`), structures and proxies cannot be reliably shared across crate boundaries within the same address space. This leads to duplicate connection pools, type casting failures, and increased binary bloat.
* **Mitigation**: 
  Standardize all workspace members to use workspace-inherited dependencies. Update the member crates' local `Cargo.toml` files to use `zbus.workspace = true` and update code to compile against the standardized `zbus` 5.x API.

---

### Incompatible Code Generators for Schema-As-Code Pipeline (`prost-build` Duplication)
* **Severity**: High
* **File**: `Cargo.toml:134`
* **Description**: 
  The codebase establishes its Protocol Buffer strategy in `Cargo.toml:134-135` using `prost = "0.13"` and `prost-types = "0.13"`. However, the dependency graph resolved in `Cargo.lock` pulls in both `prost-build 0.12.6` (used by `op-chat` and `op-grpc-bridge`) and `prost-build 0.13.5` (used by `op-cache` and `op-cognitive-mcp`).
* **Impact**: 
  Running multiple major versions of `prost-build` results in incompatible generated code layouts and differing AST generations. This breaks the strict, deterministic "schema-as-code" discipline, as the schema compiler's behavior changes depending on which crate compiles it, generating hard-to-debug deserialization errors or build pipeline failures.
* **Mitigation**: 
  Consolidate all workspace crates to inherit `prost`, `prost-types`, and `prost-build` directly from the workspace level (`Cargo.toml:134-135`). Remove ad-hoc local version declarations of codegen utilities from individual crates.

---

### Cryptographic Configuration Drift via Multiple TLS Versions (`rustls` Duplication)
* **Severity**: Medium
* **File**: `Cargo.toml:166`
* **Description**: 
  `Cargo.toml:166` specifies `rustls = "0.23"` as the workspace standard. However, the resolved dependency graph in `Cargo.lock` forces the coexistence of `rustls 0.21.12` (via older `hyper-rustls 0.24.2` integrations) and `rustls 0.23.36` (via modern `reqwest` and `quinn` components).
* **Impact**: 
  Security parameters, supported cipher suites, default cryptoproviders (such as AWS-LC vs Ring), and certificate validation verification paths differ heavily between `rustls` 0.21 and 0.23. This variation introduces a risk of "cryptographic drift" where different crates validation models diverge, potentially allowing weak ciphers or failing to enforce specific modern TLS configurations across the control plane.
* **Mitigation**: 
  Upgrade all intermediate crates relying on `rustls` 0.21 (such as updating HTTP transport clients from `reqwest 0.11` to `reqwest 0.12` as listed under `Cargo.toml:98`) to fully transition the entire dependency graph to `rustls` 0.23.

---

### Inconsistent Schema Constraints in Policy Verification (`jsonschema` Duplication)
* **Severity**: Medium
* **File**: `Cargo.toml:86`
* **Description**: 
  The workspace defines a unified schema engine in `Cargo.toml:86` as `jsonschema = { version = "0.29", default-features = false }`. However, internal components like `op-compliance` and `op-tools` override this pattern, using `jsonschema 0.18.3`, while `op-dbus` and `op-state-store` use `jsonschema 0.29.1`.
* **Impact**: 
  Divergent JSON Schema specifications can lead to discrepancy vulnerabilities (differential validation attacks). A schema payload containing specific keyword configurations might evaluate to `Valid` under `jsonschema 0.18.3` but get rejected by the newer `0.29.1` engine, causing state synchronization failures or rendering policy evaluations non-deterministic.
* **Mitigation**: 
  Force all compliance and tool evaluation crates to inherit validation rules through `jsonschema.workspace = true` to guarantee identical constraints are applied when verifying payloads.