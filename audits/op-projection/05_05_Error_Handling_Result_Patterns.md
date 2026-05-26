# Error Handling and Quality Audit Report

---

## 1. Error Handling Operator & Macro Counts

### Recovery & Unwrapping Operations
* **`.unwrap()`**: **10**
* **`.expect()`**: **1**
* **`.unwrap_or()` and variants**: **12**
  * `.unwrap_or(...)`: **8**
  * `.unwrap_or_default()`: **3**
  * `.unwrap_or_else(...)`: **1**
* **`?` Operator**: **30**

### Panic & Placeholder Macros
* **`todo!()`**: **0**
* **`unimplemented!()`**: **0**
* **`panic!()`**: **0**

---

## 2. Unwrapping Site Review

Here are the first 5 `.unwrap()` sites identified in the provided codebase. All 10 of the `.unwrap()` occurrences in this crate are situated inside the unit test suite (`#[cfg(test)] mod tests` inside `crates/op-projection/src/schema_engine.rs`).

### Site 1: `crates/op-projection/src/schema_engine.rs:542`
```rust
let version = engine.register_schema(schema).unwrap();
```
* **Context**: Located inside the `test_register_schema` test case. 
* **Recommendation**: In a test setting, calling `.unwrap()` is idiomatic. It simplifies setup and asserts that schema registration succeeds. No change required.

### Site 2: `crates/op-projection/src/schema_engine.rs:571`
```rust
let version1 = engine.register_schema(schema1).unwrap();
```
* **Context**: Located inside the `test_register_multiple_versions` test case.
* **Recommendation**: Idiomatic in tests; a failure here signals a broken registration state, which should rightly panic and fail the test run. No change required.

### Site 3: `crates/op-projection/src/schema_engine.rs:572`
```rust
let version2 = engine.register_schema(schema2).unwrap();
```
* **Context**: Located inside the `test_register_multiple_versions` test case.
* **Recommendation**: Idiomatic in tests to fail early if multiple schema registrations error out. No change required.

### Site 4: `crates/op-projection/src/schema_engine.rs:597`
```rust
engine.register_schema(schema).unwrap();
```
* **Context**: Located inside the `test_get_schema` test case.
* **Recommendation**: Idiomatic in tests. No change required.

### Site 5: `crates/op-projection/src/schema_engine.rs:600`
```rust
assert_eq!(retrieved.unwrap().version, "1.0.0");
```
* **Context**: Located inside the `test_get_schema` test case.
* **Recommendation**: Idiomatic in tests. Asserts that the retrieved schema is indeed `Some` and matches the expected version. No change required.

---

## 3. Lock Poisoning Risk Analysis

Standard library Mutexes and RwLocks (`std::sync::Mutex` and `std::sync::RwLock`) return a `Result` on locking acquisition. Calling `.unwrap()` on them exposes the thread to lock poisoning risks if a holding thread panics.

### Audit Findings
* **0 occurrences of `.unwrap()` on locking constructs.**
* This crate employs **`parking_lot::Mutex`** and **`parking_lot::RwLock`** exclusively for cross-thread state management (see `crates/op-projection/src/access_control.rs`, `crates/op-projection/src/event_materializer.rs`, and `crates/op-projection/src/ovsdb_mirror.rs`).
* **Design Rating**: **EXCELLENT**. By utilizing the `parking_lot` crate:
  1. Lock acquisition is direct and doesn't return `Result`, eliminating the syntactical need to `.unwrap()` or `.expect()`.
  2. Lock poisoning is avoided by design, guaranteeing safe state recovery even under panic conditions.

---

## 4. Schema-as-Code & Data Contract Audit

A core architectural principle of the system is the **Schema-as-Code Authority**, enforcing that any entity projected on the control plane must correspond to a structured, versioned `PluginSchema`. 

Ad-hoc strings or unstructured, unversioned JSON maps violate this contract. The audit identified two violations where entities are constructed via ad-hoc, unversioned JSON structures without corresponding schema definitions:

### Violation 1: Ad-hoc D-Bus Property Projections
* **Location**: `crates/op-projection/src/dbus_reader.rs:81`
* **Context**:
  ```rust
  entities.push(RawEntity {
      entity_type: "dbus.object".to_string(),
      entity_id: format!("{}:{}", service, child_path),
      data: json!({
          "service": service,
          "path": child_path,
      })
      .into(),
      source: self.source.clone(),
  });
  ```
* **Risk**: The entity type `"dbus.object"` is emitted as ad-hoc, unstructured JSON without any registration of a corresponding versioned `PluginSchema` in the `SchemaEngine` within `bin/projection_server.rs`. Any changes to the D-Bus serialization format could lead to downstream validation errors or silent failures during materialization.

### Violation 2: Ad-hoc gRPC Property Projections
* **Location**: `crates/op-projection/src/grpc_reader.rs:43`
* **Context**:
  ```rust
  Ok(RawEntity {
      entity_type: "grpc.service".to_string(),
      entity_id: entity_id.to_string(),
      data: json!({ "methods": [] }).into(),
      source: self.source.clone(),
  })
  ```
* **Risk**: The `"grpc.service"` entity type defines its fields as an unvalidated JSON structure containing `"methods"`. No `PluginSchema` defines or versions this contract inside the schema validator registry.

### Remediation Recommendation
Convert both types into first-class versioned schemas. Define structural validation constraints using the `PluginSchema` architecture inside the `SchemaEngine` (similar to how `nested_object_projection_schema()` is defined in `crates/op-projection/src/plugin_reader.rs:102`) and register them during system initialization in `crates/op-projection/src/bin/projection_server.rs`.