This is a revision pass on the existing netmaker-xray-identity-handoff spec
(.kiro/specs/netmaker-xray-identity-handoff/{requirements.md,design.md,tasks.md}). Read all
three fully first — they are the baseline. This revision corrects a wrong architectural
assumption they made and narrows scope based on decisions made since. Do not re-run the whole
investigation from scratch; treat the existing three files as a correct starting point except
for the specific corrections below.

## Correction 1: WireGuard termination point

The existing design.md (§2, §3, §7.2) and tasks.md (H-1, H-2) assume WireGuard terminates "at
netmaker" and that `watch_wireguard_handshakes()` watches netmaker's mesh interface. This is
wrong. Ground truth from the code: `watch_wireguard_handshakes(iface: &str)` in
crates/op-identity/src/schema_bridge.rs:1132 watches whatever interface name is passed to it;
its only caller, `run_schema_shuttle()` (schema_bridge.rs:1244), reads the interface from the
`WG_INTERFACE` env var, defaulting to `wg0` if unset. Nothing in the repo currently sets
`WG_INTERFACE`, and no runit service currently invokes the `op-identity-shuttle` binary that
would start this watcher — it is not running on the live host today.

Correct this: `WG_INTERFACE` must be set to `wg-lan` — a standalone WireGuard server built
this session specifically for identity, independent of netmaker's mesh (deliberately decoupled
to avoid netmaker's MTU constraints; see `/etc/wireguard/wg-lan.conf` and
`/etc/runit/sv/wg-lan/`). Netmaker's own mesh traffic is NOT part of this system and keeps
using its existing IP-ACL model (`OP_NETMK_*` chains) untouched — do not couple the two.
Update all diagrams, prose, and tasks referring to "netmaker/WG termination" to say `wg-lan`,
and add a task that stands up the `op-identity-shuttle` runit service (it doesn't exist yet)
with `WG_INTERFACE=wg-lan`.

## Correction 2: xray cannot inject headers into passthrough TLS — and shouldn't need to

design.md §5 proposes a new xray Go plugin or Rust sidecar to inject identity headers into
xray-routed traffic. This is unworkable and unnecessary:

- Live `/etc/xray/xray_config.json` shows the public-facing inbound (`xhttp-in`, port 8444)
  running `"security": "none"` with `"sniffing": {"routeOnly": true}` — xray only peeks at
  SNI/protocol to route, never decrypts. It structurally cannot inject HTTP headers into that
  traffic without becoming a full TLS-terminating MITM proxy, which is a much bigger, riskier
  change than the spec currently implies and is NOT in scope for this revision.
- Separately, tonic's TLS (crates/op-grpc-bridge/src/server.rs:499,
  crates/op-grpc-bridge/src/grpc_server.rs:862, via `tonic::transport::ServerTlsConfig`) is a
  completely separate TLS boundary from xray's, terminated inside the Rust gRPC server itself.

Drop the xray plugin/sidecar approach entirely. It is also the wrong target: the previously
assumed target (the `assistant` container's dokodemo-door path on port 8090) is NOT part of
this system's scope — `assistant` is already-decided host-local control-plane traffic, keep it
as-is, do not touch it or use it as a reference example.

## Correction 3: identity verification lives in a per-registration provisioned container

The actual scope of this spec is narrower than "gate all xray traffic." Concretely:

- Customer/subscriber privacy-tunnel traffic (netmaker's actual product — what a subscriber's
  WireGuard tunnel carries) MUST remain pure passthrough. xray must never inspect, decrypt, or
  inject anything into it. This is a hard constraint, not an oversight to fix later.
- Mail (mail-3tched), Qdrant, and similar services keep their own existing separate
  ingress/egress paths (hand-configured per-port `incus proxy` devices) — out of scope,
  untouched.
- The identity-verification system instead belongs to a NEW, currently-nonexistent "identity
  container" — a dedicated workspace that gets provisioned at netmaker registration time (i.e.
  triggered by the same enrollment-key/registration flow used to onboard a new peer this
  session — see netmaker's `enrollment_keys_v1`/`tenants_v1` tables and the join-token flow).
  Each registered identity gets its own provisioned container/workspace with its own egress.
  This does not exist yet and must be designed and built, not retrofitted onto xray or
  `assistant`.
- Verification logic (the transport-binding lookup: source IP/peer → verified WG pubkey →
  footprint, and the capability-grants check) lives in Rust inside that provisioned identity
  container — matching this repo's Rust-first convention (CLAUDE.md: "Rust-first: no new
  Python; scripts are shell") — not as a new xray Go plugin, not bolted onto the existing
  `assistant` path.

## What to produce

Revise requirements.md, design.md, and tasks.md in place to reflect all three corrections
above. Specifically:
1. Replace every "netmaker/WG termination" reference with `wg-lan`, and add the missing
   `op-identity-shuttle` runit service as an explicit task.
2. Remove the xray Go plugin/sidecar design and the invalid xray-core JSON example
   (`"proxySettings": {"tag": ...}` on a freedom outbound is not real xray-core schema —
   real outbound chaining uses `streamSettings.sockopt.dialerProxy`). Do not replace it with
   another xray-side injection mechanism — per Correction 2/3, injection doesn't happen in
   xray at all.
3. Add an explicit "Out of Scope" entry for customer/subscriber tunnel traffic and for
   mail/qdrant/similar, stating plainly that xray remains pure passthrough for them.
4. Add a new section (design.md) covering the per-registration identity container: what
   triggers provisioning (netmaker registration/enrollment), what it contains (transport
   binding index, verification logic, its own egress), and its lifecycle (created at
   registration — what about deprovisioning/expiry? — address this explicitly, don't leave it
   open).
5. Update tasks.md's implementation sections accordingly — drop the xray-injection tasks
   (X-1 through X-7, C-1 through C-8 as currently written), replace with tasks for: the
   `op-identity-shuttle` runit service, the `wg-lan`-scoped binding index, the
   per-registration container provisioning trigger and lifecycle, and the in-container Rust
   verification logic. Keep the existing Grants Materialization Reliability section (§3 /
   G-1 through G-7) as-is — that part of the original spec was correct and still applies.

Keep the same file locations, same evidence-backed `[x]`/`[ ]` checkbox rigor, same
fail-closed-on-every-step discipline as the existing tasks.md and as
.kiro/specs/netclient-container-netns/tasks.md.
