# Production Security & Quality Audit: op-grpc-bridge

---

### Async & Concurrency Metrics

*   **`async fn` Count**: 81
    *   `crates/op-grpc-bridge/src/grpc_client.rs`: 10
    *   `crates/op-grpc-bridge/src/grpc_server.rs`: 61
    *   `crates/op-grpc-bridge/src/schema_engine.rs`: 10
*   **`tokio::spawn` Count**: 2
    *   `crates/op-grpc-bridge/src/schema_engine.rs:538`
    *   `crates/op-grpc-bridge/src/schema_engine.rs:563`
*   **`spawn_blocking` Count**: 0

---

## Critical Vulnerabilities

### [CRITICAL] Out-of-Bounds Dereference / SIGBUS via Lack of Shared Memory Boundary Checks
*   **File & Line**: `crates/op-grpc-bridge/src/interceptor.rs:49-59`
*   **Mechanics**: 
    The gRPC interceptor opens and memory-maps the file `/dev/shm/plugin_schema.dat` to directly read the binary `IdentitySled` struct:
    ```rust
    let file = File::open("/dev/shm/plugin_schema.dat")
        .map_err(|_| Status::internal("SchemaEngine Memory Unreachable"))?;

    let mmap = unsafe {
        MmapOptions::new()
            .map(&file)
            .map_err(|_| Status::internal("Mmap failed"))?
    };
    let sled_ptr = mmap.as_ptr() as *const IdentitySled;

    let is_valid = unsafe { (*sled_ptr).is_valid };
    let current_footprint = unsafe { (*sled_ptr).hashed_footprint };
    ```
    There is no validation check confirming that the size of the mapped file `mmap.len()` is greater than or equal to `std::mem::size_of::<IdentitySled>()` (which contains 73+ bytes depending on padding). If the shared memory file is truncated or corrupted (for example, containing 0 bytes), dereferencing `sled_ptr` triggers undefined behavior, resulting in an immediate `SIGBUS` or `SIGSEGV` crash.
*   **Exploitability**: 
    **Directly Exploitable**. On Linux platforms, `/dev/shm` is world-writable (`drwxrwxrwt`) by default. Any unprivileged local attacker can execute `truncate -s 10 /dev/shm/plugin_schema.dat` or pre-create a truncated file, instantly causing a Denial of Service (DoS) of the entire high-privileged gRPC control plane by forcing a process crash whenever a new gRPC connection hits port 50051.

### [CRITICAL] Shared Memory Concurrency Data Race (Undefined Behavior)
*   **File & Line**: `crates/op-grpc-bridge/src/interceptor.rs:58-59`
*   **Mechanics**:
    ```rust
    let is_valid = unsafe { (*sled_ptr).is_valid };
    let current_footprint = unsafe { (*sled_ptr).hashed_footprint };
    ```
    The fields `is_valid` and `hashed_footprint` are read from shared memory via standard raw pointer dereferences without utilizing volatile reads (`core::ptr::read_volatile`) or atomic types (e.g., `AtomicBool`). Because the `SchemaEngine` or external processes can concurrently mutate `/dev/shm/plugin_schema.dat` while the gRPC thread is executing the interceptor, this constitutes a data race.
*   **Exploitability**:
    **Directly Exploitable**. The Rust compiler is free to optimize these reads by caching them in registers or reordering instructions, potentially causing the gRPC interceptor to permanently read stale validation values. Additionally, a concurrent write during the read of `hashed_footprint` can result in a "split-read," where the interceptor reads a partially-written, corrupted hash value, resulting in spurious request rejections or validation bypasses.

---

## Security & Architecture Findings

### 1. Synchronous File and Database I/O Blocking the Async Reactor
*   **File & Line**: `crates/op-grpc-bridge/src/interceptor.rs:49-56`
*   **Mechanics**: 
    The function `ghostbridge_interceptor` executes synchronous blocking I/O (`File::open` and `mmap`) on every incoming gRPC metadata check. Although not declared as an `async fn`, this interceptor is executed on the hot path of Tokio worker threads running the Tonic gRPC server.
*   **Impact**:
    Performing disk/shared-memory reads synchronously for every request degrades server throughput from nanoseconds to milliseconds, stalls the Tokio event loop, and can starve other concurrent async connections under high load.
*   **Remediation**:
    Initialize the memory map once when the server starts, wrap the pointer in a thread-safe atomic structure (e.g., `Arc`), and perform direct volatile reads of the mapped structure without re-opening the file on every request.

### 2. Synchronous Sled DB Write Inside Active `async fn`
*   **File & Line**: `crates/op-grpc-bridge/src/schema_engine.rs:608-621`
*   **Mechanics**:
    Inside the critical state-mutation routing path:
    ```rust
    pub async fn mutate(...) -> anyhow::Result<MutationResult> {
        // ...
        if let Err(e) = write_sled_full(
            &footprint_hex,
            change.event_id,
            &uuid, &subid, &ctrl, &ctrl_refs, &stmt_refs, &nextdns,
        ) {
            tracing::warn!("sled write after mutation failed: {}", e);
        }
        // ...
    }
    ```
    The helper function `write_sled_full` executes synchronous disk writes to update the local Sled database. Calling synchronous database writes within `async fn mutate` directly blocks the active Tokio worker thread.
*   **Impact**:
    Stalls the async reactor during NVMe/disk I/O operations, introducing latency spikes during database flushes.
*   **Remediation**:
    Execute the blocking `write_sled_full` call inside a thread pool designed for synchronous work using `tokio::task::spawn_blocking`:
    ```rust
    tokio::task::spawn_blocking(move || {
        write_sled_full(...)
    }).await??;
    ```

---

## Schema-as-Code & Quality Findings

### 1. Ad-Hoc String-Based Protobuf Generation
*   **File & Line**: `crates/op-grpc-bridge/src/proto_gen.rs:114-142`
*   **Mechanics**:
    The `ProtoGenerator` formats Protobuf definitions by manually concatenating raw strings:
    ```rust
    pub fn generate_message(&self, output: &mut String, schema: &PluginSchema) {
        let message_name = to_pascal_case(&schema.name);
        writeln!(output, "message {} {{", message_name).unwrap();
        // ...
        writeln!(output, "  {}{} {} = {};", optional_marker, proto_type, field_name, field_num).unwrap();
        // ...
    }
    ```
    This bypasses structured schemas-as-code principles. It does not utilize an abstract syntax tree (AST) or versioned DSL compiler.
*   **Impact**:
    A single format error or invalid character in the name fields of `PluginSchema` can generate syntactically broken `.proto` files at runtime, which fail downstream compilation without early validation.
*   **Remediation**:
    Use a programmatic Protobuf AST builder or generator (such as `prost-codegen` or a standard schema compiler AST) to construct and validate versioned schemas rather than manipulating ad-hoc strings.

### 2. Extremely Inefficient Triple-Serialization Marshalling
*   **File & Line**: `crates/op-grpc-bridge/src/grpc_client.rs:434-448`
*   **Mechanics**:
    Translating values between Prost (gRPC) and Simd-JSON (internal store) involves serializing the data to an intermediate format:
    ```rust
    fn simd_to_prost_value(value: &simd_json::OwnedValue) -> ProstValue {
        let json = simd_json::to_string(value).unwrap_or_else(|_| "null".to_string());
        let serde_value: serde_json::Value =
            serde_json::from_str(&json).unwrap_or(serde_json::Value::Null);
        serde_to_prost_value(&serde_value)
    }
    ```
    To translate a single payload, this code serializes a SIMD-JSON value to a String, parses it into a Serde-JSON value, and then traverses the tree to construct a Prost `Value`.
*   **Impact**:
    Massive CPU overhead and excessive heap allocations on every single gRPC mutation and property set.
*   **Remediation**:
    Implement a direct conversion from `simd_json::OwnedValue` to `prost_types::Value` recursively without string serialization or intermediate `serde_json::Value` parsing.

### 3. Ad-Hoc Type Coercion via Custom JSON Tags
*   **File & Line**: `crates/op-grpc-bridge/src/grpc_server.rs:945-985`
*   **Mechanics**:
    Dynamic D-Bus types are coerced from JSON using a custom, ad-hoc syntax:
    ```rust
    if let Some(obj) = value.as_object() {
        if let (Some(sig_val), Some(inner)) = (obj.get("sig"), obj.get("value")) {
            if let Some(sig) = sig_val.as_str() {
                return zvariant_from_sig(sig, inner);
            }
        }
    }
    ```
    This manually extracts a string-based type signature `"sig"` to drive type-casting.
*   **Impact**:
    Bypasses versioned schemas and static API contracts, introducing a brittle type system where minor typos in the `"sig"` field lead to runtime serialization errors.
*   **Remediation**:
    Enforce schema typing using a formal Protocol Buffers or OSCAL mapping definition that statically specifies the expected D-Bus signature for every property.

---
## ⚠ Citation Warnings
- `crates/op-grpc-bridge/src/schema_engine.rs:608`: file has 569 lines
