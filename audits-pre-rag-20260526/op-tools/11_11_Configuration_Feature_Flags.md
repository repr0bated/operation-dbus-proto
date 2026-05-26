### 1. `std::env::var` Reads

| File:Line | Environment Variable | Default Value | Error Handling Status / Flags |
| :--- | :--- | :--- | :--- |
| `crates/op-tools/src/mcptools.rs:53` | `OP_MCPTOOLS_BIN` | `"mcp"` | Safe: uses `unwrap_or_else` |
| `crates/op-tools/src/mcptools.rs:172` | `OP_MCPTOOLS_ALLOW_UNPREFIXED` | `false` | Safe: uses `.ok()` mapped to a boolean |
| `crates/op-tools/src/mcptools.rs:178` | `OP_MCPTOOLS_SERVERS` | None | Safe: matches on `Ok` results only |
| `crates/op-tools/src/mcptools.rs:202` | `OP_MCPTOOLS_SERVER` | None | Safe: matches on `Ok` results only |
| `crates/op-tools/src/mcptools.rs:209` | `OP_MCPTOOLS_SERVER_NAME` | `"default"` | Safe: uses `unwrap_or_else` |
| `crates/op-tools/src/mcptools.rs:318` | `OP_MCPTOOLS_CONFIG` | `"mcptools.json"` | Safe: uses `unwrap_or_else` |
| `crates/op-tools/src/builtin/agent_tool.rs:257` | `OP_AGENT_AUTOSTART_ALL` | `false` | Safe: uses `.ok()` mapped to a boolean |
| `crates/op-tools/src/builtin/agent_tool.rs:280` | Dynamic parameter (e.g., `OP_AGENT_INCLUDE`, `OP_AGENT_AUTOSTART`) | None | Safe: uses `.ok()?` inside helper helper |
| `crates/op-tools/src/builtin/agent_tool.rs:294` | `OP_AGENT_BUS` | Dynamic | Safe: uses `.ok()` and gracefully falls back |
| `crates/op-tools/src/builtin/agent_tool.rs:301` | `DBUS_SESSION_BUS_ADDRESS` | None | Safe: checked via `.is_ok()` |
| `crates/op-tools/src/builtin/anydesk.rs:512` | `DISPLAY` | None | Safe: bound-checked via `if let Ok` |
| `crates/op-tools/src/builtin/anydesk.rs:517` | `XAUTHORITY` | None | Safe: bound-checked via `if let Ok` |
| `crates/op-tools/src/builtin/anydesk.rs:521` | `DISPLAY` | None | Safe: bound-checked via `if let Ok` |
| `crates/op-tools/src/builtin/anydesk.rs:544` | `DISPLAY` | None | Safe: bound-checked via `if let Ok` |
| `crates/op-tools/src/builtin/code_search.rs:126` | `QDRANT_URL` | `"http://127.0.0.1:6333"` | Safe: uses `unwrap_or_else` |
| `crates/op-tools/src/builtin/plugin_projection.rs:47` | `DBUS_SESSION_BUS_ADDRESS` | None | Safe: checked via `.is_ok()` |
| `crates/op-tools/src/builtin/self_tools.rs:27` | `OP_SELF_REPO_PATH` | None | Safe: checked via `.ok()`; propagates custom `anyhow` error if missing during tool invocation |

*   **Flagged variables with no default and no error handling**: None. All environment variable reads are explicitly wrapped in fallback conditions or error propagation blocks.

---

### 2. Cargo Features

*   **Crate-Level Features (`crates/op-tools/Cargo.toml`)**:
    *   No custom features are defined for the `op-tools` crate itself.
*   **Workspace-Level Features (`Cargo.toml`)**:
    *   `default = ["grpc"]`
    *   `grpc = []`
*   **Feature Additivity**: Yes, the features are additive. Cargo’s features are unified across the workspace dependencies; enabling the `grpc` feature on any workspace member adds its respective logic without overriding or mutually excluding other compiled states.

---

### 3. Hardcoded Paths, Ports, and Addresses

*   **`crates/op-tools/src/builtin_old.rs:158`**: Hardcoded command shell invocation with binary `"sh"`.
*   **`crates/op-tools/src/validation.rs:113`**: Hardcoded default allowed directory list: `["/tmp", "/var/tmp", "/home"]`.
*   **`crates/op-tools/src/validation.rs:120`**: Hardcoded default forbidden system directories: `["/boot", "/dev", "/proc/sys", "/sys", "/root", "/etc/shadow", "/etc/passwd"]`.
*   **`crates/op-tools/src/security.rs:183`**: Hardcoded forbidden system paths in the restricted user profile: `/etc/shadow`, `/etc/sudoers`, `/root`.
*   **`crates/op-tools/src/security.rs:324`**: Hardcoded allowed read directories for restricted profiles: `/tmp`, `/var/log`, `/home`, `/opt`.
*   **`crates/op-tools/src/security.rs:349`**: Hardcoded allowed write directory for restricted profiles: `/tmp`.
*   **`crates/op-tools/src/builtin/anydesk.rs:400`**: Hardcoded AnyDesk configuration paths: `"/etc/anydesk/anydesk.conf"`, `"/home/jeremy/.anydesk/anydesk.conf"`, `"/home/jeremy/.anydesk/user.conf"`.
*   **`crates/op-tools/src/builtin/anydesk.rs:488`**: Hardcoded AnyDesk network ports checked via netstat: `"7070"`, `"6568"`, `"80"`, `"443"`.
*   **`crates/op-tools/src/builtin/anydesk.rs:584`**: Hardcoded X11 display `DISPLAY=:99` and AnyDesk systemd unit path `/etc/systemd/system/anydesk.service`.
*   **`crates/op-tools/src/builtin/anydesk.rs:590`**: Hardcoded X11 authority path `XAUTHORITY=/root/.Xauthority`.
*   **`crates/op-tools/src/builtin/anydesk.rs:635`**: Hardcoded target authority files `/root/.Xauthority` and `/home/jeremy/.Xauthority`.
*   **`crates/op-tools/src/builtin/code_search.rs:127`**: Hardcoded default Qdrant vector database URL `"http://127.0.0.1:6333"`.
*   **`crates/op-tools/src/builtin/ovs_tools.rs:538`**: Hardcoded Open vSwitch OVSDB Unix socket path `/var/run/openvswitch/db.sock`.
*   **`crates/op-tools/src/builtin/ovs_tools.rs:708`**: Hardcoded default privacy router tunnel ports: `"priv_wg"`, `"priv_warp"`, `"priv_xray"`.
*   **`crates/op-tools/src/builtin/ovsdb.rs:16`**: Hardcoded Open vSwitch OVSDB Unix socket path `/var/run/openvswitch/db.sock`.
*   **`crates/op-tools/src/builtin/procfs.rs:129`**: Hardcoded proc filesystem mount root `/proc`.
*   **`crates/op-tools/src/builtin/procfs.rs:192`**: Hardcoded sys filesystem mount root `/sys`.
*   **`crates/op-tools/src/builtin/shell_tool.rs:68`**: Hardcoded default working directory `/tmp`.
*   **`crates/op-tools/src/builtin/shell_tool.rs:104`**: Hardcoded system shell executable `"bash"`.
*   **`crates/op-tools/src/builtin/self_tools.rs:447`**: Hardcoded default deployment target systemd service name `"op-web"`.
*   **`crates/op-tools/src/builtin/openflow_tools.rs:71`**: Hardcoded default Open vSwitch bridge name `"ovs-br0"`.

---

### 4. Critical Security Vulnerabilities

#### Critical: Arbitrary File Write and Path Traversal Bypass via `self_write_file`
*   **Citation**: `crates/op-tools/src/builtin/self_tools.rs:188`
*   **Vulnerability**: The validation check ensures that the target directory of the write operation lies within the defined repository path (`OP_SELF_REPO_PATH`). However, it determines the parent directory to canonicalize by calling `p.exists()`. If the user defines a non-existent nested directory containing path traversal operators (such as `nonexistent_dir/../../../../etc/cron.d/malicious`), `p.exists()` returns `false`. This completely skips the boundary check on line 191.
*   **Impact**: Any user with access to the `self_write_file` tool can create non-existent folders that traverse outside the repository, letting them write arbitrary files anywhere on the local filesystem (such as scheduling malicious root cron jobs).

#### Critical: Remote Command Injection via Restricted Shell Tool argument expansion
*   **Citation**: `crates/op-tools/src/builtin_old.rs:160`
*   **Vulnerability**: The `validate` method checks only the `command` field in the input payload against the allowed commands list by parsing the first word of the input string. However, the `execute` method maps the separate unchecked `args` array directly into the formatted shell execution pattern:
    ```rust
    tokio::process::Command::new("sh")
        .arg("-c")
        .arg(format!("{} {}", command, args.join(" ")))
    ```
*   **Impact**: An untrusted caller can bypass command restrictions by providing a safe allowed command (like `cat`) in the validated `command` field and passing payload delimiters and command injections (like `["dummy", "; rm -rf /"]`) inside the completely unvalidated `args` parameter. This results in arbitrary execution of commands under system privileges.

#### Critical: Command Chaining Security Profile Bypass in Shell Execution
*   **Citation**: `crates/op-tools/src/security.rs:277` and `crates/op-tools/src/builtin/shell.rs:88`
*   **Vulnerability**: The security validator's `check_command` method restricts untrusted sessions by parsing only the first whitespace-separated segment of the command string to check against its whitelist:
    ```rust
    let base_cmd = command
        .split_whitespace()
        .next()
        .ok_or_else(|| SecurityError::ValidationFailed("Empty command".to_string()))?;
    ```
    Once validated, the full unmodified command is handed directly to `Command::new("bash").arg("-c").arg(command)`.
*   **Impact**: Any client operating under a `Restricted` profile can bypass the whitelist entirely and execute arbitrary commands by chaining commands after an allowed executable (for example: `ls -la ; cat /etc/shadow`). The parser only checks `"ls"`, lets the string pass, and bash executes both statements.

#### Critical: Arbitrary Local File Read via `FileReadTool`
*   **Citation**: `crates/op-tools/src/builtin_old.rs:223`
*   **Vulnerability**: `FileReadTool` is registered directly as an active tool at registry initialization but lacks any boundary-checking logic or directory restrictors.
*   **Impact**: Any caller can retrieve sensitive system files (such as private keys or `/etc/shadow`) by specifying absolute paths.

---

### 5. Quality & Undefined Behavior Findings

#### Undefined Behavior: Unpadded String input into `simd_json::from_str`
*   **Citation**: `crates/op-tools/src/mcptools.rs:182` and `crates/op-tools/src/mcptools.rs:191`
*   **Vulnerability**: The parser calls `simd_json::from_str` within an `unsafe` block on a mutable reference to a `String` loaded from `std::env::var("OP_MCPTOOLS_SERVERS")`. `simd-json` requires that parsed strings/buffers contain padding bytes at the end of their allocations to ensure vectorized SIMD instructions do not read or write past boundary structures. Standard `String` allocations do not guarantee this padding.
*   **Impact**: Parsing unpadded environmental string variables directly with `simd_json::from_str` triggers undefined behavior, potentially causing memory corruption, memory leaks, or segmentation faults.