# Identity and Transport

`op-grpc-bridge` turns a verified transport credential into three distinct
identifiers:

- `principal_id` is the stable authorization and audit actor key.
- `session_id` identifies one arrival and is derived from the WireGuard public
  key.
- `session_genesis` anchors that arrival in the Snowball chain. It is evidence,
  not an authorization key.

Capability grants are keyed only by exact `principal_id`. A session ID, genesis,
trace ID, catalog hash, client name, or legacy footprint never grants authority.

## Transport surface

The bridge owns the TLS fabric on `:8090` for native gRPC, gRPC-Web, generated
plugin methods, and Streamable HTTP MCP at `/mcp`. Its runit definition binds
`127.0.0.1:8090` and `10.0.0.3:8090`. It also serves the same gRPC routes on
the host-local `/run/opdbus/grpc.sock` and the container-facing
`/run/ghostbridge/container.sock`. TCP listeners require TLS.

The public fabric may pass through WARP and Xray. Those hops replace the
original source address, so production binds human authentication to a trusted
decoy signature rather than treating the observed TCP source as identity.

## Identity credentials

| Credential | Intended caller | Current entry points |
|---|---|---|
| OIA1 | A human client behind an identity-aware broker | gRPC `PluginService` and generated plugin routes, Streamable HTTP MCP, and D-Bus sender registration |
| SID1 | A local client with an already anchored identity sled | Streamable HTTP MCP |

Exactly one credential is accepted on an MCP request. Legacy
`X-Ghostbridge-Footprint`, `X-WireGuard-Pubkey`, bearer tokens, MCP session IDs,
and client names are not MCP authentication. Some domain gRPC services still
use the legacy genesis/pubkey-or-trace interceptor because they do not declare
per-method capabilities; OIA1 is not accepted by those routes.

### OIA1: short-lived human assertion

The Oracle decoy issues a binary `OIA1` envelope signed with Ed25519. It contains
the human WireGuard public key, issue and expiry times, a 16-byte nonce, the
decoy-observed inner address, and a decoy key ID. Its lifetime cannot exceed
900 seconds.

- Native gRPC carries the raw bytes as binary metadata named
  `x-oracle-identity-assertion-bin`.
- Raw HTTP carries canonical, unpadded base64url of those bytes in the header
  with the same name.
- D-Bus callers pass the raw proof to
  `org.opdbus.v1.DbusIdentityV1.RegisterCurrentSender` at
  `/org/opdbus/v1/identity/dbus`.

The bridge validates in this order:

1. Parse the exact OIA1 framing.
2. Resolve `decoy_key_id` in `/etc/opdbus/decoy-trust.json` and verify the
   signature.
3. Enforce issue time, expiry, the 900-second maximum lifetime, and 30 seconds
   of clock leeway.
4. Reject a nonce already present in the durable replay journal.
5. Apply the configured transport binding.
6. Resolve a non-revoked human principal and an anchored session.
7. Persist the nonce before dispatch.

Production defaults `OP_ORACLE_ASSERTION_SOURCE_BINDING` to
`trusted-decoy-signature`. `exact-peer-ip` is a legacy opt-in for a transport
that preserves the asserted inner address. The replay journal defaults to
`/var/lib/op-dbus/auth-replay.db`; every ingress uses the same validator, so an
assertion consumed by MCP cannot be replayed through gRPC.

D-Bus registration additionally requires `cap.identity.dbus.bind@v1`. The
binding is tied to the bus-assigned unique sender and immutable process
credentials. It is removed when that sender disconnects; callers cannot choose
their own principal, session, sender, UID, PID, or genesis.

### SID1: server-authored local identity

The MutationEngine creates SID1 only after session genesis exists and stores it
inline in the authoritative sled as:

```text
sid1:<canonical-unpadded-base64url>
```

The HTTP header `x-opdbus-sealed-id-bin` carries only the encoded portion. SID1
contains the principal, session, WireGuard key, genesis, trace, arrival catalog
and chain anchors, lifetime, and transport scope. Its SHA-256 trailer protects
the envelope bytes; it is not independently an authorization signature.

Acceptance therefore requires more than opening the envelope. The bridge
re-derives the principal and session IDs, requires `mcp` in the transport scope,
loads an active, anchored, unexpired sled, exact-matches every sled-backed
immutable claim, resolves the still-registered principal, and requires the
principal's exact grant entry to contain at least one capability. Each MCP
operation performs its own capability check after authentication.

SID1 is intended for protected local clients, but the `/mcp` handler does not
restrict it by peer address and does not consume it after use. Treat it as a
reusable possession credential. It stops working when the session becomes
inactive or expires, the principal is revoked, its grants are removed, or the
authoritative sled or registry binding changes. Clients should emit the header
immediately before a request:

```bash
op-identity-headers --session <canonical-session-uuid>
```

The helper prints one JSON header object. It does not proxy traffic, open a
listener, or create authority. Callers cannot supply or replace `sealed_id`
through identity-sled plugin methods.

## Fail-closed state

| State | Default path | Failure behavior |
|---|---|---|
| Decoy trust store | `/etc/opdbus/decoy-trust.json` | Missing or malformed data loads no trusted keys |
| OIA replay journal | `/var/lib/op-dbus/auth-replay.db` | Open, parse, append, or sync failure rejects protected requests |
| Durable capability grants | `/etc/opdbus/capability-grants.json` | Invalid grants produce an empty SHM projection |
| Runtime grant projection | `/dev/shm/opdbus/capability-grants.json` | Missing, malformed, or absent principal entry grants nothing |
| MCP audience/toolset policy | `/etc/opdbus/mcp-{audience-policy,toolsets}.json` | Invalid, non-root-owned, or group/world-readable files prevent the bridge from binding |

`opdbus-grants` continuously validates and materializes the durable grant file
into SHM. Wildcard keys and legacy 64-hex-character footprint keys are rejected.

## Operational checks

```bash
sudo sv status opdbus-grants op-grpc-bridge
sudo /usr/local/bin/op-grants-materializer check
sudo stat -c '%U %a %n' \
  /etc/opdbus/capability-grants.json \
  /etc/opdbus/mcp-audience-policy.json \
  /etc/opdbus/mcp-toolsets.json \
  /var/lib/op-dbus/auth-replay.db
```

Policy files must be root-owned and mode `0600`. Common diagnoses:

- HTTP `401`: missing, duplicated, malformed, expired, replayed, unregistered,
  revoked, unanchored, or non-matching identity credential.
- HTTP `403`: `Origin` is outside the exact allowlist, or a request carrying a
  `Sec-Fetch-*` browser marker omits `Origin`.
- gRPC `Unauthenticated`: OIA validation or session anchoring failed.
- gRPC `Unavailable`: the durable replay store is unavailable.
- `AccessDenied` in a JSON-RPC result: authentication passed, but the exact
  principal lacks the method's schema-declared capability.
