# ZeroClaw Router → Host Wiring

## Status: Final

Router-side ZeroClaw gateway wiring is specified and implementable under the
constraints below. Implementation remains gated on the verification checklist
in `tasks.md` (mesh reachability to op-web `:8080`, ops-provisioned machine
identity, fail-closed auth).

| | |
|---|---|
| Owns | Router ZeroClaw config, bind policy, init/procd, E2E verify |
| Does not own | Assertion crypto, HumanPrincipal, public CF/REALITY/mail |
| Auth model | **Machine mesh client** via GhostbridgeIdentity (ops-provisioned) |
| OpenAI HTTP surface | **op-web `:8080/v1`** (not bridge `:8090`) |
| Bridge | op-grpc-bridge gRPC/gRPC-Web at `10.0.0.2:8090` (mesh-private) |

## Scope

**In scope**
- Config template for `/fast/zeroclaw/config/config.toml`
- Bind / pairing policy for the router gateway (`:42617`)
- procd init at `/etc/init.d/zeroclaw`
- Ops provisioning steps for machine identity (no secrets in git)
- Verification that unauthorized calls fail closed

**Defers to**
- [`netmaker-xray-identity-handoff/`](../netmaker-xray-identity-handoff/) — human OracleIdentityAssertion / HumanPrincipal (humans only)
- [`3tched-ghostbridge-control-plane/`](../3tched-ghostbridge-control-plane/) — CF public surface, mesh privacy ops, OpenFlow IP:port

**Out of scope**
- Salad secrets, chatbot integration, plugin schema regeneration
- Treating the router as a human / inventing a second identity control plane
- Host `wg-lan`, CF tunnels into CP, SNI front on public `:443`

## Context (verified / assumed)

| Fact | Status |
|---|---|
| Router OpenWrt: netmaker `100.69.0.3`, wg0 `10.0.0.3`, LAN `192.168.1.1` | Assumed ops inventory |
| Bridge listens mesh-private on `10.0.0.2:8090` (gRPC + gRPC-Web) | Verified on host (2026-08) |
| op-web listens `0.0.0.0:8080`; OpenAI `/v1/models` + `/v1/chat/completions` live here and adapt into bridge gRPC | Verified in tree (`crates/op-web`) |
| Router can `ping 10.0.0.2` and `wget http://10.0.0.2:8090/` → HTTP 200 | Verified 2026-08 (bridge root only — **not** OpenAI `/v1`) |
| Router can reach `http://10.0.0.2:8080/v1/models` | **Must verify** before declaring E2E green |
| ZeroClaw binary at `/fast/zeroclaw/bin/zeroclaw`; init at `/etc/init.d/zeroclaw` | Prerequisites claimed; re-check on implement |

## Hard decisions (locked)

### D1 — Router is a machine mesh client, not a human
`OracleIdentityAssertion` / `HumanPrincipal` are for **human** devices that
terminate WireGuard at the Oracle decoy. The OpenWrt ZeroClaw gateway is a
**machine** on the NetMaker mesh. It MUST NOT pretend to be a human principal
or obtain decoy-signed human assertions.

Router → host auth uses the existing **GhostbridgeIdentity** path
(ops-provisioned `x-ghostbridge-footprint` plus trace or WireGuard pubkey),
with capability grants for the zeroclaw methods it may invoke. This is the
intentional machine use of the residual ghostbridge path documented in the
identity-handoff residual-risk section — constrained to mesh peers, never
committed to git, never the human product path.

Future hardening (machine-scoped signed assertions) belongs in a follow-on to
identity-handoff, not in this router wiring spec.

### D2 — OpenAI HTTP target is op-web `:8080`, not bridge `:8090`
- Bridge `:8090` = tonic gRPC / gRPC-Web + trivial HTTP `/` health.
- OpenAI-compatible `/v1/*` = **op-web `:8080`**, which requires Ghostbridge
  headers and calls the bridge as sole router (`crates/op-web/src/handlers/zeroclaw.rs`).

Provider URI in ZeroClaw config: `http://10.0.0.2:8080/v1`.

### D3 — Secure bind default
- Default gateway bind: `127.0.0.1:42617`
- `require_pairing = true` for any non-loopback bind
- Rejected as production default: `host = "0.0.0.0"` + `require_pairing = false`

### D4 — No secrets in git
Footprints, trace-ids, keys: placeholders + ops procedure only.

### D5 — Fail-closed
Unauthorized / missing identity ⇒ reject. No silent fallback.

## Requirements

### R1: Gateway bind policy
As D3. LAN exposure only with pairing (or equivalent auth) and a written
client/grants list in the deploy notes (not in this repo as live secrets).

### R2: Machine authentication
Every router→op-web request carries ops-provisioned Ghostbridge headers:
- `x-ghostbridge-footprint` (required)
- `x-ghostbridge-trace-id` and/or `x-wireguard-pubkey` (op-web requires trace
  or WireGuard identity in addition to footprint)

Grants on the host must allow the machine footprint the zeroclaw capabilities
needed for `/v1/models` and `/v1/chat/completions` adaptation.

### R3: Provider config shape
ZeroClaw OpenAI-compatible provider pointing at `http://10.0.0.2:8080/v1`
with `extra_headers` filled at deploy time from ops (never from this spec).

### R4: Service enablement
procd init; config dir `/fast/zeroclaw/config`; state `/fast/zeroclaw/state`.

### R5: Verification
1. Unauthorized call to gateway or directly to op-web `/v1` fails (401/403).
2. Authorized call returns models / completes chat via op-web → bridge.
3. Gateway listens only on the configured bind address.

## Identity-handoff status (accurate)

| Layer | Status |
|---|---|
| Local cargo-test mission (OIA1, HumanPrincipal, bridge validator, E2E) | Largely landed on `droid/netmaker-xray-identity-handoff` |
| Production Oracle decoy + out-of-band human enrollment | External / not this mission |
| Router machine auth | **This spec** — GhostbridgeIdentity mesh client (D1) |

Do not block router wiring on “HumanPrincipal incomplete.” Block only on the
gates in `tasks.md`.

## What changed (finalization 2026-08-07)

1. Status → **Final** (implementable under gates).
2. Corrected OpenAI surface: op-web `:8080/v1`, not bridge `:8090/v1`.
3. Locked router principal model: machine GhostbridgeIdentity, not human Oracle assertion.
4. Corrected identity-handoff readiness claims.
5. Removed dual Option A/B ambiguity; residual-risk lab mode is the same machine
   path with explicit ops expiry hygiene, not a second auth design.
