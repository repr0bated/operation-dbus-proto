# op-runit-systemctl

D-Bus service that exposes runit lifecycle and status operations on the system
bus. It is the application-facing service-management boundary for the Artix
runit host.

## Overview

The crate builds two binaries:

- `op-runit-systemctl`: owns the D-Bus interface and translates calls to runit.
- `svd`: a diagnostic client for the same interface.

Operators use `sudo sv` for routine host administration. Application code uses
this D-Bus service so lifecycle calls cross one authenticated system-bus
boundary. Mutating methods verify that the D-Bus caller has UID 0.

## D-Bus Interface

| Item | Value |
|---|---|
| Bus | system |
| Destination | `org.opdbus.v1.Runit.Systemctl` |
| Object path | `/org/opdbus/v1/plugins/runit/systemctl` |
| Interface | `org.opdbus.v1.Runit.Systemctl` |

## Method Mapping

Zbus exports the Rust method names in PascalCase.

| D-Bus method | Implementation | Result |
|---|---|---|
| `Start(service)` | Ensure live symlink, then `sv up` | `(bool, string)` |
| `Stop(service)` | `sv down` if supervised | `(bool, string)` |
| `Restart(service)` | `sv restart`, or add live symlink and start | `(bool, string)` |
| `Reload(service)` | `sv hup` | `(bool, string)` |
| `Enable(service)` | Add persistent and live symlinks; remove `down` | `(bool, string)` |
| `Disable(service)` | `sv down`; remove persistent and live symlinks | `(bool, string)` |
| `Status(service)` | Parse `sv status` | JSON string |
| `IsActive(service)` | Query `sv status` | `active` or `inactive` |
| `IsEnabled(service)` | Check persistent symlink | `enabled` or `disabled` |
| `IsEnabledMethod(service)` | Compatibility alias for `IsEnabled` | `enabled` or `disabled` |
| `Show(service)` | Compose systemctl-style properties | JSON string |
| `ListUnits()` | Scan `/run/runit/service` | JSON array string |
| `ListUnitFiles()` | Scan `/etc/runit/sv` | JSON array string |
| `Journalctl(service, lines)` | Tail the first known service log | JSON array string |
| `DaemonReload()` | Authorized no-op | `(bool, string)` |
| `DaemonStatus()` | Check `runsvdir` or its runtime tree | `running` or `not-available` |
| `GetUnitType(service)` | Read optional `type` marker | string; defaults to `longrun` |

Runit has no compiled service database. `DaemonReload` exists for compatibility
and returns success without changing state.

## Runit Layout

| Path | Meaning |
|---|---|
| `/etc/runit/sv/<service>` | Definition |
| `/etc/runit/runsvdir/default/<service>` | Persistent boot-enable symlink |
| `/run/runit/service/<service>` | Live supervised service |
| `/etc/runit/sv/<service>/down` | Do-not-auto-start marker |

Do not edit `/run/runit/service` directly. It is the supervisor's runtime view.

## Input and Authorization

Lifecycle methods (`Start`, `Stop`, `Restart`, `Reload`, `Enable`, and
`Disable`) require a root D-Bus caller. `DaemonReload` has the same root check.

Lifecycle service names must:

- be 1–128 bytes;
- contain only ASCII letters, digits, `_`, `-`, `.`, or `@`;
- not be `.` or `..`.

Errors from lifecycle methods are returned as `(false, message)`. Transport and
proxy failures remain D-Bus errors.

## Status Output

`Status` returns a serialized JSON object:

```json
{
  "name": "op-web",
  "active_state": "active",
  "sub_state": "running",
  "main_pid": 1234,
  "ready": true,
  "up_time": "56s"
}
```

The D-Bus type is a string, not a structured D-Bus object; callers must decode
the JSON payload.

## Building

```bash
cargo build -p op-runit-systemctl
cargo test -p op-runit-systemctl
```

## Running and Inspecting

```bash
# Normal deployment: runit supervises the daemon.
sudo sv status op-runit-systemctl

# Verify the destination, path, and interface.
sudo busctl --system introspect \
  org.opdbus.v1.Runit.Systemctl \
  /org/opdbus/v1/plugins/runit/systemctl \
  org.opdbus.v1.Runit.Systemctl

# Exercise the D-Bus client.
sudo ./target/debug/svd status op-web

# Direct development run; requires access to the system bus and bus name.
sudo cargo run -p op-runit-systemctl --bin op-runit-systemctl
```

The deployed run script waits for `dbus`, then executes
`/usr/local/bin/op-runit-systemctl`. Its readiness check introspects this
interface on the system bus.

See the
[service-management runbook](../../docs/src/operations/service-management.md)
for deployment workflow and troubleshooting.
