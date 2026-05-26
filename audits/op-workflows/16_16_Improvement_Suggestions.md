1. **Suggestion** | Replace ad-hoc Rust structs with versioned Protocol Buffer schemas for data contracts.
**Rationale** | The `WorkflowDefinition` struct, its nested types (`WorkflowNodeDef`, `NodePort`, etc.), and the event sourcing events (`HistoryEvent`, `EventType`) are declared as ad-hoc Rust structures with JSON-based `Value` fields. This introduces schema drift risks and prevents backward/forward compatibility guarantees between distinct services or worker nodes. Because the workspace already includes `prost` and `tonic`, defining these structures as versioned `.proto` files ensures robust compatibility, language interoperability, and schema-as-code discipline.
**Example** | `crates/op-workflows/src/flow.rs:21` and `crates/op-workflows/src/history.rs:13`

2. **Suggestion** | Execute parallelizable ready nodes concurrently instead of sequentially block-waiting in a loop.
**Rationale** | In the workflow execution loop, the engine fetches a batch of independent ready nodes (intended to run in parallel up to `max_parallel`). However, it iterates through them using a synchronous `for node_id in batch` loop and `.await`s each node execution sequentially. This blocks the execution of the entire batch on the slowest node and negates parallel processing benefits. Spawning individual task futures via `tokio::spawn` or executing them concurrently with `FuturesUnordered` would resolve this block.
**Example** | `crates/op-workflows/src/engine.rs:189`

3. **Suggestion** | Use specialized, typed error enums instead of opaque `anyhow::Result` for workflow domain errors.
**Rationale** | Deeply embedded domain operations—such as validation, cycle detection, execution state transitions, and node resolution—all utilize `anyhow::Result` or return generic string-based errors. This prevents calling modules from programmatically matching on failure modes (e.g., distinguishing a validation/validation failure from a transient timeout or deadlock). Defining a custom error enum using `thiserror` would enhance programmatic recoverability.
**Example** | `crates/op-workflows/src/node.rs:133` and `crates/op-workflows/src/flow.rs:205`

4. **Suggestion** | Eliminate JSON cloning overhead inside the execution context by wrapping variable storage values in reference-counted types.
**Rationale** | The `WorkflowContext` manages variables in a `HashMap<String, Value>` (where `Value` is `simd_json::OwnedValue`). Accessing or interpolating these variables clones them directly. For complex datasets, highly nested structures, or large array payloads, frequent cloning incurs massive allocation penalties. Wrapping the variable map or individual values in `Arc` or utilizing copy-on-write `Cow` types would prevent redundant allocations.
**Example** | `crates/op-workflows/src/context.rs:23`

5. **Suggestion** | Persist event-sourced workflow history to a durable embedded key-value store.
**Rationale** | The `WorkflowHistory` holds a sequential, event-sourced stream of historical actions in an in-memory `Vec<HistoryEvent>`. If the hosting process crashes mid-workflow, all execution tracking and event logs are unrecoverably lost. Since `cozo` with `storage-sled` is already a configured workspace dependency, these events should be persisted to Sled or CozoDB to enable reliable state reconstruction and auditing.
**Example** | `crates/op-workflows/src/history.rs:79`

6. **Suggestion** | Apply structured logging fields to execution context log entries rather than raw formatted strings.
**Rationale** | The internal logging mechanisms inside `WorkflowContext` build raw, unstructured string messages. This limits automated analysis, searching, or filtering of logs when integrated with production aggregation stacks. Storing logs using structured key-value pairs (using `tracing::field` metadata) or structured JSON payloads would greatly simplify indexing and observability.
**Example** | `crates/op-workflows/src/context.rs:65`

7. **Suggestion** | Add structured `#[instrument]` tracing spans on workflow and orchestrator execution entries.
**Rationale** | Core entrypoints like `execute_definition` and `execute_sequence` manage complex multi-step dependencies but lack structured `tracing::span` correlation context. Adding instrumentation attributes that automatically capture parent context, unique execution IDs, and trace IDs would allow seamless tracing across service boundaries during nested tool calls.
**Example** | `crates/op-workflows/src/engine.rs:114` and `crates/op-workflows/src/orchestrator.rs:360`