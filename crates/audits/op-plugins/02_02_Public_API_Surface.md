# Public API Surface Audit: op-plugins

### 1. Total Count of Public Items

An automated regex scan using `^\s*pub\s+(fn|struct|enum|trait|const|static|mod|type|use)` across all 25 audited files reveals **218** public items.

| Item Type | Count |
| :--- | :--- |
| **Modules (`pub mod`)** | 16 |
| **Structs (`pub struct`)** | 58 |
| **Enums (`pub enum`)** | 12 |
| **Traits (`pub trait`)** | 2 |
| **Type Aliases (`pub type`)** | 1 |
| **Functions/Methods (`pub fn` / `pub async fn`)** | 68 |
| **Re-exports (`pub use`)** | 61 |
| **Total** | **218** |

---

### 2. Key Exports (Top 10 Most Critical)

These 10 exports constitute the primary entry points, lifecycle definitions, and state orchestrators for the plugin ecosystem.

| # | Item | Path | File : Line |
| :--- | :--- | :--- | :--- |
| 1 | `pub trait Plugin: Send + Sync` | Core interface for all extensible modules. | `crates/op-plugins/src/plugin.rs:110` |
| 2 | `pub struct PluginRegistry` | Core catalog to index live state plugins. | `crates/op-plugins/src/registry.rs:28` |
| 3 | `pub struct DefaultPluginRegistry` | Orchestrator of default available system plugins. | `crates/op-plugins/src/default_registry.rs:61` |
| 4 | `pub struct AutoPlugin` | Auto-creator for wrapping discovered system resources. | `crates/op-plugins/src/auto_create.rs:46` |
| 5 | `pub struct DynamicLoadingPlugin` | Evaluator of lazy execution tracking & tool caching. | `crates/op-plugins/src/dynamic_loading.rs:42` |
| 6 | `pub struct PrivacyRouterPlugin` | Orchestrates the system networking privacy tunnels. | `crates/op-plugins/src/state_plugins/privacy_router.rs:211` |
| 7 | `pub struct McpStatePlugin` | External Model Context Protocol server state engine. | `crates/op-plugins/src/state_plugins/mcp.rs:133` |
| 8 | `pub struct ServiceDef` | Systemd/Dinit declarative lifecycle target definition. | `crates/op-plugins/src/service_def.rs:204` |
| 9 | `pub struct DesiredState` | Declarative goal footprint representation. | `crates/op-plugins/src/state.rs:9` |
| 10 | `pub trait StatePublisher: Send + Sync` | DBus authority change state distribution channel. | `crates/op-plugins/src/state_publisher.rs:13` |

---

### 3. Glob Re-exports

Glob re-exports risk polluting the public API surface of the crate, causing namespace collisions and leaking private or unstable sub-modules.

*   **`crates/op-plugins/src/lib.rs:48`**
    ```rust
    pub use super::state_plugins::*;
    ```
    *   *Risk:* This glob export leaks all concrete plugin structures (e.g. `AdcPlugin`, `EndpointPlugin`, `GcloudAdcPlugin`, etc.) directly into the root prelude namespace. Any future plugin added to `state_plugins/` will be unconditionally exported, bypassing API stability design reviews.

---

### 4. Ruthless Audit: Unsafe, Unwrap, Clone Abuse, and API Leakage

#### Unsafe Code Abuse
The codebase uses `simd_json::from_str` with `unsafe` to parse JSON in-place by mutating the input buffer. However, the lifetimes and mutability invariants are not sufficiently guarded:

*   **`crates/op-plugins/src/state_plugins/config.rs:40`**
    ```rust
    let parsed: ConfigStoreState = unsafe { simd_json::from_str(&mut content) }?;
    ```
    *   *Violation:* Unsafe mutable slice parsing on a temporary local string.
*   **`crates/op-plugins/src/state_plugins/mcp.rs:152`**
    ```rust
    unsafe { simd_json::from_str(&mut c_mut) }
    ```
    *   *Violation:* Lack of bounds/UTF-8 verification before performing unsafe in-place mutation.
*   **`crates/op-plugins/src/state_plugins/privacy_router.rs:567`**
    ```rust
    let mut bridge_info: HashMap<String, Value> = match unsafe {
        let mut bridge_info_json_mut = bridge_info_json;
        simd_json::from_str::<HashMap<String, Value>>(&mut bridge_info_json_mut)
    }
    ```
    *   *Violation:* Invoking raw unsafe memory manipulation in a core router plugin without explicit invariant checks or an `unsafe` explanation block.
*   **`crates/op-plugins/src/state_plugins/privacy_routes.rs:53`**
    ```rust
    let mut state: PrivacyRoutesState = unsafe { simd_json::from_str(&mut content) }?
    ```
    *   *Violation:* Raw deserialization from file content without pinning or verification.
*   **`crates/op-plugins/src/state_plugins/ovsdb_bridge.rs:163`**
    ```rust
    let v: std::result::Result<Value, _> = unsafe { simd_json::from_str(&mut buf) };
    ```
    *   *Violation:* Direct mutable-aliasing unsafety on a variable created in the same stack frame.

#### Panics & Blind Unwrapping (`unwrap`, `unwrap_or_default`)
Unchecked unwraps on critical system calls, OVS queries, and file-parsing results introduce immediate denial-of-service vectors.

*   **`crates/op-plugins/src/state_plugins/netmaker.rs:80`**
    ```rust
    Ok(output.is_ok() && output.unwrap().status.success())
    ```
    *   *Violation:* Direct panic vector if the child process command fails to spawn or returns an OS error.
*   **`crates/op-plugins/src/state_plugins/lxc.rs:608`**
    ```rust
    let port_uuid = uuid_array[1].as_str().unwrap();
    ```
    *   *Violation:* Unsafe assumption of OVSDB response shape. If OVSDB is modified or returns an unexpected schema, this will crash the entire control plane.
*   **`crates/op-plugins/src/state_plugins/lxc.rs:633`**
    ```rust
    let bridge_uuid = bridge_uuid_array[1].as_str().unwrap();
    ```
    *   *Violation:* Hard unwrap on unvalidated database rows inside OVS network cleanups.
*   **`crates/op-plugins/src/state_plugins/privacy_router.rs:404`**
    ```rust
    format!("inactive {} days", days_since_active.unwrap())
    ```
    *   *Violation:* Panics if the container state is orphaned and `days_since_active` calculates to `None`.
*   **`crates/op-plugins/src/state_plugins/privacy_router.rs:770`**
    ```rust
    let next_port = &self.config.privacy_ports[idx + 1]; // Out-of-bounds index danger
    ...
    let prev_port = &self.config.privacy_ports[idx - 1]; // Underflow index danger
    ...
    let port = action_str.strip_prefix("output:").unwrap().to_string();
    ```
    *   *Violation:* Direct panics during OpenFlow rule rendering on unexpected user configuration formats.

#### Clone Abuse
*   **`crates/op-plugins/src/registry.rs:153`**
    ```rust
    self.plugins.read().await.values().cloned().collect()
    ```
    *   *Violation:* Clones active Arc handles over database connections under a read-lock, increasing lock contention on hot catalog accesses.
*   **`crates/op-plugins/src/auto_create.rs:90`**
    ```rust
    desired.clone(), self.name.clone()
    ```
    *   *Violation:* Allocating deep clones of arbitrary JSON value payloads during standard diff calculation iterations.
*   **`crates/op-plugins/src/state.rs:182`**
    ```rust
    Some(value.clone()), Some(value)
    ```
    *   *Violation:* Redundant cloning of JSON payloads to build audit logs.

#### Public API Leakage
*   **`crates/op-plugins/src/plugin.rs:17`**
    ```rust
    pub struct PluginContext {
        pub publisher: Option<std::sync::Arc<dyn StatePublisher>>,
        pub storage_path: PathBuf,
        pub numa_node: Option<u32>,
        pub config: Value, // <-- Leak of simd_json::OwnedValue as "Value"
    }
    ```
    *   *Violation:* Publicly leaking `simd_json::OwnedValue` as `Value` forces consumer crates to bind directly to a specific minor version of `simd-json`.

---

### 5. Architectural Risks & Mitigation Strategies

1.  **Safety Invariance Violations in `simd_json`**:
    *   *Risk:* In-place JSON parsing requires mutating buffers in memory. If input string slices are reused or mapped directly from read-only memory, `simd_json::from_str` with raw `unsafe` will cause undefined behavior or segfaults.
    *   *Mitigation:* Replace `unsafe { simd_json::from_str }` with safe `simd_json::from_slice` or implement standard, safe deserialization boundaries using `serde_json` where execution performance is not on the hot path.

2.  **Panics on Remote DBus/OVSDB Data**:
    *   *Risk:* Assumptions made about DBus or OVSDB response structures (such as `uuid_array[1].as_str().unwrap()`) expose the system daemon to remote process-driven crashes. If an attacker can inject an malformed schema value, the plugin crash brings down the system-level orchestrator.
    *   *Mitigation:* Strictly use patterns like `get(1).and_then(|v| v.as_str())` with descriptive error propagation (`anyhow::bail!`) instead of direct `.unwrap()`.

3.  **Encapsulation Breakage via Prelude Glob Exports**:
    *   *Risk:* Glob-exporting all sub-modules via `pub use super::state_plugins::*;` couples external consumers with internal plugin implementations, violating clean API boundary guidelines.
    *   *Mitigation:* Remove glob re-exports in `prelude`. Explicitly list only the abstract traits (`Plugin`, `PluginCatalog`) and configuration types required for orchestration, keeping implementation structs private.