# Artix runit service recovery

This host boots **runit** as PID 1 and is controlled with `sv`. Runit has **no
compiled service database**, which removes the whole class of "the bootdb is
corrupt" failures that s6-rc had. There is nothing to recompile and nothing to
re-init: definitions on disk *are* the configuration.

If you arrived here looking for `s6-rc-compile`, `s6-rc-init`, or bundle
recovery: none of it applies. s6 is not installed on this host.

## Layout

| Path | Role |
|---|---|
| `/etc/runit/sv/<service>/run` | The service definition |
| `/etc/runit/sv/<service>/down` | Marker: do not auto-start when supervised |
| `/etc/runit/runsvdir/default/<service>` | Symlink enabling the service at boot |
| `/etc/runit/runsvdir/current` | Symlink to the active runlevel (`-> default`) |
| `/run/runit/service/<service>` | The tree `runsvdir -P` supervises |
| `/run/runit/service/<service>/supervise/` | Live state written by `runsv` |

`sv` needs root to read `supervise/ok`; an unprivileged `sv status` fails with
"access denied".

## Diagnosis order

```sh
ps -p 1 -o comm=                 # expect: runit
pgrep -a runsvdir                # expect: runsvdir -P /run/runit/service
sudo sv status <service>
ls -l /etc/runit/runsvdir/current   # which runlevel is active
```

`sv status` output reads `run: <svc>: (pid N) Ss` when healthy, `down: <svc>: Ns`
when stopped, and `warning: <svc>: unable to open supervise/ok` when `runsv` is
not supervising it at all.

## Common failures

### A service is enabled but never starts

`runsvdir` scans its directory on its own, so a missing service usually means the
symlink or the `run` script is wrong.

```sh
ls -l /etc/runit/runsvdir/default/<service>   # symlink present and not dangling?
test -x /etc/runit/sv/<service>/run           # run script executable?
sudo sh -n /etc/runit/sv/<service>/run        # syntax valid?
```

A `run` script that is not executable, or whose interpreter line is wrong, makes
`runsv` fail silently in a loop.

### A service restarts in a loop

Almost always because the process **daemonises**. Runit supervises the process it
starts; if that process forks and exits, `runsv` concludes it died and starts it
again. Fix the `run` script to keep the binary in the foreground (`-N`, `-f`,
`--foreground`, or `--nodaemon`, depending on the program) and ensure it ends
with `exec`.

### A service does not respond to `sv`

First confirm that the active runlevel contains a valid service symlink and
that the definition is executable:

```sh
sudo sv status <service>
ls -l /etc/runit/runsvdir/current
ls -l /etc/runit/runsvdir/default/<service>
test -x /etc/runit/sv/<service>/run
sudo sh -n /etc/runit/sv/<service>/run
```

Do not edit `/run/runit/service`, delete `/etc/runit/sv/<service>`, or launch
`runsv`/`runsvdir` manually. The first path is the supervisor's runtime view and
the second is the authoritative definition. If `runsvdir` itself is unhealthy,
recover it from the console rather than attempting a routine remote service
restart.

### Recovering into single-user

`/etc/runit/runsvdir/single` holds the minimal runlevel. To boot into it, append
`single` to the kernel command line, or switch at runtime:

```sh
sudo runsvchdir single      # switch runlevel
sudo runsvchdir default     # switch back
```

`runsvchdir` is the sanctioned way to change runlevels; it repoints
`/etc/runit/runsvdir/current`.

## Shipping new binaries

Deployment is **btrfs send/receive** of subvolume snapshots — see
`deploy/btrfs-layout.sh` for the base/modules/snapshots/staging layout. A release
is a snapshot sent to the target, not a file copy onto a live host.

`deploy/runit/build-golden.sh` publishes a release both ways from one build: it
populates the golden subvolume and installs the same binaries into
`/usr/local/bin`, restarting only the services whose binary actually changed.
Network-critical services are reported rather than restarted.

## Third-party installers that expect systemd

A `systemctl` compatibility shim is installed at `/usr/local/bin/systemctl` for
third-party package installers. It maps systemd verbs onto `sv`, and converts a
`.service` unit into `/etc/runit/sv/<name>/run` using
`/usr/local/sbin/systemd-unit-to-runit`. A pacman hook converts units that
packages drop, without enabling them — enabling stays an operator decision.
Operators and agents must still use `sudo sv` for service lifecycle operations,
not the compatibility shim.

To convert a unit by hand and inspect the result first:

```sh
systemd-unit-to-runit /usr/lib/systemd/system/<name>.service --dry-run
sudo systemd-unit-to-runit /usr/lib/systemd/system/<name>.service --enable
```

Conversions report what runit cannot express: `Type=forking` (will restart-loop
until you add the no-fork flag), `Type=notify` (no sd_notify socket),
`Type=oneshot` (use `sv once`), and `Restart=no` (runit always restarts, so a
`down` file is written).
