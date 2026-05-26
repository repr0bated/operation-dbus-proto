# Production Security and Quality Audit: op-web

## 1. Tests Assessment

### Total Test Functions
A total of **17** test functions were identified across the codebase.

### Representative Tests
1. **`crates/op-web/src/privacy_container.rs:262`**
   ```rust
   #[test]
   fn generates_container_name_from_uuid() { ... }
   ```
2. **`crates/op-web/src/privacy_openflow.rs:328`**
   ```rust
   #[test]
   fn merge_routes_keeps_non_privacy_flows_and_replaces_managed_ones() { ... }
   ```
3. **`crates/op-web/src/orchestrator/anti_hallucination.rs:205`**
   ```rust
   #[test]
   fn test_detects_ovs_vsctl() { ... }
   ```

### Property Testing & Fuzzing
* **No property tests** (such as `proptest` or `quickcheck` harnesses) were found.
* **No fuzzing targets** (such as `cargo-fuzz` or `libfuzzer` integrations) were found in the reviewed source code.

---

## 2. Security & Quality Findings

### Critical Severity

#### [Critical] Path Traversal and Arbitrary File Write in Transcript Handler
* **File:** `crates/op-web/src/handlers/chat.rs:378` and `crates/op-web/src/handlers/chat.rs:487`
* **Vulnerability Type:** Path Traversal / Arbitrary File Write
* **Description:** The `save_transcript_handler` extracts the `filename` parameter directly from user-controlled JSON input without any sanitization or validation. This value is subsequently formatted into `format!("/tmp/{}", filename)` and written to the filesystem via `tokio::fs::write`.
* **Exploitability:** An attacker can provide a relative path traversal sequence (e.g., `../../etc/cron.d/malicious` or `../../home/user/.ssh/authorized_keys`) to write arbitrary content anywhere on the filesystem, matching the privileges of the executing server process (which runs with high privileges to execute system commands).

#### [Critical] Insecure IP Trust leading to Localhost Security Bypass
* **File:** `crates/op-web/src/middleware/security.rs:69-93`
* **Vulnerability Type:** Security Zone Bypass / IP Spoofing
* **Description:** The `extract_ip` function checks and returns the `X-Forwarded-For` and `X-Real-IP` HTTP headers before falling back to the socket connection address. Because `op-web` binds to `0.0.0.0` by default and does not validate whether these proxy headers originate from a trusted upstream gateway, any client can spoof their source IP.
* **Exploitability:** An external attacker can send a request containing `X-Forwarded-For: 127.0.0.1` to be classified under the `Localhost` or `TrustedMesh` security zones. This bypasses administrative restrictions on endpoints like `/groups-admin` and allows unauthorized execution of sensitive tool groups.

---

### High Severity

#### [High] Multi-byte UTF-8 Slicing Panic (Denial of Service)
* **File:** `crates/op-web/src/orchestrator/anti_hallucination.rs:141-152`
* **Vulnerability Type:** Denial of Service (Panic)
* **Description:** The `extract_context` function retrieves a byte offset (`pos`) from `content.find(...)` and subsequently slices the string using byte boundaries: `&content[start..end]`. Because `content` is a standard Rust `str` containing UTF-8 characters, slicing at arbitrary byte indexes that do not align with character boundaries will cause a thread panic.
* **Exploitability:** If the LLM generates output containing multi-byte Unicode characters (e.g., emojis, Cyrillic, or CJK characters) near a forbidden command, the slicing operation will panic. This crashes the request handler and, if repeated, results in a persistent denial of service.

#### [High] Hardcoded API Bypass Keys in Production Code
* **File:** `crates/op-web/src/middleware/security.rs:13-16`
* **Vulnerability Type:** Hardcoded Credentials
* **Description:** The array `BYPASS_API_KEYS` contains hardcoded static API keys:
  ```rust
  const BYPASS_API_KEYS: &[&str] = &[
      "4f8c2b5d-9a1e-4b7c-8d2f-3a6b5c9e4d1f", // Primary MCP access key
      "test-key-huggingface-2024",            // Hugging Face test key
  ];
  ```
  These keys bypass all IP security restrictions and immediately upgrade the connection to `AccessZone::TrustedMesh` (full administrative access).
* **Exploitability:** Attackers utilizing the known Hugging Face test key or the primary access key can trivially gain administrative privileges over the system without possessing valid cryptographic credentials.

#### [High] Broken POST Message Routing in Smart Router
* **File:** `crates/op-web/src/mcp_smart_router.rs:82` and `crates/op-web/src/mcp_smart_router.rs:92`
* **Vulnerability Type:** Functional Defect / Request Dropping
* **Description:** In `smart_mcp_handler`, when forwarding incoming JSON-RPC POST requests to either the compact or agents sub-routers, the handler discards the actual incoming request body and instead passes a hardcoded null value:
  ```rust
  crate::mcp_compact::mcp_compact_message_handler(
      axum::extract::Json(serde_json::Value::Null)
  ).await.into_response()
  ```
* **Impact:** This completely breaks JSON-RPC functionality when accessed via the smart router, as the forwarded requests will always arrive with null payloads, rendering the router non-functional for POST-based MCP clients.

---

### Medium Severity

#### [Medium] CSRF Memory Cache Wipe (Denial of Service)
* **File:** `crates/op-web/src/handlers/privacy.rs:596-599`
* **Vulnerability Type:** Resource Management / Denial of Service
* **Description:** When the stored Google OAuth CSRF tokens exceed a count of 1000, the application clears the entire state mapping:
  ```rust
  if tokens.len() > 1000 {
      tokens.clear();
  }
  ```
* **Impact:** A malicious actor can easily spam 1001 authentication requests, which instantly clears the map. This revokes and invalidates all active authentications in progress for legitimate users, representing a simple Denial of Service vector against the registration flow.

#### [Medium] Plaintext WireGuard Private Key Storage
* **File:** `crates/op-web/src/handlers/privacy.rs:252`
* **Vulnerability Type:** Cryptographic Storage Flaw
* **Description:** The system generates client configurations using `user.wg_private_key_encrypted`. However, as documented by comments and implementation details, the WireGuard private key is stored unencrypted in this field:
  ```rust
  // Generate WireGuard config
  let config = generate_client_config(
      &provisioned_user.wg_private_key_encrypted, // This is the actual private key for now
      ...
  ```
* **Impact:** A compromise of the user JSON file (`/var/lib/op-dbus/privacy-users.json`) immediately exposes all client WireGuard private keys in plaintext.

---

### Low Severity & Code Quality

#### [Low] Host Header Poisoning in Discovery Endpoint
* **File:** `crates/op-web/src/mcp_discovery.rs:20-28`
* **Vulnerability Type:** Configuration Hijacking
* **Description:** The `mcp_discovery_handler` constructs the absolute redirection URLs returned by `/.well-known/mcp.json` dynamically using the untrusted `host` HTTP header supplied by the client.
* **Impact:** Clients parsing the discovery JSON may be redirected to malicious spoofed endpoints if the server is deployed without an explicit whitelist of permitted domains or reverse proxy host-rewriting configurations.

#### [Low] Unsafe Memory De-serialization via simd-json
* **File:** `crates/op-web/src/groups_admin.rs:52`
* **Vulnerability Type:** Code Quality
* **Description:** The parsing of configuration states uses `unsafe { simd_json::from_str(...) }`. While correct under standard inputs, using destructive raw parser calls with unsafe blocks when standard safe parsers (`serde_json`) are available increases the attack surface for parser-related memory safety issues if the file `/var/lib/op-dbus/tool-groups.json` is modified maliciously.

---
## ⚠ Citation Warnings
- `crates/op-web/src/handlers/chat.rs:487`: file has 453 lines
