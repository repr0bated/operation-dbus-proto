---
name: emqx-runit
description: Maintain the existing vendor-backed EMQX D-Bus plugin in OP-DBUS on runit, including broker lifecycle, authenticated event distribution, MQTT topic isolation, mutation notifications and generative-UI updates. Use for EMQX or MQTT broker work on this host; not for provisioning another broker or a cloud tenant.
metadata:
  short-description: Existing EMQX broker through D-Bus and runit
---

# EMQX on the OP-DBUS runit host

This is a local adaptation, not an upstream EMQX installation recipe. Netmaker
is retired; its retained EMQX broker is an autonomous vendor binary exposed by
the existing `emqx` plugin. Do not replace it, reinstall it from a package
manager, revive Netmaker, or create another broker/control-plane service.

Read `/srv/git/odbus/AGENTS.md` and recent broker/identity rows in `SIGNALS.md`
before implementation. Apply their mandatory skill preloads. Loading this
skill does not authorize service changes, credential rotation or deployment.

## Authoritative integration

- Plugin schema and bounded upstream adapter:
  `/srv/git/odbus/crates/op-plugins/src/state_plugins/emqx.rs`.
- Mutation dispatch and authenticated ExHook callbacks:
  `/srv/git/odbus/crates/op-grpc-bridge/src/{mutation_engine,emqx_hook_provider,server}.rs`.
- Release inputs: `/srv/git/odbus/deploy/emqx/`,
  `deploy/config/emqx.version`, and `deploy/runit/emqx/`.

Derive new broker methods/signals from typed `PluginSchema` declarations in the
existing plugin. D-Bus is the control plane; gRPC, MCP and UI are projections.
Do not add a parallel direct-admin client in the UI, chatbot or another plugin.
The existing plugin adapter may use the upstream local management API internally.
Read the pinned release and real method declarations rather than inventing CLI
flags, hook versions, capabilities or ready-made publish/subscribe methods.

The current runit D-Bus adapter has a historical `Runit.Systemctl` name. That
wire identifier does not authorize use of the `systemctl` executable; preserve
the existing D-Bus contract when calling it through the EMQX plugin.

## Service operations and release

Application lifecycle operations use the authenticated `emqx` D-Bus plugin's
declared methods. It delegates fixed-service operations to the runit D-Bus
adapter. Do not spawn service-manager subprocesses from plugin/service code.

For explicitly authorized host-operator operations, use runit:

```sh
sudo -n sv status emqx
sudo -n sv check emqx
# Only when the task authorizes a restart:
sudo -n sv restart emqx
```

The service definition is `/etc/runit/sv/emqx/run`; its enabled symlink is under
`/etc/runit/runsvdir/default`. Modify versioned deployment inputs first. Do not
edit `/run/runit/service`, launch `runsv`/`runsvdir` manually, invoke `systemctl`
(including the installed shim), or use legacy s6/service6 commands.

Do not hand-copy binaries onto the running host. From `/srv/git/odbus`, use the
canonical release workflow, inspect the dry run, then deploy only when authorized:

```sh
CXXFLAGS="-include cstdint" cargo build --workspace --release
sudo -n deploy/runit/build-golden.sh --dry-run
sudo -n deploy/runit/build-golden.sh
```

Delegate builds/tests to the cheapest suitable available subagent as requested
by the user; serialize Cargo work. Follow `/verify` before declaring the release
complete. Check the actual broker/plugin behavior after deployment, not only
process uptime. Do not auto-restart network-critical services or touch Xray.

## Transport, identity and privacy

Keep broker listeners internal under the existing deployment policy. Do not
copy the upstream sample's public `0.0.0.0:8883` listener. The authenticated
`:8090` fabric remains the external access path; do not add another public MCP
door or give browsers the broker's service credential.

Preserve the existing loopback mTLS ExHook boundary and pinned broker peer.
ExHook's audit callbacks are not the MQTT authentication/authorization gate.
Changing an `IGNORE` callback into an allow decision is not an auth repair.

Verify session identity and exact grants at the application boundary. A topic,
MQTT client ID, UI mode, project ID or message payload is not identity proof.
Derive authorized topics/scopes server-side; reject wildcard/path injection in
identifiers and prohibit cross-session/project subscriptions. Recheck revocation
and project membership before delivering private events, including replay.
Never copy upstream key-generation or rotation steps onto existing identities.
Do not print credentials or copy private message bodies into general audit logs.

## Mutations and generative UI

EMQX distributes events; it does not replace D-Bus mutation authority, Cozo
persistence, or the audit chain. Distinguish requested, accepted, committed and
failed work. A queue acknowledgement is not a successful application mutation.
Queue consumers must re-enter the authorized mutation path and recheck grants.

Use durable pending-event records with stable IDs, bounded retry/replay, expiry
and idempotent consumers. Coordinate an outbox with database commits; for
external side effects, record/reconcile outcomes rather than asserting a
transaction spanning the database and external service. MQTT QoS alone does not
make an application side effect exactly-once.

For json-render in `/srv/git/json-render-ui-vercel`, keep chatbot-generated
layout/spec patches separate from live state updates. Pin catalog/schema hashes,
sequence resource revisions, deduplicate delivery and recover gaps with an
authorized snapshot. Scope both retrieval and delivery by verified session,
personal/project/control-plane/accountability mode, and project ACL. Automatic
suggestions need an actual model-context delivery path, not just a notification
the model may never receive. No arbitrary code or grants in generated UI.

Treat these as implementation requirements, not claims that the paths are
already wired. Prove authorization denial, duplicate handling, reconnect,
revocation, broker outage recovery and absence of private audit payloads.

## Provenance and version checks

Adapted from the concepts in the community
[mqtt-tls-broker skill](https://github.com/openAut/main/blob/cf2a73e4a639347aef3489d6bc8fb4efa62b2ef2/skills/mqtt-tls-broker/SKILL.md).
Its package installation, systemd, public listener, fresh PKI and telemetry
namespace procedures are deliberately not imported. No upstream scripts/assets
are required or executed by this adaptation.

For the specific feature being changed, consult primary
[EMQX documentation](https://docs.emqx.com/en/emqx/latest/) and match it to the
deployment's pinned version; do not assume newer cloud/enterprise features exist
in the installed binary. For rendering use the local json-render skill and
[official upstream skills](https://github.com/vercel-labs/json-render/tree/main/skills).
