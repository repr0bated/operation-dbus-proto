# OP-GRPC-BRIDGE PRODUCTION SECURITY & QUALITY AUDIT

## 1. Schema-as-Code Audit

All data contracts must be expressed as versioned schemas (such as Protocol Buffer `.proto` files) rather than ad-hoc Rust structs, raw memory representations, or untyped JSON structures. The table below lists identified violations of this discipline.

### Schema-as-Code Violation Table
| Item | Type | file:line | Has .proto? | Gap Description |
| :--- | :--- | :--- | :--- | :--- |
| `IdentitySled` | Raw Rust Struct / Shared Memory Layout | `crates/op-grpc-bridge/src/interceptor.rs:18-24` | **No** | Struct acts as a critical security credential data contract. It has no `.proto` schema or versioning; it is directly mapped from volatile memory (`/dev/shm/plugin_schema.dat`) using unsafe pointer casts. |
| `GenericGetRequest` / `GenericGetResponse` | gRPC Messages | `crates/op-grpc-bridge/src/proto_gen.rs:219-230` | **Dynamic** | Exposes the schema via an untyped `google.protobuf.Struct` instead of nested versioned messages. Allows arbitrary untyped dynamic JSON schemas to bypass gRPC compile-time type verification. |
| `GenericSetRequest` / `GenericSetResponse` | gRPC Messages | `crates/op-grpc-bridge/src/proto_gen.rs:231-245` | **Dynamic** | Employs `google.protobuf.Struct` for `state`, allowing unvalidated, untyped state modifications to bypass static contract guarantees. |
| `GetStateResponse` | Rust-to-gRPC message contract | `crates/op-grpc-bridge/src/grpc_server.rs:327-340` | **Yes** | Emits untyped `ProstStruct` (`google.protobuf.Struct`) as a catch-all state response, forcing runtime parsing rather than contract-driven schema validations. |
| `FieldType::Object` | Schema Catalog mapping | `crates/op-grpc-bridge/src/proto_gen.rs:268-268` | **Yes** | Maps standard objects to generic `google.protobuf.Struct`, promoting unstructured JSON contracts in generated clients. |
| `FieldType::Any` | Schema Catalog mapping | `crates/op-grpc-bridge/src/proto_gen.rs:270-270` | **Yes** | Maps any type to generic `google.protobuf.Value`, breaking schema validation guarantees. |
| `parse_bridge_hierarchy` | Hand-rolled JSON parsing | `crates/op-grpc-bridge/src/grpc_server.rs:1143-1250` | **No** | Parses JSON raw payloads of the OVSDB mirror using ad-hoc `serde_json::Value` queries and manual walking of OVSDB sets/maps, bypassing structured, versioned schema definitions for OVSDB models. |

---

## 2. OSCAL Compliance Audit

Security controls (authentication, authorization, audit log integrity, configuration verification) implemented in code should map directly to machine-readable OSCAL profiles, Component Definitions, or System Security Plans (SSPs). The table below details gaps in code-to-compliance coverage.

### OSCAL Compliance & Control Gap Table
| Control Area | Implemented at file:line | OSCAL Artifact | Compliance Gap |
| :--- | :--- | :--- | :--- |
| **IA-2 / IA-8** (Identification & Authentication / Cryptographic Credentials) | `crates/op-grpc-bridge/src/interceptor.rs:31-77` | *None* | Authentication via `x-ghostbridge-footprint` and raw shared memory pointer reading is entirely hardcoded. There is no mapping to OSCAL authentication components, cryptographic profile parameters, or SSP controls. |
| **AC-3** (Access Enforcement) | `crates/op-grpc-bridge/src/schema_engine.rs:188-200` | *None* | The mutation pipeline commits events into the event ledger using a hardcoded `Decision::Allow` argument. No policy decision point (PDP) or OSCAL policy enforcement rules are integrated. |
| **AU-2 / AU-12** (Audit Event & Audit Generation Integrity) | `crates/op-grpc-bridge/src/schema_engine.rs:673-690` | *None* | OSCAL validation metadata (such as `SCHEMA_UUID`, `SCHEMA_SUBID`, `SCHEMA_CONTROL_SOURCE`, and NIST control references) are retrieved directly from volatile environment variables instead of loading a secure, validated machine-readable OSCAL profile. |
| **CM-2 / CM-8** (Baseline Configuration / Information System Component Inventory) | `crates/op-grpc-bridge/src/grpc_server.rs:777-781` | *None* | Active operating system service configurations queried via shelling out (`dinitctl list`) are not cross-referenced or validated against any system configuration baseline defined in an OSCAL component-definition artifact. |

---

## 3. Detailed Audit Findings & Exploitable Gaps

### [CRITICAL] Memory Safety & Denial-of-Service (DoS) via Truncated Memory Map in gRPC Gatekeeper
* **File:Line**: `crates/op-grpc-bridge/src/interceptor.rs:47-59`
* **Vulnerability Type**: Out-of-Bounds Memory Read / SIGBUS Crash
* **Impact**: Direct exploitation can trigger instant crash (Denial of Service) of the primary ingress gRPC gateway at port 50051.
* **Analysis**:
  The Tonic interceptor performs a zero-copy direct cast from `/dev/shm/plugin_schema.dat`:
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
  The code maps the file and instantly casts the raw pointer to `*const IdentitySled` and dereferences it. However, the file size of `/dev/shm/plugin_schema.dat` is never verified. 
  If `/dev/shm/plugin_schema.dat` is empty, truncated, or has a file size smaller than `std::mem::size_of::<IdentitySled>()` (minimum 73 bytes), the memory map will succeed but dereferencing `is_valid` or `hashed_footprint` will access memory pages beyond the file size limit. In Unix environments, accessing a mapped memory region that extends beyond the current file end triggers a `SIGBUS` signal, terminating the process immediately. This can be exploited by an attacker with local unprivileged access or by triggering an unclean shutdown/write of `/dev/shm/plugin_schema.dat`.

### [MAJOR] Non-Atomic Data Races on Shared Memory Reads
* **File:Line**: `crates/op-grpc-bridge/src/interceptor.rs:56-59`
* **Vulnerability Type**: Data Race / Undefined Compiler Behavior
* **Impact**: Under high concurrency, the compiler may optimize or cached fields of the raw pointer, or read partial writes, causing validation bypasses or memory corruption.
* **Analysis**:
  The shared memory `/dev/shm/plugin_schema.dat` is periodically updated by the authoritative `SchemaEngine` process, while being continuously read in parallel by `ghostbridge_interceptor` worker threads. The raw dereferences `(*sled_ptr).is_valid` and `(*sled_ptr).hashed_footprint` are performed without volatile reads, standard memory barriers, or Rust `std::sync::atomic` types. Under the Rust memory model, concurrent non-atomic reads and writes to the same memory location constitutes a data race, leading to undefined compiler optimization behavior.

### [MAJOR] Command Execution Injection Risk in Runtime Service Status Query
* **File:Line**: `crates/op-grpc-bridge/src/grpc_server.rs:835-840`
* **Vulnerability Type**: Unsanitized External Process Invocation
* **Impact**: Potential unauthorized command execution or system status manipulation via maliciously crafted service names.
* **Analysis**:
  In `get_service`, the gRPC server takes an input string `request.get_ref().service_name` and directly executes it as an argument to the system init manager `dinitctl`:
  ```rust
  let name = &request.get_ref().service_name;
  let output = tokio::process::Command::new("dinitctl")
      .args(["status", name])
      .output()
  ```
  There is no sanitization or character whitelist on `name`. Although passed as a distinct argument array parameter to avoid basic shell tokenization injections, if the local system service name contains specialized argument syntax interpreted by `dinitctl`, or if the environment's `PATH` variable can be manipulated to redirect the relative lookup of `dinitctl`, privilege escalation or arbitrary script execution could be achieved.

### [MAJOR] Ad-Hoc Volatile OSCAL Auditing via Process Environment
* **File:Line**: `crates/op-grpc-bridge/src/schema_engine.rs:680-686`
* **Vulnerability Type**: Non-Deterministic Compliance State Tracking
* **Impact**: Regulatory auditing bypass. Metadata representing system compliance can be modified dynamically, decoupling physical enforcement from OSCAL configurations.
* **Analysis**:
  The cryptographic validation sled writes compliance metadata read dynamically from environment variables:
  ```rust
  let uuid          = std::env::var("SCHEMA_UUID").unwrap_or_default();
  let subid         = std::env::var("SCHEMA_SUBID").unwrap_or_default();
  let ctrl          = std::env::var("SCHEMA_CONTROL_SOURCE")
                          .unwrap_or_else(|_| "NIST_SP_800_53_R5".into());
  ```
  This implementation violates OSCAL automation guidelines. Because compliance state is not verified against a statically mapped, cryptographic system model policy file (e.g. system-security-plan or component-definition), a privileged attacker could alter the environmental variables of the running server to log false compliance state or write misleading NIST 800-53 control tags to the immutable identity sled.

---

## 4. Recommendations and Action Plan

### 1. Secure Shared Memory Ingress Validation
To resolve the critical DoS/SIGBUS vulnerability in the shared memory interceptor, enforce size and structural integrity validation before casting raw pointers.
* **Fix**: Validate the size of `/dev/shm/plugin_schema.dat` before accessing the mapping. Use atomic read operations.
```rust
// In crates/op-grpc-bridge/src/interceptor.rs
let file = File::open("/dev/shm/plugin_schema.dat")
    .map_err(|_| Status::internal("SchemaEngine Memory Unreachable"))?;

let metadata = file.metadata()
    .map_err(|_| Status::internal("Failed to retrieve schema metadata"))?;

let required_size = std::mem::size_of::<IdentitySled>() as u64;
if metadata.len() < required_size {
    return Err(Status::internal(
        "A.N.N.A. Scribe: Cryptographic Identity Sled is truncated on disk."
    ));
}

let mmap = unsafe {
    MmapOptions::new()
        .map(&file)
        .map_err(|_| Status::internal("Mmap failed"))?
};

// Use std::ptr::read_volatile to prevent compiler optimization of shared-memory reads
let sled: IdentitySled = unsafe {
    std::ptr::read_volatile(mmap.as_ptr() as *const IdentitySled)
};

let is_valid = sled.is_valid;
let current_footprint = sled.hashed_footprint;
```

### 2. Standardize Schema-as-Code Contracts
Replace all instances of untyped payloads (`google.protobuf.Struct` / `google.protobuf.Value` / `simd_json::OwnedValue`) with strongly typed, versioned nested structures in gRPC messages.
* **Action**: Refactor the output generator in `crates/op-grpc-bridge/src/proto_gen.rs` to generate distinct Protobuf message schemas for catalog-mapped properties instead of translating dynamic objects to generic `google.protobuf.Struct` objects.

### 3. Replace Shell Execution with Safe Whitelisting
Enforce alphanumeric input sanitization on all service query APIs to mitigate command injection risks.
* **Fix**: In `crates/op-grpc-bridge/src/grpc_server.rs:835`, validate input names using a strict regular expression and use an absolute executable path:
```rust
let name = &request.get_ref().service_name;
if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
    return Err(Status::invalid_argument("Invalid character sequence in service name"));
}
let output = tokio::process::Command::new("/sbin/dinitctl")
    .args(["status", name])
    .output()
    .await
    .map_err(|e| Status::internal(format!("dinitctl execution failed: {}", e)))?;
```

### 4. Implement Cryptographic OSCAL Policy Deserialization
Store and load the OSCAL SSP policy dynamically from a cryptographically signed static file instead of relying on process-level environment variables.
* **Action**: Parse a structured, local versioned OSCAL catalog document at system boot, verifying the `SCHEMA_UUID` and policy claims before allowing mutations. Banish hardcoded `Decision::Allow` assignments within `crates/op-grpc-bridge/src/schema_engine.rs:188` in favor of verifying user capabilities against the active OSCAL access policies.

---
## ⚠ Citation Warnings
- `crates/op-grpc-bridge/src/schema_engine.rs:673`: file has 569 lines
- `crates/op-grpc-bridge/src/schema_engine.rs:680`: file has 569 lines
