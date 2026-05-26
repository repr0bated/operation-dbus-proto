### Async & Concurrency Metric Counts

* **`async fn` Total Count**: 159
* **`tokio::spawn` Total Count**: 10
* **`spawn_blocking` Total Count**: 0

---

### Critical Findings

#### Exploitable Memory & Resource Leak / DoS via Unbounded Log Stream Tasks
* **File**: `crates/op-web/src/handlers/logs.rs:129`
* **Vulnerability Type**: Resource Exhaustion / Denial of Service
* **Description**: 
  The `/api/logs/stream` endpoint initiates an SSE stream for client log tracking. Inside the handler, it spawns a background thread using `tokio::spawn` with `linemux::MuxedLines` to tail log files:
  ```rust
  let broadcaster = state.sse_broadcaster.clone();
  tokio::spawn(async move {
      use linemux::MuxedLines;
      let mut lines = MuxedLines::new().expect("Failed to create MuxedLines");
      // ...
      while let Ok(Some(line)) = lines.next_line().await {
          // ...
          broadcaster.broadcast("log", &payload);
      }
  });
  ```
  The returned `JoinHandle` is discarded, and the task lifetime is completely detached from the client's connection lifetime. There is no cancellation token or connection drop detection to terminate this infinite file-watching loop.
* **Exploitability**: 
  An unauthenticated remote attacker can trigger this endpoint repeatedly (e.g., sending 1,000 parallel requests and immediately closing them). This leaks 1,000 permanent background file-watching tasks and file descriptors, starving system inotify limits and RAM, causing a complete system denial of service.

---

### High & Medium Severity Findings

#### Blocking Tokio Event Loop via Sync Container/Process Execution in `async fn`
* **File**: `crates/op-web/src/handlers/mail.rs:21`
* **Severity**: High
* **Description**: 
  The `mail_status_handler` is an `async fn` that executes a synchronous process launch to check if `maddy` is active in an incus container:
  ```rust
  let running = Command::new("incus")
      .args(&["exec", "crd-astral", "--", "systemctl", "is-active", "maddy"])
      .output()
  ```
  Calling `std::process::Command::output()` synchronously blocks the active operating system thread on which the Tokio executor is running. Spawning a container command-line execution can block for up to several seconds under load, causing severe starvation of all other concurrent HTTP requests sharing that executor thread.

#### Reactor Thread Blocked by Synchronous File I/O in `async fn`
* **File**: `crates/op-web/src/mcp_agents.rs:430`
* **Severity**: Medium
* **Description**: 
  The `set_cognitive_agents` function is a public `async fn` that calls `save_agent_config` on line 446:
  ```rust
  let applied = agents.apply_config(next);
  save_agent_config(&applied)?;
  ```
  `save_agent_config` executes synchronous directory creation and file writes:
  ```rust
  std::fs::create_dir_all(parent)...
  std::fs::write(&path, body)...
  ```
  Performing blocking file system I/O within a high-concurrency event loop blocks the Tokio worker thread, preventing it from processing other network events or timer ticks.

#### Synchronous Process Spawning Inside Async Handlers
* **File**: `crates/op-web/src/handlers/vpn.rs:45`
* **File**: `crates/op-web/src/handlers/vpn.rs:60`
* **File**: `crates/op-web/src/handlers/vpn.rs:107`
* **Severity**: Medium
* **Description**: 
  The async handlers `vpn_status_handler` and `vpn_config_handler` query the WireGuard state using synchronous child process commands:
  * `Command::new("wg").args(&["show", interface]).output()`
  * `Command::new("wg").args(&["show", interface, "dump"]).output()`
  * `Command::new("wg").args(&["show", interface, "public-key"]).output()`
  These operations run on every state/status request and block the executor worker threads synchronously.

#### Sync Tail Command inside Loop in `async fn`
* **File**: `crates/op-web/src/handlers/logs.rs:36`
* **Severity**: Medium
* **Description**: 
  The `logs_handler` executes synchronous `Command` spawns inside a `for` loop to tail three different log files:
  ```rust
  for (log_path, component) in log_files {
      if let Ok(output) = Command::new("tail").args(&["-n", "50", log_path]).output() { ... }
  }
  ```
  This synchronous blocking logic within an active web request handler blocks the Tokio reactor thread and scales linearly with the number of log files.

#### Sync File I/O and Command Spawning inside `dashboard_metrics_handler`
* **File**: `crates/op-web/src/handlers/dashboard.rs:27`
* **File**: `crates/op-web/src/handlers/dashboard.rs:30`
* **Severity**: Medium
* **Description**: 
  The dashboard handler calls helper functions that execute blocking operations on every metrics refresh:
  * Line 44: Synchronous process execution: `Command::new("wg").args(&["show", "wg0", "peers"]).output()`
  * Line 60: Synchronous file read: `std::fs::read_to_string("/proc/loadavg")`
  * Line 70: Synchronous file read: `std::fs::read_to_string("/proc/meminfo")`
  These blocking queries degrade API responsiveness under high dashboard polling frequency.

#### Synchronous Process Execution on Event Loop Initiation
* **File**: `crates/op-web/src/state.rs:186`
* **Severity**: Medium
* **Description**: 
  During state initialization in `new_with_registry`, the `async fn` executes `WgServerConfig::default()` synchronously. This calls:
  ```rust
  Command::new("wg").args(["show", interface, "public-key"]).output()
  ```
  This blocks the system bootstrap thread synchronously while waiting for command-line output from the kernel module.

#### Synchronous File System Checks inside Network Operations
* **File**: `crates/op-web/src/privacy_network.rs:95`
* **File**: `crates/op-web/src/privacy_network.rs:115`
* **File**: `crates/op-web/src/privacy_network.rs:124`
* **Severity**: Low / Quality
* **Description**: 
  The async function `ensure_host_privacy_network_with_config` calls synchronous `Path::exists()` metadata checks across multiple network interfaces:
  * `Path::new(&format!("/sys/class/net/{}", cfg.wgcf_tunnel)).exists()`
  These should be replaced with async metadata queries (`tokio::fs::metadata`) to ensure zero-block guarantees.

---

### Task & JoinHandle Mismanagement

#### Dropped JoinHandles on Long-Running Event Bridges
* **File**: `crates/op-web/src/state.rs:267`
* **File**: `crates/op-web/src/state.rs:297`
* **File**: `crates/op-web/src/state.rs:339`
* **Severity**: Low / Quality
* **Description**: 
  The application orchestrates background metrics monitoring and gRPC updates/events by spawning detached tasks and dropping their `JoinHandle` values:
  ```rust
  tokio::spawn(async move { ... }); // Returns JoinHandle which is silently discarded
  ```
  If a panic occurs within the background monitors or connection-recovery loops, the tasks terminate silently with no notification, logging, or self-healing triggers reaching the main server control plane.

#### Discarded JoinHandle on Streaming Chat Processing
* **File**: `crates/op-web/src/handlers/chat.rs:112`
* **Severity**: Low / Quality
* **Description**: 
  A background chat processor task is spawned, but its `JoinHandle` is discarded. If the HTTP client aborts or terminates the connection mid-processing, the spawned task continues executing expensive LLM calls and tool pipelines until completion, wasting resources.

#### Discarded Event Forwarder Task Handle in WebSocket Loop
* **File**: `crates/op-web/src/websocket.rs:132`
* **Severity**: Low / Quality
* **Description**: 
  An event forwarding task is spawned inside `handle_socket` to multiplex events back to the client:
  ```rust
  tokio::spawn(async move {
      while let Some(event) = event_rx.recv().await { ... }
  });
  ```
  Dropping this handle prevents explicit shutdown coordination. When the main loop finishes, the task relies on channel teardown to exit, which is fragile and hard to monitor.