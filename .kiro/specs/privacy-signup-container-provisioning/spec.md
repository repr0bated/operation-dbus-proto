# Spec — privacy-signup-container-provisioning

## Purpose

This document is the binding contract between the requirements, design decisions, and
implementation tasks for wiring the magic-link signup path to per-user Incus container
provisioning. It supersedes the `// TODO` comments in
`deploy/scripts/provision-workspace-subscriber.sh` lines ~90–96 and the unconnected
`ensure_user_container` call in `privacy.rs::provision_verified_user`.

---

## Scope

**In scope:**
- Triggering container provisioning as a background Tokio task after `verify_magic_link` succeeds.
- Injecting the server-side WireGuard keypair (already stored on `PrivacyUser`) into the Incus
  container — eliminating the divergent in-container keygen from the shell script.
- Deriving the MCP bearer token deterministically as `UUID v5(wg_public_key)` and persisting it.
- Seeding all nine cognitive-mcp memory namespace keys via `http://100.90.37.254:3003/mcp`.
- Registering the WireGuard peer with the Netmaker API when `ghostbridge_enabled = true`.
- Enforcing the email-privacy rule (no email in CozoDB or cognitive-mcp when GhostBridge is on).
- Adding `provisioning_status`, `mcp_token`, `ghostbridge_enabled`, `netmaker_peer_id`, and
  `provisioning_error` to `PrivacyUser` and CozoDB.
- Re-queuing `Pending`/`Provisioning` users at `AppState` startup.
- Unit and integration tests with all external calls mocked.

**Out of scope:**
- Changing the magic-link email delivery or SMTP configuration.
- Google OAuth handler changes (it picks up the new provisioner automatically).
- Per-user Qdrant collection initialisation (`--semantic` flag is seeded but collection
  creation is deferred).
- Container image selection (`images:debian/12` is fixed).
- Billing / quota enforcement logic.

---

## Architectural Constraints (binding)

| # | Constraint | Source |
|---|---|---|
| C-01 | No `Command::new("incus")` in Rust service code outside `IncusCliDriver`. | AGENTS.md §4 |
| C-02 | Server-side WireGuard keypair (`PrivacyUser.wg_public_key / wg_private_key_encrypted`) is the single identity source of truth. No keypair generated inside the container. | FR-3 |
| C-03 | Email must not appear in any CozoDB field or cognitive-mcp write when `ghostbridge_enabled = true`. | FR-6, NFR-3 |
| C-04 | `GET /privacy/verify` must return within 500 ms; provisioning is fire-and-forget. | NFR-1 |
| C-05 | MCP token derivation: `uuid::Uuid::new_v5(&Uuid::NAMESPACE_OID, pubkey.as_bytes())`. Matches `uuidgen -v5 -n @oid -N "$PEER_PUBKEY"`. | FR-3 |
| C-06 | All new source under `crates/op-web/src/`. No new crates, no `src/` at workspace root. | AGENTS.md §3 |
| C-07 | All new `subid` props registered in `op-plugins` OSCAL registry before merge. | AGENTS.md §4a |
| C-08 | WireGuard private keys must never appear in logs, HTTP responses, or structured events. | NFR-2 |

---

## Key Design Decisions

| Decision | Choice | Rationale |
|---|---|---|
| Provisioning trigger | `tokio::spawn` (fire-and-forget) after verify | Verify must return in < 500 ms; container launch can take 10–30 s. |
| Identity source of truth | Server-side keypair, injected into container | Avoids two divergent identities; server already owns the pubkey used for Netmaker and OpenFlow. |
| Incus interaction boundary | `IncusDriver` trait + `IncusCliDriver` impl | Allows mock injection in tests without spawning real containers; future-proofs a native REST driver. |
| Retry strategy | 3 attempts, 5 s / 15 s / 45 s backoff, on container launch and Netmaker API | Container cold-start is slow; Netmaker API can be transient. |
| `privacy_network_connected` | Retained as derived bool (`status == Ready`) | Backward compat for existing call sites and UI checks. |
| Email wipe on GhostBridge | `clear_user_email` mutates CozoDB row | Ensures the rule is enforced after the fact even if signup stored the email. |

---

## Acceptance Criteria

| ID | Criterion | Verified by |
|----|---|---|
| AC-01 | `GET /privacy/verify?token=<valid>` returns HTTP 200 in < 500 ms with `provisioning_status: "pending"`. | Integration test |
| AC-02 | After successful provisioning, `PrivacyUser.provisioning_status` is `Ready` and `privacy_container_name` is `ws-<user_id>`. | Integration test |
| AC-03 | Re-running provisioning on a `Ready` user is a no-op (no second container launch, no error). | Unit test |
| AC-04 | When `ghostbridge_enabled = true`, `PrivacyUser.email` is `""` in CozoDB and no `identity/email` write reaches cognitive-mcp. | Unit test with mock |
| AC-05 | When `ghostbridge_enabled = true` and Netmaker registration succeeds, `PrivacyUser.netmaker_peer_id` is non-empty. | Unit test with mock |
| AC-06 | When Netmaker API returns a non-2xx error, `provisioning_status` becomes `Failed` and `provisioning_error` contains the HTTP status. | Unit test with mock |
| AC-07 | Container WireGuard files `/etc/wireguard/privkey` and `/etc/wireguard/pubkey` match `PrivacyUser.wg_private_key_encrypted` and `wg_public_key`. | Integration test |
| AC-08 | `PrivacyUser.mcp_token` equals `uuid::Uuid::new_v5(&Uuid::NAMESPACE_OID, pubkey.as_bytes()).to_string()`. | Unit test |
| AC-09 | Users in `Pending`/`Provisioning` state at `AppState` startup are re-queued without human intervention. | Integration test |
| AC-10 | `cargo clippy --workspace --all-targets --all-features -- -D warnings` passes. | CI |
| AC-11 | `cargo test --workspace --all-targets --all-features` passes with 80 %+ coverage on `privacy_provisioner.rs`. | CI |
