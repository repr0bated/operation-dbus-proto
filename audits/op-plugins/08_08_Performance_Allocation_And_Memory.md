# Production Security & Quality Audit: `op-plugins`

---

## 1. Memory Map Table

The following table tracks direct memory mapping allocations across the audited source files.

| Site | file:line | Type (ro/rw/sled) | Risk |
| :--- | :--- | :--- | :--- |
| **None** | N/A | N/A | Direct memory mapping APIs (such as `memmap2`, `mmap`, `MmapMut`, or `MmapOptions`) are not directly instantiated within the audited `op-plugins` crate files. |

*Note on Sled*: Sled is included as a workspace dependency (used by `cozo` with `storage-sled` backend) in `Cargo.toml`. However, no direct database instantiation or active file-backed Sled mappings are present in the provided source files.

---

## 2. Large Heap Allocations

The following sites perform dynamic, unbounded heap allocations on potentially large payloads (such as entire system inventories, complete file listings, or arbitrary target configurations) without capacity pre-allocation:

### A. Non-Preallocated Vectors and Arrays in System Introspection Loops
*   **`crates/op-plugins/src/state_plugins/procfs.rs:205`**: In `gather_cpuinfo()`, the `cpus` vector (`Vec::new()`) is populated dynamically in a tight loop parsing `/proc/cpuinfo` without a capacity reservation. This is replicated in `gather_net_dev()` (**line 242**), `gather_mounts()` (**line 277**), and `gather_diskstats()` (**line 309**). On nodes with high CPU/interface counts or complex mount configurations, this causes repeated vector reallocations and excessive heap copying.
*   **`crates/op-plugins/src/state_plugins/incus.rs:98`**: In `parse_instance_list()`, the parser collects deserialized container configurations into a standard `Vec` using `.collect()` without initializing the vector with the exact length of `raw_instances`.

### B. Unbounded In-Memory String Formatting and Copies
*   **`crates/op-plugins/src/state_plugins/service.rs:163`**: In `query_current_state()`, the systemd backend spawns `systemctl list-units --all` and collects the complete output into memory via `String::from_utf8_lossy(&out.stdout)`. On systems with thousands of dynamic transient scopes or mount units, this loads megabytes of unstructured text into a single heap-allocated string before line-by-line filtering.

---

## 3. Performance, Allocation & Hot-Path Findings

### A. Dynamic Allocation (`Vec::new`/`String::new`) Inside Loops
*   **`crates/op-plugins/src/auto_create.rs:20`**: In `discover_units()`, the loop over `discovered_units` allocates a fresh `String` via `unit.to_string()` and constructs a nested JSON structure using the `json!` macro on every iteration.
*   **`crates/op-plugins/src/state_plugins/incus.rs:271`**: In `sync_devices()`, the plugin loops over desired container devices and creates a temporary `vec![...]` of owned `String` objects on every iteration to build dynamic CLI arguments.
*   **`crates/op-plugins/src/state_plugins/procfs.rs:142`**: In `kv_file()`, the parser loops over lines in `/proc` files and runs `key.trim().replace(' ', "_").to_lowercase()` for every line, allocating multiple intermediate strings per key.

### B. Count of `format!` Invocations in Core Reconcilers
Frequent string formatting degrades throughput and increases allocation pressure during active control loop reconciliation:
*   **`crates/op-plugins/src/auto_create.rs:90-91`**: Computes hashes inside `calculate_diff` on every control loop step, formatting MD5 hashes with `format!("{:x}", ...)` repeatedly.
*   **`crates/op-plugins/src/state_plugins/config.rs:164-165`**: Runs `format!("{:x}", md5::compute(simd_json::to_string(current)?))` for both current and desired configurations on every reconciliation pass.
*   **`crates/op-plugins/src/state_plugins/dnsresolver.rs:175-181`**: Runs MD5 hashing and hexadecimal string formatting on full DNS arrays during difference calculations.
*   **`crates/op-plugins/src/state_plugins/incus.rs:175, 184, 211, 222, 225, 253, 267, 280, 284`**: Inside the main container state synchronization loops, `format!` is used on almost every branch to generate error messages, configure properties, construct command arguments, and track actions.

### C. Large JSON Payload Cloning via `OwnedValue.clone()`
Deep cloning of dynamic JSON values bypasses Rust's move semantics, causing massive heap thrashing when processing large state objects:
*   **`crates/op-plugins/src/auto_create.rs:76`**: `query_current_state()` returns `self.current_state.read().await.clone()`, forcing a complete copy of the internal state tree on every query.
*   **`crates/op-plugins/src/state_plugins/incus.rs:356-357`**: `calculate_diff()` clones the entire `current` and `desired` `OwnedValue` structures prior to deserialization.
*   **`crates/op-plugins/src/state_plugins/mcp.rs:303-304`**: Clones `current` and `desired` dynamic configs for diffing on every tick of the model control plane.
*   **`crates/op-plugins/src/state_plugins/privacy_router.rs:379`**: Clones the system's global `self.config` configuration payload and performs dynamic recursive deep-merging (`value.clone()` on **line 396**) when computing desired changes.

---

## 4. Schema-as-Code Violations

The system's architectural specification dictates a schema-as-code discipline using Protocol Buffers and OSCAL. The following components violate this requirement by using ad-hoc, unstructured JSON documents, raw strings, or non-versioned custom Rust structures:

*   **`crates/op-plugins/src/chat.rs:9-67`**: Defines core LLM messaging contracts (`ChatMessage`, `ChatRequest`, `ChatResponse`, `TokenUsage`) as ad-hoc, manual `serde`-annotated Rust structs. These types are decoupled from any versioned Protobuf or OSCAL schema registry.
*   **`crates/op-plugins/src/service_def.rs:12-251`**: System service specifications (`ServiceDef`, `ServiceName`, `ExecCommand`) are hand-coded as ad-hoc Rust structs that manually parse and format Chimera's `dinit` configurations, lacking versioned schemas or formal schema contracts.
*   **`crates/op-plugins/src/state_plugins/procfs.rs:114-135`**: Implements a `procfs` plugin that dynamically converts unstructured `/proc` file listings into nested JSON objects with dynamic, runtime-generated keys. This bypasses structured schema boundaries.

---

## 5. Critical Security Vulnerability: Shell Command Injection in `PciDeclPlugin`

*   **File**: `crates/op-plugins/src/state_plugins/pcidecl.rs`
*   **Lines**: `114-123` (vulnerable execution) and `175-182` (injection vector)
*   **Severity**: **Critical**
*   **Exploitability**: Directly exploitable if the system processes unauthenticated or user-supplied target states containing the PCI device address.

### Vulnerability Analysis
The function `lspci_present` takes a string slice `addr` (representing the PCI address) and executes it within a system shell context:

```rust
fn lspci_present(addr: &str) -> bool {
    if let Ok(out) = Command::new("sh")
        .arg("-c")
        .arg(format!("lspci -s {} >/dev/null 2>&1; echo $?", addr))
        .output()
    {
        return out.stdout.first().map(|b| *b == b'0').unwrap_or(false);
    }
    false
}
```

The `addr` parameter is formatted directly into the shell command string without sanitization or character escaping. 

During reconciliation, the plugin deserializes the `desired` state (which can be modified by users via D-Bus, config files, or external imports) into a `PciDecl` configuration:

```rust
let want: PciDecl =
    simd_json::serde::from_owned_value(desired.clone()).context("desired must be PciDecl")?;
let mut actions = Vec::new();
for item in &want.items {
    let live = Self::live_for(&item.address);
    let present = live.present || Self::lspci_present(&item.address);
    ...
```

If an attacker controls the target state of the `pcidecl` plugin, they can inject arbitrary shell commands through the `address` field.

### Proof of Concept (PoC)
An attacker injects the following JSON payload into the desired configuration of `pcidecl`:

```json
{
  "version": 1,
  "items": [
    {
      "id": "exploit-card",
      "mode": "enforce",
      "address": "0000:00:1f.6; touch /tmp/pwned; #",
      "expect_vendor": "8086",
      "expect_device": "15f3"
    }
  ]
}
```

When `calculate_diff` runs, it iterates over the item, finds that `/sys/bus/pci/devices/0000:00:1f.6; touch /tmp/pwned; #` does not exist on the filesystem, and calls `lspci_present("0000:00:1f.6; touch /tmp/pwned; #")`.

This executes:
```bash
sh -c "lspci -s 0000:00:1f.6; touch /tmp/pwned; # >/dev/null 2>&1; echo $?"
```

The shell parses the semicolon, terminating the `lspci` command and executing `touch /tmp/pwned` as the root user (or the user running the `op-dbus` daemon).

### Remediation
Remove `sh -c` entirely and call `lspci` directly using structured arguments, ensuring no shell parsing can occur:

```rust
fn lspci_present(addr: &str) -> bool {
    Command::new("lspci")
        .args(["-s", addr])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}
```

---

## 6. Other Security & Quality Findings

### A. Unsafe `simd_json::from_str` Usage on Non-Padded Buffers
*   **Severity**: **High** (Memory Safety / Potential Crash)
*   **Citations**:
    *   `crates/op-plugins/src/state_plugins/config.rs:51`
    *   `crates/op-plugins/src/state_plugins/mcp.rs:188`
    *   `crates/op-plugins/src/state_plugins/ovsdb_bridge.rs:198`
    *   `crates/op-plugins/src/state_plugins/privacy_routes.rs:62`
    *   `crates/op-plugins/src/state_plugins/net.rs:245`

#### Vulnerability Analysis
The codebase repeatedly reads JSON files or command outputs using `tokio::fs::read_to_string` and parses them using `unsafe { simd_json::from_str(&mut buffer) }`:

```rust
// Example from crates/op-plugins/src/state_plugins/config.rs:51
match tokio::fs::read_to_string(&self.store_path).await {
    Ok(mut content) => {
        let parsed: ConfigStoreState =
            unsafe { simd_json::from_str(&mut content) }.context("invalid config store")?;
        Ok(parsed)
    }
    ...
```

`simd_json` is an in-place SIMD-accelerated parser. It strictly requires the input buffer to have at least `simd_json::PADDING` (usually 32 bytes) of writable padding at the end of the buffer to prevent out-of-bounds reads during vector register alignment. A standard `String` loaded via `read_to_string` does **not** have this padding. Passing an unpadded string buffer to the `unsafe` SIMD parser violates its safety invariants, leading to potential out-of-bounds memory access, page faults, or unexpected crashes.

#### Remediation
Replace `simd_json::from_str` with `simd_json::from_slice` on a padded `Vec<u8>` or use the safe `simd_json::serde::from_str` wrapper which handles buffer padding automatically.

---

### B. Broken Command Chaining Logic in `NetmakerPlugin::apply_state`
*   **Severity**: **Medium** (Functional Bug)
*   **Citation**: `crates/op-plugins/src/state_plugins/netmaker.rs:296-299`

#### Analysis
The `NetmakerPlugin` attempts to install the `netclient` package by passing a chained command structure to `Command::new`:

```rust
let install_result = Command::new("apt")
    .args(["update", "&&", "apt", "install", "-y", "netclient"])
    .status()
    .await;
```

`Command::new("apt")` executes the `apt` binary directly. Shell operators such as `&&` are not parsed by the operating system kernel during binary execution; they are features of a shell (such as `/bin/sh` or `/bin/bash`). Consequently, `apt` will interpret `"&&"` and `"apt"` as literal package names, leading to a package manager error and a failure to install `netclient`.

#### Remediation
Execute the update and install operations as two distinct, sequential `Command::new("apt")` invocations, checking the exit status of the first before proceeding to the second.

---

### C. Cryptographically Broken Hash Algorithm (MD5)
*   **Severity**: **Low** (Cryptographic Quality)
*   **Citations**:
    *   `crates/op-plugins/src/auto_create.rs:90-91`
    *   `crates/op-plugins/src/state_plugins/config.rs:164-165`
    *   `crates/op-plugins/src/state_plugins/dnsresolver.rs:175-181`
    *   `crates/op-plugins/src/state_plugins/netmaker.rs:252-253`
    *   `crates/op-plugins/src/state_plugins/rtnetlink.rs:159-160`
    *   `crates/op-plugins/src/state_plugins/privacy_router.rs:728`

#### Analysis
The codebase uses `md5::compute(...)` to generate state hashes for the blockchain audit trail (`current_hash` and `desired_hash`). MD5 is highly vulnerable to collision attacks. While state hashes are primarily used for drift detection, utilizing an insecure hashing algorithm undermines the integrity of the blockchain ledger.

#### Remediation
Use `Sha256` (from the already imported `sha2` crate) to generate hashes for blockchain footprints consistently across all plugins.