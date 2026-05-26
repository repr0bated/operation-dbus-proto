# Production Security & Quality Audit: op-plugins

## Architecture & Module Map

### Overview
The `op-plugins` crate implements a domain-specific system control plane and plugin management architecture. It coordinates system state reconciliation (LXC, Incus, OpenFlow, networking, systemd, etc.) with a shared schema catalog. It provides automated state hashing to generate blockchain footprints for audit trails, ensuring high-fidelity configuration tracking on a distributed ledger.

### Module Tree
```
crates/op-plugins/src/
├── lib.rs
├── auto_create.rs
├── builtin.rs
├── chat.rs
├── default_registry.rs
├── dynamic_loading.rs
├── plugin.rs
├── registry.rs
├── service_def.rs
├── state.rs
├── state_publisher.rs
└── state_plugins/
    ├── mod.rs
    ├── adc.rs
    ├── agent_config.rs
    ├── config.rs
    ├── dinit.rs
    ├── dnsresolver.rs
    ├── endpoint.rs
    ├── full_system.rs
    ├── gcloud_adc.rs
    ├── hardware.rs
    ├── incus.rs
    ├── keypair.rs
    ├── keyring.rs
    ├── login1.rs
    ├── lxc.rs
    ├── mcp.rs
    ├── net.rs
    ├── netmaker.rs
    ├── openflow.rs
    ├── openflow_obfuscation.rs
    ├── ovsdb_bridge.rs
    ├── packagekit.rs
    ├── pcidecl.rs
    ├── privacy.rs
    ├── privacy_router.rs
    ├── privacy_routes.rs
    ├── procfs.rs
    ├── proxmox.rs
    ├── proxy_server.rs
    ├── rtnetlink.rs
    ├── schema_contract.rs
    ├── service.rs
    ├── sessdecl.rs
    ├── software.rs
    ├── systemd.rs
    ├── systemd_networkd.rs
    ├── unix_socket.rs
    ├── users.rs
    ├── web_ui.rs
    └── wireguard.rs
```

### Entry Points
*   **Library Interface**: `crates/op-plugins/src/lib.rs` - Re-exports core traits (`Plugin`), configuration registries, change tracking types, and pre-baked state plugins.
*   **Built-in Registry**: `crates/op-plugins/src/default_registry.rs` - Wires default plugins on control-plane initialization.

---

## Security & Quality Findings

### [CRITICAL] Shell Command Injection in `pcidecl` Plugin
*   **File & Line**: `crates/op-plugins/src/state_plugins/pcidecl.rs:72`
*   **Description**: 
    The `lspci_present` helper spawns a shell process to check PCI device presence via `lspci`:
    ```rust
    fn lspci_present(addr: &str) -> bool {
        if let Ok(out) = Command::new("sh")
            .arg("-c")
            .arg(format!("lspci -s {} >/dev/null 2>&1; echo $?", addr))
            .output()
        { ... }
    }
    ```
    The `addr` parameter is sourced directly from the desired state definition, which is decoded from external configurations, network payload inputs, or D-Bus method calls. If an attacker inputs an address string containing shell metacharacters (e.g., `0000:00:1f.6; malicious_command;`), they can execute arbitrary shell commands with the root privileges of the parent control plane.
*   **Remediation**:
    Avoid invoking commands via `/bin/sh -c`. Pass the arguments safely as an array directly to `Command::new("lspci")`:
    ```rust
    let status = Command::new("lspci")
        .args(["-s", addr])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    ```

---

### [CRITICAL] Path Traversal in LXC State Plugin
*   **File & Line**: `crates/op-plugins/src/state_plugins/lxc.rs:491` and `crates/op-plugins/src/state_plugins/lxc.rs:649`
*   **Description**:
    The LXC state plugin does not sanitize or validate `container.id` before performing host filesystem operations. 
    1. At `lxc.rs:491`, `container.id` is used to build snapshot paths:
       ```rust
       let container_rootfs = format!("{}/images/{}/rootfs", storage_path, container.id);
       ```
    2. At `lxc.rs:649`, `container.id` is used to write configuration files:
       ```rust
       let config_path = format!("/etc/pve/lxc/{}.conf", container.id);
       tokio::fs::write(&config_path, config).await?;
       ```
    If `container.id` contains traversal sequences (e.g., `../../../../`), a compromised or unauthorized user with permission to supply desired states can write arbitrary configurations anywhere on the host filesystem or target snapshots onto sensitive directories.
*   **Remediation**:
    Sanitize the `id` field using a strict whitelist validation regex (such as `/^[a-zA-Z0-9_-]+$/`) and verify that path boundaries are restricted to `/etc/pve/lxc/` and the target storage pool.

---

### [MEDIUM] Broken Cryptographic Integrity of Audit Trail via MD5
*   **File & Line**: `crates/op-plugins/src/lib.rs:1` (and across multiple state plugins, e.g., `crates/op-plugins/src/auto_create.rs:117`, `crates/op-plugins/src/state_plugins/config.rs:159`, `crates/op-plugins/src/state_plugins/dnsresolver.rs:241`)
*   **Description**:
    The plugin system implements automatic state-hash generation to establish immutable "blockchain footprints" for configuration verification and ledger tracking. However, the system relies on the MD5 hashing algorithm to generate these hashes:
    ```rust
    current_hash: format!("{:x}", md5::compute(simd_json::to_string(current)?)),
    ```
    MD5 is cryptographically broken and subject to collision attacks. An attacker can craft two distinct desired configurations that yield the same MD5 hash, allowing unauthorized system modifications to bypass collision-detection audits on the ledger.
*   **Remediation**:
    Standardize the audit trail hash algorithms to `SHA-256`, utilizing the `sha2` crate already imported in the workspace dependencies.

---

### [MEDIUM] Broken Syntax in `netmaker` Package Installation Command
*   **File & Line**: `crates/op-plugins/src/state_plugins/netmaker.rs:252`
*   **Description**:
    The `NetmakerPlugin` attempts to install its package dependency on Debian/Ubuntu systems using the following invocation:
    ```rust
    let install_result = Command::new("apt")
        .args(["update", "&&", "apt", "install", "-y", "netclient"])
        .status()
        .await;
    ```
    This passes the shell operator `&&` as a literal argument to the binary `apt`. Since `apt` does not parse shell chaining operators, this call will fail to execute, preventing the system from automatically updating or installing `netclient`.
*   **Remediation**:
    Execute the update and install steps as distinct sequential commands without shell symbols:
    ```rust
    Command::new("apt").arg("update").status().await?;
    Command::new("apt").args(["install", "-y", "netclient"]).status().await?;
    ```

---

### [LOW] Hardcoded Static MAC Address in Privacy Router OpenFlow Configurations
*   **File & Line**: `crates/op-plugins/src/state_plugins/privacy_router.rs:491`
*   **Description**:
    The OVS-based privacy router registers a default ARP responder utilizing a hardcoded static MAC address:
    ```rust
    actions.push(FlowAction::ArpResponder {
        mac: "00:11:22:33:44:55".to_string(),
        ip: "10.200.0.1".to_string(),
    });
    ```
    If multiple hardware nodes run this default configuration on the same Layer 2 domain, it will cause MAC address conflicts and routing failures.
*   **Remediation**:
    Read the host's actual network interface hardware MAC address dynamically, or generate a deterministic MAC address scoped to the local router.

---

### [LOW] Resource Leak via Orphaned Containers on LXC Startup Failure
*   **File & Line**: `crates/op-plugins/src/state_plugins/lxc.rs:842`
*   **Description**:
    When creating a container, the workflow triggers `create_container` first, followed by `start_container`. If container creation succeeds but the subsequent startup fails, the plugin appends the error to the list and executes a `continue` block. The partially initialized, stopped container remains on the host. Over time, this leads to configuration drift, resource pollution, and host system storage exhaustion.
*   **Remediation**:
    Implement transactional cleanup. If startup or network registration fails after container creation, trigger a roll-back operation to destroy the newly spawned container.