# ZeroClaw Router Wiring — Boundaries

## Non-negotiable

1. **Router is a machine mesh client**, not a human. Do not route router→host
   auth through `OracleIdentityAssertion` / HumanPrincipal (those are human-
   only per identity-handoff).
2. **OpenAI HTTP surface is op-web `:8080/v1`**. Bridge `:8090` is gRPC /
   gRPC-Web (topology lock). Do not treat bridge `/` HTTP 200 as OpenAI proof.
3. **Mesh-private host endpoints only** for this path. No public CF/SNI front
   for the router gateway; no CF tunnels into the control plane.
4. **No secrets in git.** Ops-provisioned Ghostbridge headers only.
5. **Fail-closed.** Missing/invalid identity ⇒ reject.
6. **Default bind `127.0.0.1`** with `require_pairing = true` for any broader
   bind. Reject `0.0.0.0` + `require_pairing = false` as production default.
7. **No second identity control plane.** No host `wg-lan`.

## Repudiated

| Approach | Why |
|---|---|
| Hardcoded footprint/trace-id in the spec or git | Credential leak; weak durable impersonation |
| Router obtains human Oracle assertions from decoy WG | Router ≠ human WG peer; wrong trust model |
| Provider URI `http://10.0.0.2:8090/v1` | Wrong surface; `/v1` is op-web `:8080` |
| `host = "0.0.0.0"` + `require_pairing = false` | Unauthenticated LAN trust expansion |
| Inventing machine assertion crypto in this spec | Belongs in identity-handoff follow-on if needed |

## External assumptions

| Assumption | Notes |
|---|---|
| NetMaker tunnel router ↔ `10.0.0.2` | Prior ping/bridge `/` verified; re-check |
| op-web on host `:8080` reachable from router | **Gate G1** |
| Bridge healthy on `:8090` | Topology lock; op-web depends on it |
| ZeroClaw binary + procd init on router | Re-check on implement |
| Host grants file can authorize machine footprint | Existing bridge grants mechanism |

## Residual risks

1. **Ghostbridge headers remain forgeable by anyone who holds them.** Mitigate
   with mesh-only reachability, rotation, grants least-privilege, no git.
2. **op-web `:8080` listen inventory** — currently `0.0.0.0:8080` on host;
   ensure firewall/mesh policy keeps it off the public internet (control-plane
   / host ops responsibility).
3. **Future machine-signed assertions** — deferred; this mission accepts
   constrained GhostbridgeIdentity for mesh machines.

## Defers to

| Topic | Authority |
|---|---|
| Human assertion crypto / HumanPrincipal / bridge sole validator | `netmaker-xray-identity-handoff/` |
| CF / REALITY / mail / OpenFlow IP:port | `3tched-ghostbridge-control-plane/` |
| Topology lock | `.kiro/specs/README.md` |
| OpenAI→bridge adaptation | `crates/op-web` (`handlers/zeroclaw.rs`) |
| gRPC auth gate | `crates/op-grpc-bridge` |
