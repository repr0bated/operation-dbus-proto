# Service Management

The host boots **runit** as PID 1. Use `sudo sv` for host services; s6-era
commands and paths no longer apply.

Runit has no compiled service database. A service definition is live as soon as
it is present and executable under `/etc/runit/sv`, and a symlink in the active
runlevel enables supervision.

| Path | Purpose |
| --- | --- |
| `/etc/runit/sv/<service>/run` | Authoritative service definition |
| `/etc/runit/sv/<service>/down` | Optional marker that prevents automatic start |
| `/etc/runit/runsvdir/default/<service>` | Symlink that enables the service at boot |
| `/etc/runit/runsvdir/current` | Symlink to the active runlevel |
| `/run/runit/service/<service>` | Supervisor's runtime view; do not edit it |

## Everyday operations

Inspect state before changing it, especially when working remotely:

```sh
sudo sv status <service>
sudo sv start <service>
sudo sv stop <service>
sudo sv restart <service>
sudo sv check <service>
```

`sv` needs root to read `supervise/ok`; an unprivileged status check can fail
with `access denied`. Do not use `service6`, `s6-*`, `systemctl`, or invoke
`runsv`/`runsvdir` directly. Container and application lifecycle operations are
separate from host supervision and must go through the D-Bus service-manager
surface.

To enable a new host service after reviewing its definition:

```sh
sudo test -x /etc/runit/sv/<service>/run
sudo ln -sf /etc/runit/sv/<service> /etc/runit/runsvdir/default/
sudo sv start <service>
sudo sv status <service>
```

## Diagnose a service

Start with the supervisor and active runlevel:

```sh
ps -p 1 -o comm=                       # expect: runit
pgrep -a runsvdir                      # expect: runsvdir -P /run/runit/service
sudo sv status <service>
ls -l /etc/runit/runsvdir/current
ls -l /etc/runit/runsvdir/default/<service>
sudo test -x /etc/runit/sv/<service>/run
sudo sh -n /etc/runit/sv/<service>/run
```

- `run: <service>: (pid N) ...` means the service is up.
- `down: <service>: ...` means it is supervised but stopped.
- `unable to open supervise/ok` usually means the enabling symlink is missing or
  dangling, or the active runlevel is not supervising it.
- A rapid restart loop usually means the process daemonized. The `run` script
  must keep the program in the foreground and end with `exec`.

Do not delete definitions or alter `/run/runit/service` to repair supervision.
If the source definition changes, edit `/etc/runit/sv/<service>/run`, validate
it, and restart with `sudo sv restart <service>`.

## Publish changed binaries

Do not copy binaries onto the running host by hand. Build once, review the
golden/live publication, then publish the same release to both:

```sh
CXXFLAGS="-include cstdint" cargo build --workspace --release
sudo deploy/runit/build-golden.sh --dry-run
sudo deploy/runit/build-golden.sh
```

`build-golden.sh` installs only changed binaries and restarts only enabled
services that reference them. It never automatically restarts these
network/session-bus services:

```text
ovs-vswitchd ovsbr0-addr ovsbr0-svc-addr ovsbr0-uplink
uplink-dhcp op-session-bus opdbus-rundirs dbus
```

The script reports held-back services. Restart them deliberately from the
console, in a reviewed order, so the deploy cannot cut remote access or reparent
the control plane. Use `--no-restart` when installation and activation must be
separate; services continue running the previous binaries until explicitly
restarted.
