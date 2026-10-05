# gRPC Services

`op-grpc-bridge` is the single process that exposes the Operation control-plane
APIs to host clients, containers, browsers, and the public ingress. It does not
own a second copy of plugin state: plugin calls pass through the bridge's
`MutationEngine`, using the same `PluginSchema` contracts as the D-Bus
projection.

The binary runs `run_zeroclaw_server`; the older `run_grpc_server` function is a
library entry point and is not a second deployed service.

## Listeners

| Listener | Default | Surface |
| --- | --- | --- |
| Host Unix socket | `/run/opdbus/grpc.sock` | Native gRPC and gRPC-Web |
| Shared container socket | `/run/ghostbridge/container.sock` | Same gRPC routes, bind-mounted into participating containers |
| TCP | `0.0.0.0:8090` | TLS-only native gRPC, gRPC-Web, and HTTP MCP |
| EMQX ExHook | disabled; deployment enables `127.0.0.1:9000` | `emqx.exhook.v3.HookProvider` only, loopback-only mutual TLS |

The production runit definition overrides the TCP setting with
`127.0.0.1:8090,10.0.0.3:8090`. It also enables the dedicated ExHook listener.
Do not expose ExHook on the shared `:8090` surface: it has a separate trust
domain and pins the broker client certificate.

TCP startup fails closed when no certificate and key are configured. The
accepted sources, in priority order, are:

1. `ZEROCLAW_TLS_CERT` and `ZEROCLAW_TLS_KEY` containing PEM data.
2. `ZEROCLAW_TLS_CERT_FILE` and `ZEROCLAW_TLS_KEY_FILE` naming PEM files.
3. `ZEROCLAW_DEV_SELF_SIGNED=1`, for development and CI only.

Unix sockets do not use TLS. Authentication and authorization still happen in
the service interceptors; permissive socket mode is required because mapped
container UIDs share the socket.

## Mounted service families

The common route builder mounts these services on both Unix sockets and TCP.
All are also gRPC-Web enabled.

| Package | Services | Purpose |
| --- | --- | --- |
| `operation.v1` | `StateSync`, `PluginService`, `EventChainService` | State, generic plugin calls, and audit-chain queries |
| `operation.v1` | `OvsdbMirror`, `RuntimeMirror`, `DbusPassthrough` | Network, runtime, and explicit D-Bus projections |
| `operation.registry.v1` | `ComponentRegistry` | Component registration and discovery |
| `operation.mail.v1` | `MailService` | Mail operations |
| `operation.privacy.v1` | `PrivacyNetworkService` | Privacy-network operations |
| `operation.registration.v1` | `RegistrationService` | Human registration and WireGuard bootstrap |
| `op_chat.chat` | `ChatService` | Operator chat |
| `zeroclaw` | `ZeroclawService` | Read or watch the configured plugin schema |
| `operation.plugin.v1` | `<Plugin>PluginMethods` | Typed RPCs generated from plugin schemas |
| `grpc.reflection.v1` | `ServerReflection` | Active-service discovery |

The source contracts are under `crates/op-grpc-bridge/proto/`,
`crates/op-grpc-bridge/src/grpc/zeroclaw.proto`, and
`crates/op-chat/proto/chat.proto`.

`HookProvider` is intentionally absent from this table because it is mounted
only on the dedicated ExHook listener. The standard gRPC health service is
mounted by the `run_grpc_server` library path, not by the deployed consolidated
binary.

## Plugin-generated RPCs

Plugin RPC source is `PluginSchema.methods`; do not hand-write per-plugin proto
files. During an `op-grpc-bridge` build, `build.rs`:

1. loads every registered plugin schema;
2. skips schemas with no methods;
3. generates `operation.plugin.v1.<Plugin>PluginMethods`;
4. compiles the generated proto into the combined descriptor set; and
5. generates route implementations that dispatch through `MutationEngine`.

For a plugin ID `cognitive_mcp` and method `invoke_tool`, the callable shape is:

```text
operation.plugin.v1.CognitiveMcpPluginMethods/InvokeTool
```

Input and output messages are typed from each method's JSON Schema. Strings,
booleans, integers, and numbers become `string`, `bool`, `int64`, and `double`;
arrays become repeated fields; objects become `google.protobuf.Struct`; and
untyped values become `google.protobuf.Value`. Enums currently remain strings.
Field numbers are stable FNV-1a hashes of field names, not declaration-order
numbers.

Reflection can also list names such as:

```text
operation.method.cognitive_mcp.invoke_tool.InvokeToolService
```

These per-method names describe sealed, frozen method contracts. The current
router does not mount handlers at those paths. Call the generated
`operation.plugin.v1.*PluginMethods` service instead; calling an
`operation.method.*` path returns `UNIMPLEMENTED`.

## Authentication and capability checks

There are two authenticated gRPC paths:

- Generated plugin RPCs and `PluginService` use a one-time Oracle identity
  assertion in binary metadata key `x-oracle-identity-assertion-bin`.
- Legacy domain services use `x-ghostbridge-genesis` plus either
  `x-ghostbridge-trace-id` or `x-wireguard-pubkey`, resolved against an active
  identity record.

`RegistrationService.SendMagicLink` and `VerifyMagicLink` are the only
identity-bootstrap exceptions. Reflection and the Zeroclaw schema envelope are
discovery surfaces and do not pass through the plugin capability interceptor.

Authentication alone does not authorize a plugin method. The caller must also:

1. declare the method's exact `required_capability`; and
2. have that capability in the exact principal's runtime grant record.

For a generated typed RPC, declare intent with the
`x-opdbus-capability` metadata header. For generic
`PluginService.CallMethod`, set the request's `capability_id` field. Missing
schema capability metadata, a missing declaration, a mismatch, or a missing
principal grant all fail closed.

## Reflection and schema activation

At startup the bridge hydrates reflection from
`/dev/shm/opdbus/plugin-blobs/`, then freezes plugin method descriptors before
building tonic routes. Reflection checks the sealed manifest's `catalog_hash`
on each reflection request, so a catalog-only reseal becomes discoverable
without polling.

Discovery and callability are separate:

- A reseal can refresh the active reflection listing on the next request.
- Generated `operation.plugin.v1` handlers are compiled into the bridge binary.
- D-Bus objects and frozen route state are created at bridge startup.

Therefore a schema or dispatch-code change requires publishing the matching
binary and deliberately restarting `op-grpc-bridge`; a catalog-only reseal is
not enough. See [Sealed Blob Catalog](../architecture/blob-catalog.md) for the
guarded reseal and activation workflow.

## Inspect and troubleshoot

Check the supervised process and sockets:

```bash
sudo sv status op-grpc-bridge
test -S /run/opdbus/grpc.sock
test -S /run/ghostbridge/container.sock
ss -ltn | rg ':(8090|9000)\b'
```

List reflected services over the host socket:

```bash
grpcurl -plaintext -unix /run/opdbus/grpc.sock list
```

Inspect the sealed catalog when a plugin is absent:

```bash
sudo /usr/local/bin/opblob catalog /dev/shm/opdbus/plugin-blobs
```

Common failure patterns:

- `TCP fabric requires TLS`: both certificate-file variables must be readable,
  or both PEM-content variables must be present.
- A plugin is absent from reflection: it has no sealed blob, is retired, or its
  schema declares no methods.
- A service is listed but an `operation.method.*` call is `UNIMPLEMENTED`: use
  the generated `operation.plugin.v1.*PluginMethods` path.
- `UNAUTHENTICATED`: identity metadata is missing, malformed, expired, unknown,
  or replayed.
- `PERMISSION_DENIED`: capability intent does not match the schema or the exact
  principal lacks the runtime grant.
- `SearchSemanticTrace` returns `FAILED_PRECONDITION`: Qdrant was unavailable
  when the bridge initialized its optional semantic shuttle; other bridge
  routes can remain healthy.
