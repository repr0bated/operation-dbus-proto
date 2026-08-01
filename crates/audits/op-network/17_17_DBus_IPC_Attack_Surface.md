# Security and Quality Audit: `op-network` Crate

## 1. D-Bus & IPC Attack Surface Audit

The provided source files for the `op-network` crate do not register any D-Bus interfaces, methods, or signals directly. No `zbus` macro attributes (`#[dbus_interface]`, `#[dbus_proxy]`) or D-Bus registration loops are defined in these files. 

However, the crate interacts heavily with other system control-plane components, utilizing environment variables, child process spawning, Netlink sockets, and network JSON-RPC connections as its primary IPC and attack surfaces.

### Spawning Processes & Caller Identity Verification
The crate spawns highly privileged child processes (frequently requiring `root` or `CAP_NET_ADMIN` privileges) across multiple utilities. These process-spawning interfaces lack caller-identity validation, trusting system-level privilege boundaries or environment variable parameters implicitly:

*   **`add_default_route_onlink`** (`crates/op-network/src/rtnetlink.rs:434-442`): Spawns the `ip` binary with `onlink` flag parameters. While the arguments are passed as discrete parameters to `execve` (preventing raw shell command injection), they are not verified against malicious input at this layer.
*   **`enable_dhcp`** (`crates/op-network/src/plugin.rs:538-543`): Spawns `dhclient` dynamically using the administrative interface name.
*   **`compile_bpf`** (`crates/op-network/src/bin/op-xdp-wg.rs:313-322`): Spawns `clang` to compile a dynamically written BPF source file `/etc/op-network/xdp/op-xdp-wg.c`.
*   **`configure_tc`** (`crates/op-network/src/bin/op-xdp-wg.rs:323-347`): Spawns `tc` to manipulate ingress qdiscs and filters.
*   **`stop_vswitchd` / `start_vswitchd`** (`crates/op-network/src/bin/op-ovsbr0-setup.rs:101-140` and `line 161`): Spawns `s6-svc` and `s6-svstat` using service name identifiers derived directly from environment variables.

### Connection Bus
The crate's configuration files do not establish D-Bus connections. However, the root workspace is configured with the `zbus = "4.0"` dependency, indicating that sibling crates (such as `op-dbus` or `op-identity`) likely attach to the **System Bus** to manage core system state. Plaintext system files (such as `/etc/op-dbus/pve-token`) further establish that these services run within the system security context.

### Deserialization of Caller-Supplied Bytes
*   **OVSDB SIMD JSON Parsing** (`crates/op-network/src/ovsdb.rs:352-358`): The `transact_simd` method deserializes an `OwnedValue` to a JSON string and re-deserializes it into a standard `serde_json::Value` before submitting it as a transaction to the OVSDB server. No verification or structural validation of the query sequence is conducted at this boundary.
*   **OpenFlow Wire Processing** (`crates/op-network/src/controller.rs:104-124`): The controller reads OpenFlow headers directly from unauthenticated TCP streams. It processes the length field `u16::from_be_bytes` and allocates memory dynamically for the payload via `vec![0u8; payload_len]`. While bounded to `65535` bytes by the `u16` type limits, there are no checks ensuring the length is proportional to the TCP stream buffer availability, exposing the service to slow-loris or resource exhaustion vectors.

---

## 2. Critical Security Findings

### CRITICAL: Arbitrary Command Execution and Privilege Escalation via `OP_XDP_WG` Environment Variable
*   **Citation**: `crates/op-network/src/bin/op-ovsbr0-afxdp.rs:142-155`
*   **Impact**: Directly Exploitable (Remote/Local Privilege Escalation to Root)

**Vulnerability Analysis**:
The utility function `rechain_xdp_steer` retrieves the path of the `op-xdp-wg` helper binary using the `OP_XDP_WG` environment variable. If the variable is present, its value is immediately used as the program target for `Command::new`:

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

Because `op-ovsbr0-afxdp` must run as `root` (or with `CAP_NET_ADMIN`) to bind netlink sockets and communicate with the kernel's OpenvSwitch module, it executes child processes in an elevated context. An attacker who has local shell access or can manipulate the environment variables of a daemon/D-Bus service invoking this tool can point `OP_XDP_WG` to an arbitrary malicious script. The script will be executed with full root privileges.

**Remediation**:
Hardcode all binary target paths to verified administrative directories (e.g., `/usr/local/sbin/op-xdp-wg`), or sanitize the environment before executing command dispatchers.

---

### CRITICAL: Plaintext, Unauthenticated OpenFlow TCP Listeners and Clients
*   **Citation**: `crates/op-network/src/controller.rs:360-394` and `crates/op-network/src/openflow.rs:118-125`
*   **Impact**: Directly Exploitable (Network/Switch Traffic Hijacking)

**Vulnerability Analysis**:
The passive OpenFlow controller binds a standard TCP socket to listen for connections from OpenvSwitch (OVS):

```rust
pub async fn run(self) -> Result<()> {
    let listener = TcpListener::bind(self.listen_addr)
        .await
        .with_context(|| format!("binding OpenFlow controller on {}", self.listen_addr))?;
    ...
    loop {
        let (stream, peer) = listener.accept().await?;
        ...
        tokio::spawn(async move {
            ...
            match handle_connection(stream, flows).await {
```

Similarly, the active client interface (`openflow.rs`) initiates a raw TCP connection:

```rust
pub async fn connect(addr: SocketAddr) -> Result<Self> {
    let rovs_addr = rovs_transport::Address::Tcp {
        host: addr.ip().to_string(),
        port: addr.port(),
    };
    ...
    match rovs_openflow::VConn::connect(&rovs_addr).await {
```

No TLS, mutual authentication, IP-based whitelisting, or cryptographic handshake is utilized. On successful connection, the controller immediately deletes all active flows on the switch and injects new ones (`crates/op-network/src/controller.rs:217`):

```rust
// 5. Delete all existing flows.
send_msg(&mut stream, &build_flow_mod_delete_all(xid)).await?;
```

Any attacker on the local network segment can spoof the OVS switch or the OpenFlow controller, connect to the unauthenticated port, purge the active packet routing state, and inject arbitrary forwarding rules to redirect network traffic.

**Remediation**:
Adopt mutual TLS (mTLS) for both active and passive connections. Leverage `rovs_transport::Address::Ssl` (or equivalent secure protocols) to authenticate the switch and the controller before transmitting `FlowMod` instructions.

---

## 3. High & Medium Severity Findings

### HIGH: Privilege Escalation and Service Tampering via Environment-Controlled s6 Service Path
*   **Citation**: `crates/op-network/src/bin/op-ovsbr0-setup.rs:55-56`, `crates/op-network/src/bin/op-ovsbr0-setup.rs:103`, `crates/op-network/src/bin/op-ovsbr0-setup.rs:111`, `crates/op-network/src/bin/op-ovsbr0-setup.rs:117`, and `crates/op-network/src/bin/op-ovsbr0-setup.rs:161`
*   **Impact**: High (Unauthorized Control of System Services)

**Vulnerability Analysis**:
The configuration setup utility retrieves the `VSWITCHD_SVC` environment variable directly without validation:

```rust
vswitchd_svc: std::env::var("VSWITCHD_SVC")
    .unwrap_or_else(|_| "/run/service/ovs-vswitchd".into()),
```

This unvalidated value is then used in administrative child process calls to `s6-svc` and `s6-svstat` under an elevated security context:

```rust
let _ = Command::new("s6-svc").args(["-d", svc]).status();
...
let _ = Command::new("s6-svc").args(["-t", svc]).status();
...
let out = Command::new("s6-svstat").arg(svc).output();
...
let _ = Command::new("s6-svc").args(["-u", svc]).status();
```

An attacker capable of setting environment variables can alter `VSWITCHD_SVC` to reference other critical system services managed by `s6`. When the `op-ovsbr0-setup` tool is executed, it will stop or start arbitrary services, causing system instability or tearing down security defenses.

**Remediation**:
Validate that the `VSWITCHD_SVC` path is restricted to a specific allowed directory prefix (e.g., `/run/service/`) and does not contain directory traversal elements (`../`), or enforce strict string-matching against allowed service names.

---

### HIGH: Proxmox API TLS Certificate Verification Bypass
*   **Citation**: `crates/op-network/src/proxmox.rs:218-225`
*   **Impact**: High (Man-in-the-Middle Attack on Virtualization Control Plane)

**Vulnerability Analysis**:
The native Proxmox API client disables TLS certificate verification globally by default:

```rust
pub fn with_config(base_url: &str, node: &str, token: Option<ProxmoxToken>) -> Self {
    // Create client that accepts self-signed certificates (Proxmox default)
    let client = Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .build()
        .expect("Failed to create HTTP client");
```

While Proxmox often ships with self-signed certificates, globally forcing `danger_accept_invalid_certs(true)` eliminates the security guarantees of HTTPS. Any attacker on the physical network or administrative VLAN can perform a Man-in-the-Middle (MitM) attack, intercept Proxmox API tokens, or inject malicious API responses (such as falsified container creation instructions).

**Remediation**:
Add support for pinning the Proxmox TLS certificate SHA-256 fingerprint, or allow loading custom CA bundles rather than disabling certificate verification completely.

---

### MEDIUM: Plaintext API Secret Exposure and Missing File Permissions Check
*   **Citation**: `crates/op-network/src/proxmox.rs:240-279`
*   **Impact**: Medium (Credential Exposure)

**Vulnerability Analysis**:
The client loads the Proxmox API token from `/etc/op-dbus/pve-token` or a user-defined environment path:

```rust
let token_file = std::env::var("PVE_TOKEN_FILE")
    .unwrap_or_else(|_| "/etc/op-dbus/pve-token".to_string());

// Try to read token from file
let (token, node) = if let Ok(content) = std::fs::read_to_string(&token_file) {
```

The file containing high-privilege virtualization keys (`PVE_API_TOKEN_SECRET`) is read in plaintext. The loading routine does not verify that the file possesses restrictive access permissions (such as `0600` or owner validation). If the file is misconfigured with broad read permissions, any local low-privileged user or compromised service can read the secrets.

**Remediation**:
Verify the file permissions and ownership of the target path using `std::os::unix::fs::MetadataExt` before reading the contents. Block execution if the file is world-readable or group-readable.

---

## 4. Schema-as-Code Compliance Review

The codebase fails to follow the "Schema-as-Code" discipline for defining its internal data structures, data contracts, and environment integrations. Instead of representing configuration boundaries and IPC envelopes as strongly versioned schemas (such as Protocol Buffers or structured OSCAL schemas), the crate expresses data structures as ad-hoc Rust structs adorned with deserialization attributes:

### Ad-hoc Serialization Configurations
*   **`NetworkPlugin` & `OvsBridge`** (`crates/op-network/src/plugin.rs:17-60`): Configures system bridge setups, internal interfaces, and routing parameters. It relies on ad-hoc structs and custom default methods:
    ```rust
    #[derive(Debug, Clone, Serialize, Deserialize, Default)]
    pub struct NetworkPlugin {
        pub bridges: Vec<OvsBridge>,
        pub interfaces: Vec<NetworkInterface>,
        pub ovsdb: OvsdbConfig,
    }
    ```
*   **`CreateContainerRequest` & `ContainerStatus`** (`crates/op-network/src/proxmox.rs:80-180`): Defines serialization formats for container deployment and state queries. There are no versioned API boundaries, creating a fragility point if the underlying virtualization engine's API structure undergoes modifications.
*   **`NetworkInterface` & `InterfaceAddress`** (`crates/op-network/src/rtnetlink.rs:13-32`): Expresses raw local interface structures using ad-hoc vectors and primitive strings.
*   **`OvsCapabilities`** (`crates/op-network/src/ovs_capabilities.rs:35-64`): Manages capabilities through a series of ad-hoc boolean fields without structural verification metadata.

### Recommendations for Schema Enforcement
1.  **Migrate Configuration State to Protocol Buffers**: Compile core structures (like `NetworkPlugin` or `OvsBridge`) into Rust code from versioned Protobuf `.proto` schemas. This ensures that any adjustments to fields are explicitly backward-compatible.
2.  **OSCAL Alignment**: For system security configuration state (such as OpenFlow policies, bridge attributes, and BPF/XDP steering configurations), leverage OSCAL Component Definition schemas. Programmatically validate these documents at runtime using `jsonschema` rather than relying on raw, unstructured JSON-to-struct parsing.

---
## ⚠ Citation Warnings
- `crates/op-network/src/plugin.rs:538`: file has 498 lines
