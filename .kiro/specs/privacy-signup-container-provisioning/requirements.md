# Requirements — privacy-signup-container-provisioning

## Problem Statement

The privacy-router signup flow and the workspace container provisioning script exist as two
disconnected paths. A user who completes magic-link verification ends up with an `email_verified`
flag and a WireGuard keypair in CozoDB but no Incus container, no cognitive-mcp memory
namespaces, and no GhostBridge peer registration.
`deploy/scripts/provision-workspace-subscriber.sh` contains the correct provisioning logic but
is never invoked by the web path.

This spec closes the gap: after a user verifies their magic link the system must
deterministically provision their Incus workspace container, resolve the WireGuard identity
source-of-truth conflict between the two paths, register GhostBridge peers with the Netmaker
API, preserve the email-privacy rule end-to-end, and track provisioning state on `PrivacyUser`
so the UI and retry path are well-defined.

---

## Functional Requirements

### FR-1 — Trigger provisioning on email verification

After `UserStore::verify_magic_link` succeeds and `email_verified` is set to `true`, the
system must enqueue or directly invoke the container provisioning sequence for that user. The
provisioning trigger must be asynchronous and non-blocking so the HTTP verify response returns
to the caller immediately, while provisioning proceeds in the background.

### FR-2 — Container provisioning (Incus)

The provisioning sequence must:

- Launch an Incus container named `ws-<user_id>` (idempotent — if the container already
  exists, skip launch and proceed with the remaining steps).
- Install the base package set: `curl ca-certificates wireguard-tools iproute2 nodejs npm`.
- Wait for the container's init system to be ready before executing further commands.
- Record the container name on `PrivacyUser.privacy_container_name` in CozoDB on success.

### FR-3 — WireGuard identity source-of-truth

The WireGuard keypair generated **server-side** in `crates/op-identity/src/registration.rs`
during signup is the single source of truth for the user's WG identity. The container
provisioning sequence must:

- **Not** generate a second keypair inside the container.
- Instead, inject the server-side public and private keys into the container
  (`/etc/wireguard/privkey`, `/etc/wireguard/pubkey`) as part of provisioning.
- Derive the MCP bearer token from the server-side pubkey using `UUID v5(pubkey)` and store
  it on `PrivacyUser.mcp_token` in CozoDB.
- Store the full pubkey in CozoDB under the container's `identity` namespace via cognitive-mcp
  (`container:<container_name>:identity/wireguard_pubkey`).

### FR-4 — MCP memory namespace seeding

After WireGuard identity injection the provisioning sequence must call cognitive-mcp at
`http://100.90.37.254:3003/mcp` to seed the following namespaces, authenticated with the
derived MCP bearer token:

- `container:<name>:identity` — pubkey, mcp_token, psk (if provided)
- `container:<name>:soul` — user profile record with `created_at`
- `container:<name>:domain:work`, `:domain:personal`, `:domain:home` — empty MEMORY_INDEX
- `container:<name>:index` — MEMORY_INDEX listing all namespaces
- `container:<name>:features` — `ghostbridge` and `semantic_search` flags

### FR-5 — GhostBridge / Netmaker peer registration

When the user's `ghostbridge_enabled` flag is `true`:

- Register the user's WireGuard public key as a peer with the Netmaker API using the
  configured `NETMAKER_API_URL` and `NETMAKER_API_TOKEN` environment variables.
- The registration call must supply the pubkey, the container name, and the assigned IP.
- On success, record the Netmaker peer ID on `PrivacyUser.netmaker_peer_id` in CozoDB.
- On failure, set provisioning status to `Failed` with the error message and do not mark
  the user as connected.

### FR-6 — Email privacy rule

Email must be stored in CozoDB **only** when GhostBridge is disabled. Specifically:

- The `PrivacyUser.email` field must be cleared (set to an empty string or omitted) in CozoDB
  when `ghostbridge_enabled` is `true` at the time of provisioning.
- The cognitive-mcp `identity/email` namespace key must only be written when
  `ghostbridge_enabled` is `false`.
- The rule must be enforced in the Rust provisioning code, not just in the shell script.

### FR-7 — Provisioning state machine

`PrivacyUser` must carry a `provisioning_status` field with the following states:

| State | Meaning |
|---|---|
| `Pending` | User verified but provisioning not yet started |
| `Provisioning` | Container launch or dependency step in progress |
| `Ready` | All steps completed; container running, MCP seeded |
| `Failed` | At least one step failed; error stored in `provisioning_error` |

`mark_privacy_network_connected` is repurposed to set status `Ready` and clear
`provisioning_error`. A new `mark_provisioning_failed` method on `UserStore` sets
`Failed` + error string. The existing `privacy_network_connected: bool` field becomes a
derived view: `true` iff status is `Ready`.

### FR-8 — Idempotency and retry

Provisioning must be idempotent at every step:

- Container launch uses `incus info` check before `incus launch` (already in the shell script;
  the Rust equivalent must match).
- MCP `cognitive_memory` store calls are idempotent by key — duplicate writes are safe.
- Netmaker peer registration must check for an existing peer before creating.
- A user in `Failed` or `Pending` state may be re-provisioned by calling the provisioning
  sequence again; a user in `Ready` state must be a no-op.
- A task-level retry with exponential backoff (max 3 attempts, 5 s / 15 s / 45 s) must be
  applied to the container launch and Netmaker API calls.

---

## Non-Functional Requirements

| ID | Requirement |
|----|-------------|
| NFR-1 | The HTTP `GET /privacy/verify` response must return within 500 ms. Provisioning runs in a background Tokio task; it must not block the HTTP response. |
| NFR-2 | WireGuard private keys must never be logged, returned in API responses, or written to disk outside the container's `/etc/wireguard/privkey` (permissions `0600`). |
| NFR-3 | When `ghostbridge_enabled` is `true`, no email value may appear in any CozoDB row, any cognitive-mcp namespace write, or any structured log field. |
| NFR-4 | All provisioning errors must be captured in `provisioning_error` on `PrivacyUser` and emitted as structured `tracing::error!` events; they must not surface raw `anyhow` chains in HTTP responses. |
| NFR-5 | The provisioning background task must be cancellation-safe: if `op-web` is restarted while a task is in flight, `Pending`/`Provisioning` users are re-queued at startup. |
| NFR-6 | `cargo clippy --workspace --all-targets --all-features -- -D warnings` passes. |
| NFR-7 | `cargo fmt --all -- --check` passes. |
| NFR-8 | 80 %+ unit/integration test coverage for new provisioning logic, with all external calls (Incus, Netmaker API, cognitive-mcp) mocked. |

---

## Out of Scope

- Changing the magic-link email delivery mechanism (`crates/op-web/src/email.rs`).
- Google OAuth provisioning path (it already calls `provision_verified_user`; it benefits from
  this work but no new handler changes are required beyond picking up FR-3 / FR-6).
- Per-user Qdrant collection creation (tracked separately; `--semantic` flag is seeded but
  collection initialization is deferred).
- Container image selection — `images:debian/12` is fixed for this iteration.
- Billing / quota enforcement beyond recording `privacy_quota_bytes`.
