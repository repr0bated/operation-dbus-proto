# Production Security & Quality Audit: `op-network`

## 1. Dependencies & Feature Inventory

Based on `crates/op-network/Cargo.toml` and the root workspace `Cargo.toml`, the direct dependencies, their version constraints, explicitly enabled features, and security postures are inventoried below:

### Direct Dependencies

| Dependency Crate | Version | Source | Explicitly Enabled Features | Pulls Default Features? | Security / CVE Risk Status |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `tokio` | Inherited (1.x) | Workspace | `["full"]` | Yes | Healthy; pinned to major 1.x. |
| `serde` | Inherited (1.x) | Workspace | `["derive"]` | Yes | Healthy. |
| `serde_json` | `1` | Registry | None | Yes | Healthy. |
| `anyhow` | Inherited (1.x) | Workspace | None | Yes | Healthy. |
| `thiserror` | Inherited (1.x) | Workspace | None | Yes | Healthy. |
| `tracing` | Inherited (0.1) | Workspace | None | Yes | Healthy. |
| `async-trait` | Inherited (0.1) | Workspace | None | Yes | Healthy. |
| `futures` | `0.3` | Registry | None | Yes | Healthy. |
| `rtnetlink` | Inherited (0.14) | Workspace | None | Yes | Older network stack crate. |
| `log` | Inherited (0.4) | Workspace | None | Yes | Healthy. |
| `uuid` | `1` (Direct) | Registry | `["v4"]` | Yes | Unpinned version constraint (`1` instead of `1.x.y`). |
| `reqwest` | Inherited (0.11)| Workspace | `["json", "stream"]`| Yes | **Medium Risk:** Pinned to `0.11` workspace-wide. Does not use modern `0.12` rustls features by default. |
| `netlink-sys` | `0.8` | Registry | None | Yes | Low-level kernel interface, high-privilege operations. |
| `netlink-packet-core`| `0.7`| Registry | None | Yes | Low-level packet serialization. |
| `netlink-packet-generic`| `0.3` | Registry | None | Yes | Low-level packet serialization. |
| `netlink-packet-utils`| `0.5` | Registry | None | Yes | Low-level packet serialization. |
| `netlink-packet-route`| `0.19`| Registry | None | Yes | Low-level routing packet serialisation. |
| `byteorder` | `1.5` | Registry | None | Yes | Healthy. |
| `libc` | Inherited (0.2) | Workspace | None | Yes | Healthy. |
| `tracing-subscriber`| Inherited (0.3)| Workspace | `["env-filter", "json"]`| Yes | Healthy. |
| `rovs-ovsdb` | `0.2` | Registry | None | Yes | Unpinned custom OVS wrapper family. |
| `rovs-openflow` | `0.2` | Registry | None | Yes | Unpinned custom OVS wrapper family. |
| `rovs-jsonrpc` | `0.2` | Registry | None | Yes | Unpinned custom OVS wrapper family. |
| `rovs-types` | `0.2` | Registry | None | Yes | Unpinned custom OVS wrapper family. |
| `rovs-transport` | `0.2` | Registry | None | Yes | Unpinned custom OVS wrapper family. |
| `bytes` | `1` | Registry | None | Yes | Healthy. |
| `simd-json` | Inherited (0.13)| Workspace | `["serde", "serde_impl"]`| Yes | Fast parsing, but carries unsafe blocks in implementation. |
| `op-core` | Path | Local | None | Yes | Internal crate. |

### Crate Features

The `op-network` crate defines **no features** in its `Cargo.toml` `[features]` section. There are no conditional `cfg(feature = ...)` gates within this crate's codebase.

---

## 2. Storage Backend Check

The codebase was audited to locate active storage engines, database drivers, and local caches:

### Identified Backends

| Backend | Found at File:Line | Role | Architecture Violation? |
| :--- | :--- | :--- | :--- |
| **OVSDB (Local Socket)** | `crates/op-network/src/ovsdb.rs:24` | Central Configuration Database / State Synchronization (IDL replica) | **No.** Required for integration with Open vSwitch control plane. |
| **OVSDB (Conf File)** | `crates/op-network/src/plugin.rs:141` | Persistence file system path for configuration store (`/etc/openvswitch/conf.db`) | **No.** Standard persistence layer for persistent system bridge configuration. |

### Architectural Findings
* **No local relational/graph storage within `op-network`:** The crate does not directly utilize or compile against `cozo` or `sqlx` locally. 
* **Missing Caching Backends:** While `op-cache` is in the workspace dependencies list, it is not imported or used here. Capability detection results in `ovs_capabilities.rs:43` are cached strictly using an in-memory `OnceLock<RwLock<Option<CachedCapabilities>>>`. 

---

## 3. Schema-As-Code Integrity Gap Analysis

The system-level discipline mandates that all structured schemas and configuration schemas must be driven by Protocol Buffers or un-aliased, versioned machine-readable models rather than ad-hoc Rust models.

### Flagged Ad-Hoc Data Contracts

* **Ad-hoc JSON Plugin Configuration:**
  In `crates/op-network/src/plugin.rs:18-124`, the core configuration contracts (`NetworkPlugin`, `OvsBridge`, `OpenFlowConfig`, `NetworkInterface`, `OvsdbConfig`) are declared as ad-hoc Rust structures with derived Serde formats. These structures govern the persistent files parsed at startup and are not mapped to versioned Protobuf models, violating the schema-as-code discipline.
* **Proxmox LXC API Contracts:**
  In `crates/op-network/src/proxmox.rs:60-192`, API contracts for container provisioning and tracking (`CreateContainerRequest`, `LxcContainer`, `ContainerStatus`) are expressed as ad-hoc, unversioned structs. These representations rely on runtime parsing (`serde_json::Value` flattened maps) rather than strict schema-first definitions.
* **Kernel Datapath Representation:**
  In `crates/op-network/src/ovs_netlink.rs:96-136`, structures representing OVS state such as `Datapath`, `DatapathStats`, `Vport`, and `KernelFlow` are constructed as hand-coded, ad-hoc structures with direct Serde serialization definitions. These represent high-impact system metadata and should ideally be generated from structural machine-readable schemas.

---

## 4. Security & Quality Audit Findings

### High Severity

#### Insecure TLS Trust-All Policy in Proxmox Client
* **Location:** `crates/op-network/src/proxmox.rs:246`
* **Impact:** 
  The HTTP client is instantiated with `.danger_accept_invalid_certs(true)`. This setting disables standard TLS hostname and certificate chain validation entirely. When connecting to a remote Proxmox API cluster over a network (configured via `PVE_API_URL`), a Man-in-the-Middle (MitM) attacker can easily intercept API tokens, hijack admin sessions, or inject falsified machine configurations.
* **Remediation:** 
  Provide a configurable option to import a trusted CA root certificate (using `Client::builder().add_root_certificate(...)`) instead of disabling verification. Do not allow `.danger_accept_invalid_certs(true)` in production environments.

---

### Medium Severity

#### Predictable Shared Compiler Paths & Compilation Race Conditions
* **Location:** `crates/op-network/src/bin/op-xdp-wg.rs:27-29`
* **Impact:** 
  The XDP orchestrator writes BPF source code directly to a static, predictable global path `/etc/op-network/xdp/op-xdp-wg.c` and compiles it to `/etc/op-network/xdp/op-xdp-wg.o`. 
  1. If multiple interfaces or instances trigger the orchestrator concurrently, they will stomp on the same files, leading to corrupted source definitions during compilation.
  2. If the directory `/etc/op-network/xdp` has loose permissions, local unprivileged attackers could pre-create or modify these files, mounting local privilege escalation or compile-hijacking attacks.
* **Remediation:** 
  Use randomized temporary files (via `tempfile`) for code compilation, compile the program to memory if possible, or ensure strictly guarded directory permissions combined with flock-based mutex locks to prevent overlapping writes.

#### Infinite Network Reading Loop Lack of Timeouts
* **Location:** `crates/op-network/src/controller.rs:120-153`
* **Impact:** 
  The OpenFlow port discovery loop processes incoming messages from a raw `TcpStream` via `recv_msg`. This network read has no timeout limits. If a connecting switch opens a connection and then stalls (half-open socket), the controller thread or task will block indefinitely. This leaks active file descriptors and memory.
* **Remediation:** 
  Wrap every `recv_msg` call or the loop iteration in a `tokio::time::timeout` wrapper to force disconnects on stalled/unresponsive clients.

#### Plaintext Configuration Secret Exposure
* **Location:** `crates/op-network/src/proxmox.rs:268-316`
* **Impact:** 
  The Proxmox token and secret keys are stored in a plaintext file `/etc/op-dbus/pve-token`. The system reads and parses this file without validating file ownership or strict UNIX permissions (such as checking if the file is world-readable via `0600`). This can allow local unprivileged processes or compromised helper containers to read the Proxmox administrative token.
* **Remediation:** 
  Before reading the file, query its metadata and assert that only the owner has read permissions. Abort execution or log critical security warnings if group/world permissions are set.

#### Unencrypted Plaintext Control-Plane Control Default
* **Location:** `crates/op-network/src/plugin.rs:91`
* **Impact:** 
  The system defaults to a plaintext OpenFlow control plane address (`tcp:10.200.0.1:6653`). This allows nodes on the transport subnet to intercept flow rules, inject malicious forward/drop tables, or mimic the active controller.
* **Remediation:** 
  Migrate the control plane default connection to use secure OpenFlow over TLS (`ssl:`).

---

### Low Severity

#### DHCP Client Failures Silently Ignored
* **Location:** `crates/op-network/src/plugin.rs:397-404`
* **Impact:** 
  If the `dhclient` command execution fails, the system logs a warning via `warn!` but returns `Ok(())` anyway. The calling infrastructure will believe the interface was successfully configured, leading to hard-to-debug blackholes where interfaces have no IP addresses but report success.
* **Remediation:** 
  Propagate the error status back to the caller instead of masking it with `Ok(())`, allowing the higher-level plugin orchestrator to gracefully try again or report a system failure.

#### Undocumented Unsafe Block
* **Location:** `crates/op-network/src/ovs_capabilities.rs:115`
* **Impact:** 
  The `detect_fresh` function calls the foreign function `libc::geteuid()` within an undocumented `unsafe` block. This violates clean-code practices.
* **Remediation:** 
  Add a `// SAFETY:` block explaining that `geteuid` is an infallible system call that does not dereference any raw pointers.

#### Arbitrary Execution via Hardcoded Host Commands
* **Location:** `crates/op-network/src/bin/op-xdp-wg.rs:360-372`
* **Impact:** 
  The binary invokes external executables (`incus`, `clang`, `tc`, `sysctl`, `ip`, `xdp-loader`) using direct shell execution. If the parent path execution environment is compromised or if inputs like interface names contain unvalidated characters, this can lead to unexpected command outcomes.
* **Remediation:** 
  Validate inputs strictly before passing them as command arguments and always use absolute binary paths for execution.