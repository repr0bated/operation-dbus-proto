# PRODUCTION QUALITY & SECURITY AUDIT REPORT

---

## 1. CRITICAL FINDINGS (Directly Exploitable)

### Memory Safety, Data Race, and Crash DoS in Gatekeeper Interceptor
* **File:** `crates/op-grpc-bridge/src/interceptor.rs`
* **Lines:** 45-56
* **Crate:** `op-grpc-bridge`
* **Impact:** Critical (Memory Safety, Undefined Behavior, Crash/Denial of Service)
* **Description:** 
  The primary gRPC gatekeeper middleware `ghostbridge_interceptor` performs an unsafe raw C-pointer cast of memory-mapped shared data from `/dev/shm/plugin_schema.dat` into a `*const IdentitySled` on every incoming request. This implementation contains three severe flaws that are directly exploitable:
  1. **No File Size Validation (Crash DoS):** The interceptor does not check the file size of `/dev/shm/plugin_schema.dat` before mapping it and dereferencing the cast pointer. The struct `IdentitySled` has a layout size of 80 bytes due to padding and alignment (`wireguard_pubkey[32]` + `mutation_index(u64)` + `is_valid(bool)` + 7 bytes alignment padding + `hashed_footprint[32]`). If the file is smaller than 80 bytes (e.g., truncated or uninitialized), accessing `hashed_footprint` will read out-of-bounds, triggering a `SIGBUS` signal that immediately terminates the gatekeeper process. Any unauthenticated network user sending a gRPC request while the file is uninitialized or truncated will crash the ingress gatekeeper.
  2. **Data Race / Invalid Bool Representation (Undefined Behavior):** The memory-mapped file is mutated concurrently by other processes (such as the Schema Engine) without any atomic synchronization. Accessing raw, non-atomic fields (`is_valid: bool` and `hashed_footprint: [u8; 32]`) across processes is a data race, which constitutes Undefined Behavior in Rust. In Rust, a `bool` must strictly be represented as `0` or `1`. If a concurrent write operation is captured mid-transition, the byte value may be anything other than `0` or `1`. Reading this invalid state violates Rust’s safety invariants and can cause unpredictable compiler optimizations or memory corruption.
  3. **File Descriptor & Resource Exhaustion:** Opening and mapping a file on *every single gRPC request* requires multiple system calls (`open`, `mmap`, `close`). Under high request volumes, this acts as a severe bottleneck and exposes the gateway to file descriptor exhaustion.

```rust
// crates/op-grpc-bridge/src/interceptor.rs:45-56
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

---

## 2. HIGH FINDINGS (High Risk / Validation Failures)

### Argument Injection in System Service Inquiries
* **File:** `crates/op-grpc-bridge/src/grpc_server.rs`
* **Lines:** 1120-1127
* **Crate:** `op-grpc-bridge`
* **Impact:** High (Command/Argument Injection)
* **Description:** 
  The `get_service` gRPC method reads the string `service_name` directly from the user-controlled `RuntimeGetServiceRequest` and passes it as an argument to `tokio::process::Command::new("dinitctl").args(["status", name])`. 
  While executing commands directly via `tokio::process::Command` (without shell invocation) prevents arbitrary shell command execution (e.g., `; rm -rf /`), it does not protect against **Argument Injection**. If an attacker passes a `service_name` starting with flags (e.g., `--help` or control flags accepted by the underlying `dinitctl` binary), they can alter the execution behavior. If `dinitctl` supports options to execute scripts, write files, or elevate privileges, this can be leveraged for unauthorized system actions.

```rust
// crates/op-grpc-bridge/src/grpc_server.rs:1120-1127
async fn get_service(
    &self,
    request: Request<RuntimeGetServiceRequest>,
) -> Result<Response<ProtoRuntimeServiceInfo>, Status> {
    let name = &request.get_ref().service_name;
    let output = tokio::process::Command::new("dinitctl")
        .args(["status", name])
```

---

### Unsanitized User-Controlled D-Bus Routing
* **File:** `crates/op-grpc-bridge/src/grpc_server.rs`
* **Lines:** 590-600
* **Crate:** `op-grpc-bridge`
* **Impact:** High (Unauthorized Resource Access / D-Bus Privilege Escalation)
* **Description:** 
  The methods `get_property`, `set_property`, and generic mutations inside the gRPC server format the local D-Bus bus name directly from the user-controlled `req.plugin_id` and construct a proxy with `req.object_path`. 
  Because there is no sanitization or strict allowlist validation (such as a regex constraint or structural verification) on `plugin_id` and `object_path`, a remote authenticated attacker can trick the gRPC bridge into routing requests to arbitrary local D-Bus services matching the wildcard `org.opdbus.<plugin_id>.v1` or target unpermitted object paths. This bypasses the structural boundaries intended between separate plugin boundaries.

```rust
// crates/op-grpc-bridge/src/grpc_server.rs:590-600
let proxy = zbus::fdo::PropertiesProxy::builder(&connection)
    .destination(format!("org.opdbus.{}.v1", req.plugin_id))
    .map_err(|e| Status::internal(e.to_string()))?
    .path(req.object_path.as_str())
```

---

## 3. MEDIUM FINDINGS (Schema-as-Code Violations)

### Ad-hoc Dynamic Protobuf Schema Generation at Runtime
* **File:** `crates/op-grpc-bridge/src/proto_gen.rs`
* **Lines:** 17-64
* **Crate:** `op-grpc-bridge`
* **Impact:** Medium (Schema-as-Code / Architectural Drift)
* **Description:** 
  The `ProtoGenerator` in `proto_gen.rs` dynamically converts the in-memory Rust structures (`PluginSchema` and `SchemaCatalog`) into raw protobuf text strings at runtime.
  This approach violates the **Schema-as-Code** discipline. Instead of maintaining statically versioned, declarative schema files as the single source of truth, schemas are dynamically constructed. This introduces synchronization drift risks, as changes to local schemas can lead to runtime mismatches between the compiled gRPC definitions (`tonic::include_proto!("operation.v1")`) and the dynamically generated schemas, causing serialization panics or payload drop-outs.

---

## 4. LOW / BEST PRACTICE FINDINGS

### Missing Minimum Rust Version (`rust-version`)
* **File:** `Cargo.toml`, `crates/op-grpc-bridge/Cargo.toml`
* **Impact:** Low (Build Instability)
* **Description:** 
  Neither the workspace `Cargo.toml` nor the crate-level `Cargo.toml` defines a `rust-version` pin. In production environments, this can lead to compilation failures when building with older, incompatible toolchains.

### Workspace Dependency Mismatch (Qdrant Client Version Override)
* **File:** `crates/op-grpc-bridge/Cargo.toml` vs `Cargo.toml`
* **Impact:** Low (Dependency Duplication / Mismatch)
* **Description:** 
  The workspace `Cargo.toml` specifies `qdrant-client = "1.7"` as a standard dependency, but the local `crates/op-grpc-bridge/Cargo.toml` overrides this version to `"1.17"` without inheriting it from the workspace. This local override forces cargo to compile and link multiple versions of the `qdrant-client` library, increasing the final binary footprint and raising risks of duplicate symbol linkage conflicts.

---

## 5. ROLE: BUILD AUDIT REPORT

This section validates the build architecture, workspace constraints, and code generation risks:

### 1. Cargo.toml Configuration
* **Edition:** The workspace defines `edition = "2021"`. `op-grpc-bridge` inherits this via `edition.workspace = true`.
* **Rust Version:** The `rust-version` field is omitted from both the workspace and crate Cargo files.
* **Bins & Examples:** No binary targets or examples are defined. It operates purely as a library crate.

### 2. Workspace Inheritance vs. Local Overrides
* **Inherited Dependencies:** `op-core`, `tonic`, `tonic-web`, `prost`, `prost-types`, `tonic-reflection`, `tonic-health`, `tokio`, `zbus`, `serde`, `serde_json`, `simd-json`, `memmap2`, `hex`, `sha2`, `tracing`, `anyhow`, `thiserror`, `async-trait`, `uuid`, `chrono`, and `tonic-build` (build dependency).
* **Local Overrides:** `qdrant-client` version is overridden locally to `1.17` (workspace specifies `1.7`).
* **Path Dependencies:** Local crates compiled by relative paths include `op-state-store`, `op-identity`, `op-network`, `op-jsonrpc`, `op-cognitive-mcp`, and `op-cache`.

### 3. Schema-As-Code Build Check
* **Proto Compilation Build Step:** `crates/op-grpc-bridge/Cargo.toml` lists `tonic-build` as a build dependency. However, `build.rs` is **not** provided in the FILES section.
* **State of Proto Source Files:** Because `build.rs` is not provided, we cannot inspect the exact paths of compiled `.proto` files. However, the crate contains `tonic::include_proto!("operation.v1")`, showing that static protobuf sources are parsed at compile-time rather than dynamically compiled at runtime.
* **Hybrid Model Risk:** The system employs a hybrid pattern. Static services are parsed at compile-time, while dynamic schema descriptions are generated as strings at runtime via `proto_gen.rs`. This deviates from strict version-controlled schema discipline.