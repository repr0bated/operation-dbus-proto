# LICENSE AUDIT

### 1. Extracted License Fields
*   **Workspace Package License**: `Cargo.toml:46` declares `license = "Apache-2.0"`.
*   **Crate License**: `crates/op-network/Cargo.toml:7` specifies `license.workspace = true`, correctly inheriting the `Apache-2.0` license from the workspace.

### 2. Copyleft Scan of `Cargo.lock`
*   **Weak Copyleft detected**: `Cargo.lock` contains `cozo` version `0.7.6`, which is licensed under the **MPL-2.0** (Mozilla Public License 2.0). 
    *   *Compatibility*: MPL-2.0 is copyleft but file-level and generally compatible with Apache-2.0. However, if any source files belonging to the `cozo` library are modified, those modifications must be made available under the MPL-2.0.
*   **Strong Copyleft (GPL/AGPL/SSPL) Scan**: No crates licensed under GPL, AGPL, or SSPL were found in the provided `Cargo.lock` dependency list.
*   **Runtime Copyleft Risk**: At runtime, `crates/op-network/src/bin/op-xdp-wg.rs:330` programmatically compiles and writes an inline BPF C program to `/etc/op-network/xdp/op-xdp-wg.c` that declares:
    ```c
    char _license[] SEC("license") = "GPL";
    ```
    This is required for the Linux kernel to allow access to GPL-only BPF helper functions (like `bpf_redirect`). While the Rust orchestrator binary itself is compiled and distributed under `Apache-2.0`, the generated and executed BPF bytecode artifact is explicitly self-licensed as `GPL`.

### 3. Crates Natively Missing License Fields
*   N/A. All internal crates in the workspace inherit `license.workspace = true` from the root workspace manifest.

---

# SCHEMA-AS-CODE COMPLIANCE

The codebase violates the schema-as-code discipline by defining critical system configurations, data contracts, and API messages as ad-hoc, unversioned Rust structs with manual `serde` annotations rather than formal, versioned Protocol Buffers or OSCAL component schemas.

### 1. Ad-Hoc Network Configuration Contracts
*   **`crates/op-network/src/plugin.rs:18-97`**: `NetworkPlugin`, `OvsBridge`, `OpenFlowConfig`, `NetworkInterface`, and `OvsdbConfig` are written as raw, unversioned Rust structs with manual default-field fallbacks. Changes to any of these fields across software versions will break parsing of existing configuration files (`state.json` etc.) without a migration path.

### 2. Ad-Hoc Proxmox VE Integration Models
*   **`crates/op-network/src/proxmox.rs:42-171`**: `LxcContainer`, `CreateContainerRequest`, `ContainerStatus`, and `TaskStatus` represent integration boundaries with an external hypervisor API using unstructured, unversioned models. 

### 3. Ad-Hoc Kernel State Representations
*   **`crates/op-network/src/ovs_netlink.rs:99-138`**: Represents low-level kernel structures (`Datapath`, `Vport`, `KernelFlow`) as ad-hoc Rust structs with generic `serde::Serialize` attributes.
*   **`crates/op-network/src/rtnetlink.rs:11-26`**: `NetworkInterface` and `InterfaceAddress` represent kernel network state structures as ad-hoc serializable models.

---

# SECURITY & QUALITY AUDIT

## Critical Findings

### [CRITICAL] Local Privilege Escalation via Arbitrary Environment Variable Execution
*   **File**: `crates/op-network/src/bin/op-ovsbr0-afxdp.rs:143-152`
*   **Impact**: Direct root compromise / Local Privilege Escalation (LPE).
*   **Description**: The binary `op-ovsbr0-afxdp` executes with elevated root privileges (`CAP_NET_ADMIN` / `root` is required to manipulate OVS DB ports and perform `rtnetlink` route flushes). Inside `rechain_xdp_steer`, the code reads the binary location of its helper program directly from the environment variable `OP_XDP_WG`:
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
    An unprivileged user who can invoke this command (or manipulate the environment of the service invoking it) can set the `OP_XDP_WG` environment variable to any arbitrary executable path. When `op-ovsbr0-afxdp` is run as root, it will execute the attacker's arbitrary payload as root.
*   **Remediation**: Eliminate the environment variable fallback. Use a strictly hardcoded, absolute canonical path (e.g., `/usr/local/sbin/op-xdp-wg`) that is verified to be owned by `root` and writeable only by `root`.

### [CRITICAL] Hypervisor Credential Leakage via Disabled TLS Certificate Validation
*   **File**: `crates/op-network/src/proxmox.rs:185-189`
*   **Impact**: Severe network-wide credential exposure / cluster takeover.
*   **Description**: The Proxmox API client disables TLS certificate validation using `.danger_accept_invalid_certs(true)`:
    ```rust
    let client = Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .build()
        .expect("Failed to create HTTP client");
    ```
    When the client authenticates using a Proxmox API token, it transmits the highly privileged secret token in plaintext headers over this unverified connection. This allows any attacker capable of performing a Man-in-the-Middle (MITM) attack on the local network segment to capture the API token and compromise the entire Proxmox VE hypervisor cluster.
*   **Remediation**: Remove `.danger_accept_invalid_certs(true)`. For systems using self-signed Proxmox certificates, require the operator to supply the CA certificate and load it securely using `.add_root_certificate()`.

---

## Medium & Low Findings

### [MEDIUM] Insecure Temporary File Compilation and Symlink Vulnerability
*   **File**: `crates/op-network/src/bin/op-xdp-wg.rs:330-348`
*   **Impact**: Arbitrary file overwrite / local privilege escalation.
*   **Description**: In `compile_bpf`, the program writes a generated C source file directly to `/etc/op-network/xdp/op-xdp-wg.c` and compiles it to `op-xdp-wg.o`. 
    ```rust
    fs::create_dir_all(BPF_DIR).with_context(|| format!("create {}", BPF_DIR))?;
    fs::write(BPF_C_PATH, src).with_context(|| format!("write {}", BPF_C_PATH))?;
    run(
        "clang",
        [
            "-O2", "-g", "-target", "bpf", "-c", BPF_C_PATH, "-o", BPF_O_PATH,
        ],
    )
    ```
    Because this process runs as root, if `/etc/op-network/` or any of its subdirectories are writeable by non-root users (due to misconfiguration), an attacker can create a symbolic link at `op-xdp-wg.c` pointing to any critical system file (e.g., `/etc/shadow`), causing it to be overwritten with C source code. Additionally, concurrent executions of `op-xdp-wg` will collide on these static files, causing compilation failures or race conditions.
*   **Remediation**: Write the generated code to a randomized, root-owned temporary directory (e.g., using `tempfile::tempdir()`) with `0700` permissions, compile it there, and clean up the temporary compilation artifacts immediately.

### [MEDIUM] Command Execution Fallback to Untrusted Shell Path
*   **File**: `crates/op-network/src/rtnetlink.rs:424-429`
*   **Impact**: Arbitrary command execution if `PATH` is untrusted.
*   **Description**: In `add_default_route_onlink`, the application delegates to `iproute2` by calling `ip` via `std::process::Command` without using an absolute path:
    ```rust
    let status = Command::new("ip")
        .args([
            "route", "replace", "default", "via", gateway, "dev", ifname, "onlink",
        ])
    ```
    If this root-privileged process inherits an untrusted `PATH` environment variable, it may execute a malicious `ip` binary supplied by a local attacker.
*   **Remediation**: Always use absolute, canonical paths to system executables (e.g., `/sbin/ip` or `/usr/sbin/ip`).

### [LOW] Unprotected Proxmox Token File Permissions
*   **File**: `crates/op-network/src/proxmox.rs:197-200`
*   **Impact**: Credential exposure to local unprivileged users.
*   **Description**: The Proxmox client loads API tokens from `/etc/op-dbus/pve-token`. The code fails to verify or enforce secure file permissions (e.g., `0600` or `0400`) before reading this file. If the file has loose permissions, any unprivileged user on the system can read the token.
*   **Remediation**: Before reading the token file, check its permissions using `std::fs::metadata` on Unix and verify that it is owned by `root` and only readable by `root`. If permissions are insecure, refuse to start.