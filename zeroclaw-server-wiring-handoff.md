# ZeroClaw Bridge Wiring — Handoff

_Updated 2026-07-29._

## Resolved architecture

`op-grpc-bridge` is the ZeroClaw runtime. Do not configure or start the
upstream `zeroclaw` daemon/gateway beside it. There is one routing authority:
the schema-backed bridge.

The authoritative contracts are:

- `crates/op-plugins/src/state_plugins/zeroclaw.rs`
- `crates/op-plugins/src/state_plugins/ghostbridge.rs`

Schemars-derived `PluginSchema` documents declare the provider catalog, model
routes, selection methods, model-role assignments, Ghostbridge identity, and
transport surfaces. Sealed plugin blobs and SHM projections carry those
contracts into the running bridge.

## Runtime flow

1. Ghostbridge authenticates the caller and attaches its identity.
2. The caller invokes a declared ZeroClaw method.
3. The mutation engine records the method call in the event chain.
4. ZeroClaw resolves the selected/requested provider and model against its
   projected schema catalog.
5. `op-llm::ChatManager` performs only the resolved upstream provider call.
6. The method result is returned through the original transport.

`Chat`, `ListModels`, `SetProvider`, `SetModel`, and the role-specific model
setters are schema methods. Selection mutations are persisted to the ZeroClaw
projection. Ghostbridge's declared read methods have domain dispatchers rather
than falling through to the generic JSON echo.

## Compatibility transports

Compatibility endpoints are adapters to schema methods, not another routing
authority:

- `POST /v1/chat/completions` → `zeroclaw.Chat`
- `GET /v1/models` → `zeroclaw.ListModels`
- `POST /api/zeroclaw/chat` → `zeroclaw.Chat`
- `POST /api/llm/chat` → `zeroclaw.Chat`
- `op_chat.chat.ChatService.Send` → `zeroclaw.Chat`

All retain Ghostbridge identity/capability enforcement and use the same
audited method dispatcher.

## Canonical D-Bus contract

The only plugin service/tree is:

- service: `org.opdbus.v1.plugins`
- base path: `/org/opdbus/v1/plugins`
- object path: `/org/opdbus/v1/plugins/<plugin>`
- interface: `org.opdbus.v1.PluginV1`
- compatibility members: `Call`, `GetProperty`, `GetAllProperties`,
  `SetProperty`

Schema method names are arguments to `PluginV1.Call`; they are not additional
D-Bus services, object trees, or per-plugin interfaces.

## Service supervision

Host `op-*` services use runit. They were verified with `sudo sv status`; do
not introduce s6 supervision. Container/application lifecycle operations must
continue through the service-manager D-Bus API via `busctl`.

## Xray invariant

Xray's live configuration must exist only at
`/etc/xray/xray_config.json` inside its container. The static bootstrap
materializes that path until the validated bridge generator replaces the same
file atomically and requests reload through D-Bus. Models must never write or
reload Xray directly.

## Remaining operational step

After tests and review, deploy the rebuilt bridge using the existing D-Bus
deployment path, then verify:

- the canonical D-Bus tree and `PluginV1` introspection;
- authenticated `ListModels` and `Chat` method calls;
- OpenAI-compatible model/chat responses on port 8090;
- persisted provider/model selection after a bridge restart.
