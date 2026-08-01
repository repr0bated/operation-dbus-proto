# Integration and Security Audit: `op-network` Crate

## 1. Workspace Integration Analysis

### Crates Depending on `op-network`
Based on the workspace `Cargo.toml` and the dependency mappings in `Cargo.lock`, the following internal crates depend on `op-network`:
*   **`op-dbus`** (Workspace Root / System Daemon)
*   **`op-dbus-mirror`** (D-Bus Event Mirroring)
*   **`op-grpc-bridge`** (gRPC to D-Bus / OVS Gateway)
*   **`op-plugins`** (LXC and Network Plugin Engine)
*   **`op-state`** (Transactional State Management)
*   **`op-tools`** (Control Plane CLI Utilities)
*   **`op-web`** (System Management Web UI)

---

### Registered D-Bus Service Names and Object Paths
The `op-network` crate itself is a low-level systems library and command-line utility provider; it **does not directly register any D-Bus service names or object paths** in its codebase. D-Bus exposure is managed entirely by the consumer crates (such as `op-dbus` and `op-dbus-mirror`) which import `op-network`.

---

### Exposed HTTP/gRPC Endpoints
The following entry points and endpoints are exposed or utilized by the binaries and modules within `op-network`:

#### Raw TCP Services
*   **OpenFlow 1.3 Passive Controller:** Exposed via `OpenFlowController` in `crates/op-network/src/controller.rs` and the binary `op-of-controller` in `crates/op-network/src/bin/op-of-controller.rs`.
    *   **Port/Address:** Configurable via the `OF_CONTROLLER_LISTEN` environment variable (defaults to `10.200.0.1:6653`).
    *   **Protocol:** Raw TCP socket speaking the OpenFlow 1.3 wire protocol (passive handshake, port discovery, and flow tables installation).

#### Outbound Client Integrations (External HTTP)
*   **Proxmox VE REST API Client:** Outbound HTTPS calls defined in `crates/op-network/src/proxmox.rs` targeting:
    *   `GET /api2/json/version`
    *   `GET /api2/json/nodes/{node}/lxc`
    *   `GET /api2/json/nodes/{node}/lxc/{vmid}/status/current`
    *   `GET /api2/json/nodes/{node}/lxc/{vmid}/config`
    *   `POST /api2/json/nodes/{node}/lxc` (LXC Container Creation)
    *   `POST /api2/json/nodes/{node}/lxc/{vmid}/status/start`
    *   `POST /api2/json/nodes/{node}/lxc/{vmid}/status/stop`
    *   `POST /api2/json/nodes/{node}/lxc/{vmid}/status/shutdown`
    *   `DELETE /api2/json/nodes/{node}/lxc/{vmid}` (Container Deletion)
    *   `GET /api2/json/nodes/{node}/tasks/{upid}/status`
    *   `POST /api2/json/nodes/{node}/lxc/{vmid}/clone`

---

### Cross-Crate Circular Dependency Risks
*   **Low/No Risk:** Under the current workspace structure, `op-network` has a strict unidirected dependency path. It depends solely on the foundational library `op-core` (specified via path dependency `op-core = { path = "../op-core" }` in `crates/op-network/Cargo.toml`). 
*   **Compilation Isolation:** The consuming crates (`op-plugins`, `op-state`, `op-web`, etc.) depend on `op-network`, but `op-network` does not import any of them. Thus, there is no threat of a compile-time dependency loop.

---

## 2. Schema-as-Code Compliance Audit

The codebase violates the *schema-as-code* discipline by defining critical interface contracts and configuration payloads as ad-hoc Rust structs parsing unstructured JSON/YAML instead of generating versioned models from unified Protocol Buffers or standard OSCAL definitions.

### Violations Detected:
1.  **Ad-Hoc Network Configuration and OVS Setup Contracts:**
    *   `crates/op-network/src/plugin.rs:18-126`: Structs `NetworkPlugin`, `OvsBridge`, `OpenFlowConfig`, `NetworkInterface`, and `OvsdbConfig` represent the core state schema used to persist control-plane state (parsed directly from a local `state.json`). These configuration files must be modeled via OSCAL Component Definitions to document security postures and configuration controls natively.
2.  **Unversioned Proxmox API Contracts:**
    *   `crates/op-network/src/proxmox.rs:60-236`: Structs `LxcContainer`, `CreateContainerRequest`, `ContainerStatus`, `TaskStatus`, and `ProxmoxVersion` are hand-crafted API mappings representing the integration boundaries with Proxmox VE. These should be auto-generated from a formal API schema (OpenAPI/Protobuf) to prevent drift and ensure schema enforcement.
3.  **Kernel Datapath Interface Structs:**
    *   `crates/op-network/src/ovs_netlink.rs:89-152`: Structs `Datapath`, `DatapathStats`, `Vport`, and `KernelFlow` are declared as ad-hoc serialized structures, bypassing formal schema definitions for physical netlink boundaries.

---

## 3. Production Security & Quality Findings

### CRITICAL: Disabled SSL/TLS Certificate Verification in Proxmox Client
*   **Location:** `crates/op-network/src/proxmox.rs:245-247`
*   **Impact:** Active Man-in-the-Middle (MitM) token/credential theft and command hijacking.
*   **Description:** The HTTP client builder explicitly disables certificate validation:
    ```rust
    let client = Client::builder()
        .danger_accept_invalid_certs(true)
    ```
    This allows an attacker situated on the local network or capable of hijacking DNS queries to intercept HTTP traffic to the Proxmox VE hypervisor. This completely exposes `PVE_API_TOKEN_SECRET` values (which are configured as root-equivalent secrets) to interception, leading to hypervisor compromise.
*   **Remediation:** Remove `.danger_accept_invalid_certs(true)` from production builds. If self-signed certificates must be supported, implement certificate fingerprint pinning or allow operators to supply a custom CA root bundle.

---

### CRITICAL: Infinite Busy Loop / Resource Exhaustion on Netlink EOF
*   **Location:** `crates/op-network/src/ovs_netlink.rs:480-529` (in `send_and_recv_raw`) and `crates/op-network/src/ovs_netlink.rs:596-630` (in `send_ovs_msg`)
*   **Impact:** Denial of Service (DoS) due to 100% CPU exhaustion.
*   **Description:** The socket read loop does not handle the case where the socket returns `Ok(0)` (which represents EOF/socket disconnected). 
    ```rust
    loop {
        match self.socket.recv(&mut recv_buf, 0) {
            Ok(n) => {
                let mut offset = 0;
                while offset < n {
                    // ... parsing logic
                    offset += msg_len;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                break;
            }
            Err(e) => return Err(e.into()),
        }
    }
    ```
    If `self.socket.recv` returns `Ok(0)` (socket closed on the kernel side), the inner `while offset < n` loop (where `n` is `0`) is skipped entirely, but the outer `loop` remains active. The program will continuously call `recv`, get `Ok(0)` instantly, and spin indefinitely consuming 100% of a CPU core.
*   **Remediation:** Explicitly check for `n == 0` on `Ok(n)` and return an error or break the loop:
    ```rust
    Ok(0) => return Err(anyhow!("Netlink socket closed unexpectedly")),
    ```

---

### HIGH: Unbounded Memory Allocation in OpenFlow Controller
*   **Location:** `crates/op-network/src/controller.rs:98-117`
*   **Impact:** Memory exhaustion (OOM Panic) and thread starvation.
*   **Description:** In `recv_msg`, the message length is read directly from the untrusted TCP header:
    ```rust
    let length = u16::from_be_bytes([hdr[2], hdr[3]]) as usize;
    let payload_len = length.saturating_sub(8);
    let mut payload = vec![0u8; payload_len];
    ```
    While `u16` limits the maximum allocation to ~64KB per packet, any network client can establish a connection and send a header indicating a large size, but never send the remaining bytes. Since the `read_exact` call blocks asynchronously without a timeout, an attacker can open numerous idle connections and hold allocated buffers indefinitely, leading to resource exhaustion.
*   **Remediation:** Enforce a strict connection-level timeout for packet parsing and introduce a maximum allowable payload size limit (e.g. 10KB) well below `u16::MAX`.

---

### HIGH: Fragile Runtime Compilation & Arbitrary Code Loading
*   **Location:** `crates/op-network/src/bin/op-xdp-wg.rs:353-388`
*   **Impact:** Local privilege escalation and target host fragility.
*   **Description:** The utility generates raw BPF C code as a string, writes it to `/etc/op-network/xdp/op-xdp-wg.c`, and invokes the host's `clang` compiler at runtime:
    ```rust
    fs::write(BPF_C_PATH, src).with_context(|| format!("write {}", BPF_C_PATH))?;
    run(
        "clang",
        [
            "-O2", "-g", "-target", "bpf", "-c", BPF_C_PATH, "-o", BPF_O_PATH,
        ],
    )
    ```
    If an attacker can modify files under `/etc/op-network/xdp` (or if permissions are misconfigured), they can inject arbitrary BPF code which is subsequently loaded directly into the kernel with root privileges. Furthermore, runtime compilation introduces a heavy dependency on the local availability of `clang` and kernel headers, violating production-ready minimal image practices.
*   **Remediation:** Distribute pre-compiled BPF object files (using CO-RE, Compile Once - Run Everywhere) embedded into the Rust binary via `include_bytes!`, or use a native library such as `aya` to avoid shell-outs.

---

### MEDIUM: Process Panic on Malformed Database State
*   **Location:** `crates/op-network/src/ovsdb.rs:92-98`
*   **Impact:** Application panic and service interruption.
*   **Description:** The helper function `uuid_ref` performs an unwrap on user-supplied or database-returned UUID strings:
    ```rust
    fn uuid_ref(uuid: &str) -> Value {
        let parsed: Uuid = uuid
            .parse()
            .unwrap_or_else(|e| panic!("uuid_ref: invalid UUID {:?}: {}", uuid, e));
        RowRef::Uuid(parsed).to_json()
    }
    ```
    If OVSDB returns a corrupted or truncated UUID, or if a malformed parameter is received, invoking this utility will panic the entire service thread.
*   **Remediation:** Refactor `uuid_ref` to return a `Result<Value, uuid::Error>` instead of panicking.