# Security & Quality Audit Report: `op-network`

## 1. Complete List of `std::env::var` Reads

The following is a comprehensive inventory of all environment variables read via `std::env::var` across the crate.

| File Path | Line | Environment Variable | Default Value / Fallback |
| :--- | :--- | :--- | :--- |
| `crates/op-network/src/proxmox.rs` | 249 | `PVE_TOKEN_FILE` | `"/etc/op-dbus/pve-token"` |
| `crates/op-network/src/proxmox.rs` | 294 | `PVE_API_URL` | `"https://localhost:8006"` |
| `crates/op-network/src/bin/op-xdp-wg.rs` | 104 | `LXC_PID` | None (Falls back to `incus_pid` query) |
| `crates/op-network/src/bin/op-of-controller.rs` | 24 | `OF_CONTROLLER_LISTEN` | `"10.200.0.1:6653"` |
| `crates/op-network/src/bin/op-of-controller.rs` | 29 | `OF_FLOW_PAIRS` | `"grpc-bridge:ovsbr0-sock"` |
| `crates/op-network/src/bin/op-of-controller.rs` | 32 | `OF_FLOW_PRIORITY` | `100` |
| `crates/op-network/src/bin/op-ovsbr0-afxdp.rs` | 38 | `BR` | `"ovsbr0"` |
| `crates/op-network/src/bin/op-ovsbr0-afxdp.rs` | 39 | `UPLINK` | `"eth0"` |
| `crates/op-network/src/bin/op-ovsbr0-afxdp.rs` | 40 | `MGMT_ADDR` | `"148.113.204.83/32"` |
| `crates/op-network/src/bin/op-ovsbr0-afxdp.rs` | 42 | `GW` | `"148.113.204.1"` |
| `crates/op-network/src/bin/op-ovsbr0-afxdp.rs` | 43 | `OVSDB_SOCKET` | Dynamically auto-detected candidate path |
| `crates/op-network/src/bin/op-ovsbr0-afxdp.rs` | 160 | `OP_XDP_WG` | `/usr/local/sbin/op-xdp-wg` (if exists) or `"op-xdp-wg"` |
| `crates/op-network/src/bin/op-ovsbr0-setup.rs` | 44 | `BRIDGE` | `"ovsbr0"` |
| `crates/op-network/src/bin/op-ovsbr0-setup.rs` | 45 | `VETH_HOST` | `"grpc-uplink"` |
| `crates/op-network/src/bin/op-ovsbr0-setup.rs` | 46 | `FAIL_MODE` | `"standalone"` |
| `crates/op-network/src/bin/op-ovsbr0-setup.rs` | 47 | `SHARED_MAC` | `"fa:16:3e:f1:71:d2"` |
| `crates/op-network/src/bin/op-ovsbr0-setup.rs` | 48 | `OVSDB_SOCKET` | Dynamically auto-detected candidate path |
| `crates/op-network/src/bin/op-ovsbr0-setup.rs` | 49 | `VSWITCHD_SVC` | `"/run/service/ovs-vswitchd"` |

---

## 2. Flagged Environment Variables (No Defaults / Unhandled Errors)

*   **`LXC_PID`** (`crates/op-network/src/bin/op-xdp-wg.rs:104`):
    *   **Risk**: Low. No default is explicitly specified via `.unwrap_or()`, but the code safely handles the `Err` case of `std::env::var` by executing a fallback query to the hypervisor daemon (`incus_pid`). There is no risk of a direct panic due to this variable being absent.
*   **`OF_CONTROLLER_LISTEN`** (`crates/op-network/src/bin/op-of-controller.rs:24`):
    *   **Risk**: Medium (Deferred Panic). Although it falls back to a default value, the parsed socket address is immediately unpacked with `.expect()` at line 27:
        ```rust
        .parse()
        .expect("OF_CONTROLLER_LISTEN must be a valid socket address");
        ```
        If the variable is present but contains an invalid socket address, the binary will crash with an unhandled panic.

---

## 3. Cargo Features & Additive Behavior

### Package-Level Features
The workspace package `op-network` does not define any features in its local `Cargo.toml` (`crates/op-network/Cargo.toml`).

### Workspace-Level Features
In the root `Cargo.toml`, the following workspace feature is declared:
```toml
[features]
default = ["grpc"]
grpc = []
```

### Additive Behavior Analysis
Cargo features are strictly **additive**. In a workspace configuration, features enabled for a crate by one package dependency are enabled for all packages sharing that dependency. 
*   **Implication**: De-selecting the `default` workspace features must be done explicitly using `default-features = false` in downstream packages. Because `grpc` is enabled by default, both the gRPC client dependencies and reflection/transport layers will always be built unless actively suppressed.

---

## 4. Flagged Hardcoded Elements

### 4.1. Hardcoded Paths (OVSDB, System Configs, Kernels)
*   **`/var/run/openvswitch/db.sock`** (`crates/op-network/src/ovs_capabilities.rs:120`, `142`; `crates/op-network/src/plugin.rs:126`): Used to verify the existence and locate the OVSDB UNIX domain socket.
*   **`/proc/modules`** (`crates/op-network/src/ovs_capabilities.rs:148`): Probes kernel module list for `openvswitch`.
*   **`/etc/op-dbus/pve-token`** (`crates/op-network/src/proxmox.rs:249`): Hardcoded target path for Proxmox API token files.
*   **`/run/openvswitch/db.sock`** & **`/var/run/openvswitch/db.sock`** (`crates/op-network/src/ovsdb.rs:31`, `32`): Candidate resolution paths.
*   **`/etc/openvswitch/conf.db`** (`crates/op-network/src/plugin.rs:130`): Default path for persistent bridge databases.
*   **`/etc/op-network/xdp`** & BPF binary assets (`crates/op-network/src/bin/op-xdp-wg.rs:30`, `31`, `32`):
    *   `BPF_DIR`: `"/etc/op-network/xdp"`
    *   `BPF_C_PATH`: `"/etc/op-network/xdp/op-xdp-wg.c"`
    *   `BPF_O_PATH`: `"/etc/op-network/xdp/op-xdp-wg.o"`
*   Candidate UNIX Socket Lists (`crates/op-network/src/bin/op-ovsbr0-afxdp.rs:58-61` & `crates/op-network/src/bin/op-ovsbr0-setup.rs:53-56`):
    *   `"/usr/local/var/run/openvswitch/db.sock"`
    *   `"/run/openvswitch/db.sock"`
    *   `"/var/run/openvswitch/db.sock"`
*   **`/usr/local/sbin/op-xdp-wg`** (`crates/op-network/src/bin/op-ovsbr0-afxdp.rs:161`): Target execution path for the XDP steer utility.
*   **`/run/service/ovs-vswitchd`** (`crates/op-network/src/bin/op-ovsbr0-setup.rs:50`): Target s6 control service location.
*   **`/usr/local/var/run/openvswitch`** (`crates/op-network/src/bin/op-ovsbr0-setup.rs:68`): Target control socket directory.
*   **`/sys/class/net/{}`** (`crates/op-network/src/bin/op-ovsbr0-setup.rs:163`): Interface verification in sysfs.

### 4.2. Hardcoded Ports, IP Addresses & MACs
*   **`"https://localhost:8006"`** (`crates/op-network/src/proxmox.rs:214`, `295`): Proxmox API target endpoint.
*   **`"fa:16:3e:f1:71:d2"`** (`crates/op-network/src/bin/op-xdp-wg.rs:27` & `crates/op-network/src/bin/op-ovsbr0-setup.rs:47`): Shared bridge MAC address.
*   **`"2607:5300:205:200::5bc7"`** (`crates/op-network/src/bin/op-xdp-wg.rs:28`): Container destination IPv6 address for XDP steering.
*   **`"10.200.0.1:6653"`** & **`"tcp:10.200.0.1:6653"`** (`crates/op-network/src/plugin.rs:92`, `406`; `crates/op-network/src/bin/op-of-controller.rs:25`): Fallback OpenFlow controller binding IPs and port (`6653`).
*   **`"10.200.0.2/30"`** (`crates/op-network/src/bin/op-xdp-wg.rs:344`): Static IPv4 endpoint assigned to the veth peer.
*   **`"148.113.204.83/32"`** (`crates/op-network/src/bin/op-ovsbr0-afxdp.rs:41`): Management public IPv4.
*   **`"148.113.204.1"`** (`crates/op-network/src/bin/op-ovsbr0-afxdp.rs:42`): Hardcoded network gateway IP.

---

## 5. Schema-As-Code Violations (Ad-Hoc Contracts)

Adherence to the **Schema-as-Code** discipline requires that core data contracts and configuration schemas be declared via versioned, deterministic definitions (such as Protocol Buffers or official JSON schemas) rather than ad-hoc Rust structs or raw serialization objects. 

The following architectural boundaries violate this discipline by exposing or consuming ad-hoc, unversioned contracts:

*   **OVS Kernel Datapath Structs** (`crates/op-network/src/ovs_netlink.rs:107-160`):
    `Datapath`, `DatapathStats`, `Vport`, `VportType`, `KernelFlow`, and `FlowStats` are declared as local Rust structures using unversioned `serde::Serialize` attributes.
*   **OVS Capabilities Contract** (`crates/op-network/src/ovs_capabilities.rs:42`):
    The `OvsCapabilities` struct defines systemic capabilities as an ad-hoc struct exposed directly to consuming services.
*   **Hypervisor Interface Contracts** (`crates/op-network/src/proxmox.rs:64-210`):
    `LxcContainer`, `CreateContainerRequest`, and `ContainerStatus` represent external virtualization control plane models as ad-hoc local Rust structs. This creates a brittle dependency on Proxmox VE API contracts without a versioned schema boundary.
*   **Network Interface & Address Contracts** (`crates/op-network/src/rtnetlink.rs:11-30`):
    `NetworkInterface` and `InterfaceAddress` expose network topology structures in an ad-hoc serializable form.
*   **Network Configuration Model** (`crates/op-network/src/plugin.rs:19-141`):
    `NetworkPlugin`, `OvsBridge`, `OpenFlowConfig`, `NetworkInterface`, and `OvsdbConfig` represent configuration models constructed via unversioned, hand-rolled Rust structs.
*   **Ad-Hoc OVSDB Transactions** (`crates/op-network/src/ovsdb.rs:144` & throughout):
    OVSDB operations are built using dynamic `serde_json::json!` macros (e.g. constructing `select`, `update`, and `insert` mutations dynamically on lines 712, 744, 765, 831). This relies on hand-crafted JSON contracts rather than compiled schema primitives.

---

## 6. High & Critical Security Risks

### High Risk: Complete Disabling of TLS Certificate Validation on Hypervisor Client
*   **Location**: `crates/op-network/src/proxmox.rs:219`
*   **Code**:
    ```rust
    let client = Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .build()
        .expect("Failed to create HTTP client");
    ```
*   **Vulnerability Analysis**:
    Disabling TLS verification via `.danger_accept_invalid_certs(true)` strips away all protection against Man-in-the-Middle (MITM) attacks. Because `ProxmoxClient` communicates with hypervisor nodes that may reside outside the immediate machine (via `PVE_API_URL`), any network attacker on the local routing segment can intercept the highly privileged API token headers (`PVEAPIToken` authorization values formulated at line 59).
*   **Remediation**:
    Enforce proper trust roots. If self-signed certificates are used in production environments, load the specific self-signed CA certificate into the client's trust store using `.add_root_certificate()` on the client builder rather than disabling verification globally.