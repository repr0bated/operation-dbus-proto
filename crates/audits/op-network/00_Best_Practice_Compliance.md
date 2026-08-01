| Pattern | File:Line | Crate Approach | Corpus Best Practice | Gap | Rating |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `format_json_manual` | `crates/op-network/src/ovs_netlink.rs:286` | Decodes byte slice to UTF-8 and trims trailing null bytes manually using `.trim_end_matches('\0')`. | Use proper C-string parsing abstractions (`CStr`) or schema-driven deserialization. | Ad-hoc byte string manipulation instead of a typed protocol parsing/schema. | Minor Gap |
| `format_json_manual` | `crates/op-network/src/ovs_netlink.rs:416` | Decodes byte slice to UTF-8 and trims trailing null bytes manually using `.trim_end_matches('\0')`. | Use proper C-string parsing abstractions (`CStr`) or schema-driven deserialization. | Ad-hoc byte string manipulation instead of a typed protocol parsing/schema. | Minor Gap |
| `unwrap_expect` | `crates/op-network/src/ovs_netlink.rs:1019` | Invokes `.expect()` on client creation inside an asynchronous block. | Graceful error propagation via `?` or structured fallback. | Potential runtime panics on async setup failure rather than structured error returns. | Minor Gap |
| `unwrap_expect` | `crates/op-network/src/ovs_netlink.rs:1023` | Invokes `.expect()` on listing datapaths. | Graceful error propagation via `?` or structured fallback. | Uncaught panic risk if the netlink connection or kernel query fails. | Minor Gap |
| `unwrap_expect` | `crates/op-network/src/ovs_netlink.rs:1032` | Invokes `.expect()` on client creation during vport querying. | Graceful error propagation via `?` or structured fallback. | Potential runtime panic during initialization of the netlink client interface. | Minor Gap |
| `unwrap_expect` | `crates/op-network/src/ovs_netlink.rs:1036` | Invokes `.expect()` on listing datapaths. | Graceful error propagation via `?` or structured fallback. | Uncaught panic risk on datapath query failures. | Minor Gap |
| `unwrap_expect` | `crates/op-network/src/ovs_netlink.rs:1041` | Invokes `.expect()` on listing vports. | Graceful error propagation via `?` or structured fallback. | Uncaught panic risk on vport query failures. | Minor Gap |
| `unsafe_block` | `crates/op-network/src/ovs_capabilities.rs:99` | Uses an `unsafe` block directly to invoke `libc::geteuid()`. | Wrap system state lookups using safe wrappers like `rustix` or `nix`. | Direct manual `unsafe` scope for an operation that has safe and standard library alternatives. | Minor Gap |
| `format_json_manual` | `crates/op-network/src/ovs_capabilities.rs:266` | Formats documentation/diagnostic strings using nested `.push_str(&format!(...))` templates. | Keep structural data contracts distinct from output generation using schemas (OSCAL/Protobuf). | Ad-hoc string templating inside the codebase rather than structured, schema-compliant outputs. | Minor Gap |
| `format_json_manual` | `crates/op-network/src/ovs_capabilities.rs:267` | Formats documentation/diagnostic strings using nested `.push_str(&format!(...))` templates. | Keep structural data contracts distinct from output generation using schemas (OSCAL/Protobuf). | Ad-hoc string templating inside the codebase rather than structured, schema-compliant outputs. | Minor Gap |
| `std_fs_in_async` | `crates/op-network/src/ovs_capabilities.rs:132` | Calls synchronous `std::fs::read_to_string` on `/proc/modules` inside an `async fn`. | Use non-blocking filesystem calls (`tokio::fs::read_to_string`) or wrap in `spawn_blocking`. | Synchronous file I/O blocks the thread of the asynchronous executor, causing runtime starvation. | Major Gap |
| `command_new` | `crates/op-network/src/rtnetlink.rs:383` | Spawns a synchronous external process `ip route replace ...` using `std::process::Command`. | Interface programmatically with the kernel using Netlink sockets (already available in scope). | Uses heavy, fragile, synchronous external shell execution rather than programmatic programmatic APIs. | Major Gap |
| `format_json_manual` | `crates/op-network/src/rtnetlink.rs:54` | Formats MAC addresses into standard notation manually using `.map(...).join(":")`. | Rely on typed address formats (e.g., using `macaddr` or safe parser structs). | Ad-hoc formatting pattern used instead of utilizing a structured schema or parser. | Minor Gap |
| `std_fs_in_async` | `crates/op-network/src/proxmox.rs:248` | Calls synchronous `std::fs::read_to_string` on credential files from an async context. | Use non-blocking filesystem calls (`tokio::fs::read_to_string`) or wrap in `spawn_blocking`. | Synchronous disk access on the active executor thread can starve async tasks under load. | Major Gap |
| `command_new` | `crates/op-network/src/plugin.rs:404` | Spawns external `dhclient` process non-blockingly using `tokio::process::Command`. | Utilize purely programmatic DHCP libraries to remain completely self-contained. | Relies on platform-specific binaries for essential networking features. | Minor Gap |
| `std_fs_in_async` | `crates/op-network/src/plugin.rs:228` | Uses `tokio::fs::create_dir_all().await` to create missing directories. | Use non-blocking async filesystem operations. | Follows best practices. | Compliant |
| `command_new` | `crates/op-network/src/bin/op-xdp-wg.rs:262` | Runs synchronous shell execution to query `xdp-loader status`. | Query device configurations directly via XDP/BPF kernel APIs (such as `aya`). | Spawns subprocesses to inspect configuration instead of direct socket or library queries. | Minor Gap |
| `command_new` | `crates/op-network/src/bin/op-xdp-wg.rs:271` | Runs synchronous shell execution to query filter rules via `tc`. | Query device configurations directly via tc/netlink library structures. | Spawns subprocesses to inspect configuration instead of direct socket or library queries. | Minor Gap |
| `command_new` | `crates/op-network/src/bin/op-xdp-wg.rs:281` | Runs synchronous shell execution to query neighborhood state via `ip`. | Query device configurations directly via programmatic interfaces. | Spawns subprocesses to inspect configuration instead of direct socket or library queries. | Minor Gap |
| `std_fs_in_async` | `crates/op-network/src/bin/op-ovsbr0-setup.rs:74` | Standard directory reading using `std::fs::read_dir` in a sequential setup tool. | Synchronous file API usage is standard for sequential initialization CLIs. | None. Context-appropriate usage of standard I/O in an orchestration utility. | Compliant |

---

### Actionable Recommendations for Major/Critical Gaps

#### 1. Eliminate Blocking Filesystem Operations in Async Runtimes
* **Files:** `crates/op-network/src/ovs_capabilities.rs:132`, `crates/op-network/src/proxmox.rs:248`
* **Issue:** The use of synchronous disk/virtual file operations (`std::fs::read_to_string`) halts the execution of the entire thread assigned to the `tokio` multi-threaded executor during the system call duration.
* **Remediation:** 
  * Replace the blocking file reads with their asynchronous equivalents. Update imports to use `tokio::fs::read_to_string` and yield control back to the async reactor with `.await`:
  ```rust
  // In crates/op-network/src/proxmox.rs:248
  let (token, node) = if let Ok(content) = tokio::fs::read_to_string(&token_file).await { ... }
  ```
  ```rust
  // In crates/op-network/src/ovs_capabilities.rs:132
  let proc_content = tokio::fs::read_to_string("/proc/modules").await.unwrap_or_default();
  let has_ovs = proc_content.contains("openvswitch");
  ```

#### 2. Avoid Process Spawning for Network Configuration in Async Code
* **File:** `crates/op-network/src/rtnetlink.rs:383`
* **Issue:** Spawning synchronous CLI wrappers (`ip route replace ...`) creates process-level overhead, lacks structured validation, and introduces platform dependencies.
* **Remediation:**
  * Utilize the core programmatic library capabilities to interact with the kernel via netlink sockets directly.
  * Use the existing `rtnetlink` structures to execute the equivalent of the route update operation:
  ```rust
  // Replace:
  // let status = Command::new("ip").args([...]).status();
  // With:
  let mut route_req = handle.route().add();
  // Programmatically construct the route payload using safe types rather than raw strings:
  route_req
      .v4()
      .destination_prefix(destination, prefix_len)
      .gateway(gateway_ip)
      .output_interface(ifindex)
      .execute()
      .await?;
  ```