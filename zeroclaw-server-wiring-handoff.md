# ZeroClaw Bridge Wiring — Handoff

_Updated 2026-08-08._

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
authority. Ordinary HTTP is served by `op-web` on port 8080; the bridge's
port 8090 remains gRPC/gRPC-Web only:

- `POST /v1/chat/completions` → `zeroclaw.Chat`
- `GET /v1/models` → `zeroclaw.ListModels`
- `POST /api/zeroclaw/chat` → `zeroclaw.Chat`
- `POST /api/llm/chat` → `zeroclaw.Chat`
- `op_chat.chat.ChatService.Send` → `zeroclaw.Chat`

All retain Ghostbridge identity/capability enforcement and use the same
audited method dispatcher. The compatibility OpenAPI document is generated
locally from the sealed `Chat` and `ListModels` Schemars contracts; there is no
external Antigravity documentation or routing dependency.

### OpenAI-compatible usage values

The three HTTP chat routes normalize `usage.prompt_tokens`,
`usage.completion_tokens`, and `usage.total_tokens` to non-negative integers.
Integer values pass through, signed values are clamped at zero, floats are
rounded, and unparseable values become zero. Other `usage` keys are preserved.
A missing or non-object `usage` value is returned as `null`.

This normalization happens only in the `op-web` HTTP adapter after
`zeroclaw.Chat` returns. It does not affect
`op_chat.chat.ChatService.Send`. It also cannot recover precision already lost
inside a provider: `op-llm`'s current Salad parser reads token counts with
`as_u64()`, so upstream float counts can become zero before the HTTP adapter
sees them. If a Salad response has zero usage, inspect the provider parsing
path rather than treating the HTTP coercion as an accounting fix.

## Canonical D-Bus contract

The only plugin service/tree is:

- service: `org.opdbus.v1.plugins`
- base path: `/org/opdbus/v1/plugins`
- object path: `/org/opdbus/v1/plugins/<plugin>`
- interface: `org.opdbus.v1.PluginV1`
- compatibility members: `Call`, `GetProperty`, `GetAllProperties`,
  `SetProperty`

Schema method names are arguments to `PluginV1.Call`; they are not additional
D-Bus services, object trees, or per-plugin interfaces. Provider/model
selection data remains in the schema and projection returned by `PluginV1`;
it is not published as route/provider child objects. Bridge listener
configuration comes from its environment and SIGHUP reload path, not a second
runtime-configuration D-Bus object.

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

## Deployment verification

The release bridge and blob sealer were deployed and the bridge was restarted
under runit on 2026-07-29. The live process checksum matches the installed
release binary.

Verified live:

- all host `op-*` services are up under runit;
- `org.opdbus.v1.plugins` is owned by `op-grpc-bridge`;
- the ZeroClaw object exposes only `org.opdbus.v1.PluginV1` with `Call`,
  `GetProperty`, `GetAllProperties`, and `SetProperty`;
- the sealed catalog contains `zeroclaw` and no `s6`/`s6_systemctl` plugin;
- authenticated `zeroclaw.ListModels` returns an audited success envelope with
  12 declared routes on consecutive calls;
- authenticated `GET /v1/models` on port 8080 returns the schema-declared
  catalog as 9 unique OpenAI-compatible models, while the same path on 8090 is
  gRPC rather than ordinary HTTP;
- `/api/plugin-schema/zeroclaw` serves Schemars field descriptions, and the
  combined chat schema derives 13 provider options and 12 model-route options
  from the same sealed plugin schema.

An upstream paid chat request was deliberately not sent during deployment.
Provider availability remains a live projection concern; routing authority
remains the bridge regardless of which provider is selected.
