# Production Security and Quality Audit: `op-grpc-bridge`

## 1. Executive Summary
This document details the security and quality findings for the `op-grpc-bridge` crate. The audit was conducted under a strict "Schema-as-Code" discipline (requiring versioned Protocol Buffers or OSCAL schemas rather than ad-hoc structs) and Rust systems programming guidelines.

---

## 2. Security Vulnerability Findings

### Finding 1: Lack of File Size Validation on Shared Memory Map Dereference (SIGBUS / Gateway Crash Denial of Service)
*   **File/Line**: `crates/op-grpc-bridge/src/interceptor.rs:47-58`
*   **Severity**: Critical
*   **Description**: 
    The gRPC interceptor `ghostbridge_interceptor` retrieves client metadata and performs a zero-copy direct read from shared memory via `/dev/shm/plugin_schema.dat`. It opens the file, maps it to memory using `MmapOptions::new().map(&file)`, and immediately casts the raw pointer to `*const IdentitySled` to access `is_valid` and `hashed_footprint`.
    
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

    There is no validation of the file size before dereferencing the memory-mapped pointer. If `/dev/shm/plugin_schema.dat` is truncated, corrupted, or has a file size of `0` bytes (e.g., during initialization, a crash of the SchemaEngine, or due to a concurrent local process action), mapping succeeds with size `0` or maps fewer bytes than `std::mem::size_of::<IdentitySled>()`. Dereferencing fields at offsets up to `std::mem::size_of::<IdentitySled>()` (which is at least 81 bytes) will trigger an out-of-bounds page access, resulting in an immediate `SIGBUS` or `SIGSEGV` crash.
*   **Exploitability**: 
    Directly exploitable on any local system where an attacker or concurrent process can write to or truncate `/dev/shm/plugin_schema.dat`. It instantly crashes the primary gRPC ingress daemon on port `50051` upon the next incoming request.
*   **Remediation**:
    Query file metadata and verify that the file size is exactly equal to or greater than `std::mem::size_of::<IdentitySled>()` prior to executing the `mmap.map()` call and casting the pointer:
    ```rust
    let metadata = file.metadata().map_err(|_| Status::internal("Metadata lookup failed"))?;
    if metadata.len() < std::mem::size_of::<IdentitySled>() as u64 {
        return Err(Status::internal("Invalid schema state: Truncated Sled file"));
    }
    ```

---

### Finding 2: Plaintext Transmission of Sensitive Control-Plane Transactions by Default
*   **File/Line**: `crates/op-grpc-bridge/src/grpc_client.rs:34`
*   **Severity**: Medium
*   **Description**: 
    The `RemoteEndpoint` configuration establishes plaintext HTTP without TLS as the default endpoint address:
    ```rust
    impl Default for RemoteEndpoint {
        fn default() -> Self {
            Self {
                address: "http://127.0.0.1:50051".to_string(),
                tls_enabled: false,
                ...
            }
        }
    }
    ```
    If these components are deployed across distributed operation-dbus systems, state changes, sensitive authorization decisions, and private keys (such as WireGuard configurations and private email lookups) are transmitted over unencrypted HTTP.
*   **Remediation**: 
    Enforce `https` and mandate `tls_enabled: true` in the default configurations unless override mechanisms are explicitly activated via local development profiles.

---

## 3. Schema-as-Code Discipline Violations

The codebase has explicit targets to express data contracts as compiled, versioned Protocol Buffers or structured OSCAL schemas, rather than ad-hoc memory-mapped C-structures or raw string-serialized types.

### Finding 3: Ad-Hoc Shared-Memory Cast (`IdentitySled`) Bypassing Versioned Schemas
*   **File/Line**: `crates/op-grpc-bridge/src/interceptor.rs:16-21`
*   **Severity**: Medium
*   **Description**:
    The layout `IdentitySled` is declared as an ad-hoc C-layout struct:
    ```rust
    #[repr(C)]
    pub struct IdentitySled {
        pub wireguard_pubkey: [u8; 32],
        pub mutation_index: u64,
        pub is_valid: bool,
        pub hashed_footprint: [u8; 32],
    }
    ```
    This binary contract maps directly to shared memory but lacks any versioning metadata, schema compilation guarantees, or OSCAL control representations. Changes to fields or alignment (e.g., changes in target architecture or compiler optimization flags) will cause silent, critical alignment corruptions. This should be defined as a structured versioned protobuf schema with precise deserialization boundaries, or explicitly validated against an OSCAL profile.

### Finding 4: Ad-Hoc Struct Definition for State Tracking (`StateChange` & `MutationResult`)
*   **File/Line**: `crates/op-grpc-bridge/src/schema_engine.rs:23` and `crates/op-grpc-bridge/src/schema_engine.rs:490`
*   **Severity**: Medium
*   **Description**:
    `StateChange` and `MutationResult` are implemented as arbitrary internal structs rather than auto-generated models derived from a versioned schema contract. This leads to manual conversion overheads and potential desynchronization between what the gRPC interfaces emit (`ProtoStateChange`) and what the internal engine processes.
*   **Remediation**:
    Regenerate `StateChange` fields using a protobuf message description as the single source of truth.

---

## 4. Documentation & Quality Audit (Role: Docs)

### 4.1 Crate-level Documentation Check
*   **Crate `lib.rs` check**: `crates/op-grpc-bridge/src/lib.rs` contains crate-level `//!` documentation outlining the D-Bus ↔ gRPC Bidirectional Bridge and its architecture. No violations found.

### 4.2 Sample of 10 Public Items Missing Rustdoc Comments
The following items are defined as `pub` but are missing any `///` rustdoc documentation:

1.  **File/Line**: `crates/op-grpc-bridge/src/lib.rs:21`
    ```rust
    pub mod grpc_client;
    ```
2.  **File/Line**: `crates/op-grpc-bridge/src/lib.rs:22`
    ```rust
    pub mod grpc_server;
    ```
3.  **File/Line**: `crates/op-grpc-bridge/src/lib.rs:24`
    ```rust
    pub mod proto_gen;
    ```
4.  **File/Line**: `crates/op-grpc-bridge/src/proto_gen.rs:127`
    ```rust
    pub fn generate_message(&self, output: &mut String, schema: &PluginSchema)
    ```
5.  **File/Line**: `crates/op-grpc-bridge/src/grpc_server.rs:111`
    ```rust
    pub struct OperationGrpcServer
    ```
6.  **File/Line**: `crates/op-grpc-bridge/src/schema_engine.rs:39`
    ```rust
    pub enum ChangeType
    ```
7.  **File/Line**: `crates/op-grpc-bridge/src/schema_engine.rs:56`
    ```rust
    pub struct SchemaEngine
    ```
8.  **File/Line**: `crates/op-grpc-bridge/src/schema_engine.rs:469`
    ```rust
    pub async fn call_dbus_method(...)
    ```
9.  **File/Line**: `crates/op-grpc-bridge/src/schema_engine.rs:500`
    ```rust
    pub struct MutationError
    ```
10. **File/Line**: `crates/op-grpc-bridge/src/schema_engine.rs:506`
    ```rust
    pub enum ErrorCode
    ```

### 4.3 Missing README.md Presence
*   **Status**: Absent. There is no `README.md` file present in the `crates/op-grpc-bridge/` directory, violating standardized documentation policies for internal control-plane crates.

### 4.4 Public Unsafe Functions without Safety Invariant Docs
*   **Status**: Passed. There are no `pub unsafe fn` declarations in the audited files (unsafe blocks are used internally in `interceptor.rs`, but no public function itself exposes an `unsafe` interface). Hence, there are no violations of safety documentation rules for public unsafe interfaces.