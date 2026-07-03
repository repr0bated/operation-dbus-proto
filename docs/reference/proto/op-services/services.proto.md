# `services.proto`

- **Crate:** `op-services`
- **Path:** `crates/op-services/proto/services.proto`
- **Package:** `opdbus.services.v1`
- **Imports:** `google/protobuf/{timestamp,duration}.proto`

Service lifecycle management (s6-style) projected over gRPC: start/stop/restart/reload,
CRUD, enable/disable, and a status watch stream.

## Services

### `ServiceManager`

| RPC | Request | Response | Stream |
|---|---|---|---|
| `Start` | `StartRequest` | `StartResponse` | - |
| `Stop` | `StopRequest` | `StopResponse` | - |
| `Restart` | `RestartRequest` | `RestartResponse` | - |
| `Reload` | `ReloadRequest` | `ReloadResponse` | - |
| `Create` | `CreateRequest` | `CreateResponse` | - |
| `Delete` | `DeleteRequest` | `DeleteResponse` | - |
| `Get` | `GetRequest` | `GetResponse` | - |
| `List` | `ListRequest` | `ListResponse` | - |
| `Enable` | `EnableRequest` | `EnableResponse` | - |
| `Disable` | `DisableRequest` | `DisableResponse` | - |
| `WatchStatus` | `WatchRequest` | `ServiceEvent` | server |

## Notes

- Backed by D-Bus service objects; per the D-Bus-first rule, service control must not
  shell out to `s6-svc`/`systemctl` from plugin/service code.

## Gaps / Assumptions

- Message field-level shapes are not enumerated here; consult the source for exact fields.
