# Production Security & Quality Audit: op-grpc-bridge

---

## 1. Executive Summary

This audit evaluates the quality, safety, and architectural integrity of the `op-grpc-bridge` crate. The codebase bridges D-Bus local IPC with external gRPC interfaces, utilizing a zero-copy shared memory layout (`IdentitySled`) for nanosecond-level validation.

The audit revealed **two Critical security vulnerabilities** in the shared-memory gRPC interceptor that lead to direct Undefined Behavior (UB), memory corruption, and remote Denial of Service (DoS) crashes. Furthermore, several architectural violations of the **Schema-as-Code** discipline were identified, where contracts are expressed as ad-hoc JSON constructions or raw string formatting rather than compiled, versioned schemas.

---

## 2. Critical Security Vulnerabilities

### Finding 1: Out-of-Bounds Memory Read via Unvalidated Mmap Length (SIGSEGV / Denial of Service)
*   **Classification**: Critical
*   **Location**: `crates/op-grpc-bridge/src/interceptor.rs:42-53`
*   **Mechanism**:
    The gRPC interceptor maps `/dev/shm/plugin_schema.dat` into virtual memory and immediately casts the raw pointer to a `*const IdentitySled` to read authorization states:
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
    The interceptor fails to verify that `mmap.len()` is greater than or equal to `std::mem::size_of::<IdentitySled>()` (which is at least 73 bytes). 
    If the shared memory file has been truncated, is newly initialized, or is empty (0 bytes), `mmap.map()` may succeed (or fail depending on OS page size limits, but typically maps 0 bytes if empty), and dereferencing `sled_ptr` will read out-of-bounds virtual memory.
*   **Exploitability**:
    Any unauthenticated remote user sending a gRPC request on port `50051` while `/dev/shm/plugin_schema.dat` is empty or truncated will trigger an immediate segmentation fault (`SIGSEGV`), crashing the entire gRPC gateway service and causing a complete Denial of Service.
*   **Recommendation**:
    Verify the length of the memory map before casting:
    ```rust
    if mmap.len() < std::mem::size_of::<IdentitySled>() {
        return Err(Status::internal("Invalid Identity Sled size in shared memory"));
    }
    ```

### Finding 2: Undefined Behavior via Unsynchronized Shared Memory Data Race
*   **Classification**: Critical
*   **Location**: `crates/op-grpc-bridge/src/interceptor.rs:52-53` and `crates/op-grpc-bridge/src/schema_engine.rs:395-416`
*   **Mechanism**:
    The gRPC interceptor (running on multiple concurrent Tokio threads) reads `is_valid` and `hashed_footprint` directly from the raw pointer `sled_ptr` without any atomic ordering, memory barriers, or locking.
    Concurrently, the `SchemaEngine` updates the very same shared memory file on mutations via `write_sled_full` inside `crates/op-grpc-bridge/src/schema_engine.rs:395`:
    ```rust
    if let Err(e) = write_sled_full(
        &footprint_hex,
        change.event_id,
        &uuid, &subid, &ctrl, &ctrl_refs, &stmt_refs, &nextdns,
    ) { ... }
    ```
    Because writes and reads to the non-atomic fields (`is_valid: bool`, `hashed_footprint: [u8; 32]`) happen concurrently across threads without synchronization, this is a **classic data race**. In Rust, concurrent unsynchronized read/write access to the same memory location is instant **Undefined Behavior**.
*   **Exploitability**:
    During high-throughput mutation workloads, the compiler is free to optimize registers or split reads of the 32-byte `hashed_footprint` array. The interceptor may read a partially written (split) footprint, leading to spurious authorization failures, or the compiler may cache the read values illegally. Additionally, a split read could allow an attacker to bypass validation if a partial match occurs.
*   **Recommendation**:
    Replace the raw C-struct casting with a memory layout of atomic types (e.g., using `std::sync::atomic` primitives for variables) or wrap the shared memory access with a cross-process file-lock (`flock` / `fs2::FileExt::lock_shared`).

---

## 3. Schema-As-Code Violations

The following locations express data contracts as ad-hoc strings or unstructured JSON values rather than versioned Protocol Buffers or OSCAL schemas:

### 1. Ad-Hoc JSON Payload Construction for D-Bus Calls
*   **Location**: `crates/op-grpc-bridge/src/grpc_server.rs:511-518` (and repeating across mail/privacy methods)
*   **Violation**: Data contracts are modeled using ad-hoc, string-keyed JSON structures:
    ```rust
    let args = simd_json::json!({
        "from": req.from_email,
        "to": req.to_email,
        "subject": req.subject,
        "body": req.body,
        "is_html": req.is_html,
        "domain": req.domain
    });
    ```
    Instead of using versioned Protobuf messages to interact with the system's local subsystems, the codebase relies on unvalidated, string-literals formatted at runtime. If fields change, compiler checks will pass, but runtime failures will occur.

### 2. Environment-Variable-Based OSCAL Inputs
*   **Location**: `crates/op-grpc-bridge/src/schema_engine.rs:400-406`
*   **Violation**: Compliance controls (OSCAL metadata) are read as ad-hoc strings from raw environment variables:
    ```rust
    let uuid          = std::env::var("SCHEMA_UUID").unwrap_or_default();
    let subid         = std::env::var("SCHEMA_SUBID").unwrap_or_default();
    let ctrl          = std::env::var("SCHEMA_CONTROL_SOURCE")
                            .unwrap_or_else(|_| "NIST_SP_800_53_R5".into());
    ```
    These compliance contracts must be defined within structured, schema-validated OSCAL JSON/YAML profiles, not injected as unvalidated environment strings.

### 3. Manual String Concatenation for Protobuf Generation
*   **Location**: `crates/op-grpc-bridge/src/proto_gen.rs:50-70`
*   **Violation**: The Protobuf generator generates dynamic schemas using ad-hoc string formatting (`writeln!(output, ...)`) rather than using an Abstract Syntax Tree (AST) parser or an official Protocol Buffer descriptor compiler. This makes schema generation highly fragile and prone to indentation or syntax mismatches.

---

## 4. Public API Surface & Dead Code

### 4.1. Public API Enumeration
*   **Total Public Items**: 57

#### Top 10 Most Impactful Public Items
| Item | Type | File Path | Line Number | Impact Description |
| :--- | :--- | :--- | :--- | :--- |
| `ghostbridge_interceptor` | Function | `crates/op-grpc-bridge/src/interceptor.rs` | 35 | Enforces ingress security validation on port 50051. |
| `run_grpc_server` | Function | `crates/op-grpc-bridge/src/grpc_server.rs` | 188 | Initialized and binds the monolithic gRPC services. |
| `SchemaEngine` | Struct | `crates/op-grpc-bridge/src/schema_engine.rs` | 59 | Primary coordinator for state modifications and logs. |
| `IdentitySled` | Struct | `crates/op-grpc-bridge/src/interceptor.rs` | 19 | Zero-copy shared memory layout used for authorization. |
| `OperationGrpcServer` | Struct | `crates/op-grpc-bridge/src/grpc_server.rs` | 125 | Tonic implementation hosting all sub-services. |
| `GrpcClientPool` | Struct | `crates/op-grpc-bridge/src/grpc_client.rs` | 44 | Load-balanced connection pool for distributed control. |
| `RemoteOperationClient` | Struct | `crates/op-grpc-bridge/src/grpc_client.rs` | 145 | High-level client for remote distributed nodes. |
| `ProtoGenerator` | Struct | `crates/op-grpc-bridge/src/proto_gen.rs` | 37 | Auto-generates Protobuf models from state definitions. |
| `StateChange` | Struct | `crates/op-grpc-bridge/src/schema_engine.rs` | 21 | Struct capturing audit trails for gRPC broadcast. |
| `MutationResult` | Struct | `crates/op-grpc-bridge/src/schema_engine.rs` | 594 | Returned status of transactional database mutations. |

---

### 4.2. Encapsulation Failures (Public Fields)
*   **`IdentitySled` (`crates/op-grpc-bridge/src/interceptor.rs:19`)**:
    Exposes raw representation arrays (`hashed_footprint: [u8; 32]`, `wireguard_pubkey: [u8; 32]`) as public fields. This is acceptable due to raw memory-map requirements, but the lack of helper accessors allows callers to copy internal segments unsafely.
*   **`SchemaEngine` (`crates/op-grpc-bridge/src/schema_engine.rs:59`)**:
    Exposes primary backend components as public fields:
    ```rust
    pub event_chain: Arc<RwLock<EventChain>>,
    pub dbus_connection: Arc<OnceCell<Connection>>,
    pub ovsdb: Arc<OvsdbClient>,
    pub nonnet: Arc<NonNetDb>,
    ```
    This allows external modules to bypass the `SchemaEngine`'s coordinating audit logs (`event_chain`) and execute direct modifications on `ovsdb` or `nonnet` database engines, violating audit-trail guarantees.

---

### 4.3. Dead Code Analysis

#### Allowed Dead Code Attributes
*   **`crates/op-grpc-bridge/src/grpc_client.rs:144`**:
    `#[allow(dead_code)] pub struct RemoteOperationClient` suppresses warnings for an entire client subsystem.

#### Dead Code & Unused Definitions
| Item | Type | File Path & Line | Recommendation |
| :--- | :--- | :--- | :--- |
| `RemoteOperationClient` | Struct | `crates/op-grpc-bridge/src/grpc_client.rs:145` | Remove or expose via public integration tests; no code in this crate uses it. |
| `to_snake_case` | Function | `crates/op-grpc-bridge/src/proto_gen.rs:356` | Remove; the utility function is defined but never called outside inactive test suites. |
| `include_validation` | Field | `crates/op-grpc-bridge/src/proto_gen.rs:17` | Remove or implement; the field in `ProtoGenConfig` is never read in code. |
| `SetStateResult` | Struct | `crates/op-grpc-bridge/src/grpc_client.rs:309` | Remove; only used as return values for dead `RemoteOperationClient` functions. |
| `StateUpdateMessage` | Struct | `crates/op-grpc-bridge/src/grpc_client.rs:315` | Remove; only used by unused remote subscription APIs. |
| `ChainEventMessage` | Struct | `crates/op-grpc-bridge/src/grpc_client.rs:326` | Remove; unused dead client state structure. |