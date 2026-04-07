# op-gateway - Tasks

## Phase 1: Authentication Core

- [ ] Implement the `Identity` trait for WireGuard-based PSK verification.
- [ ] Develop the `NonceManager` with in-memory single-use nonce tracking and expiration.
- [ ] Create a `SessionManager` using `dashmap` for concurrent session lookup.
- [ ] Implement the HKDF-based session key derivation (HKDF-SHA256).
- [ ] Add unit tests for the authentication challenge-response handshake.

## Phase 2: Encrypted Storage

- [ ] Integrate `rusqlite` with `SQLCipher` (or equivalent VFS encryption layer).
- [ ] Implement Argon2id-based master key derivation for storage encryption.
- [ ] Create the database schema for `sessions` and `registry`.
- [ ] Build a generic `PersistenceProvider` trait with methods for secure storage and retrieval.
- [ ] Verify atomic write behavior and data integrity under failure scenarios.

## Phase 3: MCP Gateway and Routing

- [ ] Implement the `McpDispatcher` to parse and route JSON-RPC 2.0 requests.
- [ ] Create a `ToolRegistry` to manage namespaces and provider mapping.
- [ ] Develop the UDS (Unix Domain Socket) and stdio transports for backend communication.
- [ ] Integrate `simd-json` for efficient and secure request parsing.
- [ ] Build a mock backend to test end-to-end request/response routing.

## Phase 4: Integration and Security

- [ ] Connect the `AuthService` with the `McpGateway` for access control.
- [ ] Implement the session token generation and verification middleware.
- [ ] Add audit logging for all authentication and routing events.
- [ ] Perform a security review of the memory management and crypto primitive usage.
- [ ] Write integration tests for the full login-to-request workflow.