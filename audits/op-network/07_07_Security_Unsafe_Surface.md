# Production Security and Quality Audit: op-network

---

## 1. Executive Summary

This production security and quality audit evaluates the `op-network` crate to identify potential security vulnerabilities, adherence to code quality policies, and architectural alignment with the project's requirements. 

### Key Findings
* **Critical Privilege Escalation Vector**: Local privilege escalation is possible through the unvalidated `OP_XDP_WG` environment variable, which dynamically defines the path of an executable executed with root privileges.
* **Architecture / Forbidden Command Violations**: The binary `op-ovsbr0-setup` directly executes the forbidden command `ovs-dpctl`, bypassing native programmatic Netlink APIs and violating the "native pure Rust" design constraint.
* **Schema-as-Code Violations**: Throughout `op-network`, data contracts for Proxmox and network configurations are declared using ad-hoc Serde-serializable structs and untyped raw `serde_json::Value` objects instead of formal Protocol Buffer or OSCAL schemas.
* **Secret & Configuration Hardcoding**: Multiple binaries contain hardcoded public IPv4/IPv6 addresses, MAC addresses, and routing destinations, creating operational rigidity and config leak risks.

---

## 2. Schema-as-Code Violations

The codebase violates the schema-as-code discipline by expressing data contracts as ad-hoc Rust structures with Serde attributes or untyped JSON primitives, rather than using versioned schemas (such as Protocol Buffers or OSCAL schemas).

### Specific Violations:

* **Ad-hoc Proxmox API Contracts**:
  * **Location**: `crates/op-network/src/proxmox.rs:56-149`
  * **Description**: The data contracts for managing LXC containers—such as `LxcContainer`, `CreateContainerRequest`, `ContainerStatus`, and `TaskStatus`—are defined as ad-hoc Rust structs with inline `#[serde(...)]` attributes. Under a strict schema-as-code discipline, these external API models should be generated from a centralized OpenAPI or Protocol Buffer specification to ensure versioning, cross-language compatibility, and schema validation.
  
* **Ad-hoc Network Plugin Configuration State**:
  * **Location**: `crates/op-network/src/plugin.rs:21-71`
  * **Description**: Configuration schemas (`NetworkPlugin`, `OvsBridge`, `OpenFlowConfig`, `NetworkInterface`, `OvsdbConfig`) are declared as ad-hoc structs. State synchronization and orchestration configs must be formally defined as versioned schemas (e.g., OSCAL Component Definitions or Protobuf) to guarantee contract immutability across control-plane updates.

* **Untyped Route Data Representation**:
  * **Location**: `crates/op-network/src/rtnetlink.rs:188`
  * **Description**: The `get_default_route` function returns an untyped `serde_json::Value` constructed on the fly using the `json!` macro:
    ```rust
    return Ok(Some(serde_json::json!({
        "gateway": gateway,
        "interface_index": oif_index,
        "interface_name": oif_name,
        "destination": "0.0.0.0/0",
    })));
    ```
    This lacks contract validation, making consumers vulnerable to runtime deserialization failures if internal fields are modified or missing.

* **OVS Netlink Raw State Structs**:
  * **Location**: `crates/op-network/src/ovs_netlink.rs:114-162`
  * **Description**: Structs like `Datapath`, `DatapathStats`, `Vport`, and `KernelFlow` represent kernel/userspace data contracts but are defined as ad-hoc structures rather than generated code.

---

## 3. Unsafe Code Blocks & Missing Safety Comments

The codebase was audited for the use of the `unsafe` keyword. Exactly **one** instance of unsafe code was detected.

### Unsafe Block Details:

* **Location**: `crates/op-network/src/ovs_capabilities.rs:114`
* **Code Context**:
  ```rust
  let is_root = unsafe { libc::geteuid() == 0 };
  ```
* **Audit Finding**: This unsafe block is **missing** a `// SAFETY:` comment explaining why the invocation is safe. 
* **Remediation**: Although `libc::geteuid` is an infallible system call that simply reads a field from the process's credentials and cannot trigger undefined memory behavior, strict safety standards require documenting why the FFI call is safe:
  ```rust
  // SAFETY: `geteuid` is an infallible, stateless system call that does not dereference 
  // raw pointers or modify thread-local state.
  let is_root = unsafe { libc::geteuid() == 0 };
  ```

---

## 4. Command Invocations Audit

An audit of all command execution sites (`Command::new` and `tokio::process::Command::new`) was performed.

### Total Command Invocations Count
There are **20** instances of command execution across the audited files.

### Forbidden Commands Detected (High Severity)
The following forbidden command invocation was discovered:

* **Location**: `crates/op-network/src/bin/op-ovsbr0-setup.rs:174-176`
* **Forbidden Command**: `ovs-dpctl`
* **Code Context**:
  ```rust
  let _ = Command::new("ovs-dpctl")
      .args(["del-dp", "system@ovs-system"])
      .status();
  ```
* **Severity**: **High**
* **Reasoning**: This directly violates the project constraint forbidding raw `ovs-*` shell utilities. It bypasses the custom, native Generic Netlink programmatic client designed in `ovs_netlink.rs` for kernel datapath communication. This introduces shell dependency, security surface, and bypasses memory-safe programmatic controls.

### Command Execution Analysis (Argument Validation & Safety)

1. **Local Privilege Escalation via Environment Variable (`OP_XDP_WG`)**
   * **Location**: `crates/op-network/src/bin/op-ovsbr0-afxdp.rs:198-204`
   * **Vulnerability Context**:
     ```rust
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
     ```
   * **Risk**: The environment variable `OP_XDP_WG` is read directly without validation. If this binary runs with elevated privileges (e.g., via `sudo` or as a privileged service daemon), any local attacker who can influence the environment can point `OP_XDP_WG` to a malicious binary, achieving arbitrary code execution under root privileges.

2. **Ad-Hoc Service Target Control (`VSWITCHD_SVC`)**
   * **Location**: `crates/op-network/src/bin/op-ovsbr0-setup.rs:114`, `125`, `133`
   * **Vulnerability Context**:
     ```rust
     vswitchd_svc: std::env::var("VSWITCHD_SVC")
         .unwrap_or_else(|_| "/run/service/ovs-vswitchd".into()),
     ```
     This value is passed directly to `s6-svc` and `s6-svstat` without validation:
     ```rust
     let _ = Command::new("s6-svc").args(["-d", svc]).status();
     ```
   * **Risk**: Allows local configuration manipulation of s6 process manager arguments.

3. **External Utilities Execution with Variable Arguments**:
   * **rtnetlink.rs:411**: Spawns `Command::new("ip")` with `gateway` and `ifname` arguments. 
   * **plugin.rs:567**: Spawns `tokio::process::Command::new("dhclient")` with `interface` as an argument.
   * **op-xdp-wg.rs:365**: Generic `Command::new(cmd)` executor that runs various external programs (`clang`, `tc`, `sysctl`, `ip`, `incus`) with dynamically built string arguments.
   * **Note**: Although `Command::new` executes programs directly via `execve` on Unix (preventing traditional shell-metacharacter injection), argument injection can still occur if parameters (such as `interface` names or `gateway` addresses) are derived from untrusted inputs.

---

## 5. Hardcoded Configurations

The codebase contains several instances of hardcoded configuration details, including public IP addresses, MAC addresses, and loopback defaults.

### Public IP Addresses:
* **Location**: `crates/op-network/src/bin/op-ovsbr0-afxdp.rs:13`
  ```rust
  let mgmt_addr_str = std::env::var("MGMT_ADDR").unwrap_or_else(|_| "148.113.204.83/32".into());
  ```
* **Location**: `crates/op-network/src/bin/op-ovsbr0-afxdp.rs:14`
  ```rust
  let CONTAINER_ADDR = ... // "15.235.37.41/32"
  ```
* **Location**: `crates/op-network/src/bin/op-ovsbr0-afxdp.rs:15`
  ```rust
  let gw_str = std::env::var("GW").unwrap_or_else(|_| "148.113.204.1".into());
  ```
* **Location**: `crates/op-network/src/bin/op-xdp-wg.rs:25`
  ```rust
  const CT_IPV6: &str = "2607:5300:205:200::5bc7";
  ```
* **Location**: `crates/op-network/src/bin/op-xdp-wg.rs:277` (BPF source code compilation template)
  ```rust
  __u8 ct[16] = {0x26,0x07,0x53,0x00,0x02,0x05,0x02,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x5b,0xc7};
  ```

### Private IP Addresses & OpenFlow Port Defaults:
* **Location**: `crates/op-network/src/plugin.rs:90`
  ```rust
  "tcp:10.200.0.1:6653".to_string()
  ```
* **Location**: `crates/op-network/src/bin/op-of-controller.rs:20`
  ```rust
  "10.200.0.1:6653"
  ```

### MAC Addresses:
* **Location**: `crates/op-network/src/bin/op-xdp-wg.rs:24`
  ```rust
  const HOST_MAC: &str = "fa:16:3e:f1:71:d2";
  ```
* **Location**: `crates/op-network/src/bin/op-ovsbr0-setup.rs:56`
  ```rust
  "fa:16:3e:f1:71:d2"
  ```

---

## 6. Security Vulnerabilities & Exploitation Risks

The following security issues are directly identifiable in the source code:

### 1. Privilege Escalation via `OP_XDP_WG` Environment Variable (Critical)
* **File**: `crates/op-network/src/bin/op-ovsbr0-afxdp.rs:198-204`
* **Mechanism**: The binary resolves the path to `op-xdp-wg` via `std::env::var("OP_XDP_WG")`. It then runs the resulting path directly as root (which is required for this binary to run Netlink calls).
* **Exploitation Path**: 
  1. An unprivileged local user who has `sudo` access to run `op-ovsbr0-afxdp` (or if it is invoked by a privileged daemon keeping the user environment) can set `OP_XDP_WG` to a malicious script (e.g., `export OP_XDP_WG=/tmp/malicious.sh`).
  2. The attacker triggers `op-ovsbr0-afxdp up`.
  3. `op-ovsbr0-afxdp` calls `rechain_xdp_steer`, which executes `/tmp/malicious.sh` with root permissions.
* **Remediation**: Eliminate the environment variable lookup. Use a strictly defined, hardcoded path to the orchestrator tool (e.g., `/usr/local/sbin/op-xdp-wg`), or sanitize the environment before starting execution.

### 2. Unauthenticated OpenFlow Controller Listener (High)
* **File**: `crates/op-network/src/controller.rs:376-418`
* **Mechanism**: The OpenFlow 1.3 controller binds to a TCP port (`listen_addr`) and processes connections from any incoming switch. There is no TLS handshake, certificate validation, or shared secret mechanism.
* **Exploitation Path**:
  1. An attacker on the local network segment connects to the controller port (defaulting to `10.200.0.1:6653`).
  2. The controller sends a `Hello` and a `FeaturesRequest`, then deletes all active flows using `build_flow_mod_delete_all(xid)`.
  3. The controller installs arbitrary mapping rules based on its configuration. This allows an attacker to intercept, divert, or block all network switch traffic.
* **Remediation**: Enforce mutual TLS (mTLS) for all OpenFlow control-plane connections. `rovs-transport` supports TLS connections; the passive TCP listener must be migrated to `tokio-rustls`.

### 3. Untrusted /var/run/openvswitch/db.sock Exposure (Medium)
* **File**: `crates/op-network/src/ovsdb.rs`
* **Mechanism**: The OVSDB JSON-RPC client connects directly to `/var/run/openvswitch/db.sock`. 
* **Exploitation Path**: If the UNIX socket file permissions are misconfigured (e.g., world-writable), any unprivileged local process can make arbitrary modifications to the OVS database, creating/deleting bridges and re-routing virtualization interfaces. While socket permissions are governed by the OS, the OVSDB client does not validate socket ownership or metadata before interacting with it.

---

## 7. D-Bus Exposure Analysis

Based *only* on the provided files, **no D-Bus interfaces or methods are exposed directly within the `op-network` crate**. 

However, looking at the workspace `Cargo.toml`, we can determine how this library integrates into the broader security model of the control plane:
* The workspace contains `op-dbus` which depends on `op-network`.
* If `op-dbus` exposes any methods to unprivileged system-bus peers that internally delegate network orchestration tasks to `op-network` (such as calling `op-ovsbr0-setup` or querying `OvsCapabilities`), **all vulnerabilities in `op-network` (like command execution, argument injection, or hardcoded path execution) become directly translatable to system-bus exploits**. 

---

## 8. Refactoring & Remediation Roadmap

To ensure production-grade security, stability, and adherence to safe systems programming standards, the following roadmap must be executed:

```
[ Phase 1: High-Priority Security Fixes ]
  ├── 1. Eliminate OP_XDP_WG and VSWITCHD_SVC Env Var Lookups
  │      └── Replace with immutable, static absolute paths (/usr/local/sbin/op-xdp-wg).
  ├── 2. Implement mTLS for the OpenFlow Controller
  │      └── Wrap TcpListener inside tokio-rustls using valid system certificates.
  └── 3. Implement Strict Argument Validation on all Command::new Calls
         └── Enforce alphanumeric/regex boundaries on interfaces and gateway inputs.

[ Phase 2: Architectural & Quality Alignment ]
  ├── 4. Remove ovs-dpctl Execution Site
  │      └── Replace with programmatic Generic Netlink OVS datapath deletions via ovs_netlink.rs.
  ├── 5. Add safety justifications (// SAFETY: ...)
  │      └── Document crates/op-network/src/ovs_capabilities.rs:114.
  └── 6. Extract Hardcoded Network Configurations
         └── Migrate default public/private IPs and MACs to configuration files.

[ Phase 3: Schema-as-Code Integration ]
  ├── 7. Define Proxmox and Plugin Configuration State in Protocol Buffers
  │      └── Generate LxcContainer, OvsBridge, and NetworkPlugin via prost-build.
  └── 8. Replace json! Macro Ad-hoc Payload Builders
         └── Implement schema-validated models for all OVSDB transactional payloads.
```