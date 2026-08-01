# Production Security and Quality Audit: op-grpc-bridge

## 1. Security & Quality Findings

### Finding 1 [Critical]: Out-of-Bounds Memory Read / Segmentation Fault in Public gRPC Interceptor via Unchecked Shared Memory Mmap Size
* **File:** `crates/op-grpc-bridge/src/interceptor.rs:48-55`
* **Type:** Memory Safety & Denial of Service (DoS)
* **Description:** The `ghostbridge_interceptor` function opens the shared memory file `/dev/shm/plugin_schema.dat` and maps it into the process address space using `mmap2`. It immediately casts the raw mapped pointer to `*const IdentitySled` and dereferences its fields:
  ```rust
  let mmap = unsafe {
      MmapOptions::new()
          .map(&file)
          .map_err(|_| Status::internal("Mmap failed"))?
  };
  let sled_ptr = mmap.as_ptr() as *const IdentitySled;

  let is_valid = unsafe { (*sled_ptr).is_valid };
  ```
  The code performs **no validation** to verify that `mmap.len()` is at least equal to `std::mem::size_of::<IdentitySled>()` (which is approximately 81 bytes depending on alignment/padding).
* **Exploitability:** Directly exploitable. If `/dev/shm/plugin_schema.dat` is empty (0 bytes) or truncated (e.g., during a Btrfs mutation, system update, or local race condition), dereferencing `sled_ptr` accesses memory outside the mapped backing file pages. This triggers a hardware page fault that causes the OS to deliver an uncaught `SIGSEGV`, instantly terminating the entire multi-threaded Tonic gRPC server on port `50051`. This allows any local user or system state event to cause a complete Denial of Service.
* **Remediation:** Validate the mapped metadata length before casting and dereferencing:
  ```rust
  if mmap.len() < std::mem::size_of::<IdentitySled>() {
      return Err(Status::internal("Identity Sled is truncated or invalid"));
  }
  ```

---

### Finding 2 [Schema-as-Code Violation]: Ad-Hoc Dynamic JSON-in-String Serialization over Local IPC
* **File:** `crates/op-grpc-bridge/src/grpc_server.rs:885`, `crates/op-grpc-bridge/src/grpc_server.rs:1133`, `crates/op-grpc-bridge/src/grpc_server.rs:1453`
* **Type:** Code Quality & Schema Validation Failure
* **Description:** The system bypasses strict "schema-as-code" rules when invoking D-Bus endpoints. In `mail_dbus_call`, `privacy_dbus_call`, and `registration_dbus_call`, the parameters are packed into ad-hoc JSON values, serialized into raw string payloads, and dispatched over untyped D-Bus connections:
  ```rust
  let args = simd_json::json!({
      "from": req.from_email,
      "to": req.to_email,
      "subject": req.subject,
      "body": req.body,
      "is_html": req.is_html,
      "domain": req.domain
  });
  let args_str = args.to_string();
  ```
  This creates an implicit, untracked, and unversioned string-based contract between the gRPC bridge and local D-Bus backends.
* **Exploitability:** High maintenance risk. Any field rename, addition, or type change in the backend D-Bus services will cause silent parsing failures at runtime, leading to complete service degradation without compile-time verification or static schema generation.
* **Remediation:** Govern local IPC payloads using structured, code-generated D-Bus interface traits or compile-time checked Protobuf definitions instead of dynamically building raw JSON strings.

---

### Finding 3 [Schema-as-Code Violation]: Ad-Hoc Shared Memory C-Layout Definition
* **File:** `crates/op-grpc-bridge/src/interceptor.rs:18`
* **Type:** Architectural / Portability Risk
* **Description:** The memory-mapped layout `IdentitySled` is written directly as an ad-hoc `#[repr(C)]` struct in raw source code:
  ```rust
  #[repr(C)]
  pub struct IdentitySled {
      pub wireguard_pubkey: [u8; 32],
      pub mutation_index: u64,
      pub is_valid: bool,
      pub hashed_footprint: [u8; 32],
  }
  ```
  Since multiple separate processes (such as the `SchemaEngine` and the gRPC gateway) read and write to this segment simultaneously, defining it ad-hoc inside a middleware module lacks central version management and does not compile from an authoritative schema source.
* **Exploitability:** If the writer process and reader process are compiled with different versions of the crate or distinct alignment configurations, memory corruption or incorrect parsing of `is_valid` / `hashed_footprint` will occur.
* **Remediation:** Define the shared memory structure layout as part of a versioned, static schema library, or use Protocol Buffer mappings that write out deterministic binary segments.

---

### Finding 4 [Schema-as-Code Violation]: Dynamic Runtime OSCAL Compliance Mappings via Environment Variables
* **File:** `crates/op-grpc-bridge/src/schema_engine.rs:424`
* **Type:** Compliance Audit Risk
* **Description:** The compliance attributes (including controls, statement references, and profile identifiers) mapped during a state mutation are resolved dynamically via raw environment variables:
  ```rust
  let uuid          = std::env::var("SCHEMA_UUID").unwrap_or_default();
  let subid         = std::env::var("SCHEMA_SUBID").unwrap_or_default();
  let ctrl          = std::env::var("SCHEMA_CONTROL_SOURCE")
                          .unwrap_or_else(|_| "NIST_SP_800_53_R5".into());
  ```
  This bypasses structural alignment with versioned OSCAL schemas (e.g., Component Definitions or System Security Plans), making it impossible to statically verify compliance footprints and leaving them susceptible to configuration injection.
* **Exploitability:** An attacker with control over local environment blocks can modify compliance footprints written to audit ledgers, compromising structural traceability.
* **Remediation:** Bind OSCAL attributes to static, code-generated schemas generated directly from the authoritative compliance policy documents, rather than fetching them dynamically from raw string environment lookups.

---

## 2. Proactive Improvement Suggestions

| No. | Suggestion | Rationale | Example (file:line) |
|---|---|---|---|
| **1** | **Strongly-typed IPC interfaces** | Replace untyped `zbus::Proxy` and `simd_json` string payloads with code-generated D-Bus client bindings using `zbus::proxy` macros to enforce compile-time interface safety. | `crates/op-grpc-bridge/src/grpc_server.rs:885` |
| **2** | **Zero-Copy Protobuf to SIMD mapping** | Avoid serializing and deserializing JSON values through intermediate string formats (e.g. `simd_json::to_string` -> `serde_json::from_str`) when converting `simd_json::OwnedValue` to `prost_types::Value`. Convert memory structures directly using zero-copy borrow mechanics. | `crates/op-grpc-bridge/src/grpc_client.rs:351` |
| **3** | **Bounded Thread Concurrency** | Use the existing `dbus_call_limiter` Semaphore (which is currently defined but unused under `#[allow(dead_code)]`) to restrict active concurrent calls to the system D-Bus daemon, preventing socket exhaustion during high-throughput gRPC traffic. | `crates/op-grpc-bridge/src/schema_engine.rs:62` |
| **4** | **Structured Context Spans** | Replace plain `info!` text messages with structured tracing spans (`tracing::info_span!`) that bind `plugin_id`, `object_path`, and `actor_id` as queryable attributes to ease distributed trace debugging. | `crates/op-grpc-bridge/src/schema_engine.rs:360` |
| **5** | **Persistent Local State Storage** | Transition the system state cache from an in-memory `RwLock<HashMap>` to a transactional embedded store (such as the workspace's configured `cozo` or `sled` engine) to prevent memory-growth bottlenecks on continuous state updates. | `crates/op-grpc-bridge/src/schema_engine.rs:55` |
| **6** | **Strict Verification on Array Unwraps** | Validate array lengths using safe pattern matching or bounds checks before directly indexing them in low-level mutations. | `crates/op-grpc-bridge/src/schema_engine.rs:347` |