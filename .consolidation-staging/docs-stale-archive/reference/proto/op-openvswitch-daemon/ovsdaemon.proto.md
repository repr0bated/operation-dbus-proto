# `ovsdaemon.proto`

- **Crate:** `op-openvswitch-daemon`
- **Path:** `crates/op-openvswitch-daemon/proto/ovsdaemon/v1/ovsdaemon.proto`
- **Package:** `ovsdaemon.v1`

Open vSwitch daemon control: bridge/port CRUD, database listing, and status.

## Services

### `OvsdbService`

| RPC | Request | Response | Stream |
|---|---|---|---|
| `CreateBridge` | `BridgeRequest` | `BridgeResponse` | - |
| `DeleteBridge` | `BridgeRequest` | `BridgeResponse` | - |
| `ListBridges` | `BridgesRequest` | `BridgesResponse` | - |
| `AddPort` | `PortRequest` | `PortResponse` | - |
| `ListPorts` | `PortsRequest` | `PortsResponse` | - |
| `ListDatabases` | `DatabaseRequest` | `DatabaseResponse` | - |
| `GetStatus` | `StatusRequest` | `StatusResponse` | - |

## Notes

- **Overlap warning:** [`ovsdb.proto`](./ovsdb.proto.md) declares a second
  `OvsdbService` in the **same** `ovsdaemon.v1` package with a superset of these RPCs
  (adds `RemovePort`, `AttachPortToNetns`, `DetachPortFromNetns`). Two services with the
  same name in one package will collide at codegen. Treat this as a duplication/cleanup
  gap; confirm which file is the authoritative source.
- The bridge-level OVSDB projection is `OvsdbMirror` in
  [`op-grpc-bridge/operation.proto`](../op-grpc-bridge/operation.proto.md).

## Gaps / Assumptions

- `op-openvswitch-daemon` may not be an active workspace member; verify against
  `Cargo.toml` before relying on generated code from these protos.
- Message field-level shapes are not enumerated here; consult the source for exact fields.
