### Critical Vulnerability: Authentication Bypass via Placebo IP Security Middleware
* **Citation**: `crates/op-web/src/middleware/security.rs:109`
* **Impact**: Critical / Remote Code Execution (RCE)
* **Description**: The `ip_security_middleware` extracts client IP information and associates an `AccessZone` (such as `Localhost`, `TrustedMesh`, or `Public`) with the request extensions. However, it never actually rejects unauthorized requests (it always blindly returns `next.run(request).await`). Furthermore, none of the system-level API or route handlers (under `/api/...` or `/groups-admin/...`) retrieve, inspect, or enforce the attached `AccessZone` extension. The security boundaries are purely enforced client-side in the JavaScript UI.
* **Exploit Scenario**: An unauthenticated attacker can bypass the client-side UI entirely and call `POST /api/tool` or `POST /api/tools/:name/execute` to execute arbitrary system d-bus, Open vSwitch, or shell-execution commands on the host as root.

---

### Critical Vulnerability: Hardcoded Static Bypass Credentials (Backdoor Keys)
* **Citation**: `crates/op-web/src/middleware/security.rs:16`
* **Impact**: Critical
* **Description**: The security middleware contains hardcoded static API keys in the `BYPASS_API_KEYS` slice, including `"4f8c2b5d-9a1e-4b7c-8d2f-3a6b5c9e4d1f"` and `"test-key-huggingface-2024"`. If any request contains these keys in the `x-api-key`, `Authorization`, or `x-op-mcp-token` headers, the client is automatically granted `TrustedMesh` (full administrative) access.
* **Exploit Scenario**: An attacker sending requests with the header `x-api-key: test-key-huggingface-2024` automatically bypasses IP restriction classification logic.

---

### Critical Vulnerability: Arbitrary File Write via Path Traversal in Chat Transcript Saving
* **Citation**: `crates/op-web/src/handlers/chat.rs:326`
* **Impact**: Critical / Remote Code Execution (RCE)
* **Description**: The `save_transcript_handler` extracts the `filename` parameter directly from user-controlled JSON body parameters without performing any validation or sanitization against path traversal (e.g., checking for `../`). This filename is formatted directly as `/tmp/{filename}` and written to the filesystem using `tokio::fs::write`.
* **Exploit Scenario**: An unauthenticated attacker can send a POST request to `/api/chat/transcript` with a payload of `{"filename": "../../etc/cron.d/malicious_job", "messages": [{"role": "user", "content": "* * * * * root command_to_execute"}]}`. Since the server runs with root-level privileges to modify system configurations, the attacker can write arbitrary payloads to system directories and achieve full host takeover.

---

### Critical Vulnerability: Information Disclosure of plaintext WireGuard Client Private Keys
* **Citation**: `crates/op-web/src/handlers/privacy.rs:247`
* **Impact**: Critical
* **Description**: The `/privacy/access` endpoint renders a human-readable webpage containing the full WireGuard configuration profile for a given `user_id`. The configuration includes the plaintext client private key, which is stored in the database field `wg_private_key_encrypted` (noted in the code as: `This is the actual private key for now`).
* **Exploit Scenario**: Because the `/api/users` endpoint (mapped at `crates/op-web/src/routes/mod.rs:55`) is completely public and unauthenticated, any user can fetch a list of all user UUIDs. An attacker can iterate over these UUIDs and query `/privacy/access?user_id=<UUID>` to harvest the WireGuard private keys for all clients in the privacy network.

---

### High Vulnerability: Unauthenticated Modification of User API Keys
* **Citation**: `crates/op-web/src/handlers/privacy.rs:366`
* **Impact**: High
* **Description**: The `/api/privacy/credentials` endpoint allows clients to configure or overwrite API keys (Gemini, Anthropic, OpenAI, etc.) for any user ID. This endpoint has no authentication guards, and the `user_id` values can be trivially retrieved from the public `/api/users` endpoint.
* **Exploit Scenario**: An attacker can overwrite the API keys of any registered user to hijack their LLM allocation, or extract existing tokens if the keys are ever returned by subsequent unsecured endpoints.

---

### Medium Vulnerability: Denial of Service via CSRF Token Eviction
* **Citation**: `crates/op-web/src/handlers/privacy.rs:428`
* **Impact**: Medium / Denial of Service (DoS)
* **Description**: The Google OAuth state-tracking memory map (`csrf_tokens`) has an aggressive eviction heuristic: if the total number of cached tokens exceeds 1000, it calls `tokens.clear()`, wiping *every* active token from memory.
* **Exploit Scenario**: An attacker can script 1001 concurrent or fast-sequential requests to `/api/privacy/google/auth`. This immediately empties the entire CSRF token cache, breaking and invalidating the active login/callback sequence for all legitimate users currently authenticating.

---

### Medium Vulnerability: Placebo Anti-Hallucination Safe Command Filter
* **Citation**: `crates/op-web/src/orchestrator/process.rs:368`
* **Impact**: Medium
* **Description**: The orchestrator's anti-hallucination logic checks the assistant's text output for forbidden patterns (e.g. `rm -rf`, `ovs-vsctl`) and appends violations to `all_forbidden`. However, the orchestrator still proceeds to parse and execute any tool calls within that same turn anyway. Furthermore, the filter only inspects the generated conversational content, failing to analyze the structured JSON arguments passed to tools like `shell_exec` or `execute_tool`.
* **Exploit Scenario**: If an LLM is manipulated via prompt injection to execute `execute_tool("shell_exec", {"command": "rm -rf /"})`, the filter fails to intercept the command block, and the orchestrator executes the command without raising a safety error.

---

### Low Quality: Hardcoded Home Directory Paths in Prompt Loader
* **Citation**: `crates/op-web/src/system_prompt_loader.rs:16`
* **Description**: The loader defines fallback search paths targeting specific absolute paths inside a developer's home directory (e.g., `/home/jeremy/git/gemini-op-dbus/...`). This is a security and configuration risk, disclosing local developer structures and potentially allowing local privilege escalation if another user on a multi-user system controls that path.

---

### Low Quality: Placebo "Trusted Networks" Stored in Memory
* **Citation**: `crates/op-web/src/groups_admin.rs:43`
* **Description**: The administrative interface allows configuring "Trusted Networks", which are held in an in-memory `RwLock<Vec<String>>` inside `GroupsConfig`. These prefixes are completely discarded upon server restart, and they are never referenced by any firewall, middleware, or access-control evaluation logic in the entire codebase.

---

### Low Quality: Non-Atomic Groups Configuration File Writes
* **Citation**: `crates/op-web/src/groups_admin.rs:114`
* **Description**: `GroupsConfig::save_to_disk` uses `tokio::fs::write` to overwrite `/var/lib/op-dbus/tool-groups.json` directly. If the application crashes, the host loses power, or multiple threads execute write operations concurrently, the file can be partially written, resulting in JSON corruption. It should instead write to a temporary file in the same directory and perform an atomic rename.