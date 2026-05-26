| Pattern | File:Line | Crate Approach | Corpus Best Practice | Gap | Rating |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `unsafe_block` | `crates/op-grpc-bridge/src/interceptor.rs:50` | Memory-maps a file via `unsafe { MmapOptions::new().map(&file) }` within the request-handling interceptor. | Validate file dimensions, ownership, alignment, and concurrency guarantees before mapping. | File mapped without size/state validation, creating potential for `SIGBUS` or UB on truncation. | Major Gap |
| `unsafe_block` | `crates/op-grpc-bridge/src/interceptor.rs:57` | Dereferences unvalidated raw pointer `sled_ptr` cast directly from mmap bytes to access `is_valid`. | Implement schema-based deserialization (e.g., FlatBuffers/Protobuf) or check size, alignment, and type invariants. | Directly violates the "schema-as-code" discipline. Instantly triggers undefined behavior if pointer is unaligned, memory is truncated, or the byte representation of `is_valid` is not a valid `bool`. | Critical Gap |
| `unsafe_block` | `crates/op-grpc-bridge/src/interceptor.rs:58` | Dereferences raw pointer `sled_ptr` to access `hashed_footprint`. | Use versioned schemas and safe deserialization wrappers. | Direct raw pointer access to ad-hoc binary state from memory-mapped file without bounds, size, or alignment verification. | Critical Gap |
| `unwrap_expect` | `crates/op-grpc-bridge/src/interceptor.rs:72` | Uses `.unwrap()` to convert header values to strings in a gRPC interceptor. | Propagate errors gracefully to the client as a `Status` representation. | Panics the thread if the header is absent or invalid, exposing the gRPC server to an easy Denial of Service (DoS) vector. | Major Gap |
| `std_fs_in_async` | `crates/op-grpc-bridge/src/interceptor.rs:10` | Uses blocking `std::fs::File` operations inside an async interceptor. | Offload synchronous block operations to a dedicated thread pool via `tokio::task::spawn_blocking` or use async alternatives. | Blocks the Tokio runtime thread pool worker, reducing server throughput and leading to thread starvation under high concurrency. | Major Gap |
| `format_json_manual` | `crates/op-grpc-bridge/src/proto_gen.rs:341` | Generates protobuf definitions via custom, ad-hoc string formatting of `FieldType`. | Use structured AST-based schema generators (e.g., `prost-build` or `prost-types`). | String interpolation for schema gen is fragile and prone to syntax errors. | Minor Gap |
| `unwrap_expect` | `crates/op-grpc-bridge/src/proto_gen.rs:48` | Uses `.unwrap()` on `writeln!` for code generation. | Propagate write errors using the `?` operator. | Panic in a code generator or build-script tool is generally acceptable, as it aborts the build cleanly. | Compliant |
| `unwrap_expect` | `crates/op-grpc-bridge/src/proto_gen.rs:49` | Uses `.unwrap()` on `writeln!` for package names. | Propagate write errors. | Standard practice in build tools. | Compliant |
| `unwrap_expect` | `crates/op-grpc-bridge/src/proto_gen.rs:50` | Uses `.unwrap()` on empty line writes. | Propagate write errors. | Standard practice in build tools. | Compliant |
| `unwrap_expect` | `crates/op-grpc-bridge/src/proto_gen.rs:51` | Uses `.unwrap()` on line writing. | Propagate write errors. | Standard practice in build tools. | Compliant |
| `format_json_manual` | `crates/op-grpc-bridge/src/grpc_client.rs:89` | Formats connection failure strings with `format!`. | Standard error formatting. | No gap. | Compliant |
| `format_json_manual` | `crates/op-grpc-bridge/src/grpc_client.rs:220` | Formats internal error status into `RemoteError`. | Standard local translation of error payloads. | No gap. | Compliant |
| `format_json_manual` | `crates/op-grpc-bridge/src/grpc_client.rs:277` | Formats internal error status into `RemoteError`. | Standard local translation of error payloads. | No gap. | Compliant |
| `command_new` | `crates/op-grpc-bridge/src/grpc_server.rs:1665` | Spawns raw command `dinitctl list` to monitor service status. | Avoid scraping unstructured CLI output. Communicate via structured control APIs, sockets, or dbus contracts. | Violates schema-as-code discipline. Uses ad-hoc string parsing of CLI output instead of structured interfaces. | Major Gap |
| `command_new` | `crates/op-grpc-bridge/src/grpc_server.rs:1726` | Spawns command `dinitctl status` with unvalidated input variable `name` from the gRPC request. | Use static/schema-validated queries or strictly whitelist service names using regex before running commands. | Arbitrary service name execution allows argument injection (e.g., passing parameter-like strings to the underlying binary). Also relies on unversioned text outputs. | Major Gap |
| `format_json_manual` | `crates/op-grpc-bridge/src/grpc_server.rs:543` | Formats dynamic DBus destination paths using unvalidated inputs: `format!("org.opdbus.{}.v1", req.plugin_id)`. | Validate routing identifiers against a static, versioned routing schema or predefined plugin registry. | Dynamic formatting of object paths and DBus interfaces based on raw user string input without schema validation. | Major Gap |
| `std_fs_in_async` | `crates/op-grpc-bridge/src/grpc_server.rs:1601` | Reads `/etc/hostname` via `tokio::fs::read_to_string`. | Use async I/O in async context. | No gap (uses correct async API). | Compliant |
| `std_fs_in_async` | `crates/op-grpc-bridge/src/grpc_server.rs:1606` | Reads `/proc/version` via `tokio::fs::read_to_string`. | Use async I/O in async context. | No gap. | Compliant |
| `std_fs_in_async` | `crates/op-grpc-bridge/src/grpc_server.rs:1613` | Reads `/proc/uptime` via `tokio::fs::read_to_string`. | Use async I/O in async context. | No gap. | Compliant |
| `std_fs_in_async` | `crates/op-grpc-bridge/src/grpc_server.rs:1622` | Reads `/proc/meminfo` via `tokio::fs::read_to_string`. | Use async I/O in async context. | No gap. | Compliant |

---

### Actionable Recommendations for Major & Critical Gaps

#### 1. Replace Ad-Hoc Pointer Dereferencing with Versioned Serialization Schemas
* **Citations:** `crates/op-grpc-bridge/src/interceptor.rs:57` & `58`
* **Remediation:** Do not cast raw memory maps directly to raw pointers and dereference them. To maintain the **schema-as-code** discipline, serialize and deserialize this configuration data using a versioned schema such as **Protocol Buffers** or **FlatBuffers**. If a memory-mapped structure must be used for performance, leverage standard safe-abstraction crates like `bytemuck` or `zerocopy` to enforce strict size, alignment, and bit-validity checks at runtime before accessing memory.

#### 2. Prevent Blocked Async Worker Threads
* **Citations:** `crates/op-grpc-bridge/src/interceptor.rs:10` & `50`
* **Remediation:** Remove synchronous file mapping operations from the hot path of the async executor. Offload the file loading and mapping operations to a blocking context using `tokio::task::spawn_blocking`, or pre-map the file upon server initialization to prevent synchronous disk wait cycles on each request.

#### 3. Eliminate Panic Vectors in the Request Hot Path
* **Citations:** `crates/op-grpc-bridge/src/interceptor.rs:72`
* **Remediation:** Refactor `.unwrap()` calls to use safe error propagation. Map missing options or invalid header values directly to a gRPC `Status::invalid_argument` or `Status::unauthenticated` response:
  ```rust
  let header_str = request
      .headers()
      .get("my-header")
      .ok_or_else(|| Status::unauthenticated("Missing required header"))?
      .to_str()
      .map_err(|_| Status::invalid_argument("Invalid header encoding"))?;
  ```

#### 4. Harden Dynamic System Calls and Argument Slices
* **Citations:** `crates/op-grpc-bridge/src/grpc_server.rs:1665` & `1726`
* **Remediation:** 
  * Avoid spawning raw subprocesses (`dinitctl`) and parsing raw console text. If integration with the `dinit` service manager is required, use its structured IPC control socket or a stable schema-conforming API.
  * If executing `dinitctl` is unavoidable, validate that the client-supplied `service_name` conforms to a strict alphanumeric whitelist regex (e.g., `/^[a-zA-Z0-9_\-\.]+$/`) to avoid parameter/argument injection. Always specify the full, absolute path of the executable (e.g., `/sbin/dinitctl`) to prevent execution hijacking via `PATH` environment pollution.

#### 5. Restrict DBus Dynamic Destination Parsing
* **Citations:** `crates/op-grpc-bridge/src/grpc_server.rs:543`
* **Remediation:** Implement a strict, static registry of authorized plugin IDs. Before formatting and binding a dynamic DBus path (`org.opdbus.{req.plugin_id}`), ensure `req.plugin_id` exists in the local schema validation list to prevent arbitrary local IPC routing attacks.