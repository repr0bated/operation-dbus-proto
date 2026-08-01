# Technical Security & Quality Audit: op-network Crate

---

### 1. Build and Metadata Audit

* **Edition:** Inherited from the workspace (`2021` edition).
* **Rust-Version:** No `rust-version` constraint is defined in either `crates/op-network/Cargo.toml` or the workspace root `Cargo.toml`.
* **Workspace Inheritance:** Correctly configured; metadata, licensing, and common dependencies (such as `tokio`, `serde`, `anyhow`, `thiserror`, `tracing`, `rtnetlink`, and `simd-json`) are cleanly inherited from the workspace root.
* **Build Script Audit:** There is no `build.rs` present in `crates/op-network/` or provided in the audited files. Consequently, there are no code-generation or shell-execution risks during compilation.

#### Schema-As-Code Build Check
* **Protocol Compiler Invocation:** No invocation of `prost-build` or `tonic-build` occurs within the audited `op-network` files (though they are declared in workspace dependencies for other crates).
* **Schema Source of Truth:** No `.proto` schema files or OSCAL models are present in the `op-network` crate.
* **Ad-Hoc Contracts (Violation of Schema-As-Code Discipline):** 
  Multiple data contracts are declared via ad-hoc Rust structs with Serde serialization/deserialization annotations rather than strongly-typed, versioned Protocol Buffer schemas or formal OSCAL schemas:
  * **Network Configuration & OVS/OVSDB Persistence:** Declared as ad-hoc Rust structures (`NetworkPlugin`, `OvsBridge`, `OpenFlowConfig`, `NetworkInterface`, `OvsdbConfig`) in `crates/op-network/src/plugin.rs:15-115`.
  * **Proxmox VE API Integration:** Declared as ad-hoc Rust structures (`CreateContainerRequest`, `LxcContainer`, `ContainerStatus`, `TaskStatus`, `ProxmoxVersion`) in `crates/op-network/src/proxmox.rs:40-180`.
  * **OVS Netlink Kernel Interface:** Declared as ad-hoc structures (`Datapath`, `DatapathStats`, `Vport`, `KernelFlow`, `FlowStats`) in `crates/op-network/src/ovs_netlink.rs:114-165`.

---

### 2. Critical Security Findings

#### [CRITICAL] Finding 1: Proxmox API Client Disables TLS Certificate Verification
* **File & Lines:** `crates/op-network/src/proxmox.rs:215-225`
* **Vulnerability Type:** Insecure TLS Configuration (CWE-295)
* **Description:** The `ProxmoxClient` disables TLS certificate verification globally for all requests made to the Proxmox VE API:
  ```rust
  let client = Client::builder()
      .danger_accept_invalid_certs(true)
      .timeout(Duration::from_secs(30))
      .connect_timeout(Duration::from_secs(10))
      .build()
      .expect("Failed to create HTTP client");
  ```
* **Exploitability & Impact:** The `ProxmoxClient` handles highly sensitive administrative tasks, including creating, starting, stopping, and deleting LXC containers. It uses the `PVE_API_TOKEN_SECRET` credential to authenticate. If the client communicates with a Proxmox VE hypervisor over a network path (which is the standard architecture for multi-node clusters), any attacker capable of intercepting network traffic (via ARP spoofing, DNS hijacking, or BGP hijacking) can present a self-signed certificate, establish a Man-in-the-Middle (MitM) session, capture the plaintext API token, and hijack the entire virtualization infrastructure.
* **Remediation:** Remove `.danger_accept_invalid_certs(true)`. Instead, load the specific self-signed CA certificate of the Proxmox VE host into the client builder's trust store using `.add_root_certificate()`.

---

#### [CRITICAL] Finding 2: Unencrypted & Unauthenticated OpenFlow Passive Controller
* **File & Lines:** `crates/op-network/src/controller.rs:355-390`, `crates/op-network/src/bin/op-of-controller.rs:24-41`
* **Vulnerability Type:** Lack of Encryption / Lack of Authentication (CWE-306 / CWE-311)
* **Description:** The `OpenFlowController` is a passive TCP listener designed to bind to a local socket (by default, `10.200.0.1:6653`) and handle incoming OpenFlow 1.3 connections from OVS switches. The implementation establishes a raw `TcpListener` and immediately spawns a connection handler without executing any TLS handshake, cryptographic validation, or authentication:
  ```rust
  let listener = TcpListener::bind(self.listen_addr)
      .await
      .with_context(|| format!("binding OpenFlow controller on {}", self.listen_addr))?;
  ```
* **Exploitability & Impact:** Anyone on the network (or any compromised container on the host sharing the network segment) can connect to the controller. Once a connection is established, the controller automatically executes an unauthenticated handshake, empties the switch's flow table via a wildcard delete (`build_flow_mod_delete_all`), and installs its own hardcoded flow pairs. An attacker can repeatedly connect to the controller to trigger constant table-flushing, leading to a persistent Denial of Service (DoS) or unauthorized network flow redirection.
* **Remediation:** Enforce TLS on the incoming connection. Use a library such as `tokio-rustls` to wrap the `TcpStream` before performing any OpenFlow handshake, and mandate client-certificate verification (mTLS) to authenticate connecting switches.

---

#### [HIGH] Finding 3: Local Privilege Escalation via Environment Override in AF_XDP Orchestration
* **File & Lines:** `crates/op-network/src/bin/op-ovsbr0-afxdp.rs:141-155`
* **Vulnerability Type:** Privilege Escalation via Environment Variable Hijacking (CWE-426 / CWE-250)
* **Description:** The `op-ovsbr0-afxdp` utility is intended to run as a high-privilege program (`root` or with `CAP_NET_ADMIN`) to perform network link migrations and program kernel interfaces. However, the command-line executor utilizes an unvalidated environment variable `OP_XDP_WG` to find and execute the `op-xdp-wg` binary:
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
      ...
  ```
* **Exploitability & Impact:** If a less-privileged user can run this utility (via a specific `sudo` configuration or through a system daemon that inherits or preserves the caller's environment), they can set `OP_XDP_WG=/path/to/malicious/binary` and execute arbitrary binaries as `root` or with elevated network capabilities.
* **Remediation:** Remove the `OP_XDP_WG` environment variable lookup. Use an absolute, immutable file path (such as `/usr/local/sbin/op-xdp-wg`) to locate the steering helper.

---

### 3. Medium & Quality Findings

#### [MEDIUM] Finding 4: Denial of Service via Port Discovery Loop Interruption
* **File & Lines:** `crates/op-network/src/controller.rs:198-250`
* **Vulnerability Type:** Improper Control Flow / Loop Termination (CWE-248)
* **Description:** The `discover_ports` loop reads raw OpenFlow messages sequentially and terminates on any message type that is not an EchoRequest (2) or a MultipartReply (19):
  ```rust
  loop {
      let msg = recv_msg(stream).await?;
      match msg.msg_type {
          2 /* EchoRequest */ => { ... }
          19 /* MultipartReply */ => { ... }
          _ => break, // Premature exit
      }
  }
  ```
* **Exploitability & Impact:** During the initial handshaking and port discovery process, an active OVS switch may naturally send asynchronous messages (such as `PortStatus` updates, packet-in notices, or unexpected error logs). Since any message type besides 2 or 19 causes the parser loop to break immediately, the discovery routine will exit prematurely. This results in an incomplete `port_map`, which prevents configured flow pairs from matching and blackholes network traffic.
* **Remediation:** Change the catch-all pattern from `_ => break` to `_ => {}` (or log the ignored frame) so that the loop continues to collect all chunks of the port description multi-part response until the `OFPMPF_REPLY_MORE` flag is cleared.

---

#### [MEDIUM] Finding 5: Environment-Controlled Path Traversal in Bridge Diagnostics
* **File & Lines:** `crates/op-network/src/bin/op-ovsbr0-setup.rs:114-118`, `126-130`
* **Vulnerability Type:** Path Traversal (CWE-22)
* **Description:** The `BRIDGE` environment variable is read to construct a `sysfs` path to check if a network interface is still present in the kernel:
  ```rust
  let sysfs_path = format!("/sys/class/net/{}", bridge);
  if !Path::new(&sysfs_path).exists() { ... }
  ```
* **Exploitability & Impact:** Since `BRIDGE` can be set to any arbitrary string, a path traversal sequence (e.g. `../../etc/shadow`) will resolve to an existence check on a target file (e.g. `/sys/class/net/../../etc/shadow` -> `/etc/shadow`). While this check is safe from direct arbitrary file read (since the path is only queried via `.exists()`), it leaks file existence information to logs and breaks clean execution assumptions in a security-sensitive binary.
* **Remediation:** Validate that the `bridge` variable contains only alphanumeric characters and safe punctuation (such as hyphens or underscores) before constructing the filesystem path.

---

#### [LOW] Finding 6: Local Token File Location Override
* **File & Lines:** `crates/op-network/src/proxmox.rs:241-243`
* **Vulnerability Type:** Path Traversal / Arbitrary File Read (CWE-22)
* **Description:** The `PVE_TOKEN_FILE` environment variable is read to locate the Proxmox token file without validating the path safety:
  ```rust
  let token_file = std::env::var("PVE_TOKEN_FILE")
      .unwrap_or_else(|_| "/etc/op-dbus/pve-token".to_string());
  ```
* **Exploitability & Impact:** If the binary runs in a privileged context, a local attacker with the ability to trigger the client and control environment variables could feed a sensitive file path to `PVE_TOKEN_FILE`. If parsing errors or logs dump the parsed content back to stdout/stderr, this could leak internal file systems.
* **Remediation:** Ensure that the input path is validated to prevent directory traversal outside of expected system configuration directories.