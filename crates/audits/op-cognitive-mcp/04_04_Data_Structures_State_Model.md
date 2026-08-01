# System Quality & Security Audit: op-cognitive-mcp

---

## 1. Data Structures & Concurrency Audit

### Concurrency & Synchronization Primitive Reference Counts

| File Path | `Arc` | `Rc` | `RefCell` | `RwLock` | `Mutex` | `OnceCell` / `OnceLock` |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: |
| `crates/op-cognitive-mcp/src/activity_filter.rs` | 2 | 0 | 0 | 2 | 0 | 0 |
| `crates/op-cognitive-mcp/src/cognitive_tools.rs` | 4 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-cognitive-mcp/src/notebooklm.rs` | 3 | 0 | 0 | 0 | 1 | 0 |
| `crates/op-cognitive-mcp/src/voyage.rs` | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-cognitive-mcp/src/qdrant_shuttle.rs` | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-cognitive-mcp/src/session.rs` | 1 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-cognitive-mcp/src/quota.rs` | 2 | 0 | 0 | 2 | 0 | 0 |
| `crates/op-cognitive-mcp/src/grpc_service.rs` | 4 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-cognitive-mcp/src/typed_tools.rs` | 3 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-cognitive-mcp/src/gemini_fallback.rs` | 1 | 0 | 0 | 1 | 0 | 0 |
| `crates/op-cognitive-mcp/src/tool_profiles.rs` | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-cognitive-mcp/src/doctor.rs` | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-cognitive-mcp/src/interceptor.rs` | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-cognitive-mcp/src/memory_store.rs` | 1 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-cognitive-mcp/src/cozo_shuttle.rs` | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-cognitive-mcp/src/main.rs` | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-cognitive-mcp/src/lib.rs` | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-cognitive-mcp/src/dbus_interface.rs` | 1 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-cognitive-mcp/src/server.rs` | 7 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-cognitive-mcp/src/rag_pipeline.rs` | 0 | 0 | 0 | 0 | 0 | 4 (`OnceLock`) |
| `crates/op-cognitive-mcp/src/bin/op-cog-admin.rs` | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-cognitive-mcp/src/bin/rag-ingest.rs` | 0 | 0 | 0 | 0 | 0 | 0 |

---

### `.clone()` Call Analysis
No single file exceeded the maximum limit of 20 `.clone()` calls. The highest count was found in:
* `crates/op-cognitive-mcp/src/grpc_service.rs` (13 calls)
* `crates/op-cognitive-mcp/src/server.rs` (11 calls)
* `crates/op-cognitive-mcp/src/typed_tools.rs` (10 calls)

---

### Large Structs (> 5 Public Fields)

The following public structs contain more than 5 public fields and require structural consolidation or builder-pattern isolation:

1. **`ActivityEvent`** (`crates/op-cognitive-mcp/src/activity_filter.rs:117`)
   * **Public Fields (17):** `id`, `timestamp`, `user_id`, `conversation_id`, `actor_id`, `op_kind`, `autonomous`, `confidence`, `plugin_id`, `field`, `is_write`, `constraint_failed`, `memory_ref`, `tool_name`, `content_hash`, `summary`, `payload`.
2. **`ConversationSession`** (`crates/op-cognitive-mcp/src/session.rs:16`)
   * **Public Fields (7):** `id`, `notebook_id`, `created_at`, `updated_at`, `query_count`, `history`, `active`.
3. **`IdentitySled`** (`crates/op-cognitive-mcp/src/interceptor.rs:6`)
   * **Public Fields (9):** `wireguard_pubkey`, `mutation_index`, `is_valid`, `_pad`, `hashed_footprint`, `schema_uuid`, `subid`, `control_source`, `nextdns_profile`.
4. **`MemoryNamespace`** (`crates/op-cognitive-mcp/src/memory_store.rs:51`)
   * **Public Fields (9):** `id`, `name`, `kind`, `description`, `linked_task_id`, `linked_cron`, `metadata`, `created_at`, `updated_at`.
5. **`MemoryEntry`** (`crates/op-cognitive-mcp/src/memory_store.rs:66`)
   * **Public Fields (10):** `id`, `namespace_id`, `key`, `value`, `tags`, `created_at`, `updated_at`, `expires_at`, `access_count`, `last_accessed`.
6. **`FileMeta`** (`crates/op-cognitive-mcp/src/rag_pipeline.rs:46`)
   * **Public Fields (7):** `language`, `file_type`, `symbols`, `doc_comments`, `imports`, `tags`, `is_test`.
7. **`Chunk`** (`crates/op-cognitive-mcp/src/rag_pipeline.rs:69`)
   * **Public Fields (10):** `repo`, `file_path`, `meta`, `content`, `embed_text`, `content_hash`, `chunk_index`, `total_chunks`, `line_start`, `line_end`.
8. **`RagResult`** (`crates/op-cognitive-mcp/src/rag_pipeline.rs:98`)
   * **Public Fields (15):** `score`, `repo`, `file_path`, `language`, `file_type`, `symbols`, `doc_comments`, `imports`, `tags`, `is_test`, `line_start`, `line_end`, `chunk_index`, `total_chunks`, `content`.

---

### Globally Mutable State
No `static mut` or `lazy_static` variables containing mutable state were identified in the audited scope. State mutation is safely managed via `CozoGraphShuttle` (persisted), `DashMap` (in-memory sessions), or local scoped `RwLock` / `Mutex` wrapping.

---

## 2. Security & Quality Audit Findings

### Critical Findings

#### CRITICAL-01: Out-of-Bounds Memory Dereference in Ghostbridge Interceptor
* **File:** `crates/op-cognitive-mcp/src/interceptor.rs:24-34`
* **Vulnerability Type:** Memory Safety (Out-of-Bounds Pointer Dereference)
* **Description:**
  The `ghostbridge_interceptor` opens and memory-maps `/dev/shm/plugin_schema.dat` via `MmapOptions::new().map(&file)`. It immediately casts the mapping's raw pointer to `*const IdentitySled` and dereferences it to read `is_valid` and `hashed_footprint`:
  ```rust
  let mmap = unsafe {
      MmapOptions::new()
          .map(&file)
          .map_err(|_| Status::internal("Mmap failed"))?
  };
  let sled_ptr = mmap.as_ptr() as *const IdentitySled;

  let is_valid = unsafe { (*sled_ptr).is_valid };
  ```
  The interceptor performs **no size check** on `mmap` prior to dereferencing `sled_ptr`. If `/dev/shm/plugin_schema.dat` is empty or truncated to a size smaller than `size_of::<IdentitySled>()` (208 bytes), the pointer dereference will access unmapped or out-of-bounds memory, leading to an immediate segmentation fault (SIGSEGV) and crashing the gRPC service.
* **Exploitability:**
  Directly exploitable by any local process that can truncate or clear `/dev/shm/plugin_schema.dat`. Because this file resides in `/dev/shm`, it is vulnerable to local tampering. A concurrent truncation triggers a denial of service (DoS) of the centralized control plane server.
* **Remediation:**
  Ensure the memory mapping has a size at least equal to `size_of::<IdentitySled>()` before performing any casting or pointer dereferences:
  ```rust
  if mmap.len() < std::mem::size_of::<IdentitySled>() {
      return Err(Status::failed_precondition("Identity Sled memory size too small."));
  }
  ```

---

#### CRITICAL-02: Structural ABI Alignment and Offsets Mismatch of `IdentitySled`
* **Files:** 
  * `crates/op-cognitive-mcp/src/qdrant_shuttle.rs:21-27`
  * `crates/op-cognitive-mcp/src/interceptor.rs:6-16`
* **Vulnerability Type:** Structural ABI Inconsistency / Memory Corruption
* **Description:**
  The struct `IdentitySled` is defined differently in `qdrant_shuttle.rs` and `interceptor.rs`:
  * **`qdrant_shuttle.rs`:**
    ```rust
    #[repr(C)]
    pub struct IdentitySled {
        pub wireguard_pubkey: [u8; 32],
        pub mutation_index: u64,
        pub is_valid: bool,
        pub hashed_footprint: [u8; 32],
    }
    ```
  * **`interceptor.rs`:**
    ```rust
    #[repr(C)]
    pub struct IdentitySled {
        pub wireguard_pubkey: [u8; 32],
        pub mutation_index: u64,
        pub is_valid: bool,
        pub _pad: [u8; 7],
        pub hashed_footprint: [u8; 32],
        pub schema_uuid: [u8; 16],
        pub subid: [u8; 64],
        pub control_source: [u8; 32],
        pub nextdns_profile: [u8; 16],
    }
    ```
  In `interceptor.rs`, there is an explicit `_pad: [u8; 7]` field after `is_valid`. In `qdrant_shuttle.rs`, this padding is missing. Furthermore, the interceptor struct contains an additional 128 bytes of identity fields (`schema_uuid`, `subid`, etc.). 
  Because both read from the exact same shared-memory file (`/dev/shm/plugin_schema.dat`), this struct layout mismatch changes the offset of `hashed_footprint`. The Qdrant shuttle reads corrupt bytes from a offset mismatch, producing invalid footprint validations, while the interceptor reads other fields.
* **Exploitability:**
  This results in memory parsing offsets being structurally misaligned. It causes authorization bypasses or persistent failures where the Qdrant loop cannot match the temporal hash computed by the gRPC interceptor, breaking audit-trail accountability.
* **Remediation:**
  Consolidate `IdentitySled` into a single canonical definition in a shared internal library (such as `op-core` or `op-state-store`) and import it in both locations to enforce absolute ABI compliance.

---

### Major Findings

#### MAJOR-01: Exponential Backtracking and Stack Overflow in Glob Matcher
* **File:** `crates/op-cognitive-mcp/src/grpc_service.rs:496-506`
* **Vulnerability Type:** Denial of Service (CPU Exhaustion / Stack Overflow)
* **Description:**
  The `add_folder` recursive glob matching helper uses an unoptimized backtracking matcher:
  ```rust
  fn glob_match_inner(pattern: &[char], name: &[char]) -> bool {
      match (pattern.first(), name.first()) {
          (None, None) => true,
          (Some('*'), _) => {
              glob_match_inner(&pattern[1..], name)
                  || (!name.is_empty() && glob_match_inner(pattern, &name[1..]))
          }
          ...
      }
  }
  ```
  The recursive execution of the wildcard pattern matching has a worst-case time complexity of $O(2^N)$ where $N$ is the number of wildcards and filename characters.
* **Impact:**
  An attacker or a compromised local plugin providing nested structures or a payload with a crafted pattern containing multiple `*` characters (e.g. `*a*b*c*d*e*f*g*h`) matching against a long filename will block the tokio worker thread, leading to thread starvation, high CPU utilization, or stack overflow (SIGSEGV) due to uncontrolled call stack growth.
* **Remediation:**
  Replace the ad-hoc recursive matching with the standard library's `glob` crate or implement a non-recursive, linear-time iterative wildcard matcher (such as a 2D dynamic programming table or a simplified state-machine).

---

#### MAJOR-02: Path Traversal / Arbitrary File Read in `AddFolder` API
* **File:** `crates/op-cognitive-mcp/src/grpc_service.rs:322-340`
* **Vulnerability Type:** Directory Traversal / Arbitrary File Disclosure
* **Description:**
  The `add_folder` RPC method permits clients to ingest entire directories into CozoDB:
  ```rust
  let path = std::path::Path::new(&req.folder_path);
  if !path.exists() || !path.is_dir() { ... }
  ...
  let walker = if req.recursive {
      walkdir(path)
  } else {
      walkdir_shallow(path)
  };
  ```
  The server does not perform any validation or canonicalization to check if `req.folder_path` is restricted to an approved workspace. It accepts absolute paths such as `/etc` or `/home/user/.ssh`.
* **Impact:**
  Allows arbitrary local directories containing highly sensitive configuration files, system logs, or private keys to be fully read, parsed, and recorded permanently into the database as accessible text sources.
* **Remediation:**
  Enforce a strict base directory restriction. Canonicalize the input path and verify it starts with a designated safe workspace folder prefix:
  ```rust
  let canonical_path = path.canonicalize()?;
  let workspace_root = std::path::Path::new("/var/lib/op-cognitive-mcp/workspace").canonicalize()?;
  if !canonical_path.starts_with(&workspace_root) {
      return Err(Status::permission_denied("Path lies outside workspace boundary."));
  }
  ```

---

#### MAJOR-03: Passive Logging Instead of Hard Enforcement for Insecure Credentials
* **File:** `crates/op-cognitive-mcp/src/grpc_service.rs:553-562`
* **Vulnerability Type:** Weak Privilege Enforcement
* **Description:**
  When configuring a Chrome profile credential via `setup_auth`, the server tests the permissions of the credential file on Unix systems. If the file has overly permissive access permissions (e.g., read/write access for groups/others), the server emits a passive warning but continues server configuration:
  ```rust
  if mode & 0o077 != 0 {
      warn!(
          path = %req.credential,
          mode = format!("{:o}", mode),
          "Chrome profile has overly permissive permissions; should be 0o600"
      );
  }
  ```
* **Impact:**
  Sensitive cookies, active sessions, and browser credentials can be read by other local users on the system, leading to cookie extraction and identity theft.
* **Remediation:**
  Refuse to process authentication setup if the permissions do not match the mandatory `0o600` (read/write only by owner) security requirement:
  ```rust
  if mode & 0o077 != 0 {
      return Err(Status::permission_denied("Insecure credentials file permissions. Must be 0o600."));
  }
  ```

---

#### MAJOR-04: Path Mismatch in PII Leakage Detection Filter
* **File:** `crates/op-cognitive-mcp/src/activity_filter.rs:150-165`
* **Vulnerability Type:** Logical Bypass / PII Data Leakage
* **Description:**
  The `is_pii` helper checks if a field contains PII by looking it up in `schema.fields`:
  ```rust
  if let Some(field_name) = field {
      if let Some(field_schema) = schema.fields.get(field_name) {
          ...
      }
  }
  ```
  However, in real execution flows, fields might be nested or passed as JSON paths (e.g., `user/email` or `/tunable/email`). The `schema.fields.get` expects a clean, top-level field name (e.g., `email`). If the path string is passed directly, the lookup fails to match, bypassing all PII checks. This allows raw personal data to bypass the chain-only filter gate and end up in the unencrypted Qdrant vector database.
* **Remediation:**
  Extract the trailing segment of the path or implement path parsing to isolate the true field name before checking constraints:
  ```rust
  let clean_field_name = field_name.rsplit('/').next().unwrap_or(field_name);
  ```

---

### Schema-as-Code Findings

To comply with strict schema-as-code engineering practices, all system components must represent their API payload schemas and data contracts as versioned Protocol Buffers or central OSCAL schema objects rather than ad-hoc inline structures.

#### SCHEMA-01: Ad-hoc Inline JSON Schema Definitions in Tool Registries
* **Files:**
  * `crates/op-cognitive-mcp/src/cognitive_tools.rs:53-90`
  * `crates/op-cognitive-mcp/src/typed_tools.rs:145-163`
  * `crates/op-cognitive-mcp/src/typed_tools.rs:276-299`
  * `crates/op-cognitive-mcp/src/typed_tools.rs:381-395`
* **Description:**
  The `input_schema` for tools is manually constructed inside Rust using the `json!` macro. This creates unversioned, ad-hoc API specifications hardcoded inside system source files.
* **Remediation:**
  Generate JSON schemas dynamically from versioned Protocol Buffer models or central schema structures using code generator steps, or reference version-controlled schemas.

---

#### SCHEMA-02: Untyped Arbitrary Payloads in Cozo DB Entites
* **File:** `crates/op-cognitive-mcp/src/memory_store.rs:58-69`
* **Description:**
  The `MemoryNamespace` and `MemoryEntry` structs store crucial domain data contracts inside untyped, unstructured `serde_json::Value` structures (`metadata` and `value`). There is no structural or schema validation on these fields at the database serialization level.
* **Remediation:**
  Replace untyped values with explicit, versioned Protobuf messages mapped to bytes or enforce a schema-validator middleware check inside `store_entry`.