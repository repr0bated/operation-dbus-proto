# MCP Gateway

The only external MCP endpoint is the Streamable HTTP router built into
`op-grpc-bridge`:

```text
https://10.0.0.3:8090/mcp
```

There is no standalone `op-cognitive-mcp` or `compact-mcp`, and no externally
exposed per-provider MCP endpoint. NotebookLM and MongoDB providers do retain
loopback-only listeners on ports `3101` and `3102`. The bridge projects their
typed tools and dispatches admitted calls through the MutationEngine's
in-process plugin dispatchers.

## Protocol contract

The endpoint is stateless JSON-RPC 2.0 and implements MCP protocol version
`2026-07-28`. It supports:

- `initialize`, `notifications/initialized`, `ping`, and `server/discover`
- `tools/list` and `tools/call`
- `resources/list` and `resources/read`

`initialize` negotiates the protocol version and does not require an
`MCP-Protocol-Version` header. Every later request must send:

```http
MCP-Protocol-Version: 2026-07-28
```

`Mcp-Method` and `Mcp-Name` are optional integrity headers. When present, they
must match the JSON-RPC method and tool or resource name. Request bodies are
limited to 1 MiB, requests time out after 90 seconds, and list calls default to
100 entries with a maximum page size of 500.

An initialize request has this shape:

```http
POST /mcp HTTP/1.1
Content-Type: application/json
X-Oracle-Identity-Assertion-Bin: <fresh-canonical-base64url-OIA1>

{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2026-07-28","capabilities":{},"clientInfo":{"name":"example","version":"1.0"}}}
```

The assertion is illustrative: clients must obtain a fresh value from their
identity broker for every request, not copy one into static configuration.

## Authentication

Each request requires exactly one of:

- `X-Oracle-Identity-Assertion-Bin`: canonical unpadded base64url OIA1 for a
  registered human principal.
- `X-Opdbus-Sealed-Id-Bin`: canonical unpadded base64url SID1 from an active,
  anchored local identity sled.

Supplying neither or both returns HTTP `401`. SID1-aware local clients can use:

```bash
op-identity-headers --session <canonical-session-uuid>
```

The helper prints a JSON header object for the selected current session. See
[Identity and Transport](../architecture/identity-transport.md) for credential
validation, replay protection, and D-Bus identity binding.

Browser requests must also provide an `Origin` in the exact
`OP_MCP_ALLOWED_ORIGINS` allowlist. An unlisted origin returns HTTP `403`; so
does an originless request carrying a `Sec-Fetch-*` browser marker.

## Authorization and tool projection

Authentication establishes `principal_id`, `session_id`, and
`session_genesis`; only `principal_id` selects grants. The runtime grant
projection is `/dev/shm/opdbus/capability-grants.json`.

The bridge derives every tool's required capability from the current compiled
plugin schemas. HOT descriptors come from `cognitive_mcp_plugin_schema()` and
typed descriptors from `DefaultPluginRegistry`; sealed SHM blobs are currently
used for MCP resources, not tool descriptor lookup. Clients cannot choose
authority: `X-Opdbus-Capability` is rejected, tools without a schema-declared
capability are denied, and generic invoke tools cannot substitute for a typed
capability.

Without a toolset selector, `tools/list` considers the fixed HOT surface and
returns only entries authorized for the caller:

```text
memory_recall
memory_store
workflow_query
workflow_run
toolsets
```

The deployed toolset manifest groups additional typed tools such as
`plugin.emqx.get_status` or `plugin.mongodb_mcp.find`. Call `toolsets` with
`{"operation":"list"}` to discover sets visible to the current principal, then
with `{"operation":"select","toolset_id":"<id>"}` to obtain a selector. Attach
that selector to subsequent `tools/list` or `tools/call` parameters:

```json
{
  "_meta": {
    "opdbus_toolset": {
      "id": "emqx_ops",
      "generation": 3
    }
  }
}
```

The generation must match the current manifest; after a generation change the
client must relist. A set marked `requires_provider_health` is callable only
while `/run/opdbus/runit-ready/<provider>` is a regular file. Toolset membership
never overrides the caller's exact capability grants.

The bridge currently validates and loads the singleton audience policy at
startup, but the Streamable HTTP tool projection is grant- and toolset-driven.
Do not infer compact tools or additional authority from a client name or the
configured singleton principal alone.

## Resources

`resources/list` and `resources/read` expose sanitized views of the sealed plugin
blob catalog to principals with `cognitive_mcp.read`. The sanitizer removes a
fixed denylist including capability grants, principal membership lists, local
path fields named `internal_path`, `socket_path`, `db_path`, `key_path`, or
`cert_path`, key IDs, approval-verifier configuration, and names ending in
`_secret`, `_token`, `_password`, `_private_key`, or `_api_key`. It is not a
general path or identity-field redactor; schema authors must not assume other
path, principal, or session fields are removed.

## Deployment and checks

Release-owned policy files are:

```text
deploy/security/capability-grants.json
deploy/config/mcp-audience-policy.json
deploy/config/mcp-toolsets.json
```

`deploy/runit/build-golden.sh` validates and installs them as root-owned mode
`0600` files under `/etc/opdbus`. `opdbus-grants` continuously materializes the
grant document into SHM; audience and toolset policy are loaded when
`op-grpc-bridge` starts.

Validate a candidate release before deployment:

```bash
target/release/op-grants-materializer validate \
  deploy/security/capability-grants.json
target/release/op-grants-materializer validate-audience \
  deploy/config/mcp-audience-policy.json
target/release/op-grants-materializer validate-toolsets \
  deploy/config/mcp-toolsets.json
```

Useful runtime checks:

```bash
sudo sv status opdbus-grants op-grpc-bridge
sudo /usr/local/bin/op-grants-materializer check
```

Common failures:

- HTTP `401`: identity credential validation failed.
- HTTP `403`: browser origin validation failed.
- HTTP `400`: JSON parse/envelope failure, protocol/integrity-header mismatch,
  missing tool/resource names caught during header validation, or a
  caller-supplied capability header.
- JSON-RPC `-32602` over HTTP `200`: other dispatch-time invalid parameters.
- HTTP `413`: request body exceeds 1 MiB.
- HTTP `408`: dispatch exceeded 90 seconds.
- JSON-RPC `AccessDenied`: the principal lacks the tool's required capability
  or the tool is outside the current HOT/toolset projection.
- `provider_unavailable`: the selected toolset's runit readiness file is absent.
- `toolset_generation_changed`: policy changed; relist and use the new selector.
