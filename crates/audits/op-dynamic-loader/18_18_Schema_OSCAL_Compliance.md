### 1. Schema-as-Code Table

| Item | Type | file:line | Has .proto? | Gap |
| :--- | :--- | :--- | :--- | :--- |
| `critical_tools` Policy | Hardcoded Policy / Array | `crates/op-dynamic-loader/src/loading_strategy.rs:88` | No | Critical tool classifications are expressed as an ad-hoc, hardcoded list of strings (`["respond_to_user", "cannot_perform", "systemd_status", "file_read", "agent_status"]`) rather than being defined in a versioned schema or database contract. |
| `DynamicLoaderError` | Enum | `crates/op-dynamic-loader/src/error.rs:4` | No | Error states propagated across module and potentially RPC boundaries (such as `ToolNotFound` or `LoadingError`) are defined as ad-hoc Rust types instead of a standardized, versioned schema (e.g., extending a gRPC/Protobuf `google.rpc.Status` contract). |

---

### 2. OSCAL Coverage Table

| Control Area | Implemented at file:line | OSCAL Artifact | Gap |
| :--- | :--- | :--- | :--- |
| **AC-3 (Access Enforcement) / CM-7 (Least Functionality)** | `crates/op-dynamic-loader/src/loading_strategy.rs:88` | None | Hardcoded capability definitions directly bypass normal tool-loading constraints and execution pattern evaluations. This logic determines whether high-privilege tools (e.g. `file_read`, `systemd_status`) are always loaded and prioritized. No corresponding OSCAL `component-definition` maps these rules to NIST SP 800-53 controls. |
| **AU-2 (Event Logging)** | `crates/op-dynamic-loader/src/dynamic_registry.rs:59` | None | The caching mechanism modifies operational tool states (dynamic load, cache hits, and cache evictions) without emitting structured security or audit logs. There is no mapping to trace registry cache statistics adjustments to system security plans (SSP). |

---

### 3. Recommendations

#### Recommendation 1: Move Hardcoded Policy Decisions to Schema-Based Configuration
*   **Target**: `crates/op-dynamic-loader/src/loading_strategy.rs:88-100`
*   **Action**: Avoid hardcoding operational privileges inside compilation units. Define a Protocol Buffer schema (e.g. `tool_policy.proto`) or a formal JSON Schema representing a versioned policy configuration. Implement loading of this configuration at runtime so that changes to "critical" tools do not require code modification, matching the schema-as-code discipline.

#### Recommendation 2: Map Tool Load Rules to OSCAL Component Definition
*   **Target**: `crates/op-dynamic-loader/src/dynamic_registry.rs:11-25`
*   **Action**: Create a machine-readable OSCAL `component-definition` mapping the `op-dynamic-loader` library to NIST SP 800-53 controls **CM-7 (Least Functionality)** and **AC-3 (Access Enforcement)**. Document the exact criteria under which tools are loaded (such as execution history checks and caching policies) to demonstrate compliance with least-privilege automation guidelines.

#### Recommendation 3: Standardize Inter-Crate Error Definitions
*   **Target**: `crates/op-dynamic-loader/src/error.rs:4`
*   **Action**: For error propagation across RPC boundaries, map `DynamicLoaderError` states to a standardized RPC status contract (e.g., using `tonic::Status` or a structured protobuf equivalent). This eliminates the risk of diagnostic drift when error enums are processed by remote gRPC-bridge callers.