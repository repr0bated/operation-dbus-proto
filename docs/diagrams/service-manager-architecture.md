# Service Manager Architecture

No trait hierarchy. Single `ServiceManager` struct. `DinitProxy` is primary; `ProcessManager` is
fallback when dinit D-Bus is unavailable. Store reads/writes via `org.opdbus.StateManager` D-Bus —
no local SQLite.

```mermaid
---
config:
  theme: neo-dark
---
graph TD
    GRPC["gRPC Server<br/>opdbus.services.v1.ServiceManager"]
    DBUS_IF["D-Bus Interface<br/>org.opdbus.services.v1.Manager"]
    SOCK["SocketEndpoint<br/>/org/opdbus/services/endpoints/{name}<br/>(gateway, mail, dns — socket path discovery)"]

    MGR["ServiceManager<br/>start / stop / restart / create / delete / list"]

    DINIT["DinitProxy<br/>org.chimera.dinit.Manager<br/>(primary — via zbus)"]
    PROC["ProcessManager<br/>(fallback — direct tokio::process)"]

    STORE["Store<br/>reads/writes via org.opdbus.StateManager<br/>D-Bus proxy — no local SQLite"]

    GRPC --> MGR
    DBUS_IF --> MGR
    SOCK --> DBUS_IF

    MGR -->|"dinit D-Bus available"| DINIT
    MGR -->|"dinit D-Bus unavailable"| PROC
    MGR --> STORE

    DINIT -->|"StartService / StopService<br/>GetServiceStatus / ListServices"| DINIT_BUS["org.chimera.dinit<br/>/org/chimera/dinit"]
    STORE -->|"QueryState / ApplyContractMutation"| STATE_MGR["org.opdbus.StateManager<br/>/org/opdbus/state"]
```

## Key facts for models

- **No systemd backend** — dinit only, via `org.chimera.dinit` D-Bus interface
- **No StubBackend / trait hierarchy** — `DinitProxy` and `ProcessManager` are concrete structs, not trait impls
- **No SQLite** — `Store` is a D-Bus client for `org.opdbus.StateManager`; service definitions live in the op-dbus state tree
- **`ProcessManager`** — fallback only; spawns processes directly via `tokio::process::Command` when dinit D-Bus is not reachable
- **`SocketEndpoint`** — publishes socket paths on D-Bus so consumers (op-web etc.) don't hardcode paths
- **State change events** — `ServiceManager` broadcasts `ServiceEvent` via `tokio::sync::broadcast` channel; gRPC `WatchStatus` streams from this
