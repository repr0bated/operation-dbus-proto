# Cognitive client ingress

## Canonical topology

`op-grpc-bridge` owns the only live `CognitiveMcpServer` and its Cozo store.
All Cognitive calls enter that process through one of these transports:

| Caller | Transport | Endpoint |
| --- | --- | --- |
| Remote native client | TLS gRPC / gRPC-Web | `https://<control-plane>:8090` |
| Host-native client | gRPC over Unix socket | `/run/opdbus/grpc.sock` |
| Container client | gRPC over Unix socket | `/run/ghostbridge/container.sock` |
| Host-local MCP-only client | stdio compatibility adapter | `/usr/local/bin/op-mcp-server --stdio -m cognitive` |

The adapter is only a protocol translation boundary. It dispatches into the
bridge-owned plugin runtime and does not open another Cognitive store or
network listener.

The standalone `op-cognitive-mcp` service is intentionally down. Ports `3003`,
`50051`, and `50052` are retired and must not listen. Port `8090` does not
currently expose MCP Streamable HTTP; clients that only speak MCP use the local
stdio adapter.

## MCP client configuration

Use this configuration for a client running on the control-plane host:

```json
{
  "mcpServers": {
    "cognitive-mcp": {
      "type": "stdio",
      "command": "/usr/local/bin/op-mcp-server",
      "args": ["--stdio", "-m", "cognitive"]
    }
  }
}
```

Repository-ready examples are in:

- `.mcp.json`
- `mcp-client-config.json`
- `deploy/config/factory-mcp.json`

Do not put identity material in those files. The bridge derives host Unix
socket identity from `/etc/op-dbus/host-session-id`; explicitly trusted local
UIDs map to that session through `/etc/op-dbus/host-session-uid-map` using
comma-separated `uid=session_id` entries. Remote callers use the Ghostbridge
identity accepted by the TLS interceptor. Capability grants remain
authoritative in the bridge and are audited against the resolved session.

## Native discovery

The canonical service and all generated per-method services are available by
reflection:

```sh
grpcurl -insecure <control-plane>:8090 list
grpcurl -plaintext -unix /run/opdbus/grpc.sock list
```

An unauthenticated TLS call must fail closed. A host UDS call succeeds only
when the peer uid maps to the configured active Identity Sled session.

## Verification

Run the end-to-end, non-secret verification from the repository root:

```sh
scripts/verify-cognitive-ingress.sh
```

It verifies supervision, retired ports, TLS reflection, rejection of missing
identity, and authenticated host-socket access. The Factory compatibility
entry point, `scripts/verify-factory-cognitive-mcp.sh`, delegates to the same
canonical check and validates the checked-in Factory adapter configuration.

## Operating rule

Models and MCP clients never open the Cognitive database directly. They call
the bridge, the bridge applies identity and capability policy, and the plugin
runtime owns reads and mutations. Add new remotely callable operations to the
plugin schema and regenerate/seal through the normal bridge pipeline; do not
create another port or hand-maintained RPC surface.
