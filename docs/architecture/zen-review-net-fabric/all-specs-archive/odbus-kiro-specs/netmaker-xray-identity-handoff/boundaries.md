# Boundaries — NetMaker / Xray Identity Handoff

This document is the authoritative boundary statement for the corrected
identity handoff. It has three parts: non-negotiable architecture boundaries,
the repudiated design path, and the external integration boundaries that this
mission documents but does not deploy.

## 1 · Non-negotiable architecture boundaries

1. **Oracle decoy is the sole incoming WireGuard termination point.** The
   main host has no incoming identity WireGuard interface; never add
   `wg-lan` or any host WG interface.
2. **Exactly one NetMaker transport.** NetMaker is transport, not the human
   identity authority. Multiple tunnels caused MTU issues.
3. **Assertion carriage is INNER.** The short-lived Ed25519-signed
   `OracleIdentityAssertion` rides as gRPC metadata
   (`x-oracle-identity-assertion-bin`) inside the existing TLS channel.
   Xray is a passthrough and never sees plaintext identity state.
4. **`op-grpc-bridge` is the sole validator and the application
   authorization boundary.** Validation order: parse → structural lifetime
   (`expires_at <= issued_at`) → trusted decoy key → signature → expiry →
   replay cache → source-IP binding → HumanPrincipal resolution → existing
   capability gate. Every step fail-closed.
5. **Human identity, WireGuard key, login session, workspace container, and
   display alias are separate concepts.** A workspace container is not the
   human. System containers are never users. The display alias is
   display-only and never authoritative.
6. **Connection/login arrival triggers resolution.** No handshake watchers,
   no polling loops, no D-Bus watchers, no background expiry tasks (the
   replay cache purges lazily on access).
7. **Xray live config exists only at `/etc/xray/xray_config.json` inside the
   xray container.** Models do not write or reload xray directly.
   `op-xray-daemon` remains lifecycle-only.
8. **PluginSchema is the source of truth.** Identity operations are methods
   on the `human_principal` plugin via the generated gRPC surface
   (`PluginService` → D-Bus → `MutationEngine`). No hand-written per-plugin
   proto, no direct backend RPC, no second identity control plane.

## 2 · Repudiated design path (do not reintroduce)

The rejected spec `claude-redo/netmaker-xray-identity-handoff/` proposed the
following, all of which are forbidden in this architecture:

| Rejected mechanism | Why it is rejected |
|---|---|
| `wg-lan` host WireGuard interface | Violates boundary 1; WG terminates only at the Oracle decoy |
| `op-identity-shuttle` handshake-watcher service | Polling-based; violates boundary 6 |
| `TransportBindingIndex` (src_ip → pubkey SHM table) | Source-IP assertion is not a cryptographic binding; 180 s race window |
| Per-peer OpenFlow identity tagging (`NXM_NX_REG` tags) | Datapath theater; the real gate remained the IP lookup |
| Per-registration identity containers as verifiers | Turns a network segment into a trusted intermediary; verification belongs at the single existing gate |
| Xray header injection into TLS | Impossible without xray becoming a TLS-terminating MITM |

These tokens are enforced by the negative topology gates
(`crates/op-grpc-bridge/tests/negative_topology_gates.rs`,
`scripts/check-identity-topology.sh`).

## 3 · External integration boundaries (documented, NOT deployed by this mission)

The mission implements both sides LOCALLY: the decoy issuer as a library plus
a local decoy simulator for E2E tests over real TLS on ephemeral localhost
ports. The following are external assumptions that production deployment must
satisfy; they are documented here and tested only through their local
stand-ins.

### 3.1 Oracle decoy (EXTERNAL)

- Terminates the human's WireGuard tunnel; the kernel verifies the peer.
- Issues the signed assertion (decoy Ed25519 key identified by
  `decoy_key_id`) with TTL ≤ 900 s for the **WG-authenticated peer pubkey**.
- **Enrollment is out-of-band, not at connect time.** Humans are
  pre-registered via `human_principal.register_key` (ops/admin through the
  generated gRPC/`PluginService` surface) before they are expected to
  authenticate. The decoy does **not** call live `resolve_key` on the bridge
  as a prerequisite to issuance; unknown/revoked keys are rejected at the
  bridge's resolve step when the assertion is presented.
- The decoy's verifying keys are provisioned to the bridge's trust store
  (`OP_DECOY_TRUST_STORE`, default `/etc/opdbus/decoy-trust.json`) by an
  operational process outside this mission.

### 3.2 NetMaker transport (EXTERNAL)

- Exactly one tunnel carries decoy→host traffic.
- **Inner-IP preservation assumption**: the human's NetMaker inner IP
  (`netmaker_inner_ip`) must be the source IP observed by the bridge on the
  TLS connection — i.e., no NAT rewrites the inner source address along the
  decoy → NetMaker → xray → bridge path. If a deployment cannot preserve
  this, the source-IP binding step must be re-specified before enabling the
  assertion path there.
- NetMaker ACLs (`OP_NETMK_*`) remain the transport-level policy; they are
  not identity.

### 3.3 Xray container (EXTERNAL, unchanged)

- Passthrough only: SNI/protocol sniffing, no TLS termination for this
  traffic, no header injection, no identity logic.
- Live config only at `/etc/xray/xray_config.json` inside the container;
  models never write or reload xray.

## 4 · Adjacent ops spec (documented, not implemented here)

**Cross-reference:** `.kiro/specs/3tched-ghostbridge-control-plane/`

| Concern | Owner |
|---|---|
| Public CF surface, mail half-and-half, REALITY camouflage ops | control-plane spec |
| Thin email enroll *channel* (not assertion crypto) | control-plane spec |
| Human WG terminates at Oracle decoy; no host `wg-lan` | BOTH (this mission enforces in code gates) |
| NetMaker = transport only, not identity authority | BOTH |
| `OracleIdentityAssertion` / HumanPrincipal / bridge validator | THIS mission |
| OpenFlow IP:port demux (non-identity) | control-plane / existing OVS work |

Do not reintroduce CF tunnels/live streams as identity transport. Assertion
carriage remains INNER gRPC metadata after decoy WG auth.

## 5 · Mission safety boundaries (workers NEVER violate)

- No deploy, no sudo, no `/etc` edits, no service restarts, no live-host
  mutation. Cargo tests only.
- Never read credential files in `~/` (`master_key.txt`, `regkeys.txt`,
  `token.txt`, `netmk-rollback-*`).
- Rust-first: no new Python; scripts are shell.
- All cargo commands use `CXXFLAGS="-include cstdint"`; bridge lib tests run
  with `-- --test-threads=1` (pre-existing env/SHM race).
