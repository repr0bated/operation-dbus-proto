# Production Security and Quality Audit: Error Handling & Schema-as-Code

## 1. Error Handling Metrics

| Metric / Operator | Count |
| :--- | :--- |
| `.unwrap()` | 0 |
| `.expect()` | 0 |
| `.unwrap_or()` | 0 |
| `?` operator | 10 |
| `todo!()` | 0 |
| `unimplemented!()` | 0 |
| `panic!()` | 0 |

---

## 2. Analysis of `.unwrap()` Sites & Lock Poisoning

No occurrences of `.unwrap()`, `.expect()`, or panic macros were found in the audited codebase. The crate consistently leverages idiomatic Rust error handling, bubble-up propagation using the `?` operator, and `anyhow::Result` for application-level execution paths.

Additionally, there are no references to standard library or third-party locking primitives (e.g., `std::sync::Mutex`, `parking_lot::RwLock`) within this crate, resulting in **zero lock poisoning risks**.

---

## 3. Detailed Survey of `?` Operator Sites

The audited code utilizes the `?` operator at the following locations to propagate database, serialization, and deserialization errors back to the caller as `anyhow::Result`:

1. **`crates/op-dbus-model/src/lib.rs:27`**
   ```rust
   ).execute(pool).await?;
   ```
   *Recommendation:* **Result** (Correct). Handles network/IO disruptions to the SQLite pool during table creation.

2. **`crates/op-dbus-model/src/lib.rs:42`**
   ```rust
   ).execute(pool).await?;
   ```
   *Recommendation:* **Result** (Correct). Handles SQLite DDL query failure during table creation.

3. **`crates/op-dbus-model/src/lib.rs:53`**
   ```rust
   let encoded = serde_json::to_string(document)?;
   ```
   *Recommendation:* **Result** (Correct). Handles serialization errors gracefully without crashing the service thread.

4. **`crates/op-dbus-model/src/lib.rs:67`**
   ```rust
   .execute(&self.pool).await?;
   ```
   *Recommendation:* **Result** (Correct). Gracefully handles transient database write or lock failures on document upsert.

5. **`crates/op-dbus-model/src/lib.rs:74`**
   ```rust
   .fetch_optional(&self.pool).await?;
   ```
   *Recommendation:* **Result** (Correct). Propagates potential database read failures.

6. **`crates/op-dbus-model/src/lib.rs:84`**
   ```rust
   let encoded: String = row.try_get("base_object")?;
   ```
   *Recommendation:* **Result** (Correct). Prevents panics if the database schema drifted or a column type mismatch occurs.

7. **`crates/op-dbus-model/src/lib.rs:85`**
   ```rust
   let document = serde_json::from_str(&encoded)?;
   ```
   *Recommendation:* **Result** (Correct). Gracefully handles corruption or architectural drift of JSON documents in the database.

8. **`crates/op-dbus-model/src/lib.rs:91`**
   ```rust
   .fetch_all(&self.pool).await?;
   ```
   *Recommendation:* **Result** (Correct). Standard SQLite read propagation.

9. **`crates/op-dbus-model/src/lib.rs:95`**
   ```rust
   let name: String = row.try_get("name")?;
   ```
   *Recommendation:* **Result** (Correct). Handles type coercion error safely.

10. **`crates/op-dbus-model/src/lib.rs:96`**
    ```rust
    let encoded: String = row.try_get("base_object")?;
    ```
    *Recommendation:* **Result** (Correct). Handles type coercion error safely.

---

## 4. Schema-as-Code Compliance Review

The codebase does not strictly adhere to a formal schema-as-code discipline (such as versioned Protocol Buffers or OSCAL schemas) for its data contracts, relying instead on ad-hoc structs and unstructured JSON parsing:

### Finding 1: Ad-hoc JSON values used for structural data contracts
* **Location:** `crates/op-dbus-model/src/models.rs:5` and `crates/op-dbus-model/src/models.rs:13`
* **Context:**
  ```rust
  pub struct Plugin {
      pub name: String,
      pub service_name: String,
      pub base_object: simd_json::OwnedValue,
      pub created_at: DateTime<Utc>,
  }
  // ...
  pub struct Schema {
      pub id: String,
      pub plugin_name: String,
      pub definition: simd_json::OwnedValue,
      pub discovered_from: Option<String>,
      pub discovered_at: Option<DateTime<Utc>>,
      pub created_at: DateTime<Utc>,
  }
  ```
* **Impact:** Using `simd_json::OwnedValue` bypasses type-safe validation guarantees at compile time. Downstream consumers must handle untyped, unstructured JSON, raising risks of runtime panic or silent processing errors when structural contracts mutate.
* **Remediation:** Declare `Plugin` and `Schema` payloads as versioned Protocol Buffers messages or strict OSCAL documents, generating native Rust structures via a build script (such as `prost-build`).

### Finding 2: Unstructured string paths and raw JSON database storage
* **Location:** `crates/op-dbus-model/src/models.rs:33` and `crates/op-dbus-model/src/lib.rs:53`
* **Context:**
  ```rust
  pub struct PluginCatalogDocument {
      pub schema: PluginSchema,
      pub dbus_path: String,
      pub service_name: String,
      pub storage_path: String,
      pub source: String,
  }
  ```
* **Impact:** Parameters such as `dbus_path`, `storage_path`, and `source` are stored as ad-hoc strings instead of well-defined, schema-validated types. Serialization relies on unversioned JSON round-tripping (`serde_json::to_string` on line 53 and `serde_json::from_str` on line 85 of `lib.rs`), which exposes the system to silent data corruption or parsing failures if schemas drift over time.
* **Remediation:** Transition the `PluginCatalogDocument` and its components to a versioned registry format, utilizing strongly typed URI/path wrappers instead of plain `String` primitives.