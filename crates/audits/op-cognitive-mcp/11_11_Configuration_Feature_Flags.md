# Code Quality and Security Audit Report

## 1. Environment Variable Configuration Analysis

### All `std::env::var` / `std::env::var_os` / `env::var` Reads

| Env Var Name | File Location | Default / Handling |
|:---|:---|:---|
| `COGNITIVE_MCP_NOTEBOOKLM_ENABLED` | `crates/op-cognitive-mcp/src/notebooklm.rs:37` | Defaults to `true` via helper `env_flag`. |
| `COGNITIVE_MCP_NOTEBOOKLM_COMMAND` | `crates/op-cognitive-mcp/src/notebooklm.rs:39` | Defaults to `"npx"`. |
| `COGNITIVE_MCP_NOTEBOOKLM_ARGS` | `crates/op-cognitive-mcp/src/notebooklm.rs:41` | Defaults to `["-y", "notebooklm-mcp@latest"]` via helper `env_list`. |
| `COGNITIVE_MCP_NOTEBOOKLM_SERVER_NAME` | `crates/op-cognitive-mcp/src/notebooklm.rs:47` | Defaults to `"notebooklm"`. |
| `COGNITIVE_MCP_NOTEBOOKLM_PROFILE` | `crates/op-cognitive-mcp/src/notebooklm.rs:49` | Defaults to `"minimal"`. |
| `COGNITIVE_MCP_NOTEBOOKLM_DISABLED_TOOLS` | `crates/op-cognitive-mcp/src/notebooklm.rs:51` | Handled via `.ok()` (resolves to `Option<String>`). |
| `NOTEBOOKLM_COOKIE` | `crates/op-cognitive-mcp/src/notebooklm.rs:64` | Checked with `if let Ok(cookie)`. |
| `VOYAGE_API_KEY` | `crates/op-cognitive-mcp/src/voyage.rs:36` | No default. Propagated via `anyhow::Result`. |
| `VOYAGE_MODEL` | `crates/op-cognitive-mcp/src/voyage.rs:38` | Defaults to `"voyage-law-2"`. |
| `COGNITIVE_MCP_QDRANT_URL` | `crates/op-cognitive-mcp/src/qdrant_shuttle.rs:41` | Defaults to `"http://127.0.0.1:6334"`. |
| `COGNITIVE_MCP_QDRANT_COLLECTION` | `crates/op-cognitive-mcp/src/qdrant_shuttle.rs:43` | Defaults to `"ctl_plane_reasoning_episodes"`. |
| `COGNITIVE_MCP_SCHEMA_SLED_PATH` | `crates/op-cognitive-mcp/src/qdrant_shuttle.rs:45` | Defaults to `"/dev/shm/plugin_schema.dat"`. |
| `COGNITIVE_MCP_VOYAGE_API_KEY` | `crates/op-cognitive-mcp/src/qdrant_shuttle.rs:179` | Handled with fallback to `VOYAGE_API_KEY`. |
| `VOYAGE_API_KEY` | `crates/op-cognitive-mcp/src/qdrant_shuttle.rs:180` | No default. Propagated via `anyhow::Result`. |
| `COGNITIVE_MCP_VOYAGE_API_URL` | `crates/op-cognitive-mcp/src/qdrant_shuttle.rs:183` | Defaults to `"https://api.voyageai.com/v1/embeddings"`. |
| `COGNITIVE_MCP_VOYAGE_QUERY_MODEL` | `crates/op-cognitive-mcp/src/qdrant_shuttle.rs:185` | Defaults to `"voyage-4"`. |
| `COGNITIVE_MCP_VOYAGE_OUTPUT_DIMENSION` | `crates/op-cognitive-mcp/src/qdrant_shuttle.rs:187` | Defaults to `1024` if parsing fails. |
| `COGNITIVE_MCP_GEMINI_API_KEY` | `crates/op-cognitive-mcp/src/gemini_fallback.rs:32` | Handled with fallback to `GEMINI_API_KEY`. |
| `GEMINI_API_KEY` | `crates/op-cognitive-mcp/src/gemini_fallback.rs:33` | No default. Returns `None` via `Option` if absent. |
| `COGNITIVE_MCP_GEMINI_ENABLED` | `crates/op-cognitive-mcp/src/gemini_fallback.rs:36` | Defaults to `true`. |
| `COGNITIVE_MCP_GEMINI_API_URL` | `crates/op-cognitive-mcp/src/gemini_fallback.rs:40` | Defaults to `"https://generativelanguage.googleapis.com/v1beta"`. |
| `COGNITIVE_MCP_GEMINI_MODEL` | `crates/op-cognitive-mcp/src/gemini_fallback.rs:42` | Defaults to `"gemini-2.5-flash"`. |
| `COGNITIVE_MCP_TOOL_PROFILE` | `crates/op-cognitive-mcp/src/tool_profiles.rs:81` | Checked with fallback to `NOTEBOOKLM_PROFILE`. |
| `NOTEBOOKLM_PROFILE` | `crates/op-cognitive-mcp/src/tool_profiles.rs:82` | Defaults to standard tool profile if absent. |
| `COGNITIVE_MCP_AUTH_METHOD` | `crates/op-cognitive-mcp/src/doctor.rs:115` | Defaults to `"chrome_profile"`. |
| `VOYAGE_API_KEY` | `crates/op-cognitive-mcp/src/rag_pipeline.rs:98` | Handled with fallback to `COGNITIVE_MCP_VOYAGE_API_KEY`. |
| `COGNITIVE_MCP_VOYAGE_API_KEY` | `crates/op-cognitive-mcp/src/rag_pipeline.rs:99` | No default. Propagated via `anyhow::Result`. |
| `COGNITIVE_MCP_VOYAGE_MODEL` | `crates/op-cognitive-mcp/src/rag_pipeline.rs:101` | Checked with fallback to `VOYAGE_MODEL`. |
| `VOYAGE_MODEL` | `crates/op-cognitive-mcp/src/rag_pipeline.rs:102` | Defaults to `"voyage-4-lite"`. |
| `COGNITIVE_MCP_QDRANT_URL` | `crates/op-cognitive-mcp/src/rag_pipeline.rs:600` | Defaults to `"http://127.0.0.1:6334"`. |
| `RUST_LOG` | `crates/op-cognitive-mcp/src/bin/rag-ingest.rs:65` | Defaults to `"info"`. |
| `HOME` | `crates/op-cognitive-mcp/src/bin/rag-ingest.rs:70` | Handled via `std::env::var_os`. |

### Flagged Environment Variables (No Default & Missing/Incomplete Error Handling)

*   **`VOYAGE_API_KEY` / `COGNITIVE_MCP_VOYAGE_API_KEY`**
    *   **Locations**: `crates/op-cognitive-mcp/src/voyage.rs:36`, `crates/op-cognitive-mcp/src/qdrant_shuttle.rs:179-180`, `crates/op-cognitive-mcp/src/rag_pipeline.rs:98-99`.
    *   **Defect**: These variables have no default value. While the code attempts to handle them using `anyhow::Result` context propagation (`.context(...)?`), failure to set this key at startup prevents initialization of the `VoyageClient` or `RagPipeline`. In `QdrantSemanticShuttle::new()`, a failure of `VoyageClient::from_env()?` gracefully degrades, but in `rag_pipeline.rs` and `voyage.rs`, missing credentials bubble up directly and can cause unhandled command-line tool crashes if not caught.

---

## 2. Cargo Features & Additivity

### Crate Features: `op-cognitive-mcp`
*   No custom features are defined inside `crates/op-cognitive-mcp/Cargo.toml`.

### Crate Features: `op-dbus` (Workspace Manifest)
*   **`default = ["grpc"]`**
*   **`grpc = []`**

### Feature Additivity
*   Standard Cargo features are **additive**. The default feature set enables `"grpc"`, and disabling default features allows compiling without the gRPC transport components.

---

## 3. Hardcoded Paths, Ports, & Loopback Addresses

*   **Hardcoded Ports and Bind Addresses**:
    *   `crates/op-cognitive-mcp/src/main.rs:15`: `"0.0.0.0:3003"` (HTTP/SSE default socket bind)
    *   `crates/op-cognitive-mcp/src/main.rs:19`: `"0.0.0.0:50052"` (gRPC default socket bind)
    *   `crates/op-cognitive-mcp/src/qdrant_shuttle.rs:15`: `"http://127.0.0.1:6334"` (Qdrant service port bind)
    *   `crates/op-cognitive-mcp/src/rag_pipeline.rs:601`: `"http://127.0.0.1:6334"` (Qdrant loopback endpoint)

*   **Hardcoded Filesystem Paths**:
    *   `crates/op-cognitive-mcp/src/main.rs:26`: `"/var/lib/op-cognitive-mcp/memory.db"` (CozoDB persistence path)
    *   `crates/op-cognitive-mcp/src/qdrant_shuttle.rs:17`: `"/dev/shm/plugin_schema.dat"` (Identity/Schema Sled file)
    *   `crates/op-cognitive-mcp/src/interceptor.rs:25`: `"/dev/shm/plugin_schema.dat"` (Ghostbridge Memory path)
    *   `crates/op-cognitive-mcp/src/bin/op-cog-admin.rs:11`: `"/var/lib/op-dbus/cognitive.db"` (CozoDB administration database file)

*   **Hardcoded API Domains**:
    *   `crates/op-cognitive-mcp/src/voyage.rs:51`: `"https://api.voyageai.com/v1/embeddings"`
    *   `crates/op-cognitive-mcp/src/qdrant_shuttle.rs:19`: `"https://api.voyageai.com/v1/embeddings"`
    *   `crates/op-cognitive-mcp/src/rag_pipeline.rs:118`: `"https://api.voyageai.com/v1/embeddings"`
    *   `crates/op-cognitive-mcp/src/gemini_fallback.rs:16`: `"https://generativelanguage.googleapis.com/v1beta"`

---

## 4. Production Security & Quality Findings

### CRITICAL: Shared Memory ABI Mismatch & Out-of-Bounds Read in gRPC Interceptor
*   **Citations**: 
    *   `crates/op-cognitive-mcp/src/interceptor.rs:7-17` (Definition of `IdentitySled`)
    *   `crates/op-cognitive-mcp/src/qdrant_shuttle.rs:25-31` (Definition of `IdentitySled`)
    *   `crates/op-cognitive-mcp/src/interceptor.rs:25-34` (Raw pointer cast on mapped memory)
    *   `crates/op-cognitive-mcp/src/qdrant_shuttle.rs:289-294` (Safety-guarded mapping logic)
*   **Description**: 
    The shared memory sled file (`/dev/shm/plugin_schema.dat`) is accessed concurrently by the `QdrantSemanticShuttle` and the `ghostbridge_interceptor`. However, they define the `IdentitySled` structure with radically different sizes, fields, and byte offsets:
    *   In `qdrant_shuttle.rs`, the struct is **73 bytes** long and has no padding between `is_valid` (bool) and `hashed_footprint`.
    *   In `interceptor.rs`, the struct is **223 bytes** long, contains a 7-byte padding array (`_pad: [u8; 7]`), and defines multiple additional fields (`schema_uuid`, `subid`, `control_source`, `nextdns_profile`).
*   **Impact & Exploitability**:
    1. **Out-of-Bounds Memory Read**: `interceptor.rs` performs a direct pointer cast (`mmap.as_ptr() as *const IdentitySled`) without any safety bounds check. If the sled file on disk was written by a component using the smaller 73-byte schema, reading any fields past `is_valid` in `interceptor.rs` causes an out-of-bounds pointer dereference, leading to immediate server crashes or potential memory leakage.
    2. **Temporal Hash Mismatch (Auth Failure)**: The explicit padding `_pad: [u8; 7]` on line 11 of `interceptor.rs` shifts the `hashed_footprint` field to offset 48, whereas `qdrant_shuttle.rs` evaluates `hashed_footprint` at offset 41. Thus, the two modules read different memory locations for the identical security parameter, causing the interceptor to reject valid client calls.

---

### CRITICAL: Missing Bounds & Size Checking on Memory Map Dereferencing
*   **Citations**:
    *   `crates/op-cognitive-mcp/src/interceptor.rs:28-34`
*   **Description**:
    The `ghostbridge_interceptor` function maps `/dev/shm/plugin_schema.dat` and immediately executes an unsafe dereference on the resulting pointer:
    ```rust
    let mmap = unsafe {
        MmapOptions::new()
            .map(&file)
            .map_err(|_| Status::internal("Mmap failed"))?
    };
    let sled_ptr = mmap.as_ptr() as *const IdentitySled;

    let is_valid = unsafe { (*sled_ptr).is_valid };
    ```
    If an attacker or local system process clears or truncates this shared-memory file to 0 bytes, `mmap.as_ptr()` will point to invalid or unmapped memory regions. Accessing `(*sled_ptr).is_valid` will cause a Segmentation Fault, bringing down the entire gRPC control plane.

---

### SCHEMA-AS-CODE DISCIPLINE: Ad-Hoc Data Contracts and Payloads
*   **Citations**:
    *   `crates/op-cognitive-mcp/src/activity_filter.rs:119` (`ActivityEvent` struct defining system contracts)
    *   `crates/op-cognitive-mcp/src/activity_filter.rs:166` (Freeform payload `serde_json::Value`)
    *   `crates/op-cognitive-mcp/src/session.rs:21` (`ConversationSession`)
    *   `crates/op-cognitive-mcp/src/session.rs:33` (`QueryTurn`)
    *   `crates/op-cognitive-mcp/src/quota.rs:15` (`QuotaTier`)
    *   `crates/op-cognitive-mcp/src/gemini_fallback.rs:56` (`GeminiRequest`)
    *   `crates/op-cognitive-mcp/src/gemini_fallback.rs:88` (`GeminiResponse`)
    *   `crates/op-cognitive-mcp/src/voyage.rs:13` (`EmbeddingRequest`)
    *   `crates/op-cognitive-mcp/src/rag_pipeline.rs:79` (`Chunk`)
*   **Description**:
    The crate defines critical transaction payloads, state objects, and network communications via ad-hoc, manually serialized Rust structs and loose `serde_json` objects. Under a strict schema-as-code discipline, all cross-process data structures, transaction envelopes (`ActivityEvent`), and remote LLM request/response representations must be generated from versioned schemas (such as Protocol Buffers or structured OSCAL schemas) rather than using ad-hoc serializable structs and free-form JSON blobs.

---

### CODE QUALITY: Insecure SetupAuth Fail-Open Behavior
*   **Citations**:
    *   `crates/op-cognitive-mcp/src/grpc_service.rs:693-705`
*   **Description**:
    The `setup_auth` gRPC service inspects the file permissions of private credential profiles. If it detects a highly permissive mode (e.g., world-readable folder configurations violating private `0o600` constraints), it logs a warning but proceeds to complete the operation successfully:
    ```rust
    if mode & 0o077 != 0 {
        warn!(
            path = %req.credential,
            mode = format!("{:o}", mode),
            "Chrome profile has overly permissive permissions; should be 0o600"
        );
    }
    ```
    This constitutes a "fail-open" design. For secure execution environments, the server must actively reject overly permissive authorization layouts to prevent unauthorized local users from reading secrets.