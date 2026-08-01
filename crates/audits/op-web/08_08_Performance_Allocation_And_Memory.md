# PRODUCTION SECURITY & QUALITY AUDIT: OP-WEB

---

## 1. CRITICAL FINDINGS

### Arbitrary File Write & Path Traversal via Unsanitized `filename` in Chat Transcript Handler
- **File**: `crates/op-web/src/handlers/chat.rs:445` (and parsed at line `608` in `save_transcript_to_file`)
- **Vulnerability Type**: Path Traversal / Arbitrary File Write (CWE-22 / CWE-23)
- **Vulnerability Description**: 
  In `save_transcript_handler`, the server extracts a `filename` directly from user-controlled JSON parameters without any validation:
  ```rust
  let filename = params
      .get("filename")
      .and_then(|v| v.as_str())
      .map(str::to_string)
      .unwrap_or_else(|| format!("chat-transcript-{}.txt", chrono::Utc::now().timestamp()));
  ```
  The unsanitized string is then formatted directly into a path under `/tmp` inside `save_transcript_to_file`:
  ```rust
  let filepath = format!("/tmp/{}", filename);
  match tokio::fs::write(&filepath, &transcript).await {
  ```
  If an attacker passes a relative path such as `../etc/cron.d/malicious_job` or `../root/.ssh/authorized_keys`, they can write arbitrary text (containing chat message inputs they control) directly to sensitive system locations.
- **Exploitability**: Directly exploitable. The `/api/chat/transcript` route is exposed via public routes without any authentication guards in `crates/op-web/src/routes/mod.rs:56`.

---

## 2. HIGH & MEDIUM SEVERITY FINDINGS

### Unsafe `simd_json::from_str` Invocation on Unpadded Memory
- **File**: 
  - `crates/op-web/src/groups_admin.rs:52`
  - `crates/op-web/src/state_manager_client.rs:38`
  - `crates/op-web/src/users.rs:77`
  - `crates/op-web/src/websocket.rs:135`
  - `crates/op-web/src/handlers/websocket.rs:74`
  - `crates/op-web/src/orchestrator/parsing.rs:32`, `68`, `93`, `118`
- **Vulnerability Type**: Undefined Behavior / Out-of-bounds Read (CWE-125)
- **Vulnerability Description**: 
  `simd-json` requires that input string slices are padded with at least `simd_json::PADDING_SIZE` bytes of scratch space. This padding allows vector instructions to read chunks of memory safely without triggering page faults. 
  The codebase repeatedly invokes `unsafe { simd_json::from_str(&mut raw) }` on standard `String` and `&mut str` buffers cloned directly from `std::fs::read_to_string`, websocket messages, or D-Bus proxy results. None of these strings are guaranteed to have the required SIMD padding, leading to undefined behavior and potential memory access violations under load.
- **Remediation**: Use `simd_json::from_slice` on a padded vector, or allocate padded memory explicitly via `simd_json::to_padded_bin` / `from_str_padded`.

### Hardcoded Bypass API Keys committed to Source Code
- **File**: `crates/op-web/src/middleware/security.rs:18`
- **Vulnerability Type**: Hardcoded Credentials (CWE-798)
- **Vulnerability Description**: 
  The security middleware contains statically committed API keys used for bypassing IP checks:
  ```rust
  const BYPASS_API_KEYS: &[&str] = &[
      "4f8c2b5d-9a1e-4b7c-8d2f-3a6b5c9e4d1f", // Primary MCP access key
      "test-key-huggingface-2024",            // Hugging Face test key
  ];
  ```
  Any client sending these headers automatically receives the highest permission level (`AccessZone::TrustedMesh`), completely rendering network layout protections useless.

### Plaintext Storage of WireGuard Private Keys
- **File**: `crates/op-web/src/handlers/privacy.rs:180`
- **Vulnerability Type**: Insecure Storage of Sensitive Information (CWE-312)
- **Vulnerability Description**: 
  As admitted in the source comments (`// Create user (we'll encrypt the private key later, for now just store it)`), the raw WireGuard private key is stored unencrypted in the `wg_private_key_encrypted` user struct field. This is written in plaintext to `/var/lib/op-dbus/privacy-users.json` using the standard `tokio::fs::write` umask. If the directory permissions are lax, any local process can read and steal these private keys.

### Authentication Bypass & DoS in CSRF Token Map Cleanup
- **File**: `crates/op-web/src/handlers/privacy.rs:431`
- **Vulnerability Type**: Denial of Service via Map Clearing (CWE-400)
- **Vulnerability Description**: 
  In the Google OAuth initiation, the server mitigates map bloat by wiping *all* stored CSRF tokens if the count exceeds 1000:
  ```rust
  if tokens.len() > 1000 {
      tokens.clear();
  }
  ```
  An attacker can easily flood the auth endpoint with 1001 requests to trigger `tokens.clear()`, immediately invalidating every active, legitimate user session currently in the login pipeline.

### Unauthenticated D-Bus PTY Auth Bridge Endpoints
- **File**: `crates/op-web/src/handlers/auth_bridge.rs:64`
- **Vulnerability Type**: Missing Authentication (CWE-306)
- **Vulnerability Description**: 
  The routes defined in `auth_bridge_routes` (such as `/api/auth-bridge/pending` and `/api/auth-bridge/:id/complete`) are exposed directly to the network without any middleware auth checks. Anyone on the network can view system terminal authorization codes, OAuth URLs, or complete pending local administrative actions.

---

## 3. SCHEMA-AS-CODE DISCIPLINE VIOLATIONS

The project defines critical domain boundaries, configuration payloads, and service communication states using ad-hoc, unversioned JSON structures. Rather than compiling unified Protocol Buffer schemas or using OSCAL schemas for system assessment, the data contracts are hand-written Rust structs containing string types, exposing the control plane to parsing drift and data mismatch vulnerabilities.

| Data Contract | File:Line | Ad-Hoc Struct/String | Recommended Schema-as-Code Remediation |
| :--- | :--- | :--- | :--- |
| **Tool Groups Configuration** | `groups_admin.rs:32` | `EnabledGroups` struct serialized to unversioned `/var/lib/op-dbus/tool-groups.json` | Model via versioned Protocol Buffers; generate deserializers statically. |
| **MCP Agent Selection** | `mcp_agents.rs:114` | `AgentSelectionConfig` parsed directly via JSON slice | Define structured agent capability profiles using Protocol Buffers or JSON Schema. |
| **Incus Instance Representation** | `privacy_container.rs:32` | `IncusInstance` capturing system hypervisor desires | Use explicit schema bindings matching the Incus REST/D-Bus API contracts. |
| **OpenFlow Bridge Config** | `privacy_openflow.rs:20` | `OpenFlowConfig` and `BridgeFlowConfig` | Express as a unified networking schema or standardized config proto. |
| **Privacy VPN Route State** | `privacy_routes.rs:20` | `PrivacyRoute` containing loose String parameters | Schema-define using Protobuf definitions with exact network types. |
| **User Store Audit Ledger** | `users.rs:14` | `PrivacyUser` holding loose configuration fields | Codify user profiles and cryptography blocks under schema-as-code versioning. |
| **LLM Chat Request** | `handlers/chat.rs:25` | `ChatRequest` containing optional unvalidated strings | Model as unified Protobuf message specifications. |
| **Dashboard Telemetry Metrics** | `handlers/dashboard.rs:14` | `DashboardMetrics` with loose floats | Standardize on open compliance telemetry schemas (e.g. OpenTelemetry schemas). |

---

## 4. PERFORMANCE, ALLOCATION & MEMORY MAP

### Loop Allocations & Inefficiencies
1. **`Vec::new` inside loop**:
   - `crates/op-web/src/groups_admin.rs:252` (`list_groups`): Generates a `Vec` for every unique domain inside the loop via `.entry().or_default()`. Pre-allocate based on expected domain size.
   - `crates/op-web/src/groups_admin.rs:311`: Allocates nested `json!` maps during iterations on domains.
2. **`String::new` / `collect()` allocations**:
   - `crates/op-web/src/privacy_container.rs:188` (`user_suffix`): Allocates a new `String` dynamically per character iterator call without capacity.
3. **Spawning command tail in request handler**:
   - `crates/op-web/src/handlers/logs.rs:40` (`logs_handler`): Spawns multiple `tail` processes via `Command::new` inside a request loop. This creates severe CPU bottlenecks and process limit exhaustion risks.
4. **Log parser string allocations inside hot path**:
   - `crates/op-web/src/handlers/logs.rs:65` (`parse_logs`): Inside the `.lines()` loop, allocates `id`, `timestamp`, `level`, `component`, and `message` strings for every single line of log text processed.

### Hot-Path `format!()` Counter
- `crates/op-web/src/handlers/logs.rs:69`: `format!("{}-{}", component, i)` called on every parsed log line.
- `crates/op-web/src/handlers/status.rs:244` / `250`: `format!("/sys/class/net/{}/operstate", name)` called per interface on every status poll.
- `crates/op-web/src/orchestrator/formatting.rs:29`: `format!("✅ **{}**\n", r.name)` formatted iteratively inside rendering loops.
- `crates/op-web/src/orchestrator/formatting.rs:68`: `format!("  • **{}**: {}\n", key, formatted_value)` called recursively for every payload element.

### `simd_json::OwnedValue` Clones
- `crates/op-web/src/mcp_compact.rs:434`: `res.clone()` deep-clones potentially massive execution responses.
- `crates/op-web/src/state.rs:317`: `parsed_obj.get("result").cloned()` replicates memory buffers representing external payload returns.
- `crates/op-web/src/state_manager_client.rs:42`: `existing.clone()` deep-copies serialized configuration states.

---

### Memory Map Table

No direct uses of raw system memory mapping (`memmap2`, `mmap`, `MmapMut`) were found inside the audited files. However, the crate relies on `cozo` which links to `sled`. Sled memory-maps databases internally.

| Site | file:line | Type | Risk |
| :--- | :--- | :--- | :--- |
| **cozo (sled)** | `Cargo.toml` | sled (Internal mmap) | Sled utilizes writable memory-maps. If databases are mounted on `tmpfs` or `noexec` directories, operations may fail or crash during page flushes. |