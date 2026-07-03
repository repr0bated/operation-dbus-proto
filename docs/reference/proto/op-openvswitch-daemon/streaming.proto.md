# `streaming.proto`

- **Crate:** `op-openvswitch-daemon`
- **Path:** `crates/op-openvswitch-daemon/proto/ovsdaemon/v1/streaming.proto`
- **Package:** `ovsdaemon.v1`

Server-streaming subscriptions for OVSDB updates, flow updates, and topology events.

## Services

### `OvsdbStreamService`

| RPC | Request | Response | Stream |
|---|---|---|---|
| `SubscribeOvsdbUpdates` | `SubscribeRequest` | `OvsdbUpdate` | server |
| `SubscribeFlowUpdates` | `SubscribeRequest` | `FlowUpdate` | server |
| `SubscribeTopology` | `SubscribeRequest` | `TopologyEvent` | server |

## Notes

- Complements the request/response `OvsdbService` with live push updates.

## Gaps / Assumptions

- `op-openvswitch-daemon` may not be an active workspace member; verify against
  `Cargo.toml`.
- Message field-level shapes are not enumerated here; consult the source for exact fields.
