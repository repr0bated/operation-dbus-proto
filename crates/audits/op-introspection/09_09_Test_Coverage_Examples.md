### Test Suite Metrics & Status

#### Test Count
* **Total test functions found**: 2

#### Representative Tests
1. **`test_indexer_creation`**  
   * **File**: `crates/op-introspection/src/indexer.rs`
   * **Line**: 611
   * **Description**: Verifies the creation of the SQLite-backed `DbusIndexer` in-memory and ensures that no initial statistics exist on a fresh database.

2. **`test_bus_type_display`**  
   * **File**: `crates/op-introspection/src/projection.rs`
   * **Line**: 249
   * **Description**: Tests the debug formatting of the `BusType` enum, confirming that `BusType::System` and `BusType::Session` formats match `"system"` and `"session"`.

*(Note: Only 2 tests exist across the entire workspace in the provided files. No third test function could be cited from the source. The lack of test coverage across core functionality such as parsing, caching, and CPU feature detection constitutes a **High Risk**).*

#### Property-Based Testing & Fuzzing
* **Property Tests**: None. There is no usage of `proptest`, `quickcheck`, or equivalent property-testing frameworks in the provided codebase.
* **Fuzzing**: None. No fuzz targets, `arbitrary` crate integrations (apart from those in transitive dependency locks), or fuzzing configurations were detected in the workspace for this crate.

---

### Production Code Audit & Vulnerability Findings

#### 1. Schema-As-Code Violations
The codebase has a pervasive structural quality issue where critical serialization contracts, system profiles, and D-Bus interfaces are declared via ad-hoc Rust structs instead of versioned, declarative schemas. This violates the strict schema-as-code discipline.

* **Ad-hoc Serialization Structs**:
  * `crates/op-introspection/src/cpu_features.rs:20-31`: `CpuFeatureAnalysis` and associated nested structures are exposed directly via Serde derivation (`#[derive(Serialize, Deserialize)]`).
  * `crates/op-introspection/src/hierarchical.rs:21-33`: `HierarchicalIntrospection` snapshot is cached directly as an ad-hoc JSON structure.
  * `crates/op-introspection/src/mod.rs:15-28`: `IntrospectionReport` defines the high-level control plane topology as an unversioned Rust struct.
  * `crates/op-introspection/src/indexer.rs:18-29`: `IndexStatistics` defines metadata statistics as an ad-hoc struct.
* **Impact**: External services, frontend clients, or recovery tools relying on these JSON/BTRFS snapshots will experience parsing failures or silent corruption when internal fields are modified. Contracts must be migrated to versioned declarative formats such as Protocol Buffers or OSCAL-compliant schemas to maintain structural compatibility.

---

#### 2. High Risk: Command Injection and Path Hijacking in System Command Execution
* **File**: `crates/op-introspection/src/mod.rs`
* **Lines**: 376, 480
* **File**: `crates/op-introspection/src/cpu_features.rs`
* **Lines**: 212, 311, 361
* **Vulnerability Type**: Path Hijacking / Command Execution Vulnerability

```rust
// crates/op-introspection/src/mod.rs:376
let output = Command::new("pgrep").arg("-c").arg("qemu").output();

// crates/op-introspection/src/mod.rs:480
let output = Command::new("systemctl")
    .args([
        "list-units",
        "--type=service",
        "--all",
        "--no-pager",
        "--no-legend",
    ])
    .output()
```

* **Analysis**: The functions use relative paths for command invocation (`pgrep`, `systemctl`, `rdmsr`, `modprobe`). Because the executable is resolved using the host's `PATH` environment variable, an attacker who gains local access with execution privileges can manipulate the `PATH` variable to redirect execution to a malicious binary (e.g., a custom `pgrep` or `systemctl` placed in a writable directory like `/tmp` or `/user/bin`). This is especially dangerous when the introspection binary is run with elevated system privileges (which is likely, given it reads `/dev/cpu/0/msr` and `/sys/kernel/iommu_groups`).
* **Remediation**: Use absolute paths for all system binaries (e.g., `/usr/bin/pgrep`, `/usr/bin/systemctl`, `/usr/sbin/modprobe`). Alternatively, verify and sanitize the `PATH` environment variable prior to execution.

---

#### 3. Medium Risk: Arbitrary File Write via Symlink Attacks on Cache Directory
* **File**: `crates/op-introspection/src/hierarchical.rs`
* **Lines**: 504–520
* **Vulnerability Type**: Insecure Temporary/Cache File Handling (Symlink Race Condition)

```rust
// crates/op-introspection/src/hierarchical.rs:509
let snapshot_path = cache_path.join(&filename);

let json = simd_json::to_string_pretty(data)?;
tokio::fs::write(&snapshot_path, json).await?;

// Also save as "latest.json" for easy access
let latest_path = cache_path.join("latest.json");
let json = simd_json::to_string_pretty(data)?;
tokio::fs::write(&latest_path, json).await?;
```

* **Analysis**: If `cache_dir` is configured to point to a shared or world-writable directory (such as `/tmp` or `/var/tmp`), an unprivileged attacker can pre-create a symbolic link named `latest.json` pointing to an arbitrary file on the system (e.g., `/etc/shadow` or `/etc/cron.d/malicious_job`). When the high-privilege introspection service runs and calls `save_to_cache`, it will follow the symlink and overwrite the target file with the JSON-formatted D-Bus introspection data, causing denial of service or arbitrary file write.
* **Remediation**: 
  1. Ensure that the parent cache directory is created with restricted permissions (`0700`) owned by the service user.
  2. Avoid following symbolic links by utilizing the `O_NOFOLLOW` flag during file creation or checking if the file is a symlink prior to writing.

---

#### 4. Medium Risk: Denial of Service via FTS5 Query Syntax Panics
* **File**: `crates/op-introspection/src/indexer.rs`
* **Lines**: 528, 557, 589
* **Vulnerability Type**: Uncontrolled Input to SQLite FTS5 Query Parser

```rust
// crates/op-introspection/src/indexer.rs:534
"SELECT service_name, object_path, interface_name, method_name,
        description, rank
 FROM methods_fts
 WHERE methods_fts MATCH ?1
 ORDER BY rank
 LIMIT ?2"
```

* **Analysis**: The query parameter `?1` is passed directly into the SQLite FTS5 `MATCH` operator. While binding variables prevents classic SQL injection (as the input is parsed as a query expression rather than raw SQL commands), FTS5 queries have their own strict syntax (e.g., matching parentheses, quotation marks, and operator structures like `AND`, `OR`, `NOT`). If an attacker passes unescaped search strings containing unbalanced parentheses or operators (e.g., `(network AND`), SQLite's FTS5 parser will return a syntax error. If this error is not handled correctly or is bubbled up to a panic-prone context in the calling service, it can lead to application Denial of Service.
* **Remediation**: Sanitize the FTS search query by stripping or escaping special FTS5 operators and unbalanced punctuation characters (such as `*`, `"`, `:`, `(`, `)`) before passing the string to the database query binder.

---

#### 5. Low Risk: Unhandled Error Propagation from Command Failures
* **File**: `crates/op-introspection/src/mod.rs`
* **Lines**: 376, 480
* **File**: `crates/op-introspection/src/cpu_features.rs`
* **Lines**: 311, 361
* **Vulnerability Type**: Insecure Error Handling / Silent Failure

```rust
// crates/op-introspection/src/cpu_features.rs:311
let output = Command::new("rdmsr").arg("0x3A").output();
if let Ok(out) = output {
    if out.status.success() { ... }
}
```

* **Analysis**: When executing commands such as `rdmsr` or `dmesg`, failures are caught using `.ok()` or by ignoring non-zero exit codes. If `rdmsr` fails due to permission issues (e.g., CAP_SYS_RAWIO missing), the function silently assumes VMX is disabled or unlocked. This can result in the control plane operating on inaccurate hardware state information.
* **Remediation**: Log standard error (`stderr`) output of failed commands and bubble up specific actionable errors to the service layer so that permission misconfigurations are explicitly visible.