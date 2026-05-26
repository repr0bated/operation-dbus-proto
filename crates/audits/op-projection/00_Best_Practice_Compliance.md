| Pattern | File:Line | Crate Approach | Corpus Best Practice | Gap | Rating |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `format_json_manual` | `crates/op-projection/src/dbus_reader.rs:64` | Manual formatting of DBus paths using `format!` string interpolation. | Use dedicated path-handling structures or DBus path validation utilities. | Ad-hoc path assembly using `format!` without validating against DBus object path naming rules. | Minor Gap |
| `format_json_manual` | `crates/op-projection/src/dbus_reader.rs:66` | Inline DBus path hierarchy assembly using fallback `format!("{}/{}", path, child)`. | Use validated path APIs or type-safe object path builders. | Lack of validation of nested DBus paths leading to potential double-slash or structural errors. | Minor Gap |
| `format_json_manual` | `crates/op-projection/src/dbus_reader.rs:71` | Constructing untyped event data using `serde_json::json!` macro with ad-hoc key/value pairs. | Strictly follow schema-as-code discipline using versioned serialization schemas (Protobuf/OSCAL/strongly-typed structs). | Violates schema-as-code discipline by defining payloads with ad-hoc inline schemas, bypassing type-safety and interface contracts. | Major Gap |
| `format_json_manual` | `crates/op-projection/src/event_materializer.rs:79` | Manual dynamic formatting of dynamic error message strings into logs. | Use structured errors and log objects that preserve error context. | Ad-hoc error string generation losing structural error context during serialization. | Minor Gap |
| `format_json_manual` | `crates/op-projection/src/event_materializer.rs:84` | Building dynamic keys via string concatenation: `format!("{}:{}", entity_type, entity_id)`. | Use composite key types or formal schema serializers to prevent delimiter injection attacks/structural collisions. | Potential identifier collisions if variables contain colons due to unescaped string formatting. | Minor Gap |
| `unwrap_expect` | `crates/op-projection/src/plugin_reader.rs:523` | Using `.expect()` in test functions to assert state. | Compliant inside test/assertion contexts. | None | Compliant |
| `unwrap_expect` | `crates/op-projection/src/schema_engine.rs:579` | Using `.unwrap()` in test assertions for schema registration. | Compliant inside test/assertion contexts. | None | Compliant |
| `unwrap_expect` | `crates/op-projection/src/schema_engine.rs:608` | Using `.unwrap()` inside unit testing scenarios. | Compliant inside test/assertion contexts. | None | Compliant |
| `unwrap_expect` | `crates/op-projection/src/schema_engine.rs:609` | Using `.unwrap()` inside unit testing scenarios. | Compliant inside test/assertion contexts. | None | Compliant |
| `unwrap_expect` | `crates/op-projection/src/schema_engine.rs:632` | Using `.unwrap()` inside unit testing scenarios. | Compliant inside test/assertion contexts. | None | Compliant |
| `unsafe_block` | `crates/op-projection/src/sled_reader.rs:67` | Dereferencing a raw pointer `ptr` without null/alignment checks or safety documentation. | Explicit safety pre-condition checks (null verification, alignment bounds) and a `// SAFETY:` block documenting pointer guarantees. | Direct raw pointer dereference without checking for null or validity, presenting a severe memory-safety risk. | Critical Gap |

---

### Actionable Recommendations

#### Major Gap: Schema-as-Code Compliance (`crates/op-projection/src/dbus_reader.rs:71`)
* **Impact:** Using untyped dynamic JSON objects (`json!`) bypasses Rust’s compile-time type-checking, making APIs brittle, prone to runtime failures, and decoupled from versioned schema changes.
* **Remediation:** 
  1. Define structural types using a schema-first approach (e.g., Protocol Buffers or shared `struct` definitions annotated with `serde::Serialize`/`Deserialize`).
  2. Implement safe type conversions into these structured models instead of dynamic mapping.

#### Critical Gap: Unsafe Raw Pointer Dereference (`crates/op-projection/src/sled_reader.rs:67`)
* **Impact:** Dereferencing `ptr` without establishing its non-null status or structural validity leads to undefined behavior, potential memory-safety exploitation, or runtime segmentation faults if `read_sled()` fails or returns invalid pointers.
* **Remediation:**
  1. Validate that the pointer is not null prior to dereferencing: `if ptr.is_null() { return Err(...); }`.
  2. Document the exact safety invariant justifying the operation using a standard `// SAFETY:` comment block.
  3. Modify `read_sled` to return a safe abstraction (e.g., `Result<impl AsRef<Sled>, Error>`) rather than raw pointers to push pointer manipulation boundaries out of reader-level code.