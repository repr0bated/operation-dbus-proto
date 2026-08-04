# Factory mission 2 — single NetMaker identity handoff implementation

Implement `.kiro/specs/netmaker-xray-identity-handoff/` as the intended
architecture, not the rejected `wg-lan` recovery path.

## Goal

Implement one trust chain:

```text
human device
 -> WireGuard authentication at Oracle decoy
 -> registered key resolves to HumanPrincipal
 -> short-lived Oracle IdentityAssertion
 -> one NetMaker transport
 -> Xray container validates and forwards identity context
 -> TLS op-grpc-bridge validates session/capability
 -> PluginSchema / generated PluginService / D-Bus MutationEngine
```

## Non-negotiable boundaries

- Oracle decoy is the sole incoming WireGuard termination point.
- Main host has no incoming identity WireGuard interface; never add `wg-lan`.
- NetMaker is transport, not the human identity authority.
- Human identity, WireGuard key, login session, workspace container, and
  display alias are separate concepts.
- A workspace container is not the human. System containers are never users.
- Connection/login arrival triggers resolution; no handshake watcher or
  polling service.
- Xray live config exists only at
  `/etc/xray/xray_config.json` inside the Xray container.
- Models do not write or reload Xray directly.
- Do not use `op-identity-shuttle`, `TransportBindingIndex`, or per-peer
  OpenFlow assumptions.

## gRPC requirements

- Preserve TLS `op-grpc-bridge` as the application authorization boundary.
- Preserve `PluginService` → D-Bus → `PluginSchema`/`MutationEngine`.
- If identity operations are needed, add them to `PluginSchema` and use the
  generated gRPC surface.
- Do not create a hand-written per-plugin proto, direct backend RPC, or a
  second identity control plane.
- Do not claim Xray can inject HTTP headers into opaque TLS. Select and
  document an authenticated inner or sideband assertion-carriage mechanism.

## Deliverables

- Implement the smallest coherent code path supported by the current code.
- Add tests for unknown/revoked keys, expired/replayed assertions, alias/IP/
  container substitution, session freshness, and bridge authorization.
- Document external Oracle/NetMaker/Xray integration boundaries that cannot be
  implemented locally.
- Review and commit on the receiving checkout’s working branch.

## Verification and safety

Run focused Rust/gRPC/schema/D-Bus tests and negative topology checks. Do not
deploy, restart services, use `sudo`, edit `/etc`, or mutate the live host.

