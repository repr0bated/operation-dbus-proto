# Service Management

The host boots **runit** as PID 1. Use `sudo sv` for host operations; s6 and
systemd commands do not apply.

Application code uses the audited
`org.opdbus.v1.Runit.Systemctl` system-bus service instead of spawning a
service-manager CLI. The daemon is implemented by the `op-runit-systemctl`
crate and is itself supervised as `op-runit-systemctl`.

## Layout

| Path | Purpose |
|---|---|
| `/etc/runit/sv/<service>/run` | Service definition |
| `/etc/runit/sv/<service>/check` | Optional readiness check |
| `/etc/runit/sv/<service>/log/run` | Optional `svlogd` companion |
| `/etc/runit/sv/<service>/down` | Prevent automatic start while supervised |
| `/etc/runit/runsvdir/default/<service>` | Boot-enable symlink |
| `/run/runit/service/<service>` | Live supervisor view |
| `/var/log/op-dbus/<service>/current` | Log file for repository definitions that use `svlogd` |

Definitions under `/etc/runit/sv` are the source of truth. Do not edit
`/run/runit/service`: it is runtime state maintained by the supervisor.
Runit has no compiled service database, so there is no daemon-reload or
recompile step after changing a definition.

## Routine host operations

Read state before making a change, and use one lifecycle operation at a time:

```sh
sudo sv status op-web
sudo sv restart op-web
sudo sv check op-web
sudo sv status op-web
```

`sv restart` is a native operation; do not replace it with a chained stop and
start. `sv` needs root to read `supervise/ok`, so an unprivileged status command
can fail with `access denied` even when the service is healthy.

To inspect logs:

```sh
sudo tail -n 100 /var/log/op-dbus/op-web/current
sudo tail -f /var/log/op-dbus/op-web/current
```

Not every service has a log companion. Check
`/etc/runit/sv/<service>/log/run` before assuming the log path exists.

### Enable or disable a service

Enabling is a symlink operation; `runsvdir` notices it without a reload:

```sh
service=SERVICE_NAME
sudo ln -sfn "/etc/runit/sv/$service" \
  "/etc/runit/runsvdir/default/$service"
sudo sv start "$service"
```

Disable deliberately:

```sh
service=SERVICE_NAME
sudo sv stop "$service"
sudo rm -f "/etc/runit/runsvdir/default/$service"
```

Do not delete `/etc/runit/sv/<service>` to disable or recover a service; that
directory contains its definition.

## D-Bus service-control interface

Programmatic callers use:

| Item | Value |
|---|---|
| Bus | system |
| Destination | `org.opdbus.v1.Runit.Systemctl` |
| Object path | `/org/opdbus/v1/plugins/runit/systemctl` |
| Interface | `org.opdbus.v1.Runit.Systemctl` |

Inspect the live interface:

```sh
sudo busctl --system introspect \
  org.opdbus.v1.Runit.Systemctl \
  /org/opdbus/v1/plugins/runit/systemctl \
  org.opdbus.v1.Runit.Systemctl
```

The main method mappings are:

| D-Bus method | Runit action | Important behavior |
|---|---|---|
| `Start` | `sv up` | Adds the service to the live view if needed; does not boot-enable it |
| `Stop` | `sv down` | Leaves boot enablement unchanged |
| `Restart` | `sv restart` | Starts an unsupervised service after adding it to the live view |
| `Reload` | `sv hup` | Sends SIGHUP |
| `Enable` | Add persistent and live symlinks | Removes a `down` marker |
| `Disable` | `sv down`, then remove both symlinks | Removes boot and live supervision |
| `Status` | `sv status` | Returns a JSON string |
| `ListUnits` | Scan the live view | Returns a JSON array string |
| `ListUnitFiles` | Scan definitions | Reports boot enablement |
| `DaemonReload` | No-op | Runit has no compiled service database |

Lifecycle methods accept only service names containing ASCII letters, digits,
`_`, `-`, `.`, or `@`, with a maximum length of 128 bytes. They also reject
non-root D-Bus callers. Higher-level capability checks occur before this
system-bus boundary.

For a direct diagnostic call:

```sh
sudo busctl --system call \
  org.opdbus.v1.Runit.Systemctl \
  /org/opdbus/v1/plugins/runit/systemctl \
  org.opdbus.v1.Runit.Systemctl \
  Status s op-web
```

The installed `svd` client exercises the same D-Bus interface:

```sh
sudo svd status op-web
sudo svd list-units
```

Use `sudo sv` for normal host administration. Use `svd` or `busctl` when
diagnosing the application-facing D-Bus path.

## Deploying service changes

Do not copy a rebuilt binary directly onto the host. Build once, then publish
the golden btrfs content and live installation from the same release:

```sh
CXXFLAGS="-include cstdint" cargo build --workspace --release
sudo deploy/runit/build-golden.sh --dry-run
sudo deploy/runit/build-golden.sh
```

The publisher restarts enabled services whose release-owned definition or
binary changed, then checks important listeners and D-Bus endpoints. It never
automatically restarts these network or session-bus services:

```text
ovs-vswitchd  ovsbr0-addr  ovsbr0-svc-addr  ovsbr0-uplink
uplink-dhcp   op-session-bus  opdbus-rundirs  dbus
```

It reports held-back services for a deliberate console restart. Other useful
modes are `--golden-only`, `--live-only`, and `--no-restart`; the last mode
leaves existing processes running the previous binaries.

## Troubleshooting

| Symptom | Check |
|---|---|
| `access denied` from `sv` | Re-run with `sudo`; `supervise/ok` is root-only |
| `unable to open supervise/ok` | Verify the enable symlink and executable `run` script |
| Service repeatedly restarts | Ensure the process stays in the foreground and the script ends with `exec` |
| D-Bus destination is missing | Check `sudo sv status op-runit-systemctl`, then run the introspection command above |
| `sv check` never succeeds | Run `sudo sv status`, inspect the service's `check` script, and read its log |
| Definition changed but behavior did not | Restart the service; there is no daemon-reload step |

For network-related dependencies and boot ordering, see
[Network Operations](network.md).
