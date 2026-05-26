# Production Security and Quality Audit: op-plugins

## Test Suite Assessment

### 1. Test Discovery
The codebase has a comprehensive unit testing structure embedded directly in the source files under `#[cfg(test)]` modules.

### 2. Test Function Count
*   **Total Test Functions Counted**: 39

### 3. Representative Test Samples
*   `crates/op-plugins/src/state.rs:289` (`test_desired_state_hash`)
*   `crates/op-plugins/src/state_plugins/incus.rs:572` (`test_instances_equivalent_detects_config_and_device_changes`)
*   `crates/op-plugins/src/state_plugins/web_ui.rs:548` (`test_plugin_state`)

### 4. Property-Based Testing and Fuzzing
*   **Property-Based Testing (`proptest`, `quickcheck`)**: No property-based tests were found.
*   **Fuzzing**: No fuzzing targets or harnesses are configured in the provided source files.

---

## Security & Quality Findings

### [Critical] Arbitrary File Write & Privilege Escalation via PCI Address Path Traversal
#### File: `crates/op-plugins/src/state_plugins/pcidecl.rs:120`

**Description**:
The `set_driver_override` function accepts an unsanitized `addr` string from the user-controlled `PciItem` JSON configuration. It constructs a file path by formatting the string with `/sys/bus/pci/devices/` and appends `/driver_override` at the end:

```rust
fn sys_path(addr: &str) -> String {
    format!("/sys/bus/pci/devices/{}", addr)
}

fn set_driver_override(addr: &str, val: &str) -> Result<()> {
    let p = format!("{}/driver_override", Self::sys_path(addr));
    fs::write(&p, format!("{}\n", val)).context("write driver_override")?;
    Ok(())
}
```

Because the `addr` string is not validated to ensure it represents a valid PCI address (e.g., `0000:00:1f.6`), a malicious actor who can set the desired configuration can perform a path traversal attack. For example, by specifying the address `../../../../etc/cron.d`, the path resolves to:

`/sys/bus/pci/devices/../../../../etc/cron.d/driver_override` $\rightarrow$ `/etc/cron.d/driver_override`

The plugin then writes the user-controlled `val` into `/etc/cron.d/driver_override`, allowing them to execute arbitrary commands as `root` via the system cron daemon.

**Remediation**:
Strictly validate the `addr` parameter using a regular expression that only matches valid PCI addresses, such as `^[0-9a-fA-F]{4}:[0-9a-fA-F]{2}:[0-9a-fA-F]{2}\.[0-9a-fA-F]$`, before using it to construct filesystem paths.

---

### [Critical] Path Traversal and Arbitrary BTRFS Subvolume Access via Storage Configurations
#### File: `crates/op-plugins/src/state_plugins/lxc.rs:415`

**Description**:
When creating a container from a BTRFS golden image, the `golden_image_name` and `storage` values are fetched directly from the desired container properties specified in the user-supplied JSON payload:

```rust
let storage = props
    .and_then(|p| p.get("storage"))
    .and_then(|v| v.as_str())
    .unwrap_or("local-btrfs");

// ...

let storage_path = format!("/var/lib/pve/{}", storage);
let golden_image_path = format!("{}/templates/subvol/{}", storage_path, golden_image_name);
let container_rootfs = format!("{}/images/{}/rootfs", storage_path, container.id);
```

Since neither `storage` nor `golden_image_name` are sanitized or validated against directory traversal sequences (`..`), an attacker can manipulate these properties to point to any directory on the host. 

For instance, they can snapshot any arbitrary subvolume on the system (even those belonging to other tenants or the host OS) into `/var/lib/pve/local-btrfs/images/{id}/rootfs` by executing:

```rust
let snapshot_output = tokio::process::Command::new("btrfs")
    .args([
        "subvolume",
        "snapshot",
        &golden_image_path,
        &container_rootfs,
    ])
    .output()
    .await?;
```

If the container is deployed with `"unprivileged": false`, the attacker can then easily access, read, and write to these files within the container, bypassing all host-to-guest and tenant boundaries.

**Remediation**:
1.  Validate that the `storage` parameter belongs to an allowed set of storage pools.
2.  Sanitize `golden_image_name` using path-sanitization helpers (such as `Path::components()`) to prevent directory traversal, or strictly enforce alphanumeric naming conventions.

---

### [High] Insecure Temporary File Creation (ToCTOU / Symlink Race)
#### File: `crates/op-plugins/src/state_plugins/dnsresolver.rs:114`

**Description**:
The `write_resolv_conf` function writes the configuration to a hardcoded temporary file in a world-writable directory (`/etc/`) before swapping it with the active `resolv.conf`:

```rust
let tmp_path = "/etc/resolv.conf.sysdecl.tmp";
fs::write(tmp_path, buf.as_bytes()).context("write temp resolv.conf")?;
```

Because `/etc/resolv.conf.sysdecl.tmp` is static and predictable, a local unprivileged attacker could create a symbolic link at this location pointing to a sensitive file (e.g., `/etc/shadow` or `/etc/passwd`). When `write_resolv_conf` is called by the high-privilege agent, it will follow the symlink and overwrite the target file, resulting in a Denial of Service or arbitrary file corruption.

**Remediation**:
Use the `tempfile` crate to securely generate a random, non-predictable temporary file within `/etc` (ensuring it is on the same mount partition as `/etc/resolv.conf`), and then atomic-rename it to overwrite `/etc/resolv.conf`.

---

### [High] Unsanitized / Unchecked Shell Executions via Relative Path Binaries
#### File: `crates/op-plugins/src/state_plugins/service.rs:252`

**Description**:
The codebase repeatedly spawns system processes by relying on relative executable names, such as `"systemctl"`, `"apt-get"`, `"dnf"`, `"pacman"`, `"lsblk"`, `"uname"`, `"id"`, and others, rather than invoking absolute paths:

```rust
let out = tokio::process::Command::new("systemctl")
    .args(["show", name, "--property=ActiveEnterTimestamp"])
```

If the plugin manager runs as `root` and can inherit or accept modified environment variables (especially `PATH`), a malicious local actor could modify the `PATH` environment variable to point to a directory they control, placing a Trojan executable (such as a malicious `systemctl`) there. When the agent triggers this command, it will execute the malicious binary with the agent's elevated privileges.

**Remediation**:
Specify absolute paths for all system binaries (e.g., `/usr/bin/systemctl` or `/bin/systemctl`), or explicitly sanitize the `PATH` environment variable of the spawned `Command` using `.env_clear()` and explicitly resetting a safe `PATH`.

---

### [Medium] Denial of Service via Sequential Shell Execution in Reconciliation Loops
#### File: `crates/op-plugins/src/state_plugins/service.rs:188`

**Description**:
In `query_current_state`, the service plugin iterates over all discovered system services and sequentially calls `check_lifecycle` for each service:

```rust
for svc_name in service_list {
    if let Ok(lifecycle) = self.check_lifecycle(&svc_name).await {
        services.insert(svc_name, json!({ "lifecycle": lifecycle }));
    }
}
```

The `check_lifecycle` function spawns a `systemctl show` process for *every single service* sequentially. On modern systems containing hundreds of active or inactive units, this results in hundreds of sequential process forks. Spawning processes sequentially in an async executor is incredibly slow, blocks the Tokio worker threads, causes high CPU load, and will result in timeouts and Denial of Service (DoS) of the main controller during routine state collections.

**Remediation**:
Retrieve all unit properties in a single bulk query (e.g., `systemctl show "*"` or `systemctl show --all`), or query systemd's state directly using D-Bus properties without spawning external CLI processes.

---

### [Medium] Weak Cryptographic Hash Usage for Audit Trail Verification
#### File: `crates/op-plugins/src/auto_create.rs:104`

**Description**:
The plugin framework uses MD5 to calculate configuration hashes for both state diffing and building "blockchain footprints" used in the audit trails:

```rust
current_hash: format!("{:x}", md5::compute(simd_json::to_string(current)?)),
desired_hash: format!("{:x}", md5::compute(simd_json::to_string(desired)?)),
```

MD5 is cryptographically broken and vulnerable to collision attacks. A malicious user could craft two different desired state configurations that yield the identical MD5 hash. If these hashes are recorded on a blockchain or ledger for audit trail integrity, this collision can be used to bypass forensic verification or disguise unauthorized mutations.

**Remediation**:
Replace MD5 with a secure SHA-256 or SHA-512 hashing algorithm across all state calculation and comparison utilities. This is already implemented in some plugins (such as `openflow.rs` and `web_ui.rs`) but must be standardized globally.

---
## ⚠ Citation Warnings
- `crates/op-plugins/src/state.rs:289`: file has 282 lines
