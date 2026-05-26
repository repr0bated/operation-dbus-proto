# Production Security & Quality Audit: op-mcp-proxy

## 1. Architecture & Module Map

### Overview
The `op-mcp-proxy` crate is a system proxy that acts as a bridge between local clients and both downstream services (via DBus/gRPC) and upstream Cloud AI APIs (Google Cloud Code Assist / Vertex AI). It handles session management based on WireGuard public keys, manages Google Cloud OAuth token acquisition/refresh flows, reads structured identity information from a shared memory segment, and exposes an optional OpenAI-compatible HTTP completions server.

### Module Tree
The entry point `main.rs` defines the top-level module structure as follows:

*   **`main.rs`** (Binary entry point)
    *   `cloudaicompanion` (`cloudaicompanion.rs`): Handles raw HTTP communications with the Google Cloud Code Assist endpoint, including bootstrapping, tier resolution, onboarding, and Gemini CLI token interactions.
    *   `direct_llm` (`direct_llm.rs`): Direct-mode prompt extractor and dispatcher that wraps `CloudAICompanion` and implements background OAuth token auto-refresh loops.
    *   `gcloud_auth` (`gcloud_auth.rs`): Authentication coordinator that chains local cache files, VSCode extension credentials, gcloud CLI configurations, and Application Default Credentials (ADC).
    *   `http_server` (`http_server.rs`): Exposes an OpenAI-compatible web API (`/v1/chat/completions`) using Axum, integrating custom rate-limiting and mapping requests to Vertex AI gRPC or CloudAI.
    *   `session` (`session.rs`): Implements persistent session tracking and WireGuard peer-to-user mappings using an embedded SQLite database.
    *   `sled` (`sled.rs`): Implements low-level zero-copy parsing of an external metadata struct residing in memory-mapped shared storage (`/dev/shm`).
    *   `vertex_grpc` (`vertex_grpc.rs`): Contains a tonic-based gRPC client implementation for communicating directly with Google Vertex AI's prediction service.

### Entry Points
*   **`crates/op-mcp-proxy/src/main.rs`**: Main process entry point. It initializes logger contexts, parses memory-mapped parameters from the shared identity segment, boots up the local SOCKS proxy routing, registers background auto-refresh tasks, spawns the HTTP completions server if configured, and starts the standard input/output (stdio) JSON-RPC listening loop.

---

## 2. Security & Quality Findings

### [CRITICAL] Rate Limiter Thundering Herd and Complete Throttling Bypass
*   **Citation**: `crates/op-mcp-proxy/src/http_server.rs:149-162`
*   **Vulnerability Type**: API Rate Limit Bypass / Resource Exhaustion
*   **Description**: The OpenAI-compatible HTTP server utilizes a shared `TokenBucket` state to rate-limit clients based on configured Requests Per Minute (RPM). When a burst of requests arrives and exhausts the bucket, `try_consume` returns a `Duration` specifying how long the client must wait. If this wait duration is under 5 seconds, the server logs a warning and yields execution via `tokio::time::sleep(delay).await;`.

    However, once the sleep finishes, **the handler immediately proceeds to generate the LLM response without re-trying to consume a token or deducting it from the bucket**.
*   **Exploitation**: An attacker or a high-concurrency client can send hundreds of concurrent requests simultaneously. Request 1 succeeds and drains the bucket. Requests 2 through 1,000 will fail `try_consume`, calculate a sub-second wait time, sleep concurrently, and then wake up *simultaneously* to blast the upstream Vertex AI gRPC or CloudAI endpoints. This completely bypasses the rate limiter, allowing immediate thundering-herd resource exhaustion, upstream rate-limit blocking, and unbounded API billing charges on the associated Google Cloud platform project.
*   **Remediation**: Transition the rate-limiting mechanism to use a proper scheduling middleware like `tower_governor` (available in the workspace dependencies). Alternatively, restructure the rate-limiter so that a token is actually acquired *after* waking up from sleep, or block/queue requests within the mutex guard rather than bypassing the consumption check entirely.

---

### [HIGH] Undefined Behavior & SIGBUS Crash via Unsafe Memory-Mapped Shared Memory Sled
*   **Citation**: `crates/op-mcp-proxy/src/sled.rs:38-44`
*   **Vulnerability Type**: Concurrency / Memory Safety
*   **Description**: The zero-copy memory mapping mechanism in `SledSnapshot::read()` accesses `/dev/shm/plugin_schema.dat` via `memmap2` using `unsafe`. Rust’s references require that the underlying memory data does not change concurrently in a way that violates aliasing rules. 
    
    Because `/dev/shm/plugin_schema.dat` is a shared-memory file on a tmpfs partition, other processes can truncate or modify the file concurrently. If the file is truncated to a size smaller than `SLED_SIZE` while the proxy is holding a slice reference, any access to `bytes` will trigger a hardware page fault and immediately terminate the daemon with a `SIGBUS` signal. Additionally, non-atomic reads from memory modified concurrently by another process constitute a data race, leading to undefined behavior in the Rust runtime.
*   **Exploitation**: Any unprivileged local process with access to `/dev/shm/` can truncate `/dev/shm/plugin_schema.dat` to 0 bytes, instantly crashing the `op-mcp-proxy` service and creating a persistent local Denial of Service (DoS) vulnerability.
*   **Remediation**: Avoid raw memory-mapping of shared files for IPC metadata unless robust synchronization (such as POSIX robust mutexes or file locking) is guaranteed. Transition to safe file-reading (`std::fs::read`) or copy the file atomically into memory, ensuring length validation is performed before parsing.

---

### [MEDIUM] Ad-Hoc Binary Data Contract Mapping (Schema-as-Code Violation)
*   **Citation**: `crates/op-mcp-proxy/src/sled.rs:48-55`
*   **Vulnerability Type**: Schema-as-Code Compliance & Maintainability
*   **Description**: The zero-copy snapshot implementation parses the binary layout of the sled file using hardcoded slice indices (`bytes[32..40]`, `bytes[96..160]`, etc.). This layout is asserted to match a `#[repr(C)]` structure from `op-identity::schema_bridge` but is defined entirely as ad-hoc code. 
    
    There is no compilation dependency, type verification, or schema-driven generation (such as Protocol Buffers or versioned structs) connecting these two crates. If the layout of the sled file changes in `op-identity`, `op-mcp-proxy` will compile successfully but silently extract garbage values (e.g., misinterpreting the NextDNS profile as a subid or wireguard pubkey), resulting in silent failures and incorrect traffic routing.
*   **Remediation**: Do not use ad-hoc slice offsets for inter-process data contracts. Define the sled data payload using a versioned Protocol Buffer schema, or share a single, version-pinned struct definition directly from a common library crate.

---

### [LOW] JSON-RPC Protocol Compliance Violation for Client IDs
*   **Citation**: `crates/op-mcp-proxy/src/main.rs:149-151`
*   **Vulnerability Type**: Protocol Compliance / Interoperability
*   **Description**: In the JSON-RPC proxy routing loop, the request `id` field is parsed via `req["id"].as_str()`. However, the JSON-RPC 2.0 specification allows the `id` member to be a structured string, an integer, or null. 

    If an incoming request contains an integer ID (e.g., `{"jsonrpc": "2.0", "id": 105, "method": "..."}`), `as_str()` returns `None`, defaulting the gRPC message ID to `"null"`. The client will receive a response with `id: "null"`, failing type matching. Many strictly compliant JSON-RPC and MCP clients will immediately drop these mismatched responses, leading to hanging client loops.
*   **Remediation**: Preserve the raw `simd_json::OwnedValue` or serialize the raw type representation of the `id` parameter directly through to the response, rather than forcing an ad-hoc string coercion.

---

### [LOW] Ad-Hoc Embedded SQL Schema String (Schema-as-Code Violation)
*   **Citation**: `crates/op-mcp-proxy/src/session.rs:53-76`
*   **Vulnerability Type**: Schema-as-Code Compliance
*   **Description**: The session database schema is declared in-line as an ad-hoc SQL string batch within `session.rs`. This design lacks a formal migrations system, version tracking, and schema contract definition, leading to potential data corruption if table definitions evolve across software versions.
*   **Remediation**: Define the database schemas as versioned migrations managed by SQLx or structured migration scripts, ensuring deterministic updates.

---

### [LOW] Ad-Hoc OpenAI Struct Declarations (Schema-as-Code Violation)
*   **Citation**: `crates/op-mcp-proxy/src/http_server.rs:47-97`
*   **Vulnerability Type**: Schema-as-Code Compliance
*   **Description**: The request and response structures representing the OpenAI API contract (`ChatCompletionRequest`, `ChatMessage`, `ChatCompletionResponse`, etc.) are declared as local, ad-hoc Rust structs in the HTTP server implementation file. They are not mapped to a versioned API schema or defined via a shared contract definition.
*   **Remediation**: Extract these structures into a dedicated schema crate, or generate them from an OpenAPI specification to ensure strict contract-to-code alignment.

---

### [LOW] Lack of Rate Limiting on Stdio MCP Interface
*   **Citation**: `crates/op-mcp-proxy/src/main.rs:93-144`
*   **Vulnerability Type**: Denial of Service
*   **Description**: While the HTTP completions server implements a (bypassable) rate limiter, the standard input/output loop has no throttling. A malicious or malfunctioning IDE extension could flood the stdio channel, triggering massive direct API calls via `direct_llm` and rapidly exhausting the user's Google Cloud quota or causing unexpected API charges.
*   **Remediation**: Apply a token-bucket rate limiter to the stdio message processor loop.