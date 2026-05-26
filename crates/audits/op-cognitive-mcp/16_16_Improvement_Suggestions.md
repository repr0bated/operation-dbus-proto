### ARCHITECTURE

1. **ABI Alignment / Mismatched IdentitySled Structs**
   * **Suggestion**: Consolidate the `IdentitySled` struct definitions into a unified workspace crate (such as `op-core` or `op-identity`) and eliminate duplicate, out-of-sync declarations.
   * **Rationale**: Currently, `interceptor.rs:13` and `qdrant_shuttle.rs:24` define their own separate versions of the `IdentitySled` ABI layout. The version in `interceptor.rs` is significantly larger (incorporating padding, `schema_uuid`, `subid`, etc.) than the version in `qdrant_shuttle.rs`. Because both parts of the codebase map these structures directly over a shared memory-mapped file (`/dev/shm/plugin_schema.dat`), this mismatch shifts the offset of the schema bytes. `qdrant_shuttle.rs:283` attempts to parse JSON schema bytes from `mmap[size_of::<IdentitySled>()..]` (offset 80), but if the file is written using the larger layout (offset 208), the shuttle will read raw cryptographic variables and padding as JSON, causing immediate parser failures or memory corruption.
   * **Example**: `crates/op-cognitive-mcp/src/interceptor.rs:13`

2. **Deduplication State Durability**
   * **Suggestion**: Replace the in-memory sliding window `VecDeque` in the `ActivityFilter` with CozoDB-backed storage or a persistent, shared cache.
   * **Rationale**: In `activity_filter.rs:197`, the deduplication window is kept purely in-memory. Because the cognitive server acts as a system daemon (as indicated by the system D-Bus registration in `server.rs:194`), restarting the daemon entirely wipes the history of processed hashes. An attacker can exploit this by crashing the daemon or triggering a restart, allowing them to bypass sliding-window deduplication constraints and flood the blockchain audit trail with identical event payloads.
   * **Example**: `crates/op-cognitive-mcp/src/activity_filter.rs:197`

3. **Subprocess Lifecycle Watchdog**
   * **Suggestion**: Implement a robust supervisor pattern or health-check monitor to automatically restart the child subprocess inside `ExternalMcpClient`.
   * **Rationale**: The NotebookLM sidecar is spawned as an external node process over stdio (`notebooklm.rs:125`). If this sidecar process unexpectedly dies, exits, or hangs, the parent MCP server continues running in a degraded state without any automated mechanism to respawn or supervise the child.
   * **Example**: `crates/op-cognitive-mcp/src/notebooklm.rs:125`

---

### API ERGONOMICS

4. **Eliminate Boolean Blindness in Significance Derivation**
   * **Suggestion**: Replace consecutive boolean arguments in `derive_significance` with a dedicated struct or enum flags.
   * **Rationale**: The function `derive_significance` (`activity_filter.rs:44`) accepts three trailing `bool` parameters (`is_write`, `constraint_failed`, `autonomous`). This forces callers to use opaque invocations such as `derive_significance(&schema, None, true, false, false)`, which is highly error-prone and severely degrades code readability.
   * **Example**: `crates/op-cognitive-mcp/src/activity_filter.rs:44`

5. **Strongly Typed Quota Check Return**
   * **Suggestion**: Return a structured enum representing the quota decision instead of an unpacked raw tuple.
   * **Rationale**: The function `check_and_increment` (`quota.rs:49`) returns a tuple of type `(bool, u32, u32)`. Callers must manually unpack this tuple (as seen in `grpc_service.rs:65` and `grpc_service.rs:196`) and memorize which index corresponds to eligibility, remaining allowance, and limit, which introduces unnecessary complexity.
   * **Example**: `crates/op-cognitive-mcp/src/quota.rs:49`

---

### PERFORMANCE

6. **Avoid Vector Allocations on Hot D-Bus Execution Path**
   * **Suggestion**: Pass the raw string slice to a zero-copy parser or use a mutable thread-local scratch buffer pool instead of allocating a fresh `Vec`.
   * **Rationale**: In `dbus_interface.rs:56`, the `parse_simd` function clones the D-Bus payload into an owned vector using `s.as_bytes().to_vec()`. Since `simd_json` requires a mutable buffer for zero-copy parsing, this allocation occurs on every single tool execution initiated via D-Bus, creating severe memory allocator pressure under high tool call loads.
   * **Example**: `crates/op-cognitive-mcp/src/dbus_interface.rs:56`

7. **Zero-Copy Ingestion for RAG Pipeline Chunks**
   * **Suggestion**: Leverage `bytes::Bytes` or `Arc<str>` inside the `Chunk` struct instead of allocating owned `String` fields.
   * **Rationale**: The `Chunk` struct in `rag_pipeline.rs:60` stores `content` and `embed_text` as heavily redundant, newly-allocated `String` structures. During massive repository RAG ingests, hundreds of thousands of lines are read, split, and duplicated, producing significant GC/allocator overhead that can be avoided with zero-copy reference counts.
   * **Example**: `crates/op-cognitive-mcp/src/rag_pipeline.rs:60`

8. **Unified Outbound Connection Pooling**
   * **Suggestion**: Share a single, globally-configured `reqwest::Client` across the `VoyageClient` and the `GeminiFallback` client rather than spawning individual client pools.
   * **Rationale**: Both `voyage.rs:40` and `gemini_fallback.rs:136` initialize separate instances of `reqwest::Client`. Having separate client pools prevents reuse of TCP handshakes and TLS sessions across distinct cognitive providers, increasing latency for outbound API operations.
   * **Example**: `crates/op-cognitive-mcp/src/voyage.rs:40`

---

### OBSERVABILITY

9. **Forensic Context on Authentication Failures**
   * **Suggestion**: Log detailed, structured diagnostic information when a temporal hash or footprint validation mismatch is encountered.
   * **Rationale**: In `interceptor.rs:46`, when a temporal hash mismatch occurs, the interceptor rejects the request with a generic `Status::permission_denied`. It does not trace the mismatched hash values, the caller's wireguard public key, or the identity index, hindering the ability of administrators to diagnose synchronization issues or detect spoofing attempts.
   * **Example**: `crates/op-cognitive-mcp/src/interceptor.rs:46`

10. **Structured Tracing Fields for Chat/Agent Sessions**
    * **Suggestion**: Migrate log statements from string formatting to structured key-value fields.
    * **Rationale**: Throughout the ingress methods in `grpc_service.rs` (such as `list_notebooks` at line 149 and `add_source` at line 290), critical identifiers are logged using unstructured message formatting (`info!(kind_filter = %req.kind_filter, "ListNotebooks")`). Using structured tracing attributes consistently across all services allows log aggregators to automatically index session flows.
    * **Example**: `crates/op-cognitive-mcp/src/grpc_service.rs:149`

11. **Contextual Thread-Local Propagation**
    * **Suggestion**: Inject and propagate active tracing spans across thread boundaries in `start_dual`.
    * **Rationale**: In `server.rs:154`, the gRPC server transport loop is spawned inside an un-annotated `tokio::spawn` task. This strips away active span context, causing startup and runtime logs emitted inside the newly spawned thread to lose correlation with the parent process context.
    * **Example**: `crates/op-cognitive-mcp/src/server.rs:154`

---

### STORAGE

12. **Persist Session History in CozoDB**
    * **Suggestion**: Replace the transient, in-memory `DashMap` storage inside `SessionManager` with persistent relations inside the already existing `CozoGraphShuttle`.
    * **Rationale**: In `session.rs:37`, conversation history and query history are kept in a local `DashMap`. If the `op-cognitive-mcp` service crashes or restarts, all active sessions and grounding turns are destroyed. Since CozoDB is already configured as the server's central persistent database (`server.rs:42`), moving the session tables to CozoDB prevents conversation loss and ensures continuous context tracking.
    * **Example**: `crates/op-cognitive-mcp/src/session.rs:37`

13. **Bulk Database-Side Expiration Cleanup**
    * **Suggestion**: Implement the database expiration cleanup as a single datalog rule executed directly inside CozoDB.
    * **Rationale**: In `memory_store.rs:188`, `cleanup_expired` performs an N+1 database operation: it queries all expired records into memory, iterates through them in Rust, and then triggers separate delete scripts per key. Performing the entire cleanup loop in a single relational Datalog rule avoids multiple sequential script executions.
    * **Example**: `crates/op-cognitive-mcp/src/memory_store.rs:188`

14. **Database-Side Key Pattern Querying**
    * **Suggestion**: Push the `key_pattern` regex or substring match down into the CozoDB datalog query instead of filtering results in-memory.
    * **Rationale**: In `memory_store.rs:163`, `query_entries` queries the database for all entries matching a namespace and subsequently filters out non-matching keys using a Rust `retain` iterator loop. If a namespace contains thousands of elements, this causes significant transmission and memory overhead.
    * **Example**: `crates/op-cognitive-mcp/src/memory_store.rs:163`

---

### SCHEMA-AS-CODE COMPLIANCE

15. **Strict Schema Type-Safety Over D-Bus Interfaces**
    * **Suggestion**: Define strongly-typed structs or marshaled protobuf structures for D-Bus payload inputs and returns, instead of using raw unvalidated JSON strings.
    * **Rationale**: The interface methods in `dbus_interface.rs:31` pass inputs and outputs as raw serialized JSON strings (`s` type signature). This circumvents the static schema capabilities of the workspace and increases the likelihood that a mismatch in field expectations between the agent and host registries will cause silent runtime failures.
    * **Example**: `crates/op-cognitive-mcp/src/dbus_interface.rs:31`

16. **Versioned Protobufs/OSCAL schemas for Metadata Payloads**
    * **Suggestion**: Refactor `serde_json::Value` payload configurations into strictly versioned, protobuf-validated structs.
    * **Rationale**: In `activity_filter.rs:136` (the `payload` field), `memory_store.rs:77` (the `metadata` field), and `memory_store.rs:91` (the `value` field), arbitrary untyped JSON is permitted. Lacking versioned schema enforcement, database relations can easily suffer schema drift, which can lead to parsing errors during vector search indexing.
    * **Example**: `crates/op-cognitive-mcp/src/memory_store.rs:91`