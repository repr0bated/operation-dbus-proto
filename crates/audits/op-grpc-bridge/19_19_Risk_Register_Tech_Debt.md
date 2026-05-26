# Production Security and Quality Audit

## Executive Risk Register

| Severity | Issue | Evidence (file:line) | Recommendation |
| :--- | :--- | :--- | :--- |
| **High** | **Memory Safety & Undefined Behavior**: Reading from a raw pointer of a memory-mapped file without verifying that the file length is at least as large as the struct layout. | `crates/op-grpc-bridge/src/interceptor.rs:50` | Verify the file size using metadata checks (e.g., `file.metadata()?.len()`) and ensure that the memory map length (`mmap.len()`) is at least `std::mem::size_of::<IdentitySled>()` before performing raw pointer dereferences. |
| **High** | **Concurrency & Memory Model Violations (Data Tearing)**: Unsynchronized concurrent reads and writes on shared memory map leading to potential authentication bypass or state corruption. | `crates/op-grpc-bridge/src/interceptor.rs:57` | Implement a robust synchronization mechanism, such as using atomic types (e.g., `AtomicBool` and atomic integer fields) or utilizing advisory file locking (e.g., `flock` or `fs2`) to guarantee mutual exclusion during reads and writes. |
| **High** | **Command Hijacking & Argument Injection**: Direct invocation of system binaries without absolute paths, combined with passing client-controlled arguments directly to a sub-process. | `crates/op-grpc-bridge/src/grpc_server.rs:698` | Invoke system binaries using their absolute paths (e.g., `/sbin/dinitctl`) to prevent `PATH` manipulation attacks, and strictly validate the input argument against an alphanumeric whitelist to prevent argument injection. |
| **High** | **Silent Security Bypass (Ignored TLS Configuration)**: Client pool endpoint configuration includes `tls_enabled` but completely ignores it, sending cleartext data over the wire. | `crates/op-grpc-bridge/src/grpc_client.rs:82` | Conditionally configure the `Endpoint` connection channel to use TLS (e.g., `.tls_config(...)` on the Tonic `Endpoint`) if `tls_enabled` is set to `true`. |
| **High** | **OSCAL Compliance Gaps**: Compliance and governance metadata is retrieved dynamically via unversioned, unstructured, and unvalidated environment variables. | `crates/op-grpc-bridge/src/schema_engine.rs:718` | Replace ad-hoc environment variable reads with a typed, validated, and versioned configuration struct compiled from official OSCAL XML or JSON compliance schemas. |
| **High** | **Schema-as-Code Discipline Violations**: Data contracts are expressed as ad-hoc C-repr structs or raw JSON strings rather than typed, versioned Protocol Buffer messages. | `crates/op-grpc-bridge/src/interceptor.rs:18` | Refactor the shared-memory identity sled and dynamic states to utilize structured, versioned Protocol Buffer messages, eliminating raw pointer casts and unstructured JSON passing. |
| **Medium** | **Unrestricted D-Bus Interface Probing**: Unvalidated routing of client-supplied plugin identifiers and object paths to system D-Bus interfaces. | `crates/op-grpc-bridge/src/grpc_server.rs:373` | Apply strict validation to `plugin_id` and restrict `object_path` parsing to a whitelist of allowed schema-cataloged D-Bus endpoints. |

---

## Detailed Findings & Technical Analysis

### 1. Memory Safety & Undefined Behavior in Memory Map Casting
*   **Severity**: High
*   **Evidence**: `crates/op-grpc-bridge/src/interceptor.rs:50-58`
*   **Description**:
    The gRPC interceptor reads validation data from a shared memory file `/dev/shm/plugin_schema.dat`. It maps this file into memory using `memmap2::MmapOptions::new().map(&file)` without validating the file's size:
    ```rust
    let mmap = unsafe {
        MmapOptions::new()
            .map(&file)
            .map_err(|_| Status::internal("Mmap failed"))?
    };
    let sled_ptr = mmap.as_ptr() as *const IdentitySled;

    let is_valid = unsafe { (*sled_ptr).is_valid };
    let current_footprint = unsafe { (*sled_ptr).hashed_footprint };
    ```
    If `/dev/shm/plugin_schema.dat` is empty or has been truncated (e.g., to 1 byte), mapping may succeed but casting it to `*const IdentitySled` (which has an aligned size of 80 bytes) and dereferencing its fields results in a raw out-of-bounds read. This triggers undefined behavior in Rust and will cause a `SIGSEGV` or `SIGBUS` signal, crashing the entire gRPC ingress server (Port 50051) and creating a local Denial of Service (DoS) vulnerability.

### 2. Memory Model Violations & Data Tearing on Shared Memory Ingress
*   **Severity**: High
*   **Evidence**: `crates/op-grpc-bridge/src/interceptor.rs:57-58`
*   **Description**:
    The shared memory map located at `/dev/shm/plugin_schema.dat` is continuously mutated by the `SchemaEngine` on another thread/process and directly read by `ghostbridge_interceptor` via raw pointers:
    ```rust
    let is_valid = unsafe { (*sled_ptr).is_valid };
    let current_footprint = unsafe { (*sled_ptr).hashed_footprint };
    ```
    Because these operations are performed without atomic instructions (e.g., `std::sync::atomic::compiler_fence` or atomic pointer reads), the compiler is free to reorder or optimize these reads. Furthermore, since the write side does not use locked transactions or memory barriers, the interceptor can read a partially-written `IdentitySled`. This results in **data tearing**, allowing an outdated or half-written cryptographic `hashed_footprint` to pass authentication or causing valid requests to be falsely rejected.

### 3. Command Hijacking & Argument Injection in Runtime Mirror
*   **Severity**: High
*   **Evidence**: `crates/op-grpc-bridge/src/grpc_server.rs:698`
*   **Description**:
    In the `RuntimeMirror` service, the implementation of `get_service` spawns a sub-process to run `dinitctl`:
    ```rust
    let name = &request.get_ref().service_name;
    let output = tokio::process::Command::new("dinitctl")
        .args(["status", name])
        .output()
        ...
    ```
    This implementation contains two critical flaws:
    1.  **Command Hijacking**: Invoking `"dinitctl"` as a relative command name means the operating system resolves it using the environment's `PATH` variable. If the gRPC service runs in a shared environment where `PATH` is mutable, an attacker can hijack the binary execution.
    2.  **Argument Injection**: Although `Command::new` avoids shell parsing, passing a completely unvalidated `service_name` (like `--help` or other dinitctl flags) allows a client to control the argument vector passed to the executing process, potentially triggering unsafe execution pathways within the init-system CLI.

### 4. Ignored TLS Configuration in Client Connections
*   **Severity**: High
*   **Evidence**: `crates/op-grpc-bridge/src/grpc_client.rs:82`
*   **Description**:
    The system defines a `RemoteEndpoint` configuration structure containing the `tls_enabled` boolean field. However, in `GrpcClientPool::get_channel`, the connection logic completely ignores this flag when creating the `Endpoint`:
    ```rust
    let endpoint = Endpoint::from_shared(address.to_string())
        .map_err(|e| GrpcClientError::ConnectionFailed(e.to_string()))?
        .connect_timeout(self.default_config.connect_timeout)
        .timeout(self.default_config.request_timeout);
    ```
    Regardless of whether `tls_enabled` is set to `true`, the pool establishes an unencrypted, cleartext gRPC connection over the network. This silently bypasses transit encryption requirements, exposing vectorized payloads, actor IDs, and cryptographic context to network sniffing.

### 5. Gaps in OSCAL Schema-as-Code Compliance
*   **Severity**: High
*   **Evidence**: `crates/op-grpc-bridge/src/schema_engine.rs:718-725`
*   **Description**:
    The system reads compliance and control metadata from dynamic environment variables inside the authoritative mutation path:
    ```rust
    let uuid          = std::env::var("SCHEMA_UUID").unwrap_or_default();
    let subid         = std::env::var("SCHEMA_SUBID").unwrap_or_default();
    let ctrl          = std::env::var("SCHEMA_CONTROL_SOURCE")
                            .unwrap_or_else(|_| "NIST_SP_800_53_R5".into());
    ```
    These are then passed to `write_sled_full`. Utilizing raw, unvalidated environment string variables to define crucial compliance parameters completely violates the OSCAL schema-as-code discipline. A missing or mutated environment variable can lead to malformed compliance reports or render the audit log non-compliant with standard schemas (such as FedRAMP/NIST SP 800-53).

### 6. Violations of Schema-as-Code Discipline
*   **Severity**: High
*   **Evidence**: `crates/op-grpc-bridge/src/interceptor.rs:18`
*   **Description**:
    The core connection gatekeeping data contract (`IdentitySled`) is modeled as an ad-hoc Rust struct with custom C alignment representation (`#[repr(C)]`):
    ```rust
    #[repr(C)]
    pub struct IdentitySled {
        pub wireguard_pubkey: [u8; 32],
        pub mutation_index: u64,
        pub is_valid: bool,
        pub hashed_footprint: [u8; 32],
    }
    ```
    Furthermore, many interfaces (such as `grpc_client.rs:208` and `grpc_server.rs:448`) leverage unversioned JSON structures (`simd_json::OwnedValue`) and ad-hoc string concatenation (`format!("[\"{}\", {}]", db, ops)`) to pass system state instead of relying on strictly defined and versioned Protocol Buffer schema messages. This leads to high fragility across system updates, architectural mismatches, and parsing errors.

### 7. Unrestricted D-Bus Property Probing
*   **Severity**: Medium
*   **Evidence**: `crates/op-grpc-bridge/src/grpc_server.rs:373`
*   **Description**:
    The D-Bus synchronization handlers `get_property` and `set_property` format the D-Bus destination and read property signatures directly from client-controlled request objects:
    ```rust
    let proxy = zbus::fdo::PropertiesProxy::builder(&connection)
        .destination(format!("org.opdbus.{}.v1", req.plugin_id))
        .map_err(|e| Status::internal(e.to_string()))?
        .path(req.object_path.as_str())
    ```
    There is no restriction on what D-Bus endpoints can be targeted. Any client passing the initial interceptor footprint check can execute arbitrary property gets/sets on any service on the system bus matching the `org.opdbus.*.v1` prefix. This allows clients to probe for internal properties and interfaces that are not part of their authorized schema definitions.

---
## ⚠ Citation Warnings
- `crates/op-grpc-bridge/src/schema_engine.rs:718`: file has 569 lines
- `crates/op-grpc-bridge/src/schema_engine.rs:718`: file has 569 lines
