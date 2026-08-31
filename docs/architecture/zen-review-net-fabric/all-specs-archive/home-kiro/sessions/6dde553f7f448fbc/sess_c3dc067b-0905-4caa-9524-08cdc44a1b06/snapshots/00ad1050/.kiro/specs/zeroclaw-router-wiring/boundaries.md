# ZeroClaw Router Wiring — Boundaries

## Non-Negotiable

These constraints are inherited from Active topology lock and identity-handoff spec:

1. **Assertion-based auth for product path**
   - Router → bridge auth uses `OracleIdentityAssertion` + `HumanPrincipal`
   - Bridge is sole validator of assertions
   - Per [`netmaker-xray-identity-handoff/`](../netmaker-xray-identity-handoff/)

2. **Mesh-private bridge endpoint**
   - gRPC/HTTP at 10.0.0.2:8090
   - Not exposed to public internet
   - Per topology lock in [`README.md`](../README.md)

3. **No secrets in git**
   - Footprints, trace-ids, keys, tokens: placeholder only
   - Ops provisioning for actual values
   - Never commit live identity material

4. **Fail-closed auth**
   - No fallback to weaker auth if assertion validation fails
   - Unauthorized requests rejected, not degraded

5. **Default secure bind**
   - `127.0.0.1` or mesh-only default
   - LAN exposure requires explicit auth gate

## Repudiated

These approaches are explicitly rejected:

1. **Hardcoded ghostbridge headers as product path**
   - Self-asserted `x-ghostbridge-footprint` / `x-ghostbridge-trace-id` in git
   - Shipping weak identity path as default
   - **Status**: Residual risk for existing containers; not product path for new edge clients

2. **`host = "0.0.0.0"` + `require_pairing = false`**
   - Exposes gateway to all interfaces without auth
   - LAN trust expansion without capability grants

3. **Inventing second identity control plane**
   - This spec does not define new identity mechanisms
   - Defers to identity-handoff for assertion model

4. **Host wg-lan, CF tunnels into CP, SNI front on public :443**
   - Per topology lock
   - Defers to control-plane spec for public surface

## External Assumptions

This spec assumes but does not validate:

1. **Netmaker tunnel operational**
   - Router (100.69.0.3) can reach host (10.0.0.2)
   - *Verified 2026-08*: ping OK, wget HTTP 200

2. **op-grpc-bridge running and healthy**
   - Listening on 10.0.0.2:8090
   - *Verified 2026-08*: HTTP 200

3. **ZeroClaw binary functional**
   - At /fast/zeroclaw/bin/zeroclaw
   - Gateway mode works
   - *Assumed*: not independently verified this pass

4. **Oracle decoy WG available (for product path)**
   - Assertion signing implemented
   - *Not verified*: blocked

5. **HumanPrincipal registry operational (for product path)**
   - Bridge can resolve principals from assertions
   - *Not verified*: blocked

## Residual Risks

If lab-mode fallback used (before product path ready):

1. **Self-asserted headers are weak identity**
   - Anyone with header values can impersonate
   - Values must be kept out of git, rotated periodically
   - Expiry date required

2. **Lab mode not in Active topology**
   - Must be clearly labeled `residual-risk`
   - Separate from production deployment
   - Calendar reminder for expiry review

3. **Transition gap**
   - Period between lab-mode expiry and product-path readiness
   - May require service unavailability if not planned

## Defers To

| Topic | Authority Spec |
|-------|----------------|
| Assertion crypto, signing, validation | [`netmaker-xray-identity-handoff/`](../netmaker-xray-identity-handoff/) |
| HumanPrincipal registry | [`netmaker-xray-identity-handoff/`](../netmaker-xray-identity-handoff/) |
| Public CF surface, REALITY, mail | [`3tched-ghostbridge-control-plane/`](../3tched-ghostbridge-control-plane/) |
| Topology lock definition | [`README.md`](../README.md) |
| Bridge auth gate implementation | `op-grpc-bridge` crate (not specced here) |
