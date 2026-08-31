# ZeroClaw Router → op-grpc-bridge Wiring

## Status: Draft — blocked on identity/auth redesign

**Blockers**:
1. Auth model undefined — must use OracleIdentityAssertion + HumanPrincipal (per identity-handoff spec), not self-asserted ghostbridge headers
2. LAN exposure policy unresolved — cannot ship `0.0.0.0` bind without auth gate

## Scope

This spec owns **router-side ZeroClaw gateway wiring only**:
- Config file structure for zeroclaw on router
- Network path to bridge at 10.0.0.2:8090
- Init script / service enablement

**Defers to**:
- [`netmaker-xray-identity-handoff/`](../netmaker-xray-identity-handoff/) — assertion crypto, HumanPrincipal registry, bridge validation
- [`3tched-ghostbridge-control-plane/`](../3tched-ghostbridge-control-plane/) — CF public surface, mesh privacy, OpenFlow IP:port

**Does not own**:
- Identity assertion generation or validation
- Bridge auth gate implementation
- Host wg-lan, CF tunnels, SNI front on public :443

## Context

- **Router**: OpenWrt at 100.69.0.3 (netmaker) / 10.0.0.3 (wg0) / 192.168.1.1 (LAN)
- **Host bridge**: op-grpc-bridge at 10.0.0.2:8090 (mesh-private, per topology lock)
- **Mesh**: Netmaker tunnel operational
- **ZeroClaw binary**: On router at /fast/zeroclaw/bin/zeroclaw

## Network Path (Verified 2026-08)

```
Router (100.69.0.3) 
  → netmaker tunnel 
  → Host mesh peer (10.0.0.2) 
  → op-grpc-bridge :8090
```

Connectivity verified:
- `ping 10.0.0.2` from router: 2/2 received, ~158ms RTT
- `wget http://10.0.0.2:8090/` from router: HTTP 200

**Unverified**: Auth handshake with assertion model (blocked).

## Requirements

### R1: Gateway Bind Policy
- **Default**: Bind `127.0.0.1` or mesh interface only
- **LAN exposure**: Requires either:
  - `require_pairing = true` with device pairing flow, OR
  - Assertion-based auth with HumanPrincipal validation
- **Rejected**: `host = "0.0.0.0"` + `require_pairing = false` as production default

### R2: Provider Authentication (BLOCKED)
Router → bridge requests must authenticate via one of:

**Option A (Product path — preferred)**:
- OracleIdentityAssertion signed by Oracle decoy WG
- Bridge validates assertion → resolves HumanPrincipal
- Per [`netmaker-xray-identity-handoff/`](../netmaker-xray-identity-handoff/) spec

**Option B (Lab-mode fallback — time-boxed)**:
- Labeled `residual-risk`
- Explicit expiry date in config
- NOT in Active topology
- Self-asserted headers with ops-provisioned values (never hardcoded in spec/git)

Current status: Option A implementation incomplete; Option B not yet scoped for residual-risk labeling.

### R3: No Secrets in Git
- Footprints, trace-ids, API keys: **placeholder only** in spec/config examples
- Ops provisioning steps document how to obtain/inject values
- Never commit live identity material

### R4: Service Enablement
- Init script: `/etc/init.d/zeroclaw` (procd)
- Config dir: `/fast/zeroclaw/config`
- State dir: `/fast/zeroclaw/state`
- Enable and start via procd

### R5: End-to-End Verification
- Gateway responds on configured bind address/port
- Request through gateway authenticates with bridge
- Response returns to client

## Out of Scope (This Pass)
- Salad provider secrets configuration
- Plugin schema regeneration
- Chatbot ↔ zeroclaw integration
- Bridge-side assertion validation implementation

---

## What Changed / Remaining Blockers

### Changes from previous draft (2026-08)
1. **Status**: "Ready for Implementation" → "Draft — blocked on identity/auth redesign"
2. **Removed**: Hardcoded `x-ghostbridge-footprint` and `x-ghostbridge-trace-id` values
3. **Removed**: `host = "0.0.0.0"` + `require_pairing = false` as default
4. **Added**: Explicit auth model requirement (assertion or labeled residual-risk)
5. **Added**: Cross-references to authority specs
6. **Added**: `boundaries.md`

### Remaining Blockers
1. **Identity handoff incomplete**: OracleIdentityAssertion signing/validation not implemented
2. **HumanPrincipal registry**: Not yet operational for router client registration
3. **Lab-mode scope undefined**: If Option B needed for interim, requires residual-risk labeling and expiry

### To Unblock
- Complete identity-handoff cargo-test mission
- Document bridge auth gate for assertion validation
- If lab-mode needed: scope residual-risk envelope with expiry date
