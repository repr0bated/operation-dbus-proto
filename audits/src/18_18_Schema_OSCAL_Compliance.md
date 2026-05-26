### Schema-as-Code Audit

| Item | Type | file:line | Has .proto? | Gap |
| :--- | :--- | :--- | :--- | :--- |
| `op-dbus-model` | Internal Workspace Member | `Cargo.toml:35` | No | Defines control plane data contracts as ad-hoc Rust structs (`sqlx`, `serde_json`, `simd-json`) rather than compiling from versioned Protocol Buffer definitions. |
| `op-compliance` | Internal Workspace Member | `Cargo.toml:37` | No | Expresses validation rules and data contracts using JSON Schema (`jsonschema`) and ad-hoc untyped JSON (`serde_json::Value`) instead of versioned Proto schemas. |
| `quick-xml` | Workspace Dependency | `Cargo.toml:103` | No | Facilitates parsing of XML structures into ad-hoc Rust data structures, bypassing the strict schema-as-code requirement for uniform versioned contract interfaces. |
| `serde_json` | Workspace Dependency | `Cargo.toml:84` | No | Allows processing of untyped JSON payloads (`serde_json::Value`) instead of forcing all communication to be parsed into concrete, version-controlled Protobuf message types. |
| `simd-json` | Workspace Dependency | `Cargo.toml:83` | No | Parses un-versioned ad-hoc JSON representations, presenting an architectural gap where API and contract evolutions cannot be statically checked or validated. |

---

### OSCAL Coverage Audit

| Control Area | Implemented at file:line | OSCAL Artifact | Gap |
| :--- | :--- | :--- | :--- |
| **System and Communications Protection (SC)** / Boundary Protection | `Cargo.toml:5` | None | The `op-gateway` module operates as a control plane border controller, but lacks any formal OSCAL Component Definition mapping its cryptographic protocols (AES-GCM, Argon2, X25519) to NIST SP 800-53 controls. |
| **Assessment, Authorization, and Monitoring (CA)** / Compliance Automation | `Cargo.toml:37` | None | The `op-compliance` crate performs ad-hoc rules validation via JSON Schema but does not ingest, produce, or map outputs to OSCAL System Security Plans (SSP) or Assessment Results (SAR). |
| **Identification and Authentication (IA)** / Device Identification | `Cargo.toml:26` | None | The `op-identity` crate manages host-level identities and cryptographic keys via keyrings and Dalek primitives, but has no machine-readable OSCAL representation validating NIST SP 800-53 IA-family control coverage. |
| **Audit and Accountability (AU)** / Event Logging | `Cargo.toml:105` | None | Tracing and telemetry dependencies are defined, but control plane security actions (such as DBus API method execution or state-store updates) are not mapped to OSCAL control implementations. |

---

### Recommendations

#### 1. Eliminate Ad-Hoc Data Contracts and Migrate to Proto-First Definitions
* **Severity:** Major Gap
* **Location:** `Cargo.toml:84`, `Cargo.toml:103`
* **Finding:** The architecture permits extensive use of untyped JSON (`serde_json`) and quick-xml parsing for internal and external communications. This violates the schema-as-code discipline.
* **Remediation:** 
  1. Mandate that all external and inter-process communication boundaries (including D-Bus and internal RPCs) compile payloads from versioned `.proto` schemas.
  2. Deprecate and restrict the use of `serde_json::Value` or generic JSON schemas within `op-compliance` and `op-dbus-model`.
  3. Integrate `prost-build` and `tonic-build` directly into all boundary crates to enforce that Rust structs are purely code-generated artifacts of schema-as-code contracts.

#### 2. Implement OSCAL Component Definitions for Compliance Automation
* **Severity:** Major Gap
* **Location:** `Cargo.toml:37` (`op-compliance`)
* **Finding:** The `op-compliance` crate attempts compliance checks using basic JSON Schema validation instead of standard-compliant OSCAL models.
* **Remediation:**
  1. Transition the `op-compliance` crate to parse official NIST OSCAL schema documents (specifically the Component Definition and SSP schemas).
  2. Implement validation policies as OSCAL-formatted machine-readable files (JSON/YAML) that catalog the system's compliance status dynamically, matching implemented controls (e.g., cryptographic algorithms configured in `op-gateway`) to concrete NIST SP 800-53 control identifiers.