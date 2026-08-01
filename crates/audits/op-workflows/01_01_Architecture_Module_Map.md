# ROLE: Architecture & Module Map

This module map describes the `op-workflows` crate, a workflow execution engine designed for Linux system orchestration and agent execution. It supports flow-based programming, event sourcing, task orchestration, and D-Bus integration.

### Overview
*   **Total `.rs` Files**: 13 files
*   **Top-Level Modules**: `builtin`, `context`, `engine`, `flow`, `history`, `node`, `orchestrator`, `workflows`
*   **Bin Targets**: None (configured as a workspace library crate)
*   **Entry Point**: `crates/op-workflows/src/lib.rs`

### Module Tree
The hierarchical structure of the crate and its submodules is as follows:
*   `lib.rs` (Crate Entry Point) [Citation: `crates/op-workflows/src/lib.rs:1`]
    *   `context` (`context.rs`): Manages execution variables, state tracking, and string/value interpolation.
    *   `engine` (`engine.rs`): Executes workflow graphs, schedules nodes, and coordinates node transitions.
    *   `flow` (`flow.rs`): Defines structural workflow topologies, node layouts, and static validations.
    *   `history` (`history.rs`): Implements durable event log tracking using an event-sourcing paradigm.
    *   `node` (`node.rs`): Defines core traits for execution nodes, ports, and default validation behavior.
    *   `orchestrator` (`orchestrator.rs`): Orchestrates capabilities-based multi-tool sequences (workstacks), caching, and optimization tracking.
    *   `workflows` (`workflows.rs`): Implements specialized state machines and managers using the PocketFlow pattern for MCP agents.
    *   `builtin` (`builtin/mod.rs`): Contains concrete standard node definitions:
        *   `dbus_node` (`builtin/dbus_node.rs`): Standard node calling D-Bus interfaces.
        *   `definitions` (`builtin/definitions.rs`): Built-in workflow definitions (e.g., cargo tasks, deployment, security reviews).
        *   `plugin_node` (`builtin/plugin_node.rs`): Standard node wrapping system plugins (query, diff, apply).
        *   `tool_node` (`builtin/tool_node.rs`): Standard node invoking registered external command-line tools.

### Entry Points
*   **Library Interface**: `crates/op-workflows/src/lib.rs` is the primary entry point exposing the public interfaces for the `Orchestrator`, `WorkflowEngine`, `WorkflowContext`, and related traits.

### Notes
*   **Core Dependencies**: Utilizes `simd-json` for high-performance parsing and AST manipulation, `pocketflow_rs` for agent flow states, and the internal `op-core`/`op-tools` workspace dependencies for system capabilities.

---

# PRODUCTION SECURITY & QUALITY AUDIT

## 1. High-Risk Findings

### [High] Non-Deterministic Variable Interpolation via Randomized HashMap Iteration Order
*   **File**: `crates/op-workflows/src/context.rs`
*   **Line(s)**: 112–127
*   **Description**: 
    The `interpolate` method substitutes variables into standard template strings by iterating over the `variables` map:
    ```rust
    for (name, value) in vars.iter() {
        let pattern = format!("${{{}}}", name);
        let replacement = match value {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        result = result.replace(&pattern, &replacement);
    }
    ```
    Rust's `std::collections::HashMap` uses a randomized hash seed by default to prevent HashDoS attacks. This means the iteration order of the variables is non-deterministic between different program executions or even between different read lock acquisitions.
    
    If the value of variable `A` contains a reference to variable `B` (e.g. `A = "${B}"`), the final string's contents depend entirely on whether key `A` or key `B` is processed first in the randomized loop.
    *   *Scenario 1 (A before B)*: `${A}` is replaced by `${B}`, which is then replaced by the value of `B` on the next iteration. This leads to recursive interpolation.
    *   *Scenario 2 (B before A)*: `${B}` is evaluated first, but `${A}` has not yet been substituted. When `A` is subsequently evaluated, `${A}` becomes the literal text `${B}`, leaking variable templates and bypassing intended substitution.

*   **Impact**: 
    Highly unstable runtime behavior, potential security-control bypasses, and data exposure depending on execution-specific hash seeds.
*   **Remediation**:
    Implement a single-pass tokenizer-based template parser (e.g., matching on the regex regex `\$\{([^}]+)\}`) instead of sequential, non-deterministic `replace` passes on a randomized hash map.

---

### [High] Sequential Execution Bottleneck of Supposedly Parallel Nodes
*   **File**: `crates/op-workflows/src/engine.rs`
*   **Line(s)**: 154–189
*   **Description**: 
    The `WorkflowEngine` claims to execute ready nodes in parallel up to a configured threshold:
    ```rust
    // Execute ready nodes (in parallel up to max_parallel)
    let batch: Vec<_> = ready_nodes.into_iter().take(self.max_parallel).collect();

    for node_id in batch {
        // ...
        // Execute
        match node.execute(node_inputs).await { ... }
    }
    ```
    Although the comment asserts parallel execution up to `self.max_parallel`, the subsequent `for` loop synchronously awaits each node's completion futures sequentially inside the current task context.
*   **Impact**: 
    Severe performance degradation. A single slow, blocking, or polling node (such as a system sleep or external API call) completely blocks the progression of all other independent nodes in the active execution batch. This renders the parallel task orchestrator purely sequential.
*   **Remediation**:
    Spawn concurrent asynchronous tasks using `tokio::spawn` or execute the batch concurrently using futures utilities like `futures::future::join_all` or `tokio::task::JoinSet`. Ensure proper lock containment when accessing shared workspace structures.

---

### [High] Plaintext Exposure of Sensitive Secrets in Execution Logs and Event Sourcing History
*   **File**: `crates/op-workflows/src/history.rs` & `crates/op-workflows/src/orchestrator.rs`
*   **Line(s)**: `crates/op-workflows/src/history.rs:52`, `crates/op-workflows/src/history.rs:65`, `crates/op-workflows/src/orchestrator.rs:341-359`
*   **Description**: 
    The workspace records detailed parameter states to an immutable history event trail and tool execution record:
    ```rust
    // crates/op-workflows/src/history.rs:52
    WorkflowExecutionStarted {
        workflow_type: String,
        workflow_id: String,
        inputs: Value,
    },
    // crates/op-workflows/src/history.rs:65
    NodeTaskScheduled {
        node_id: String,
        node_type: String,
        inputs: Value,
    },
    ```
    When `Orchestrator` starts tool execution, it copies the raw, unfiltered JSON input directly to the execution tracker database:
    ```rust
    // crates/op-workflows/src/orchestrator.rs:341
    let exec_record = self
        .execution_tracker
        .start_execution(tool_name, Some(input.clone()), session_id)
        .await;
    ```
    There is no mechanism for masking, redacting, or encrypting sensitive values (such as database credentials, API tokens, D-Bus access keys, or personal identifiers) before they are serialized and persistently written to logs and history databases.
*   **Impact**: 
    Unauthorized disclosure of operational secrets and credential leakage to local audit logs, execution databases, or debugging diagnostic files.
*   **Remediation**:
    Establish a strict secret-masking filter or require nodes to declare sensitive parameters in their `NodePort` schemas (using a `sensitive: true` flag), allowing the engine to redact these values dynamically before telemetry serialization.

---

## 2. Medium-Risk Findings

### [Medium] $O(N)$ Linear Scan Under Write Lock in `IntermediateCache` Eviction
*   **File**: `crates/op-workflows/src/orchestrator.rs`
*   **Line(s)**: 207–215
*   **Description**: 
    The cache implementation for intermediate execution values performs eviction dynamically when insertion requests exceed limits:
    ```rust
    if cache.len() >= self.max_entries {
        if let Some(oldest_key) = cache
            .iter()
            .min_by_key(|(_, v)| v.created_at)
            .map(|(k, _)| k.clone())
        {
            cache.remove(&oldest_key);
        }
    }
    ```
    Calculating the minimum execution timestamp requires a full linear traversal over the entire `cache` map inside an active async write lock (`cache.write().await`).
*   **Impact**: 
    As `max_entries` scales, concurrent write actions will suffer severe thread contention and lock starvation. The active task scheduler will experience high CPU usage and latency spikes during eviction events.
*   **Remediation**:
    Replace the basic `HashMap` with an explicit cache structure designed with $O(1)$ eviction capabilities, such as a double-ended queue paired with a map (LRU Cache), or use the `lru` crate which is already available in the workspace dependencies.

---

### [Medium] Unenforced Cycle Validation on Workflow Definitions
*   **File**: `crates/op-workflows/src/flow.rs`
*   **Line(s)**: 260–261
*   **Description**: 
    The validation suite for `WorkflowDefinition` skips cycle checks, marking it as a placeholder to be completed later:
    ```rust
    // Check for cycles (simple DFS)
    // TODO: Implement proper cycle detection

    Ok(())
    ```
    While the engine's scheduler will successfully catch deadlocks and log a warning ("No nodes ready to execute") if all nodes depend on incomplete predecessors, it cannot identify complex cyclical loops containing partial completions, leading to runtime failures instead of compile/registration-time rejection.
*   **Impact**: 
    Workflows with circular node dependencies are successfully registered as valid definitions, leading to predictable execution deadlock scenarios in production environment pipelines.
*   **Remediation**:
    Implement proper directed acyclic graph (DAG) cycle validation using Kahn’s algorithm or a Depth-First Search (DFS) with node coloring during registration in the `validate` function.

---

### [Medium] Missing Input Type and Schema Enforcement on Node Execution
*   **File**: `crates/op-workflows/src/node.rs`
*   **Line(s)**: 117–130
*   **Description**: 
    The default `validate_inputs` implementation checks only if required input keys are provided or have defaults:
    ```rust
    fn validate_inputs(&self, inputs: &HashMap<String, Value>) -> Result<()> {
        for port in self.inputs() {
            if port.required && !inputs.contains_key(&port.id) {
                if port.default_value.is_none() {
                    return Err(anyhow::anyhow!(
                        "Required input '{}' not provided for node '{}'", ...
                    ));
                }
            }
        }
        Ok(())
    }
    ```
    The validation completely ignores the `data_type` parameter (e.g. `"string"`, `"number"`, `"object"`) declared on the `NodePort`. It passes structurally malformed JSON values directly to node implementations.
*   **Impact**: 
    Shift of type-safety responsibilities entirely to individual nodes, increasing the risk of unhandled runtime panics, type-casting failures, and input injection vectors.
*   **Remediation**:
    Enforce basic type matching of the `simd_json::OwnedValue` variants against the expected `data_type` string inside the default implementation of `validate_inputs`.

---

## 3. Quality & Schema-as-Code Violations

### [Quality] Violation of Schema-as-Code via Ad-Hoc Struct Configurations
*   **File**: `crates/op-workflows/src/flow.rs`, `crates/op-workflows/src/node.rs`, `crates/op-workflows/src/history.rs` & `crates/op-workflows/src/orchestrator.rs`
*   **Line(s)**: `crates/op-workflows/src/flow.rs:18-46`, `crates/op-workflows/src/node.rs:77-101`, `crates/op-workflows/src/history.rs:34-93`, `crates/op-workflows/src/orchestrator.rs:44-59`
*   **Description**: 
    This workspace uses ad-hoc structs and strings to represent core data structures instead of formal, versioned, or standard schema definitions:
    *   `WorkflowDefinition` and `WorkflowNodeDef` are defined as arbitrary JSON structs with nested unstructured configuration values.
    *   `NodePort` defines types as unstructured string parameters (`data_type: String`), rather than typed schema constraints.
    *   `EventType` defines internal events via standard serialization layouts instead of utilizing versioned Protocol Buffers or standardized OSCAL (Open Security Controls Assessment Language) system/component boundaries.
    *   `WorkflowResult` and `StepResult` use ad-hoc fields for execution tracking payloads.
*   **Impact**: 
    Brittle boundary interfaces, lack of cross-language execution capabilities, and zero compliance with formalized system configurations (e.g. OSCAL component tracking).
*   **Remediation**:
    Define workflow templates, step inputs, port data parameters, and structural execution payloads using Protobuf definitions (using the available `prost` workspace dependencies), or express them using versioned JSON schemas validated against an explicit schema repository.