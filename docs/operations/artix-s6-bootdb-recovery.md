# Artix s6 Bootdb Recovery — Lessons from 2026-07-02

Host: `3tched` (Artix Linux, s6-rc)

This note preserves the recovery procedure and root-cause findings from a 2026-07-02 incident where the s6 boot database was left pointing at a deleted compiled database, making the host unbootable.

## What happened

1. A `daemon_reload` D-Bus call invoked `s6 set commit` (which compiled a new db and garbage-collected the old one) but the client connection died before `s6 live install` could repoint `/etc/s6/rc/compiled` (bootdb).
2. This left bootdb dangling at a deleted target, and the system became unbootable.
3. Recovery was performed via live rescue media plus chroot into the root filesystem, then repointing `/etc/s6/rc/compiled` to the last known-stable pre-incident compiled database, validating it with `s6-rc-db check`, and rebooting.

## Root causes found and fixed

### `op-s6-systemctl` stale helper path

`op-s6-systemctl` checks the `S6D_RELOAD_SCRIPT` environment variable before falling back to the correct repo-path script. A stale installed copy at `/usr/local/sbin/op-s6-recompile-and-update` existed and was self-referentially broken: it computed `PROJECT_ROOT` from its own `$0`, so when run from `/usr/local/sbin/...` it resolved to `/usr` instead of the real repo root.

Workaround: delete the stale helper so the daemon falls back to the correct repo script. The permanent fix is to correct the env-var precedence in the daemon itself so the stale path cannot win.

### `op-s6-systemctl` process was orphaned

The running process was a child of PID 1, not tracked by any s6 servicedir. This meant it had to be restarted manually and would not auto-respawn. Recommendation: place `op-s6-systemctl` under proper s6 supervision.

### `gemma` oneshot masked failures (historical)

The original `/etc/s6/sv/gemma/up` had a trailing `exit 0` that masked failures from `shell_up`. The current staging s6 definition no longer contains that unconditional `exit 0`; the `foreground` block now propagates `shell_up`'s exit status. The incident is preserved as a cautionary example of why execline oneshots must not swallow failure codes.

### `xray` config lost across reboots

`xray` was configured through a symlink to a tmpfs-backed file (`/dev/shm/xray-ghostbridge.json`) that is wiped every reboot. Because `gemma` was silently failing at the time, the regeneration chain (`op-gemma` → `op-identity-shuttle`) never completed at boot, so the config was never regenerated. The canonical Xray live config path today is `/dev/shm/xray_config.json` per `AGENTS.md`, and the gemma failure-masking bug is fixed in the current staging tree.

### Port 443 conflict with caddy

An Incus proxy device on a `caddy` container bound host port 443, competing with `xray`. Since `xray` already handles reverse-proxy duty, the caddy container was removed. After removal, `xray` restarted via D-Bus and stabilized.

## Safe wrapper: `s6-apply` (host-local, not in repo)

The recovery host had a wrapper called `s6-apply` at `/usr/local/bin/s6-apply` that performed `s6 set commit` / `s6 live install` / bootdb-sync atomically with rollback on failure. This wrapper is not present in the current repo. Until a canonical `s6-apply` is added, use the raw `s6` steps with extreme care; if a transition is killed mid-flight, the rollback logic may not run.

## `op-web-srv` notification-fd wedge (resolved in current staging)

`op-web-srv` had a `notification-fd` configured but its Rust binary never wrote the readiness byte. This caused `s6-svlisten -U` to block forever during `s6-rc` transitions, holding the global s6-rc lock. The current staging s6 definitions for `op-web-srv` and `op-assistant-grpc-srv` no longer contain `notification-fd` files, so this issue is resolved in the current tree.

## Key paths to inspect

- `/etc/s6/sv/gemma/up` + `shell_up` — verify no trailing `exit 0` masks failures
- `/etc/s6/sv/op-web-srv/notification-fd` — should not exist in current definitions
- `/etc/s6/sv/op-assistant-grpc-srv/notification-fd` — should not exist in current definitions
- `crates/op-s6-systemctl/src/dbus.rs` — `S6D_RELOAD_SCRIPT` precedence bug

<!-- Extracted from /mnt/opt-inspect/home/git/operation-dbus-proto/docs/s6-boot-recovery-gemma-ollama-handoff.md on 2026-07-20 and corrected against the current codebase on 2026-07-20 -->
