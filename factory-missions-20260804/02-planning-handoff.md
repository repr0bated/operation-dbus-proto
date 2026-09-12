# Mission 02 — Planning Handoff (planning session on inc3tched, 2026-08-04)

Mission: implement `factory-missions-20260804/02-netmaker-xray-identity.md` (single
NetMaker identity handoff). Working checkout: `/srv/git/odbus` (branch main);
review and commit on a working branch per the mission doc.

## Authoritative decisions (confirmed with user)

1. `claude-redo/netmaker-xray-identity-handoff/` is the REJECTED path
   (wg-lan, op-identity-shuttle, TransportBindingIndex, per-peer OpenFlow tagging).
   Do NOT implement it. Write the corrected spec to
   `kiro/specs/netmaker-xray-identity-handoff/` FIRST as a mission deliverable.
2. No WireGuard on the VPS. WG terminates ONLY at the Oracle decoy. Multiple
   tunnels caused MTU issues — exactly ONE NetMaker transport.
3. "IdentitySled BECOMES the provisioned container" — already partially implemented:
   `crates/op-plugins/src/state_plugins/identity_sled.rs` (ContainerIdentitySled
   embeds IncusInstance; session_id == container name == derived from WG pubkey) and
   `crates/op-grpc-bridge/src/identity_sled_dispatch.rs` (provision_container). Reuse.
4. Assertion carriage: INNER mechanism — short-lived Ed25519-signed
   OracleIdentityAssertion as gRPC metadata inside the existing TLS channel through
   passthrough xray. Fields: human_pubkey | issued_at | expires_at | nonce |
   netmaker_inner_ip | decoy_key_id. op-grpc-bridge is the SOLE validator:
   signature -> expiry -> replay cache (nonce TTL) -> source-IP binding
   (ConnectInfo vs netmaker_inner_ip) -> HumanPrincipal resolution -> existing
   capability gate. No watchers, no polling — connection/login arrival triggers
   resolution.
5. Scope: BOTH sides locally — decoy issuer + local decoy simulator for E2E tests.
   External Oracle/NetMaker/Xray boundaries documented, not deployed.
6. Registered-key -> HumanPrincipal registry: NEW PluginSchema-backed plugin in
   Cozo with issue/resolve/revoke via the generated gRPC surface (canonical plugin
   pattern: inventory::submit!, schemars, dispatch module in op-grpc-bridge,
   MutationEngine arm).
7. Display alias is a separate, display-only concept (never authoritative;
   alias-substitution test required).

## Reuse (verified live in code)

- identity_sled plugin + provision_container dispatch (above)
- GhostbridgeInterceptor per-identity path (verify_per_identity via Cozo
  identity_sled): crates/op-grpc-bridge/src/interceptor.rs
- Capability gate enforce_bridge_capability in PluginService::call_method /
  StateSync::mutate (grpc_server.rs); grants from schema blob +
  /dev/shm/opdbus/capability-grants.json, wildcard fallback, fail-closed
- MutationEngine -> EventChain -> snowball audit -> per-plugin dispatch
  (mutation_engine.rs)
- TLS at bridge: tonic ServerTlsConfig (ZEROCLAW_TLS_CERT/KEY or self-signed)
- incus plugin (full lifecycle via PluginSchema), netmaker plugin (netclient + REST)
- op-xray-daemon: lifecycle only — DO NOT extend for identity

## Required tests (from mission doc)

unknown/revoked keys, expired/replayed assertions, alias/IP/container substitution,
session freshness, bridge authorization. E2E via local decoy simulator over real
TLS to the real bridge (ephemeral localhost ports).

## Boundaries (NEVER violate)

- No deploy, no sudo, no /etc edits, no service restarts, no live-host mutation.
  Cargo tests only.
- No wg-lan, no op-identity-shuttle service, no TransportBindingIndex, no per-peer
  OpenFlow identity code.
- Xray live config only at /etc/xray/xray_config.json inside the Xray container;
  models never write/reload xray.
- Never read credential files in ~/ (master_key.txt, regkeys.txt, token.txt,
  netmk-rollback-*) on this machine.
- Repo conventions: Rust-first (no new Python), D-Bus is the only control plane,
  PluginSchema is source of truth, OSCAL subid taxonomy (see repo CLAUDE.md).

## Environment (verified 2026-08-04 on mail-vps)

- 16 vCPU / 31GB RAM, Artix Linux; rustc/cargo 1.97.1 at /usr/bin (on PATH)
- protoc, pkg-config, openssl 3.6.3, libclang, clang present
- /srv/git/odbus on main @ bf7a9090, writable by jeremy, git HTTPS remote works
- Dep-tree warm-up: cargo check -p op-identity -p op-plugins -p op-cozo-store
  -p op-grpc-bridge (log: /tmp/odbus-warmup.log)
- ed25519-dalek 2.2.0 resolves from crates.io (verified on inc3tched)
- Baseline: cargo test -p op-identity --lib passed 27/27 on inc3tched (re-verify)

## Next steps for the mission session on this machine

1. Re-run validation-readiness baseline: cargo test -p op-plugins --lib,
   -p op-grpc-bridge --lib, -p op-cozo-store --lib (record pre-existing failures)
2. Write corrected spec to kiro/specs/netmaker-xray-identity-handoff/
   (requirements/design/tasks/boundaries)
3. Propose mission (suggested milestones: 1=spec, 2=assertion core + principal
   registry, 3=bridge integration + decoy simulator + E2E, 4=boundary docs +
   negative topology gates)
4. Validation surface: terminal only (cargo tests + gate scripts), no browser/TUI
