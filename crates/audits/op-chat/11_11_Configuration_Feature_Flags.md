# Production Security and Quality Audit: Configuration & Schema Analysis

## 1. Environment Variable Configuration (`std::env::var` Reads)

The table below lists all reads from `std::env::var` within the `op-chat` crate. 

| Environment Variable | File & Line Citation | Fallback/Default Value | Error Handling Strategy |
| :--- | :--- | :--- | :--- |
| `OP_DBUS_GRPC_ADDR` | `crates/op-chat/src/grpc_client.rs:36` | `"http://10.200.0.2:50051"` | Handled via safe fallback using `.unwrap_or_else()`. |
| `OP_RUN_ON_CONNECTION_AGENTS` | `crates/op-chat/src/grpc_client.rs:166` | `"rust_pro,backend_architect,sequential_thinking,memory,context_manager"` | Handled via safe fallback using `.unwrap_or_else()`. |
| `OP_CHAT_LISTEN` | `crates/op-chat/src/main.rs:13` | `"0.0.0.0:50052"` | Missing keys are resolved via `.unwrap_or_else()`. A parsing error in the string representation is bubbled up to the main application context using `?`. |
| `CUSTOM_SYSTEM_PROMPT` | `crates/op-chat/src/system_prompt.rs:357` | None (checks alternative file paths) | Handled safely using `if let Ok(...)` matching; no panic if missing. |
| `OP_SELF_REPO_PATH` | `crates/op-chat/src/system_prompt.rs:433` | None | Condition evaluated using `.is_ok()`. |
| `OP_SELF_REPO_PATH` | `crates/op-chat/src/system_prompt.rs:446` | None | Safely converted to an `Option` via `.ok()`. |
| `OP_AGENT_POOL_ADDRESS` | `crates/op-chat/src/orchestration/grpc_pool.rs:93` | Keep internal default (`"http://127.0.0.1"`) | Safely evaluated using `if let Ok(...)` matching. |
| `OP_AGENT_CONNECT_TIMEOUT_MS` | `crates/op-chat/src/orchestration/grpc_pool.rs:97` | Keep internal default (`5s`) | Handled via `if let Ok(...)` matching, then parsed as `u64`. Invalid strings are silently ignored. |
| `OP_AGENT_REQUEST_TIMEOUT_MS` | `crates/op-chat/src/orchestration/grpc_pool.rs:103` | Keep internal default (`30s`) | Handled via `if let Ok(...)` matching, then parsed as `u64`. Invalid strings are silently ignored. |
| `OP_RUN_ON_CONNECTION_AGENTS` | `crates/op-chat/src/orchestration/grpc_pool.rs:109` | Keep internal default (`["rust_pro", "backend_architect", ...]`) | Handled via `if let Ok(...)` matching. |

### Environment Variables Lack of Default or Error Handling
No environment variables were found to lack defaults or error handling. All instances utilize safe standard library combinators (`unwrap_or_else`, `if let Ok(...)`, `.ok()`, `.is_ok()`) to prevent runtime panics when variables are unset or missing.

---

## 2. Cargo Features & Additivity Analysis

The `op-chat` crate depends on features defined in the root workspace `Cargo.toml`.

### Declared Features
```toml
[features]
default = ["grpc"]
grpc = []
```

### Additivity Evaluation
In Rust/Cargo, features are strictly additive. The root workspace `Cargo.toml` specifies a `default` feature containing `["grpc"]`. Enabling this feature transactively activates `grpc`. There are no competing, contradictory, or mutually exclusive configurations (such as custom `std` vs `no_std` exclusions).

---

## 3. Hardcoded Ports, Addresses, and File Paths

The following hardcoded system paths, ports, and addresses have been identified. In a secure production profile, these should be moved to dynamic, schema-backed configurations.

### 3.1 Hardcoded Network Ports & Socket Addresses
* **gRPC Server Address Fallback:** `http://10.200.0.2:50051` in `crates/op-chat/src/grpc_client.rs:37`
* **Local Loopback Base Address:** `http://127.0.0.1` in `crates/op-chat/src/orchestration/grpc_pool.rs:57`
* **Listening Socket Address:** `0.0.0.0:50052` in `crates/op-chat/src/main.rs:14`
* **Agent Port Assignments:** The following ports are statically hardcoded in `crates/op-chat/src/orchestration/grpc_pool.rs:116`:
  * `rust_pro` $\rightarrow$ `50051`
  * `backend_architect` $\rightarrow$ `50052`
  * `sequential_thinking` $\rightarrow$ `50053`
  * `memory` $\rightarrow$ `50054`
  * `context_manager` $\rightarrow$ `50055`
  * `python_pro` $\rightarrow$ `50056`
  * `debugger` $\rightarrow$ `50057`
  * `mem0` $\rightarrow$ `50058`
  * `search_specialist` $\rightarrow$ `50059`
  * `deployment` $\rightarrow$ `50060`

### 3.2 Hardcoded Static File Paths & System Sockets
* **Custom Prompt Locations:** `"/etc/op-dbus/custom-prompt.txt"`, `"./custom-prompt.txt"`, `"../custom-prompt.txt"` in `crates/op-chat/src/system_prompt.rs:17-19` and `system_prompt.rs:412`
* **OVSDB Socket Location:** `/var/run/openvswitch/db.sock` in `crates/op-chat/src/system_prompt.rs:131` and `system_prompt.rs:238`
* **D-Bus System Socket:** `/var/run/dbus/system_bus_socket` in `crates/op-chat/src/system_prompt.rs:239`
* **Netmaker Socket:** `/var/run/netclient/netclient.sock` in `crates/op-chat/src/system_prompt.rs:240`
* **Network Interface State Paths:** `/sys/class/net`, `/sys/class/net/{}/operstate`, `/sys/class/net/{}/address` in `crates/op-chat/src/tool_loader.rs:535`, `tool_loader.rs:545`, and `tool_loader.rs:552`
* **Systemd Manager Paths:** `/org/freedesktop/systemd1` and `org.freedesktop.systemd1.Manager` in `crates/op-chat/src/tool_loader.rs:599-600` and `tool_loader.rs:835-836`
* **Hardcoded Sensitive Path Blacklist:** `/etc/shadow`, `/etc/sudoers` in `crates/op-chat/src/tool_loader.rs:414`
* **Hardcoded Sensitive Directory Prefix Restrictions:** `/etc/`, `/boot/`, `/sys/`, `/proc/` in `crates/op-chat/src/tool_loader.rs:494`

### 3.3 Hardcoded System Namespaces & Services
* **D-Bus Orchestration Service:** `com.system.orchestrator` in `crates/op-chat/src/orchestration/dbus_orchestrator.rs:34`
* **D-Bus Orchestration Path:** `/com/system/orchestrator/Manager` in `crates/op-chat/src/orchestration/dbus_orchestrator.rs:37`
* **D-Bus Orchestration Interface:** `com.system.orchestrator.Manager` in `crates/op-chat/src/orchestration/dbus_orchestrator.rs:40`
* **Agent D-Bus Namespace:** `com.system.agents.{}` in `crates/op-chat/src/orchestration/dbus_orchestrator.rs:140`

### 3.4 Hardcoded Target Topology (System Prompt Specifications)
The following IP ranges and topologies are hardcoded inside the immutable prompt block in `crates/op-chat/src/system_prompt.rs`:
* **External Gateway IP:** `80.209.240.244/24` and `80.209.240.1` (Line 161)
* **Unified Switch Datapath IP:** `10.0.0.1/16` (Line 174)
* **VLAN Subnets:** `10.100.0/24`, `10.200.0/24`, `10.30.0/24`, `10.50.0/24` (Lines 184-187)
* **Allocations / Ranges:** `10.100.0.128/25`, `10.100.0.129` (Lines 213-220)
* **Overlay/VPN IPS:** `10.50.0.129/25` (Line 224 & Line 248)
* **WireGuard Port & MTU:** Port `51820/UDP`, MTU `1420` (Line 203 & Line 248)

---

## 4. Schema-as-Code Compliance Audit

The codebase violates the Schema-as-Code discipline in several modules by defining critical data contracts and system configuration schemas as ad-hoc, untyped Rust structures or JSON strings rather than formally versioned schemas (e.g., Protocol Buffers or OSCAL specifications).

* **Ad-hoc RPC Request and Response Models:**
  * `crates/op-chat/src/actor.rs:52` & `104`: The `RpcRequest` and `RpcResponse` data contracts are expressed directly as ad-hoc Serde JSON payload representations. While some orchestration gRPC services utilize standard Protobuf definitions, these entry-point payloads bypass versioned schemas.
* **Ad-hoc Skill and Constraint System:**
  * `crates/op-chat/src/orchestration/skills.rs:51`: The `Skill`, `SkillContext`, and `SkillConstraint` types represent complex structural schemas used to modify active system state, but they are defined strictly as ad-hoc Rust structs.
* **Ad-hoc Workflow Execution Plans:**
  * `crates/op-chat/src/orchestration/workflows.rs:61`: The `Workflow` and `WorkflowStep` structures are declared inside Rust files with inline Serde annotations, lacking a versioned, language-agnostic schema.
* **Ad-hoc Workstack and Phase Configurations:**
  * `crates/op-chat/src/orchestration/workstacks.rs:62`: `Workstack` and its metadata configurations use ad-hoc serializations without formal validation against versioned schemas.
* **Ad-hoc Session State Storage:**
  * `crates/op-chat/src/session.rs:11`: The `ChatSession` struct represents the data schema for persistent chat, authentication details, and gateway context, but it is expressed as an ad-hoc struct with no versioned schema.