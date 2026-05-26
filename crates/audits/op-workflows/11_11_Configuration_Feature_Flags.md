# Production Security and Quality Audit: Configuration

## 1. Environment Variable Reads (`std::env::var`)

No direct reads of environment variables using `std::env::var` or `dotenvy` were found within the provided `crates/op-workflows` source files.

---

## 2. Environment Variables with No Default and No Error Handling

No environment variables are directly read or configured in the `op-workflows` crate codebase. Thus, no unhandled or default-less environment variables exist within this crate.

---

## 3. Cargo Features and Additivity

### `crates/op-workflows/Cargo.toml`
* **Features Defined:** None. The crate does not declare a `[features]` block. All of its dependencies are mandatory when the crate is compiled.

### Workspace Root `Cargo.toml`
The workspace root package (`op-dbus`) defines the following features:
* `default = ["grpc"]`
* `grpc = []`

### Additivity Analysis
The workspace-level features are **additive**. In Rust, Cargo features are designed to be strictly additive; enabling a feature like `grpc` simply adds the corresponding dependencies and enables the gRPC interface without disabling or altering other compilation paths destructively.

---

## 4. Hardcoded Paths, Ports, and Addresses

The following hardcoded paths were identified:

### Hardcoded Configuration Paths
* **`crates/op-workflows/src/builtin/definitions.rs:31`**: The `cargo_check_workflow` has a hardcoded configuration path `.` for the `"path"` parameter of `"tool:cargo_check"`.
* **`crates/op-workflows/src/builtin/definitions.rs:39`**: The `cargo_check_workflow` has a hardcoded configuration path `.` for the `"path"` parameter of `"tool:cargo_clippy"`.
* **`crates/op-workflows/src/builtin/definitions.rs:47`**: The `cargo_check_workflow` has a hardcoded configuration path `.` for the `"path"` parameter of `"tool:cargo_fmt"`.

*Severity: Low / Informational. These represent mock/default definitions for built-in workflows and do not constitute an active security vulnerability on their own, but they assume execution from the workspace root directory.*

---

## 5. Schema-as-Code Violations

The codebase utilizes ad-hoc structures and dynamic JSON values rather than versioned, strongly-typed schemas (such as Protocol Buffers or OSCAL) for internal and external workflow execution states, variables, and history events.

### Ad-Hoc JSON Value Configurations
* **`crates/op-workflows/src/flow.rs:17`** (`WorkflowDefinition`): The node configuration and overall parameters use unstructured types (e.g. `config: Value` using `simd_json::OwnedValue`).
* **`crates/op-workflows/src/flow.rs:37`** (`WorkflowNodeDef`): Node configuration (`config`) is expressed as an ad-hoc JSON value instead of a versioned schema definition.
* **`crates/op-workflows/src/node.rs:252`** (`config_schema`): Configuration schemas are constructed dynamically as ad-hoc JSON objects rather than compiled from schema definitions.

### Ad-Hoc History Sourcing Contracts
* **`crates/op-workflows/src/history.rs:14`** (`HistoryEvent`): Execution logs and historic snapshots are serialized into custom ad-hoc JSON structures (`Value`).
* **`crates/op-workflows/src/history.rs:33`** (`EventType`): Sourced event types (e.g., `WorkflowExecutionStarted`, `NodeTaskCompleted`, `SignalReceived`) wrap generic `Value` payloads rather than versioned Protobuf contracts.

### Ad-Hoc Execution Contracts
* **`crates/op-workflows/src/orchestrator.rs:52`** (`WorkflowResult`): Output properties use unstructured, non-versioned `simd_json::OwnedValue` dynamic payloads.
* **`crates/op-workflows/src/orchestrator.rs:71`** (`StepResult`): Diagnostic results utilize unstructured strings and arbitrary latency/success fields.

---
## ⚠ Citation Warnings
- `crates/op-workflows/src/node.rs:252`: file has 215 lines
