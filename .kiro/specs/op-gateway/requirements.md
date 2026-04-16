# op-gateway - Requirements

## Problem Statement

The system needs a secure entry point (Gateway) that handles authentication via WireGuard, provides a unified Model Context Protocol (MCP) interface, and securely manages persistent state using encrypted storage.

## Goals

1.  **Secure Authentication**: Use WireGuard identity (PSK) for user authentication.
2.  **Smart Routing**: Act as an MCP gateway, routing requests to appropriate backend services.
3.  **Encrypted Persistence**: Securely store sensitive configuration and session data.
4.  **Zero-Trust Model**: PSK as identity, non-rotating PSK combined with server nonces for session security.

## Functional Requirements

### FR1: WireGuard Authentication
- Use WireGuard PSK as the primary user identity.
- Implement a challenge-response mechanism using server nonces to prevent replay attacks.
- Derive per-login session keys using HKDF (HMAC-based Extract-and-Expand Key Derivation Function).
- Generate session-scoped MCP access tokens.

### FR2: MCP Gateway
- Act as a central hub for MCP requests.
- Route requests based on tool namespaces or metadata.
- Support multiple backend MCP servers (aggregators, specialized agents).
- Handle request/response mapping and error propagation.

### FR3: Encrypted Storage
- Use SQLite with at-rest encryption for persistent data.
- Store session information, user metadata, and system configuration.
- Implement secure key management for storage encryption (using Argon2 for key derivation).
- Ensure atomic writes and data integrity.

### FR4: Session Management
- Track active sessions and their associated metadata.
- Enforce session timeouts and provide manual revocation.
- Limit the number of concurrent sessions per identity.

## Non-Functional Requirements

### NFR1: Security
- No per-login PSK rotation (to avoid lockout).
- Use `simd-json` at the wire edge for performance and security (serde for internal).
- Implement zeroize for sensitive data in memory.
- Use strong cryptographic primitives (Ring, X25519-dalek, ChaCha20Poly1305).

### NFR2: Performance
- Minimal authentication overhead (< 50ms per login).
- Low-latency request routing (< 10ms overhead for gateway logic).
- Efficient JSON parsing using `simd-json`.

### NFR3: Reliability
- Graceful handling of backend service failures.
- Robust database recovery and integrity checks.
- Audit logging of all authentication events.

## Success Criteria

1.  Successful authentication using WireGuard PSK and server nonce.
2.  MCP requests correctly routed to backend services.
3.  Data securely stored in encrypted SQLite and recoverable.
4.  Passes security audit for replay protection and session isolation.
