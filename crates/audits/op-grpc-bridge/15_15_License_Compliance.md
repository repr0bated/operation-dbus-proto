# License Audit

## Extracted License
* **Crate**: `op-grpc-bridge`
* **License Field**: `license.workspace = true` in `crates/op-grpc-bridge/Cargo.toml` (Line 5), resolving to **`Apache-2.0`** defined in the root workspace `Cargo.toml` (Line 46).

## Cargo.lock Scan for GPL/AGPL/SSPL Crates
* No GPL, AGPL, or SSPL licensed crates were found in the provided `Cargo.lock` section.

## Crates with No License Field
* None. All workspace members and workspace packages defined in the files specify a valid license (inheriting `Apache-2.0` from the workspace).

---

# Security & Quality Audit Findings

## Critical Findings

### 1. Undefined Behavior via Loading Invalid Rust `bool` from Mapped File
* **Citation**: `crates/op-grpc-bridge/src/interceptor.rs:22`
* **Impact**: Critical / Remote Crash / Denial of Service
* **Description**: The `IdentitySled` struct is mapped directly from shared memory using `memmap2` and cast to a raw C pointer:
  ```rust
  #[repr(C)]
  pub struct IdentitySled {
      pub wireguard_pubkey: [u8; 32],
      pub mutation_index: u64,
      pub is_valid: bool,
      pub hashed_footprint: [u8; 32],
  }
  ```
  The field `is_valid` is represented as a Rust `bool`. In Rust, a `bool` must strictly hold the byte value `0x00` (false) or `0x01` (true). Since `/dev/shm/plugin_schema.dat` can be modified by external processes, corrupted, or left uninitialized, reading any other bit pattern (e.g., `0x02` or `0xFF`) as a `bool` results in immediate **Undefined Behavior** (UB) under the Rust memory model. This allows a corrupted or malicious shared memory state to trigger unexpected optimizations, compiler-driven exploits, or process crashes in the gRPC interceptor on every incoming request.
* **Remediation**: Change `is_valid` in `IdentitySled` to `u8` (or use a wrapper structure with safe conversion logic) and validate that the byte is strictly `0` or `1` before converting it to a boolean.

### 2. Unchecked Memory Dereference of Raw Pointer from Shared Memory
* **Citation**: `crates/op-grpc-bridge/src/interceptor.rs:48-58`
* **Impact**: Critical / Memory Corruption / Crash
* **Description**: The gRPC interceptor maps `/dev/shm/plugin_schema.dat` and casts its pointer directly without validating the size of the mapped memory:
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
  If the file on disk is empty or smaller than `std::mem::size_of::<IdentitySled>()` (which can happen during initialization, truncation, or system memory pressure), the dereference `*sled_ptr` will perform an out-of-bounds read. This leads to immediate segmentation faults or disclosure of adjacent memory. Furthermore, memory alignment is not checked, which causes undefined behavior on alignment-sensitive architectures.
* **Remediation**: Before casting `mmap.as_ptr()`, verify that `mmap.len() >= std::mem::size_of::<IdentitySled>()` and assert that the pointer alignment is compatible with `IdentitySled`.

---

## Low & Medium Findings

### 3. Violation of Schema-as-Code Discipline via Ad-Hoc JSON Contracts
* **Citation**: `crates/op-grpc-bridge/src/grpc_server.rs:659-666`, `crates/op-grpc-bridge/src/grpc_server.rs:701-706`, `crates/op-grpc-bridge/src/grpc_server.rs:730-736`, `crates/op-grpc-bridge/src/grpc_server.rs:815-819`, `crates/op-grpc-bridge/src/grpc_server.rs:967-970`
* **Impact**: Medium / Architectural Debt / Integration Fragility
* **Description**: Throughout `grpc_server.rs`, typed gRPC messages are converted into ad-hoc dynamic JSON maps using `simd_json::json!({ ... })` (e.g., in `send_email`, `get_inbox`, `ensure_privacy_network`, etc.) to pass arguments to local D-Bus calls. These ad-hoc JSON structures do not conform to the project's **schema-as-code** discipline. Dynamic, unversioned JSON structures are prone to drift and bypass compile-time contract enforcement.
* **Remediation**: Define these structural data contracts as versioned Protocol Buffers or versioned schemas, generating the serialization/deserialization code automatically rather than constructing dynamic maps on the fly.

### 4. Hardcoded Shared Memory Path Restricts Deployment Environments
* **Citation**: `crates/op-grpc-bridge/src/interceptor.rs:48`
* **Impact**: Low / Portability / Testability
* **Description**: The file path `/dev/shm/plugin_schema.dat` is hardcoded directly inside the gRPC interceptor logic. This prevents the component from running in containerized, read-only, or sandboxed environments where `/dev/shm` is not mounted, or where custom paths must be supplied. It also complicates integration testing.
* **Remediation**: Parameterize the shared memory file path via configuration or environment variables, falling back to `/dev/shm/plugin_schema.dat` only as a default.