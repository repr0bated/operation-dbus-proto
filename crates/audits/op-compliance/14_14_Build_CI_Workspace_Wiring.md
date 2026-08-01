# Production Security & Quality Audit: op-compliance

## 1. Build & Dependency Analysis

### Cargo.toml & workspace configuration
*   **Edition:** `2021` (configured globally in `Cargo.toml` and locally in `crates/op-compliance/Cargo.toml`).
*   **Rust-version:** Not explicitly set in either the root workspace `Cargo.toml` or the crate `crates/op-compliance/Cargo.toml`.
*   **Bins / Examples:** There are no binary targets or examples defined in `crates/op-compliance/Cargo.toml`.

### Workspace Inheritance vs. Local Overrides
The `op-compliance` crate fails to inherit dependencies from the workspace manifest, violating workspace package discipline:
*   The crate defines explicit, hardcoded version numbers for dependencies rather than using workspace inheritance (e.g., `anyhow = "1"`, `serde_json = "1"`, etc. instead of `anyhow.workspace = true`).
*   There is a version mismatch for `jsonschema`: the workspace relies on version `0.29` (`Cargo.toml`), whereas `op-compliance` uses version `0.18` (`crates/op-compliance/Cargo.toml:10`). This results in dual-compilation of different crate versions in the cargo tree, increasing compilation times and binary footprint.
*   The version of `op-compliance` is hardcoded as `0.1.0` (`crates/op-compliance/Cargo.toml:3`) rather than inheriting `version.workspace = true` from the root workspace manifest which is set to `1.0.0`.

---

## 2. Schema-As-Code Build Check

*   **Tonic / Prost Build Invocation:** There is no `build.rs` present in `crates/op-compliance`, meaning no code generation of protocol buffers occurs during the build step of this crate.
*   **Checked-In `.proto` Sources:** No `.proto` schemas or compiled output files are checked into the `crates/op-compliance` directory.
*   **Runtime Compilation Flag:** The crate compiles its plugin JSON schema structure dynamically at runtime (on every invocation) instead of parsing strongly-typed contracts generated at build time.

---

## 3. Vulnerability & Code Quality Findings

### Finding 1: Denial of Service via Dynamic Schema Recompilation (High Severity)
*   **Citation:** `crates/op-compliance/src/lib.rs:83-86`
*   **Description:** On every execution of `LawFirm::review_schema`, the JSON meta-schema is read, parsed via `serde_json::from_str`, and compiled into a state-machine using `JSONSchema::compile`:
    ```rust
    let meta_schema = include_str!("../../../schemas/opdbus-plugin-schema.json");
    let meta_v: Value = serde_json::from_str(meta_schema)?;

    let compiled = JSONSchema::compile(&meta_v).map_err(|e| anyhow!("Schema error: {}", e))?;
    ```
    Compiling a JSON schema is an expensive operation that performs structural parsing and memory allocation. Compiling it dynamically on every single transaction under high concurrency creates massive CPU and lock-contention overhead. An attacker uploading or triggering repeated schema reviews can easily trigger a Denial of Service (DoS) of the compliance evaluation control loop.
*   **Remediation:** Parse and compile the schema once at startup using a `lazy_static!` or `std::sync::OnceLock` to ensure the compilation cost is paid exactly once at initialization.

---

### Finding 2: Compliance Bypass via Fragile Ad-Hoc Substring Matching (High/Medium Severity)
*   **Citation:** `crates/op-compliance/src/lib.rs:46-51`
*   **Description:** The GDPR engine (`PennyPrivacy::validate_privacy`) attempts to detect sensitive PII fields and check for a corresponding retention policy using case-insensitive substring matching on the serialized string representation of the schema:
    ```rust
    let schema_str = s.to_string().to_lowercase();
    if (schema_str.contains("email")
        || schema_str.contains("user_id")
        || schema_str.contains("phone"))
        && !schema_str.contains("retention")
    ```
    This is highly insecure and trivial to bypass:
    *   **False Negatives (PII Leak):** If an adversary specifies a database field named `usr_id`, `mail`, `cellphone`, or `contact_number`, the ad-hoc checks are completely bypassed, and raw PII can be handled without any GDPR compliance validation.
    *   **False Positives (Bypassing retention check):** If any non-sensitive field name or description text in the schema happens to contain the substring `"retention"` (for example, a field named `"pretention_factor"` or a description with the word `"retention"`), the code returns `Ok(())` even if none of the target PII fields actually have a defined retention policy.
*   **Remediation:** Define data contracts as versioned schemas. Perform structured traversal of the deserialized `Value` nodes or map them into a strongly-typed Rust structure, verifying policy properties on designated schema metadata fields rather than relying on raw string substring searches.

---

### Finding 3: Ad-Hoc Model Violations and Incomplete Transparency Checks (Medium Severity)
*   **Citation:** `crates/op-compliance/src/lib.rs:24-34`
*   **Description:** `EugeneRisk::validate_ai_risk` performs compliance checks only if `plugin_type` is explicitly set to `"custom"`:
    ```rust
    if schema.get("plugin_type").and_then(|v| v.as_str()) == Some("custom") {
        if let Some(meta) = schema.get("schema") {
            if meta.get("model_name").is_some()
                && meta.get("training_data_source").is_none()
            {
                return Err(anyhow!("EU AI Act violation..."));
            }
        }
    }
    ```
    If a plugin leverages an AI model but declares its `plugin_type` as `"service"` (or any other type), it completely evades the AI Act compliance checks. This ad-hoc string-based logic fails to provide a robust audit trail for model provenance.
*   **Remediation:** Require schema designs to encapsulate model components inside structured, strongly-typed contracts so that the validator can reliably identify model fields independent of the outer `plugin_type` designation.

---

### Finding 4: Workspace Package Version and Dependency Out-of-Sync (Low Severity / Code Quality)
*   **Citation:** `crates/op-compliance/Cargo.toml:1-12`
*   **Description:** The root workspace defines common rules for package configuration and standard library dependencies. The `op-compliance` crate ignores these, managing dependencies like `anyhow`, `serde`, `serde_json`, `jsonschema`, and `tracing` locally. This introduces version drift and violates standard workspace build consistency.
*   **Remediation:** Update `crates/op-compliance/Cargo.toml` to inherit versions from the workspace manifest:
    ```toml
    [package]
    name = "op-compliance"
    version.workspace = true
    edition.workspace = true

    [dependencies]
    anyhow.workspace = true
    serde = { workspace = true, features = ["derive"] }
    serde_json.workspace = true
    jsonschema.workspace = true
    tracing.workspace = true
    op-core = { path = "../op-core" }
    ```