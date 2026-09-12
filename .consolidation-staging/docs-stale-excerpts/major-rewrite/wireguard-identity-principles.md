# WireGuard Identity Principles

## 1. Zero-Trust Identity Rationale

- **Root of trust**: a WireGuard Curve25519 public key. The peer is identified by its long-term WireGuard public key; no username, password, or certificate authority is required.
- **Login is the handshake**: a successful WireGuard key agreement is the authentication event. If the peer can complete the handshake, it already proves possession of the corresponding private key.
- **No host WireGuard**: the host control plane does not rely on a host-level WireGuard interface, AF_XDP, or `wg-quick`. WireGuard identity is decoupled from host network configuration.
- **Container name is the session id**: an Incus container's identity is exactly its derived session id. The session id is stable across reconnections for the same pubkey.
- **No credentials in transit**: secrets are never sent over the network for authentication. The session id and PSK are derived locally from keys the peer already proves possession of.

## 2. Cryptographic Derivation

- **PSK derivation**: the per-peer pre-shared key (PSK) is derived from the static WireGuard public key plus a timestamp/epoch using Argon2id. The salt is the peer's public key, so each peer gets a unique key-derivation path.
- **Session id derivation**: the session id is derived from the PSK via `Argon2(PSK, salt=pubkey)` and formatted as a deterministic UUID-like identifier. The same public key always yields the same session id, enabling reconnection without server-side private-key custody.
- **Domain separation**: every derivation uses a fixed context string (for example, `WG-PSK-` and `op-identity session-id v1`) so that PSK and session-id outputs cannot be confused or replayed across different domains.
- **No randomness in identity**: identity derivation is deterministic from the pubkey, so re-provisioning the same device resumes the same logical session.

## 3. PSK Rotation Policy

- **Periodic rotation**: PSKs rotate on a configurable interval. Each rotation produces a new PSK and a `next_rotation` timestamp, with the previous key valid until `valid_until`.
- **Forced rotation**: administrators can force immediate rotation for a peer, for example when a device is suspected to be compromised.
- **Graceful transition**: the current key remains valid until the new key is fully distributed and the peer has acknowledged it, preventing disconnects during rotation.
- **Rotation does not re-key identity**: rotating the PSK does not change the WireGuard public key or the stable session id. It only expires the current symmetric secret.

## 4. Session Lifecycle

- **Creation**: a session is created when a WireGuard peer is first observed or when a client explicitly presents a pubkey. The session id is deterministic.
- **Validity**: a session records `created_at`, `last_used`, and `expires_at` timestamps. Activity updates `last_used`.
- **TTL**: sessions expire after a configurable TTL (for example, one hour of inactivity). Expired sessions are removed by a background cleanup task.
- **Re-authentication**: when a peer reconnects after expiration, the same deterministic session id is recovered, so state can be resumed without creating duplicate sessions.
- **Invalidation**: a session can be explicitly invalidated (for example, on logout or device revocation). Invalidation clears the active session record without changing the underlying identity derivation.

## 5. Control-Plane Integration

- **D-Bus only**: all identity operations are exposed through the D-Bus control plane at `org.opdbus.v1`. There is no standalone `wg-auth-service`, JSON-RPC endpoint, or NetworkManager plugin.
- **No new gRPC services**: identity state is not a separate gRPC service. External clients reach the system through the existing `op-cognitive-mcp` gateway or the tonic-web gRPC bridge.
- **Plugins**: identity-related state lives in plugins registered in `crates/op-plugins/src/default_registry.rs`. The sealed blob catalog in `/dev/shm/opdbus/plugin-blobs` is the authoritative present state.
- **s6 supervision**: service lifecycle is managed through `sudo service6 ...`, not systemd or raw `s6-*` commands.

## 6. Network and Transport Constraints

- **Port 8090**: the gRPC bridge / OpenClaw gateway listens on port 8090. Port 18789 is retired.
- **No host WireGuard**: the host has no WireGuard interface, no `wg0`, and no `wg-quick` configuration.
- **Container sockets**: containers have no NIC or IP. All container I/O uses Unix domain sockets. Container attachment is expressed as OVS socket ports, not bridged virtual NICs.

## 7. Security Properties

- **Forward secrecy for PSKs**: PSK rotation limits the window of compromise. A stolen PSK is only useful until the next rotation.
- **Memory safety**: sensitive material is zeroized when dropped. Key material is not persisted beyond the session record.
- **Constant-time operations**: cryptographic operations use constant-time primitives where required to avoid timing side-channels.
- **Input validation**: all derived inputs (pubkey strings, timestamps) are validated for correct length and format before being used in KDFs.
- **No SQL for session state**: active session state is kept in memory (DashMap/SHM) to avoid Btrfs mutation loops and unintended disk I/O. Durability is the immutable snowball.

<!-- Extracted from WG-SESSION-ID.md on 2026-07-20 -->
