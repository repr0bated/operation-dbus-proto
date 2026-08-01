# Production Quality & Security Audit: Error Handling & Schema-as-Code

This audit provides a security and quality evaluation of error handling mechanisms and schema discipline across the `op-cognitive-mcp` crate.

---

## 1. Error Handling Diagnostics & Counts

Below is the exhaustive audit of error-propagation operators, fallback defaults, and panic-inducing macros throughout the provided codebase.

### Global Operator & Macro Tally

| File | `.unwrap()` | `.expect()` | `.unwrap_or()` / variants | `?` operator | `todo!()` | `unimplemented!()` | `panic!()` |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| `src/activity_filter.rs` | 2 (test) | 0 | 0 | 0 | 0 | 0 | 0 |
| `src/cognitive_tools.rs` | 0 | 0 | 6 | 25 | 0 | 0 | 0 |
| `src/notebooklm.rs` | 0 | 0 | 9 | 4 | 0 | 0 | 0 |
| `src/voyage.rs` | 0 | 0 | 1 | 4 | 0 | 0 | 0 |
| `src/qdrant_shuttle.rs` | 2 (test) | 0 | 8 | 28 | 0 | 0 | 0 |
| `src/session.rs` | 6 (test) | 0 | 0 | 3 | 0 | 0 | 0 |
| `src/quota.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| `src/grpc_service.rs` | 0 | 0 | 22 | 18 | 0 | 0 | 0 |
| `src/typed_tools.rs` | 0 | 0 | 2 | 12 | 0 | 0 | 0 |
| `src/gemini_fallback.rs` | 0 | 0 | 9 | 10 | 0 | 0 | 0 |
| `src/tool_profiles.rs` | 2 (test) | 0 | 1 | 0 | 0 | 0 | 0 |
| `src/doctor.rs` | 1 (test) | 0 | 3 | 0 | 0 | 0 | 0 |
| `src/interceptor.rs` | 1 | 0 | 2 | 4 | 0 | 0 | 0 |
| `src/memory_store.rs` | 0 | 0 | 37 | 35 | 0 | 0 | 0 |
| `src/cozo_shuttle.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| `src/main.rs` | 0 | 0 | 0 | 5 | 0 | 0 | 0 |
| `src/lib.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| `src/dbus_interface.rs` | 0 | 0 | 1 | 0 | 0 | 0 | 0 |
| `src/server.rs` | 0 | 4 | 0 | 14 | 0 | 0 | 0 |
| `src/rag_pipeline.rs` | 9 | 0 | 21 | 15 | 0 | 0 | 0 |
| `src/bin/op-cog-admin.rs` | 0 | 0 | 0 | 4 | 0 | 0 | 0 |
| `src/bin/rag-ingest.rs` | 0 | 0 | 1 | 11 | 0 | 0 | 0 |
| **TOTALS** | **23** | **4** | **121** | **193** | **0** | **0** | **0** |

---

## 2. Deep-Dive on Plain `.unwrap()` Sites

This section highlights the first 5 encountered `.unwrap()` sites from the codebase, detailing their context, exploitation/panic risk, and recommended remedies.

### Site 1: Raw Header Retrieval
* **Location**: `crates/op-cognitive-mcp/src/interceptor.rs:41`
* **Context**:
  ```rust
  let request_footprint = footprint_value
      .as_ref()
      .unwrap()
      .to_str()
      .map_err(|_| Status::invalid_argument("Invalid footprint encoding"))?;
  ```
* **Risk Evaluation**: This is a production gRPC interceptor. While `footprint_value` is checked for `is_none()` on line 20, relying on `.unwrap()` on checked option values introduces panic vulnerabilities during subsequent code refactoring or multithreaded optimization. If the check is bypassed or nested incorrectly, any unauthenticated request will trigger a thread-level panic.
* **Recommendation**: Replace with a safe match pattern or use `ok_or_else`:
  ```rust
  let request_footprint = footprint_value
      .as_ref()
      .ok_or_else(|| Status::unauthenticated("Missing footprint value"))?
      .to_str()
      .map_err(|_| Status::invalid_argument("Invalid footprint encoding"))?;
  ```

### Site 2: Fragile Vector Extraction
* **Location**: `crates/op-cognitive-mcp/src/rag_pipeline.rs:649`
* **Context**:
  ```rust
  if !meta.doc_comments.is_empty() {
      header.push_str(&format!("DOCS: {}\n", meta.doc_comments.first().unwrap()));
  }
  ```
* **Risk Evaluation**: Although guarded by `!meta.doc_comments.is_empty()`, wrapping `.first().unwrap()` is an anti-pattern. If a concurrent change or internal struct refactoring changes the underlying collection representation, this call will panic.
* **Recommendation**: Directly match the element safely:
  ```rust
  if let Some(doc) = meta.doc_comments.first() {
      header.push_str(&format!("DOCS: {doc}\n"));
  }
  ```

### Site 3: Global Regex Compilations
* **Location**: `crates/op-cognitive-mcp/src/rag_pipeline.rs:432`
* **Context**:
  ```rust
  let re_item = RE_ITEM.get_or_init(|| {
      Regex::new(
          r"^\s*pub(?:\(crate\))?\s+(fn|struct|enum|trait|type|mod|const|static|impl)\s+(\w+)",
      )
      .unwrap()
  });
  ```
* **Risk Evaluation**: The regular expression pattern is hardcoded and validated. It will only panic if the developer provides a malformed regex string in the code. However, doing so at runtime on first match invocation makes startup validation impossible.
* **Recommendation**: Safe as-is for release, but replacing it with `lazy_static!` or performing static compile-time regex evaluation prevents runtime panic paths entirely.

### Site 4: Activity Filter Event Verification (Test Suite)
* **Location**: `crates/op-cognitive-mcp/src/activity_filter.rs:244`
* **Context**:
  ```rust
  let d = filter.evaluate(&event, Some(&schema)).await.unwrap();
  ```
* **Risk Evaluation**: This is situated within the unit testing module (`#[cfg(test)]`). Panics here are acceptable, as they cause the test harness to correctly flag assertions.
* **Recommendation**: Retain `.unwrap()` for unit tests to simplify assertion testing.

### Site 5: Health Probe Verification (Test Suite)
* **Location**: `crates/op-cognitive-mcp/src/activity_filter.rs:281`
* **Context**:
  ```rust
  let d = filter.evaluate(&event, None).await.unwrap();
  ```
* **Risk Evaluation**: Test suite code. Safe to panic on test failure.
* **Recommendation**: Retain `.unwrap()` inside the test module.

---

## 3. Lock Poisoning & Concurrency Risk Evaluation

In standard Rust systems, executing `.unwrap()` on `std::sync::MutexGuard` or `std::sync::RwLockReadGuard` results in **Lock Poisoning**. If a thread panics while holding a lock, the lock is poisoned, preventing other threads from accessing the shared state and triggering widespread service denial.

### Evaluation of Locks in Crate
* **File `src/activity_filter.rs`**: Uses `self.tunables.write().await`, `self.window.write().await`, and `self.window.read().await`. These utilize **Tokio's asynchronous lock implementations** (`tokio::sync::RwLock`).
* **File `src/notebooklm.rs`**: Uses `Arc<Mutex<ExternalMcpClient>>` locked via `self.client.lock().await` (Tokio's asynchronous `Mutex`).
* **File `src/quota.rs`**: Uses Tokio's `RwLock` throughout status checking and tier modifications.

### Conclusion on Lock Poisoning Risk
Because **Tokio's async Mutex and RwLock implementations do not feature lock poisoning**, thread panics do not poison the lock state for other futures. Therefore, there is **no directly exploitable lock poisoning risk** present in the concurrency primitives of this crate.

---

## 4. Schema-as-Code Violations

The codebase uses a "schema-as-code" discipline based on Protocol Buffers and OSCAL. Ad-hoc structs, dynamic strings, or manual JSON structures representing data contracts violate this discipline and increase integration risks.

### Identified Violations

#### Violation 1: Hardcoded JSON Input Schemas
* **File & Line**: `crates/op-cognitive-mcp/src/cognitive_tools.rs:77-111`
* **Description**: The MCP tool input schema is defined as a manual, hardcoded `simd_json::json!` value block inside `input_schema()` instead of compiling a unified Protocol Buffer representation.
* **Remedy**: Compile the schema from a central `.proto` descriptor set and serialize it to JSON dynamically.

#### Violation 2: Ad-Hoc OSCAL Extraction in Shared Memory
* **File & Line**: `crates/op-cognitive-mcp/src/interceptor.rs:52-54`
* **Description**:
  ```rust
  let control_source = unsafe { &(*sled_ptr).control_source };
  let end = control_source.iter().position(|&b| b == 0).unwrap_or(32);
  let oscal_header = std::str::from_utf8(&control_source[..end]).unwrap_or("");
  ```
  The interceptor directly casts shared memory bytes into a raw struct and performs unsafe conversions to parse the OSCAL control header out of a 32-byte array.
* **Remedy**: Use a structured Protocol Buffer or a schema compiler generated binary payload wrapper rather than raw C-style pointer casting and unsafe array indexing.

#### Violation 3: Manual String Schema Generation
* **File & Line**: `crates/op-cognitive-mcp/src/qdrant_shuttle.rs:356-425`
* **Description**: The `render_schema_embedding_text` function dynamically builds text representations of schema categories, types, constraints, and conditions using ad-hoc text string formatting (`lines.push(format!(...))`).
* **Remedy**: Replace this serialization logic with a structured OSCAL/Protobuf schema walker that outputs normalized metadata.