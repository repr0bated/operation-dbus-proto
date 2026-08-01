# Production Security and Quality Audit: `op-network`

---

## 1. Data Structures & Quality Metrics

### 1.1 Primitive & Synchronization Type Counts per File

The table below catalogs all occurrences of `Arc`, `Rc`, `RefCell`, `RwLock`, `Mutex`, and `OnceCell` (including `OnceLock` equivalents), as well as explicit `.clone()` calls across the monitored codebase.

| File Path | `Arc` | `Rc` | `RefCell` | `RwLock` | `Mutex` | `OnceCell`/`OnceLock` | `.clone()` Count |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| `crates/op-network/src/ovs_error.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-network/src/ovs_netlink.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-network/src/lib.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-network/src/ovs_capabilities.rs` | 0 | 0 | 0 | 1 | 0 | 1 | 3 |
| `crates/op-network/src/rtnetlink.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 3 |
| `crates/op-network/src/proxmox.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 12 |
| `crates/op-network/src/controller.rs` | 1 | 0 | 0 | 0 | 0 | 0 | 2 |
| `crates/op-network/src/openflow.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 2 |
| `crates/op-network/src/ovsdb.rs` | 2 | 0 | 0 | 0 | 1 | 0 | 8 |
| `crates/op-network/src/plugin.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 1 |
| `crates/op-network/src/bin/op-xdp-wg.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-network/src/bin/op-of-controller.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-network/src/bin/op-ovsbr0-afxdp.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-network/src/bin/op-ovsbr0-setup.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 1 |

> **Clone Count Flag:** No individual file exceeds the threshold of 20 `.clone()` calls.

---

### 1.2 Large Structs (> 5 Public Fields)

The following public structs exceed the architectural guideline of having at most 5 public fields. These should be refactored into hierarchical structures or configuration blocks.

1. **`OvsCapabilities`** (`crates/op-network/src/ovs_capabilities.rs:29`)
   * **15 public fields**: `can_list_bridges`, `can_create_bridges`, `can_add_ports`, `can_delete_bridges`, `can_query_flows_openflow`, `can_add_flows_openflow`, `can_list_datapaths`, `can_create_datapaths`, `can_list_vports`, `can_dump_kernel_flows`, `is_root`, `ovs_running`, `ovsdb_socket_exists`, `kernel_module_loaded`, `ovsdb_socket_path`.
2. **`NetworkInterface`** (`crates/op-network/src/rtnetlink.rs:12`)
   * **8 public fields**: `name`, `index`, `mac_address`, `mtu`, `flags`, `state`, `kind`, `addresses`.
3. **`LxcContainer`** (`crates/op-network/src/proxmox.rs:59`)
   * **10 public fields**: `vmid`, `name`, `status`, `cpu`, `mem`, `maxmem`, `disk`, `maxdisk`, `uptime`, `extra`.
4. **`CreateContainerRequest`** (`crates/op-network/src/proxmox.rs:88`)
   * **18 public fields**: `vmid`, `ostemplate`, `hostname`, `memory`, `swap`, `cores`, `rootfs`, `net0`, `unprivileged`, `features`, `start`, `onboot`, `protection`, `nameserver`, `searchdomain`, `password`, `ssh_public_keys`, `storage`.
5. **`ContainerStatus`** (`crates/op-network/src/proxmox.rs:132`)
   * **14 public fields**: `status`, `vmid`, `name`, `cpu`, `mem`, `maxmem`, `diskread`, `diskwrite`, `netin`, `netout`, `uptime`, `pid`, `ha`, `extra`.
6. **`FlowEntry`** (`crates/op-network/src/openflow.rs:33`)
   * **6 public fields**: `priority`, `match_fields`, `actions`, `idle_timeout`, `hard_timeout`, `cookie`.
7. **`OvsBridge`** (`crates/op-network/src/plugin.rs:33`)
   * **8 public fields**: `name`, `datapath_type`, `ports`, `internal_ports`, `address`, `dhcp`, `vlan`, `openflow`.

---

### 1.3 Globally Mutable State

The codebase contains one instance of thread-safe globally shared mutable state:

* **`CAPABILITY_CACHE`** (`crates/op-network/src/ovs_capabilities.rs:21`)
  * **Type:** `static CAPABILITY_CACHE: OnceLock<RwLock<Option<CachedCapabilities>>>`
  * **Risk:** Although thread-safe via `OnceLock` and `RwLock` primitives, runtime state changes (such as modifications to the underlying socket file or OVS engine state) can yield out-of-sync cached capabilities because of the 5-minute hardcoded expiration window.

---

## 2. Schema-As-Code Violations

The codebase frequently falls back to expressing structured schemas, configurations, and network payload data contracts as ad-hoc Rust structs, raw JSON strings, or unchecked dynamic `serde_json::Value` objects instead of using versioned, declarative data schemas (such as Protocol Buffers or OSCAL).

1. **Ad-Hoc Network Configurations** (`crates/op-network/src/plugin.rs:33`, `crates/op-network/src/plugin.rs:88`)
   * Structs like `OvsBridge`, `OpenFlowConfig`, `NetworkInterface`, and `OvsdbConfig` represent configuration data formats directly as ad-hoc Rust structures serialized to/from JSON. These contracts should be validated via versioned Protocol Buffer definitions.
2. **Proxmox REST API Contracts** (`crates/op-network/src/proxmox.rs:88`, `crates/op-network/src/proxmox.rs:132`)
   * The container creation requests and status structures (`CreateContainerRequest`, `ContainerStatus`, `LxcContainer`) are modeled via ad-hoc structs and loose `HashMap<String, serde_json::Value>` bags. These should align with a declarative, schema-validated contract.
3. **Manual JSON-RPC Transaction Assembly** (`crates/op-network/src/ovsdb.rs:622`, `crates/op-network/src/ovsdb.rs:663`, `crates/op-network/src/bin/op-ovsbr0-afxdp.rs:309`)
   * The system manually formats and constructs raw OVSDB JSON-RPC transaction payloads using `serde_json::json!` macros (e.g., executing `select`, `update`, or `mutate` operations). These ad-hoc database transactions represent raw query contracts rather than strongly versioned schema models.

---

## 3. Security Findings & Vulnerabilities

### CRITICAL: Proxmox API Client Disables TLS Validation Default
* **Location:** `crates/op-network/src/proxmox.rs:217`
* **Impact:** High probability of Man-in-the-Middle (MitM) attacks leading to sensitive credential theft.
* **Exploitability:** Directly exploitable. Disabling certificate validation allows any local network attacker to spoof the Proxmox VE gateway, intercept the authentication headers containing `PVE_API_TOKEN_SECRET` (`crates/op-network/src/proxmox.rs:290`), and use the stolen administrative token to compromise the hypervisor node.
* **Source Evidence:**
  ```rust
  let client = Client::builder()
      .danger_accept_invalid_certs(true)
      .timeout(Duration::from_secs(30))
      .connect_timeout(Duration::from_secs(10))
      .build()
      .expect("Failed to create HTTP client");
  ```
* **Remediation:** Remove `.danger_accept_invalid_certs(true)` from production client configurations. Ensure the Proxmox CA certificate is loaded into the host system store or explicitly passed to the client builder as a trusted root anchor.

---

### CRITICAL: Privilege Escalation via `OP_XDP_WG` Environment Variable
* **Location:** `crates/op-network/src/bin/op-ovsbr0-afxdp.rs:154`
* **Impact:** Arbitrary Command Execution as `root`.
* **Exploitability:** Directly exploitable. The binary executes with root/CAP_NET_ADMIN privileges to perform netlink actions. However, it resolves the file path to its helper executable `op-xdp-wg` via the environment variable `OP_XDP_WG`. An unprivileged user executing the binary under sudo or via system services with preserved environments can point `OP_XDP_WG` to a malicious binary, causing arbitrary code execution under a root security context.
* **Source Evidence:**
  ```rust
  fn rechain_xdp_steer(ifname: &str) -> Result<()> {
      let helper = std::env::var("OP_XDP_WG").unwrap_or_else(|_| {
          if Path::new("/usr/local/sbin/op-xdp-wg").exists() {
              "/usr/local/sbin/op-xdp-wg".into()
          } else {
              "op-xdp-wg".into()
          }
      });

      let status = Command::new(&helper)
          .arg("chain")
          .status()
          .with_context(|| format!("failed to execute {}", helper))?;
  ```
* **Remediation:** Remove environment variable dynamic path resolution for executable binaries. Hardcode the absolute path `/usr/local/sbin/op-xdp-wg` to ensure only trusted, system-controlled binaries can be launched with root privileges.

---

### HIGH: TOCTOU Local Privilege Escalation via World-Writable BPF compilation folder
* **Location:** `crates/op-network/src/bin/op-xdp-wg.rs:366`
* **Impact:** Kernel-Level Privilege Escalation / Arbitrary Code Execution.
* **Exploitability:** Exploitable by local users. The BPF compilation function writes source and compiled object code directly to `/etc/op-network/xdp` using `fs::create_dir_all` without imposing strict file-system permissions. If the default system umask is lax, or if the system path is readable/writable by unauthorized system users, a local attacker can swap out the compiled BPF object file `op-xdp-wg.o` prior to loading (Time-of-Check to Time-of-Use), loading a malicious XDP program directly into the host kernel with root authority.
* **Source Evidence:**
  ```rust
  fs::create_dir_all(BPF_DIR).with_context(|| format!("create {}", BPF_DIR))?;
  fs::write(BPF_C_PATH, src).with_context(|| format!("write {}", BPF_C_PATH))?;
  // ...
  run(
      "clang",
      [
          "-O2", "-g", "-target", "bpf", "-c", BPF_C_PATH, "-o", BPF_O_PATH,
      ],
  )
  ```
* **Remediation:** Restrict file-system access of the generation output folder. Under Unix environments, explicitly lock down directory permissions during creation to `0700` (root-only access) using `std::os::unix::fs::DirBuilderExt` or apply equivalent permission masks immediately upon creation.

---

### HIGH: Denial of Service via Dynamic Panic in OVSDB Client Core
* **Location:** `crates/op-network/src/ovsdb.rs:52`
* **Impact:** Complete panic crash of the orchestration control plane.
* **Exploitability:** Exploitable if OVSDB state is corrupted or manipulated by malicious database inputs.
* **Description:** The function `uuid_ref` performs an unvalidated parsing action on UUID strings and panics if parsing fails. However, the UUID strings are loaded dynamically from the live, unvalidated OVSDB database server (`crates/op-network/src/ovsdb.rs:649` via the slow-path JSON result). If an attacker gets write access to OVSDB or if OVSDB is modified to have non-standard UUID strings, any transaction call utilizing `uuid_ref` will trigger a panic in the running Rust controller thread.
* **Source Evidence:**
  ```rust
  fn uuid_ref(uuid: &str) -> Value {
      let parsed: Uuid = uuid
          .parse()
          .unwrap_or_else(|e| panic!("uuid_ref: invalid UUID {:?}: {}", uuid, e));
      RowRef::Uuid(parsed).to_json()
  }
  ```
* **Remediation:** Change `uuid_ref` to return a `Result<Value, uuid::Error>` instead of panicking on invalid input, enabling the calling application to handle format parsing failures gracefully.

---

### MEDIUM: Argument Injection in Route Configuration
* **Location:** `crates/op-network/src/rtnetlink.rs:379`
* **Impact:** Potential bypass of intended network path controls or system network state manipulation.
* **Exploitability:** Sourced from inputs like database parameters or container requests.
* **Description:** The function `add_default_route_onlink` invokes the system `ip` command directly. Although `Command::new` does not invoke an intermediate shell, the inputs `gateway` and `ifname` are passed as raw arguments without validation. An attacker with control over the configuration parameters can pass unexpected string flags (e.g. arguments starting with `-`), triggering unexpected parsing behaviors in `iproute2`.
* **Source Evidence:**
  ```rust
  pub async fn add_default_route_onlink(ifname: &str, gateway: &str) -> Result<()> {
      use std::process::Command;
      let status = Command::new("ip")
          .args([
              "route", "replace", "default", "via", gateway, "dev", ifname, "onlink",
          ])
          .status()
  ```
* **Remediation:** Validate that `gateway` represents a valid IP address by parsing it as an `IpAddr` object, and sanitize `ifname` to contain only valid system interface name alphanumeric characters.