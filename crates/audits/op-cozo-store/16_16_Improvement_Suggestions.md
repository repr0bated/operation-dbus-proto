### SCHEMA-AS-CODE DISCIPLINE & OSCAL COMPLIANCE

1. **Replace Ad-hoc Schema Definitions with Versioned Schemas** | The `seed_schema` method defines unified database relations, constraints, and arbitrary JSON-like columns (e.g., `props`, `metadata`, `tags`) using raw, ad-hoc embedded query strings. This violates strict schema-as-code principles. Instead of hardcoded strings, schemas and nested documents should be defined using compiled, strongly-typed Protocol Buffers or versioned OSCAL schemas to guarantee automated backward compatibility, avoid structural mutation drift, and prevent runtime parse failures during datalog evaluation. | `crates/op-cozo-store/src/lib.rs:65`

---

### ARCHITECTURE

2. **Decompose Multi-Domain Database Shuttle into Domain-Specific Stores** | The `CozoGraphShuttle` struct couples identity-graph mechanics with ephemeral authentication sessions (`sessions`), user lookup registries (`users`), and execution configurations (`memory_namespaces`, `memory_entries`). This architectural coupling makes it difficult to upgrade database relations independently. The session, graph storage, and MCP memory sub-modules should be split into isolated, decoupled traits or distinct crates so that a localized database schema migration in one domain does not risk causing downtime for the entire security authorization framework. | `crates/op-cozo-store/src/lib.rs:26`

---

### API ERGONOMICS

3. **Avoid Fail-Open Semantics and Raw Strings for Critical Authorization Rules** | The `evaluate_mutation` method silently catches database errors by returning a fail-open `PolicyVerdict { allow: true, ... }` if the compliance graph is unseeded or encounters an issue. Additionally, timestamps and inputs are passed as raw `&str` primitives. This is error-prone. The compliance engine should return a strongly-typed `Result<PolicyVerdict, StoreError>` to enforce fail-closed security. It should also use robust calendar types like `chrono::DateTime<Utc>` for expiry properties instead of raw, unparsed string arguments. | `crates/op-cozo-store/src/lib.rs:175`

---

### PERFORMANCE

4. **Implement Zero-Copy DB Mapping to Avoid Intermediary Serde JSON Allocations** | Methods like `run_query` and `traverse_graph` invoke helper functions that convert results into heap-allocated `serde_json::Value` trees. Under high concurrent query loads, converting structural types into intermediate dynamic JSON structures causes continuous serialization-deserialization overhead and excessive CPU memory churn. Returning typed iterator bindings or utilizing zero-copy reference structures would significantly reduce request latency. | `crates/op-cozo-store/src/lib.rs:166`

---

### OBSERVABILITY

5. **Instrument Crucial Security Decisions with Trace Spans and Structured Fields** | High-consequence database mutations, user creations, and policy evaluations have no tracing instrumentation or context correlation logging. If a compliance check is bypassed or fails, there is no way to diagnose the execution path in production. Adding structured trace spans (e.g., `#[tracing::instrument(skip(self), fields(plugin = %plugin_id, op = %operation))]`) with key-value fields will make audits and security monitoring straightforward and effective. | `crates/op-cozo-store/src/lib.rs:175`

---

### STORAGE

6. **Offload Blocking Synchronous Sled Engine I/O to a Dedicated Threadpool** | The `new_persistent` database instantiates the embedded CozoDB store using the synchronous `sled` engine. Because Sled runs blocking file I/O operations directly on the calling thread, calling these methods inside a highly-concurrent async workspace will block the Tokio worker threads and cause thread pool starvation. Synchronous database routines should be scheduled onto a dedicated threadpool using `tokio::task::spawn_blocking` to preserve core gateway and runtime performance. | `crates/op-cozo-store/src/lib.rs:46`