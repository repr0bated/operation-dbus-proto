# `ovsdb.proto`

- **Crate:** `op-openvswitch-daemon`
- **Path:** `crates/op-openvswitch-daemon/proto/ovsdaemon/v1/ovsdb.proto`
- **Package:** `ovsdaemon.v1`

Extended Open vSwitch daemon control: bridge/port CRUD plus network-namespace port
attach/detach.

## Services

### `OvsdbService`

| RPC | Request | Response | Stream |
|---|---|---|---|
| `CreateBridge` | `BridgeRequest` | `BridgeResponse` | - |
| `DeleteBridge` | `BridgeRequest` | `BridgeResponse` | - |
| `ListBridges` | `BridgesRequest` | `BridgesResponse` | - |
| `AddPort` | `PortRequest` | `PortResponse` | - |
| `RemovePort` | `PortRequest` | `PortResponse` | - |
| `ListPorts` | `PortsRequest` | `PortsResponse` | - |
| `ListDatabases` | `DatabaseRequest` | `DatabaseResponse` | - |
| `AttachPortToNetns` | `AttachPortRequest` | `AttachPortResponse` | - |
| `DetachPortFromNetns` | `DetachPortRequest` | `DetachPortResponse` | - |
| `GetStatus` | `StatusRequest` | `StatusResponse` | - |

## Notes

- **Overlap warning:** declares `OvsdbService` in the **same** `ovsdaemon.v1` package as
  [`ovsdaemon.proto`](./ovsdaemon.proto.md). This is a duplicate service name in one
  package and will collide at codegen. This file is the superset (adds `RemovePort` and
  the netns attach/detach RPCs). Confirm which is authoritative and retire the other.

## Gaps / Assumptions

- `op-openvswitch-daemon` may not be an active workspace member; verify against
  `Cargo.toml`.
- Message field-level shapes are not enumerated here; consult the source for exact fields.
