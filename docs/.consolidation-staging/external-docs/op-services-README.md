# op-services

System-wide service manager - systemd replacement with dinit backend.

## Overview

op-services provides a complete systemd replacement using:
- **dinit** as the lightweight init system (PID 1)
- **gRPC** for internal service-to-service communication
- **D-Bus** for system integration and systemctl compatibility

## Architecture

```
┌─────────────────────────────────────────┐
│              op-web / op-mcp            │
│                    │                    │
│               gRPC (50051)              │
└────────────────────┼────────────────────┘
                     ▼
┌─────────────────────────────────────────┐
│              op-services                │
│  ┌─────────────┐  ┌─────────────────┐   │
│  │ gRPC Server │  │ D-Bus Interface │   │
│  └──────┬──────┘  └────────┬────────┘   │
│         └────────┬─────────┘            │
│                  ▼                      │
│         ┌────────────────┐              │
│         │ ServiceManager │              │
│         └────────┬───────┘              │
│                  ▼                      │
│    ┌─────────────┴─────────────┐        │
│    ▼                           ▼        │
│ DinitProxy              ProcessManager  │
│ (D-Bus to dinit)        (fallback)      │
└─────────────────────────────────────────┘
```

## Binaries

- `op-services` - Main daemon (gRPC + D-Bus server)
- `systemctl` - gRPC client (remote/network)
- `systemctl-native` - D-Bus client (local, works at boot)

## Schema-as-Code

Service definitions are Rust types - the source of truth:

```rust
pub struct ServiceDef {
    pub name: ServiceName,
    pub service_type: ServiceType,
    pub exec: ExecConfig,
    pub depends_on: Vec<ServiceName>,
    pub restart: RestartPolicy,
    pub environment: HashMap<String, String>,
    pub resources: Option<ResourceLimits>,
    pub health_check: Option<HealthCheck>,
    pub enabled: bool,
}
```

## gRPC API

Port: 50051

```protobuf
service ServiceManager {
    rpc Start(StartRequest) returns (StartResponse);
    rpc Stop(StopRequest) returns (StopResponse);
    rpc Restart(RestartRequest) returns (RestartResponse);
    rpc List(ListRequest) returns (ListResponse);
    rpc WatchStatus(WatchRequest) returns (stream ServiceEvent);
}
```

## D-Bus Interface

Bus name: `org.opdbus.services`
Object path: `/org/opdbus/services`

Methods:
- `Start(name: string) -> string`
- `Stop(name: string) -> string`
- `Restart(name: string) -> string`
- `GetStatus(name: string) -> string`
- `ListServices() -> array<string>`

## Usage

```bash
# Start daemon
op-services

# Use systemctl (gRPC)
systemctl start myservice
systemctl status myservice
systemctl list-units

# Use native systemctl (D-Bus, no network)
systemctl-native start myservice
```

## Configuration

Services stored in SQLite: `/var/lib/op-services/services.db`

dinit service files: `/etc/dinit.d/`
