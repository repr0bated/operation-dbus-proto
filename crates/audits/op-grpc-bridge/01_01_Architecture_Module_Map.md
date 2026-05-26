# Architecture & Module Map

### Overview
The `op-grpc-bridge` crate serves as the bidirectional control-plane highway between local D-Bus interfaces (local IPC) and remote gRPC endpoints (remote RPC). It synchronizes property changes, signals, and mutations, enforcing audit trail logging and cryptographic integrity checks via an event chain.

### Module Tree
*   **`lib.rs`** (Library entry point)
    *   `grpc_client` — gRPC Client pool and operation proxy for remote gRPC endpoints.
    *   `grpc_server` — Shared-server topology serving `StateSync`, `PluginService`, `EventChainService`, `OvsdbMirror`, and `RuntimeMirror`.
    *   `interceptor` — Gatekeeper gRPC interceptor utilizing a zero-copy memory-mapped file for nanosecond footprint verification.
    *   `proto_gen` — Dynamic protobuf definition generator converting operational schemas to Protobuf descriptors.
    *   `schema_engine` — The authoritative state engine that orchestrates mutations across OVSDB, NonNet databases, and the event chain.
    *   `proto` — Namespace containing the auto-generated tonic Protobuf definitions.

### Entry Points
*   **Library Entry Point**: `crates/op-grpc-bridge/src/lib.rs`
*   **Primary Middleware Hook**: `crates/op-grpc-bridge/src/interceptor.rs:ghostbridge_interceptor`
*   **Primary Server Ingress**: `crates/op-grpc-bridge/src/grpc_server.rs:run_grpc_server`

### Notes
*   This crate is highly performance-sensitive, employing zero-copy pointer casts and `simd-json` parsed data structures.
*   System interactions are accomplished via direct procfs/sysfs parsing and executing `dinitctl` commands.

---

# Production Security Audit Findings

### Finding 1: Out-of-Bounds Memory Read via Missing Shared Memory Size Verification
*   **Severity**: Critical
*   **File**: `crates/op-grpc-bridge/src/interceptor.rs:49-57`
*   **Description**:
    The Tonic middleware `ghostbridge_interceptor` opens and memory-maps `/dev/shm/plugin_schema.dat`. It immediately casts the mapped pointer to `*const IdentitySled` and dereferences its fields without validating that the mapped file size matches or exceeds `std::mem::size_of::<IdentitySled>()`.
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
    ```
*   **Exploitability**:
    An unprivileged local attacker can truncate `/dev/shm/plugin_schema.dat` to a length of 0 bytes or any size smaller than the `IdentitySled` struct. When the next gRPC request hits port `50051`, the interceptor will read out-of-bounds mapped memory, triggering an immediate process crash via a segmentation fault (`SIGSEGV`) or bus error (`SIGBUS`), causing a complete Denial of Service (DoS) of the system gatekeeper.

---

### Finding 2: Undefined Behavior via Invalid `bool` Type Representation
*   **Severity**: Critical
*   **File**: `crates/op-grpc-bridge/src/interceptor.rs:53-55`
*   **Description**:
    In Rust, a `bool` must strictly be represented in memory as `0x00` (false) or `0x01` (true). The interceptor casts raw shared memory bytes from a memory-mapped file directly into the `IdentitySled` structure, which contains the `is_valid: bool` field, and dereferences it.
*   **Exploitability**:
    Since `/dev/shm` is world-writable, any local attacker can write arbitrary bytes (such as `0x02` or `0xFF`) into the offset corresponding to the `is_valid` field. Loading this invalid byte pattern as a Rust `bool` results in immediate Undefined Behavior (UB). The Rust compiler's optimizer assumes `bool` only contains `0` or `1`, which can result in unexpected control-flow bypasses, memory corruption, or unpredictable crashes.

---

### Finding 3: Tonic Gatekeeper Bypass via Insecure Shared Memory Location
*   **Severity**: High
*   **File**: `crates/op-grpc-bridge/src/interceptor.rs:44`
*   **Description**:
    The gatekeeper interceptor relies on `/dev/shm/plugin_schema.dat` as the single source of truth to check the cryptographic footprint of gRPC requests.
*   **Exploitability**:
    The path `/dev/shm` is a world-writable temporary directory (`drwxrwxrwt`). If the `SchemaEngine` does not explicitly secure file ownership and enforce restricted permissions (e.g., `0600`) during initialization, a local attacker can pre-create or modify this file to contain a forged `hashed_footprint` and set `is_valid = true`. This would allow the attacker to spoof authentication, completely bypassing the Tonic gatekeeper's authorization checks on port `50051`.

---

### Finding 4: Data Race / Torn Reads on Memory-Mapped Struct Fields
*   **Severity**: Medium
*   **File**: `crates/op-grpc-bridge/src/interceptor.rs:55-56`
*   **Description**:
    The gatekeeper reads `is_valid` and the 32-byte `hashed_footprint` array from shared memory using standard Rust pointer dereferencing. There are no volatile read operations, memory barriers, or synchronization primitives used.
*   **Exploitability**:
    Concurrent modifications of `/dev/shm/plugin_schema.dat` by the `SchemaEngine` process while the interceptor is reading will trigger a data race (Undefined Behavior in Rust). This can cause torn reads of the 32-byte `hashed_footprint`, resulting in legitimate gRPC connections being dropped sporadically due to transient hash mismatches.

---

### Finding 5: Command Flag Injection in `dinitctl` Process Spawning
*   **Severity**: Medium
*   **File**: `crates/op-grpc-bridge/src/grpc_server.rs:841-848`
*   **Description**:
    The gRPC method `get_service` accepts an unvalidated, user-controlled string `service_name` and passes it directly as an argument to `Command::new("dinitctl")`:
    ```rust
    let name = &request.get_ref().service_name;
    let output = tokio::process::Command::new("dinitctl")
        .args(["status", name])
        .output()
        .await
    ```
*   **Exploitability**:
    Although shell injection is not possible here, if a user passes a `service_name` starting with a hyphen (such as `--help` or other flags supported by `dinitctl`), it will be interpreted as a command-line option rather than a positional argument. To prevent argument/flag injection, the args array must use a double-dash separator to declare the end of flags: `.args(["status", "--", name])`.

---

# Schema-As-Code & Quality Audit Compliance

### Violation 1: Marshal of D-Bus Arguments via Ad-Hoc JSON Objects
*   **File**: `crates/op-grpc-bridge/src/grpc_server.rs:926-933` (and repeated throughout the D-Bus fallback endpoints for mail, privacy, registration).
*   **Description**:
    Instead of using the Protocol Buffer models or a schema catalog configuration, D-Bus parameters are marshaled into unstructured, ad-hoc JSON strings using `simd_json::json!({ ... })` and sent over D-Bus:
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
    This is an ad-hoc contract that lacks version control and compile-time schema validation. Changes to fields on either side of the D-Bus boundary can lead to silent failures, violating the schema-as-code discipline.

---

### Violation 2: Dynamic Protobuf Generation using Raw String Manipulation
*   **File**: `crates/op-grpc-bridge/src/proto_gen.rs:49-335`
*   **Description**:
    The dynamic protobuf generator outputs `.proto` files using raw string buffer writes with `writeln!(output, ...)`.
    ```rust
    writeln!(output, "syntax = \"proto3\";").unwrap();
    writeln!(output).unwrap();
    writeln!(output, "package {};", self.config.package_name).unwrap();
    ```
    Building data schemas through ad-hoc string formatting rather than programmatically manipulating an AST (Abstract Syntax Tree) is error-prone. If the underlying `PluginSchema` contains special characters or malformed fields, the generated output will produce invalid Protobuf syntax that fails parsing at runtime.