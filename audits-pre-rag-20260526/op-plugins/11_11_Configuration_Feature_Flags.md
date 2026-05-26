### List of `std::env::var` Reads

- **`crates/op-plugins/src/default_registry.rs:33`**: Reads `OP_DBUS_WG_ONLY`.
- **`crates/op-plugins/src/state_plugins/privacy_router.rs:79`**: Reads `PRIVACY_BRIDGE_NAME`.
- **`crates/op-plugins/src/state_plugins/privacy_router.rs:81`**: Reads `PRIVACY_UPLINK_PORT`.
- **`crates/op-plugins/src/state_plugins/privacy_router.rs:83`**: Indirectly reads `PRIVACY_ATTACH_UPLINK_TO_BRIDGE` via the `bool_env` helper.
- **`crates/op-plugins/src/state_plugins/privacy_router.rs:84`**: Reads `PRIVACY_MGMT_PORT`.
- **`crates/op-plugins/src/state_plugins/privacy_router.rs:86`**: Reads `PRIVACY_SOCKET_PORT`.
- **`crates/op-plugins/src/state_plugins/privacy_router.rs:88`**: Reads `PRIVACY_GRPC_BRIDGE_PORT`.
- **`crates/op-plugins/src/state_plugins/privacy_router.rs:90`**: Reads `PRIVACY_MGMT_CIDR`.
- **`crates/op-plugins/src/state_plugins/privacy_router.rs:92`**: Reads `PRIVACY_OPENFLOW_CONTROLLER`.
- **`crates/op-plugins/src/state_plugins/privacy_router.rs:94`**: Reads `PRIVACY_DATAPATH_TYPE`.
- **`crates/op-plugins/src/state_plugins/privacy_router.rs:96`**: Reads `PRIVACY_FAIL_MODE`.
- **`crates/op-plugins/src/state_plugins/privacy_router.rs:442`**: Reads `PRIVACY_SYSTEM_STORAGE_POOL`.
- **`crates/op-plugins/src/state_plugins/privacy_router.rs:443`**: Reads `INCUS_STORAGE_POOL`.

---

### Environment Variables Verification

All of the `std::env::var` calls identified above utilize safe fallbacks:
- `OP_DBUS_WG_ONLY` has a fallback of `false` via `.unwrap_or(false)`.
- All `PRIVACY_*` variables in `PrivacyHostBootstrapConfig::from_env` use `.unwrap_or_else` or `bool_env` with predefined fallback constants (e.g., `DEFAULT_UPLINK_PORT`, `DEFAULT_MGMT_PORT`).
- There are no cases of unhandled `.unwrap()` on env var retrievals in the provided files.

---

### Cargo Features Analysis

- **`op-plugins` features**: No crate-specific features are declared in `crates/op-plugins/Cargo.toml`.
- **Workspace-level features** (from root `Cargo.toml`):
  - `default = ["grpc"]`
  - `grpc = []`
- **Additive Behavior**: Yes, Cargo features are inherently additive.

---

### Hardcoded Paths, Ports, and Addresses

#### Hardcoded Paths
- **`crates/op-plugins/src/dynamic_loading.rs:55`**: `/var/lib/op-dbus/plugins/dynamic_loading`
- **`crates/op-plugins/src/dynamic_loading.rs:60`**: `/var/lib/op-dbus/plugins/dynamic_loading`
- **`crates/op-plugins/src/plugin.rs:31`**: `/var/lib/op-dbus/plugins/default`
- **`crates/op-plugins/src/registry.rs:245`**: `/var/lib/op-dbus/plugins`
- **`crates/op-plugins/src/default_registry.rs:101`**: `/etc/op-dbus/mcp-config.json`
- **`crates/op-plugins/src/default_registry.rs:105`**: `/etc/op-dbus/config-store.json`
- **`crates/op-plugins/src/default_registry.rs:113`**: `/etc/op-dbus/privacy-config.json`
- **`crates/op-plugins/src/state_plugins/config.rs:14`**: `/etc/op-dbus/config-store.json`
- **`crates/op-plugins/src/state_plugins/dnsresolver.rs:98`**: `/etc/resolv.conf`
- **`crates/op-plugins/src/state_plugins/dnsresolver.rs:103`**: `/etc/resolv.conf`
- **`crates/op-plugins/src/state_plugins/dnsresolver.rs:128`**: `/etc/resolv.conf.sysdecl.tmp`
- **`crates/op-plugins/src/state_plugins/hardware.rs:43`**: `/proc/cpuinfo`
- **`crates/op-plugins/src/state_plugins/hardware.rs:68`**: `/proc/meminfo`
- **`crates/op-plugins/src/state_plugins/incus.rs:67`**: `/usr/bin/incus`
- **`crates/op-plugins/src/state_plugins/incus.rs:384`**: `/usr/bin/incus`
- **`crates/op-plugins/src/state_plugins/lxc.rs:81`**: `/sys/fs/cgroup/system.slice/pve-container@{}.service`
- **`crates/op-plugins/src/state_plugins/lxc.rs:344`**: `/var/lib/pve/{}`
- **`crates/op-plugins/src/state_plugins/lxc.rs:354`**: `/etc/pve`
- **`crates/op-plugins/src/state_plugins/lxc.rs:407`**: `/etc/pve/lxc/{}.conf`
- **`crates/op-plugins/src/state_plugins/lxc.rs:482`**: `/etc/op-dbus/netmaker.env`
- **`crates/op-plugins/src/state_plugins/lxc.rs:741`**: `/etc/pve`
- **`crates/op-plugins/src/state_plugins/ovsdb_bridge.rs:196`**: `/var/run/openvswitch/db.sock`
- **`crates/op-plugins/src/state_plugins/pcidecl.rs:47`**: `/sys/bus/pci/devices/{}`
- **`crates/op-plugins/src/state_plugins/privacy_routes.rs:10`**: `/var/lib/op-dbus/privacy-routes.json`
- **`crates/op-plugins/src/state_plugins/service.rs:46`**: `/etc/dinit.d`
- **`crates/op-plugins/src/state_plugins/service.rs:173`**: `/etc/systemd/system`
- **`crates/op-plugins/src/state_plugins/systemd_networkd.rs:31`**: `/etc/systemd/network`
- **`crates/op-plugins/src/state_plugins/systemd_networkd.rs:124`**: `/org/freedesktop/systemd1/unit/systemd_2dnetworkd_2eservice`
- **`crates/op-plugins/src/state_plugins/users.rs:43`**: `/etc/passwd`
- **`crates/op-plugins/src/state_plugins/dinit.rs:150`**: `/run/dbus/system_bus_socket`
- **`crates/op-plugins/src/state_plugins/full_system.rs:207`**: `/etc/os-release`
- **`crates/op-plugins/src/state_plugins/full_system.rs:216`**: `/etc/localtime`
- **`crates/op-plugins/src/state_plugins/full_system.rs:223`**: `/proc/uptime`
- **`crates/op-plugins/src/state_plugins/full_system.rs:236`**: `/sys/class/net`
- **`crates/op-plugins/src/state_plugins/full_system.rs:240`**: `/sys/class/net/{}/address`
- **`crates/op-plugins/src/state_plugins/full_system.rs:246`**: `/sys/class/net/{}/operstate`
- **`crates/op-plugins/src/state_plugins/full_system.rs:252`**: `/sys/class/net/{}/mtu`
- **`crates/op-plugins/src/state_plugins/full_system.rs:264`**: `/etc/resolv.conf`
- **`crates/op-plugins/src/state_plugins/full_system.rs:350`**: `/etc/passwd`
- **`crates/op-plugins/src/state_plugins/full_system.rs:394`**: `/proc/mounts`
- **`crates/op-plugins/src/state_plugins/net.rs:430`**: `/etc/network/interfaces`
- **`crates/op-plugins/src/state_plugins/net.rs:537`**: `/var/run/openvswitch/db.sock`
- **`crates/op-plugins/src/state_plugins/privacy_router.rs:30`**: `/etc/wireguard/wgcf.conf`

#### Hardcoded Ports & IP Addresses
- **`crates/op-plugins/src/state_plugins/privacy_router.rs:25`**: `10.200.0.1/24` (Management network gateway IP).
- **`crates/op-plugins/src/state_plugins/privacy_router.rs:26`**: `10.200.0.1:6653` (OpenFlow controller address).
- **`crates/op-plugins/src/state_plugins/proxy_server.rs:13`**: Port `8080` as the hardcoded default fallback.
- **`crates/op-plugins/src/state_plugins/netmaker.rs:129`**: IP discovery endpoint `https://api.ipify.org`.

---

### Security & Quality Findings

#### CRITICAL: Arbitrary Command Injection in `pcidecl` Plugin
- **File & Line**: `crates/op-plugins/src/state_plugins/pcidecl.rs:112` (triggered via lines 144–151)
- **Impact**: Arbitrary command execution with root privileges.
- **Description**: The plugin deserializes a user-supplied target state containing `PciItem` declarations. For each item, it parses the `address` field and immediately forwards it to `Self::lspci_present(&item.address)` to determine device presence. This helper evaluates:
  ```rust
  Command::new("sh")
      .arg("-c")
      .arg(format!("lspci -s {} >/dev/null 2>&1; echo $?", addr))
  ```
  Since `addr` is an unsanitized string value extracted from desired state JSON, a malicious user can inject command separators (e.g., `0000:00:1f.6; malicious_command`) to run shell payloads directly.

#### HIGH: Unsafe Use of `simd_json::from_str`
- **File & Line**: 
  - `crates/op-plugins/src/state_plugins/config.rs:38`
  - `crates/op-plugins/src/state_plugins/mcp.rs:188`
  - `crates/op-plugins/src/state_plugins/ovsdb_bridge.rs:172`
  - `crates/op-plugins/src/state_plugins/privacy_routes.rs:56`
- **Impact**: Potential undefined behavior or memory safety violations on invalid inputs.
- **Description**: `simd-json` parses strings *in-place* to achieve zero-copy deserialization, mutating the underlying buffer. If the payload is parsed as an owned `String` via `simd_json::from_str(&mut string)`, and parsing subsequently fails, the string buffer may be left containing invalid UTF-8 sequences. Accessing or dropping this mutated string object thereafter violates Rust's invariant guarantees, resulting in Undefined Behavior (UB).

#### MEDIUM: Predicted Temp Path & Unnecessary Shell Execution
- **File & Line**: `crates/op-plugins/src/state_plugins/dnsresolver.rs:128–131`
- **Impact**: Predictable temporary files can lead to Denial of Service or local privilege escalation attacks (such as symlink targeting) if `/etc` write access boundaries are misconfigured.
- **Description**: The plugin writes resolved configurations to a hardcoded file path (`/etc/resolv.conf.sysdecl.tmp`) before moving it:
  ```rust
  let tmp_path = "/etc/resolv.conf.sysdecl.tmp";
  fs::write(tmp_path, buf.as_bytes()).context("write temp resolv.conf")?;
  let mv_cmd = format!("mv -f {} /etc/resolv.conf", tmp_path);
  let mv_ok = Command::new("sh")
      .arg("-c")
      .arg(&mv_cmd)
  ```
  Spawning an external shell binary (`sh -c`) to move a file is inefficient and presents unnecessary safety overheads. It should be replaced with `std::fs::rename`.

#### LOW: Non-Functional Argument Array in `netmaker` Plugin
- **File & Line**: `crates/op-plugins/src/state_plugins/netmaker.rs:268–270`
- **Impact**: Process execution failure.
- **Description**: The netmaker installation logic executes:
  ```rust
  Command::new("apt")
      .args(["update", "&&", "apt", "install", "-y", "netclient"])
  ```
  `Command::new` spawns process executables directly without a wrapping shell. Chaining operators like `"&&"` or nested commands like `"apt"` are passed as literal string arguments to the original process rather than being interpreted by a shell environment. This causes the update command to fail execution.

---
## ⚠ Citation Warnings
- `crates/op-plugins/src/registry.rs:245`: file has 195 lines
