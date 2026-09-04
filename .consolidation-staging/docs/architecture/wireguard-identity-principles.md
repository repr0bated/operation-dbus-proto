# WireGuard Identity Principles

## Zero-Trust Authentication Model

### Core Concept

WireGuard identity is established through cryptographic proof of knowledge, not network location or configuration state. The authentication system implements a zero-trust model where:

- **Identity = WireGuard public key** — The peer's WireGuard public key is the sole authoritative identifier
- **Session binding through PSK rotation** — Ephemeral pre-shared keys (PSKs) are derived deterministically from the static WireGuard key and rotated periodically
- **No ambient authority** — Every connection requires fresh cryptographic proof; there is no "logged in" state that persists without cryptographic binding

### Session Identity Derivation

Each authenticated session is identified by a session ID derived from the current PSK:

```
session_id = BLAKE2s(PSK)[0..16]
```

The session ID is:
- **Deterministic** — Same PSK always produces same session ID
- **Opaque** — Cannot be reversed to recover the PSK
- **Unique per rotation** — New PSK → new session ID
- **Short-lived** — Tied to PSK rotation interval (default: 24 hours)

A peer's **identity** is its WireGuard public key; its **session** is the current time-bound PSK proving recent key agreement.

## Cryptographic Principles

### PSK Derivation with Argon2

Pre-shared keys are derived using Argon2 (memory-hard KDF) to prevent offline brute-force attacks on the master key:

```
PSK = Argon2id(
    password: master_key,
    salt: kdf_salt || peer_pubkey,
    context: "WG-PSK-" || timestamp_epoch_hour,
    memory: 65536 KiB,
    iterations: 3,
    parallelism: 4,
    output_len: 32 bytes
)
```

**Rationale for Argon2:**
- **Memory hardness** — Requires 64 MiB RAM per derivation, making GPU/ASIC attacks impractical
- **Resistance to side-channel attacks** — Data-independent memory access patterns prevent timing attacks
- **Tunable cost** — Parameters can be increased as hardware improves without protocol changes
- **Standardized** — RFC 9106 ensures interoperability and cryptographic review

### Key Derivation Chain

The full key derivation follows this hierarchy:

1. **Master key** (32 bytes) — Generated once at service initialization, locked in memory, never exported
2. **HKDF extraction** — Combines master key with peer's static WireGuard public key using HMAC-SHA256
3. **Argon2 strengthening** — Derives time-bound PSK from HKDF output + timestamp
4. **Session ID** — BLAKE2s hash of PSK for session tracking

This layered approach ensures:
- **Forward secrecy** — Compromise of old PSK doesn't reveal past session keys
- **Post-quantum preparation** — Can upgrade HKDF algorithm without changing session layer
- **Rate limiting** — Memory-hard Argon2 prevents rapid PSK guessing

### Session Lifecycle

Sessions transition through these states:

1. **Creation** — Client requests session; server derives PSK and session ID, stores in secure memory
2. **Active** — Session ID valid for rotation interval (default 24h); server periodically validates session freshness
3. **Rotation** — New PSK derived when rotation interval expires; new session ID generated; old session marked stale
4. **Expiration** — Stale sessions cleaned up after grace period (default 5 minutes)

**No persistent session state** is stored on disk — sessions exist only in memory and are re-derived deterministically on service restart from the master key and current timestamp.

## Key Rotation Rationale

### Why Rotate PSKs?

1. **Limit cryptographic exposure** — Even if PSK is compromised, damage window is bounded by rotation interval
2. **Detect stale peers** — Failed rotation indicates peer is offline or misconfigured
3. **Audit trail** — Each rotation creates immutable snowball record of key lifecycle events
4. **Comply with zero-trust principles** — Continuous re-authentication prevents "set and forget" configurations

### Rotation Triggers

PSK rotation occurs on:
- **Time-based** — Default 24-hour interval
- **Event-based** — Manual rotation via D-Bus/JSON-RPC method call (force=true flag)
- **Security-triggered** — Automatic rotation if anomaly detected (e.g., unexpected source IP)

### Rotation Coordination

The server is authoritative for rotation timing. Clients:
1. Poll server every minute for session validity
2. Receive "rotation pending" notification 5 minutes before rotation
3. Fetch new PSK and session ID
4. Update WireGuard configuration atomically (`wg set wg0 peer <pubkey> preshared-key <new_psk>`)

**No shared state** exists between peers — each peer independently rotates with the server, which maintains the canonical PSK schedule.

### Rotation Interval Selection

Default 24-hour interval balances:
- **Security** — Short enough to limit exposure window
- **Reliability** — Long enough that brief network outages don't break connectivity
- **Performance** — Argon2 derivation is expensive; hourly rotation would consume significant CPU on large deployments

Deployments with higher security requirements can configure:
- **1-hour rotation** — For high-sensitivity environments (requires 24× more CPU for Argon2)
- **Event-only rotation** — Disable time-based rotation; rotate manually or on security events

## Transport and Identity Headers

### Xray Integration (Port 8090)

The xray router injects identity headers into all upstream connections:

```
X-Ghostbridge-Footprint: <session_id_hex>
X-WireGuard-Pubkey: <peer_pubkey_base64>
```

These headers are:
- **Cryptographically bound** — `X-Ghostbridge-Footprint` is derived from current PSK (via session ID)
- **Unforgeable** — Only peers with valid PSK can generate correct session ID
- **Ephemeral** — Session ID changes on every PSK rotation

**Authorization decision** is made by inspecting these headers, not source IP or TLS client certificate. This implements defense-in-depth: even if network layer is compromised, application layer still requires cryptographic proof of identity.

### Zero-Trust Enforcement

Services validate requests by:
1. Extracting `X-Ghostbridge-Footprint` header
2. Looking up session via D-Bus call: `auth.validateSession(session_id)`
3. Verifying session is active and not expired
4. Checking peer public key against access control list

**No IP-based trust** — A peer moving between networks keeps the same identity (WireGuard public key) and session (current PSK), so authorization remains valid.

## Security Properties

### Threat Model

The system defends against:
- **Offline brute-force** — Argon2 memory hardness makes PSK guessing infeasible
- **Replay attacks** — Session IDs expire and cannot be reused after rotation
- **Network eavesdropping** — WireGuard provides authenticated encryption; PSK adds additional layer
- **Compromised peer** — Revoking peer's WireGuard public key invalidates all sessions immediately

The system does NOT defend against:
- **Compromise of server master key** — Attacker can derive all PSKs
- **Timing side-channels on validation** — Session lookup is not constant-time (acceptable for identity system)
- **Physical memory extraction** — PSKs and master key are in RAM; full disk encryption required for defense

### Recovery from Compromise

If master key is compromised:
1. Generate new master key (triggers re-initialization)
2. Force rotation for all peers
3. All existing sessions invalidated (new session IDs derived from new master key)
4. Peers reconnect and fetch new PSKs

No protocol changes required — rotation is part of normal operation.

<!-- Extracted from /mnt/opt-inspect/home/git/operation-dbus-proto/docs/WG-SESSION-ID.md on 2026-07-20 -->
