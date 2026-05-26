# Security and Quality Audit: `op-network`

## 1. Tests Report

### Test Count Summary
* **Total Test Functions:** 21
* **Unit Tests (within modules):** 21
* **Integration Tests (in `tests/` directory):** 0 (No separate `tests/` directory files were provided in the analyzed codebase)

### Representative Tests
1. **`test_error_suggestions`** — `crates/op-network/src/ovs_error.rs:190`
   * *Description:* Validates that suggestions and category checks (`needs_root`, `needs_ovs`) for OvsError variants are accurately returned.
2. **`test_vport_type_conversion`** — `crates/op-network/src/ovs_netlink.rs:787`
   * *Description:* Checks the conversion logic from OVS netlink raw port types (`u32`) to the typed Rust representation `VportType`.
3. **`test_capabilities_detect`** — `crates/op-network/src/ovs_capabilities.rs:212`
   * *Description:* Tests runtime OVS capability detection path with and without OVS daemon availability, ensuring the cache path works.

### Property and Fuzz Testing
* **Property-Based Testing (proptest, quickcheck):** None found.
* **Fuzzing Harnesses:** None found.

---

## 2. Schema-as-Code Violations

The codebase utilizes ad-hoc serialization contracts rather than formal, versioned schemas (such as Protocol Buffers or OSCAL component definitions) to define configuration parameters and external API interfaces:

* **Ad-hoc Configuration Structures** — `crates/op-network/src/plugin.rs:18-142`
  * Structures such as `NetworkPlugin`, `OvsBridge`, `OpenFlowConfig`, `NetworkInterface`, and `OvsdbConfig` represent configuration models constructed strictly as ad-hoc, unversioned JSON structures. Modification of fields will break compatibility silently across different control-plane versions without a schema evolutionary path.
* **Proxmox Virtualization API Payloads** — `crates/op-network/src/proxmox.rs:87, 137`
  * API requests and responses like `CreateContainerRequest` and `ContainerStatus` use typed but unversioned structures with serialized flat fields, along with generic JSON hashmap catching (`#[serde(flatten)] pub extra: HashMap<String, Value>`). Any change to the upstream API response forces recompilation and breaks backward compatibility without migration boundaries.
* **Network Status Models** — `crates/op-network/src/rtnetlink.rs:11, 25`
  * `NetworkInterface` and `InterfaceAddress` are custom Rust structures mapping netlink details into serialized JSON representation. These interfaces lack translation boundaries to decoupled schemas.

---

## 3. Security & Quality Findings

### [CRITICAL] Man-in-the-Middle (MitM) via Disabled TLS Certificate Verification
* **Location:** `crates/op-network/src/proxmox.rs:211`
* **Impact:** High probability of credential theft (`PVEAPIToken` header leakage) and unauthorized LXC virtualization management if network path is compromised.
* **Description:** 
  The Proxmox API client initializes a raw `reqwest` HTTP client with certificate validation explicitly turned off:
  ```rust
  let client = Client::builder()
      .danger_accept_invalid_certs(true)
      .timeout(Duration::from_secs(30))
      ...
  ```
  While done to support self-signed certificates by default on Proxmox instances, this allows an attacker capable of local network redirection (such as ARP spoofing or DNS poisoning) to intercept all traffic, masquerade as the target Proxmox host, and capture sensitive `PVE_API_TOKEN_SECRET` values or issue arbitrary container commands.
* **Remediation:** 
  Provide a configuration option to supply a trusted CA bundle (such as the Proxmox self-signed certificate public key) and load it into the `reqwest` client, keeping certificate verification enabled (`danger_accept_invalid_certs(false)`).

---

### [MEDIUM] Local Privilege Escalation via Environment Variable Override
* **Location:** `crates/op-network/src/bin/op-ovsbr0-afxdp.rs:188`
* **Impact:** Elevation of privilege if a local user can invoke the setuid helper or trigger wrapper binary execution under preserved root context.
* **Description:** 
  The binary executes an external helper defined by the `OP_XDP_WG` environment variable:
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
  Because this control plane program requires running with highly elevated system privileges (`CAP_NET_ADMIN` or raw `root` socket control), trusting the parent environment's variable inputs allows an attacker to control the command path used for execution.
* **Remediation:** 
  Hardcode the expected path of `op-xdp-wg` (e.g., strictly `/usr/local/sbin/op-xdp-wg` or `/usr/sbin/op-xdp-wg`) instead of permitting runtime resolution via `std::env::var`.

---

### [MEDIUM] Potential Argument Injection in `dhclient` Invocation
* **Location:** `crates/op-network/src/plugin.rs:410`
* **Impact:** Manipulation of system command-line flag execution, allowing potential arbitrary file reads or interface corruption.
* **Description:** 
  The network plugin invokes the system's `dhclient` wrapper by passing an unvalidated interface string directly:
  ```rust
  let output = tokio::process::Command::new("dhclient")
      .arg("-v")
      .arg(interface)
      .output()
      .await?;
  ```
  If `interface` is parsed from an untrusted JSON state payload and starts with a hyphen (e.g., `-r` or `-pf`), it is evaluated as an argument to `dhclient` instead of an interface identifier.
* **Remediation:** 
  Validate that the `interface` string conforms strictly to POSIX network interface naming standards (typically alphabetic prefix followed by integer, strictly alphanumeric, and never starting with a hyphen) before invoking the process. Use the double-hyphen (`--`) parameter separator if supported by the platform's `dhclient` implementation.