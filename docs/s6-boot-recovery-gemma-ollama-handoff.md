# S6 Boot Recovery + Gemma/Ollama Bring-Up — Handoff

Date: 2026-07-02
Host: `3tched` (Artix Linux, s6-rc)

## What happened, in order

1. **Original goal**: bring the `gemma4:12b` LLM up via zeroclaw's Ollama provider, and separately get the `gemma` s6 oneshot (xray/OpenFlow config generator) running.
2. While chasing a stale `op-s6-systemctl` D-Bus daemon (see below), a `daemon_reload` D-Bus call ran `s6 set commit` (which compiled a new db and garbage-collected the old one) but the client connection died before `s6 live install` could run and repoint `/etc/s6/rc/compiled` (bootdb). This left bootdb dangling at a deleted target — **the system became unbootable.**
3. Recovered via live rescue media + chroot into `/dev/sda3`. Repointed `/etc/s6/rc/compiled` to the last known-stable pre-incident compiled db (`.current:@400000006a369bdb346d3c96:YESXfi`, dated Jun 20 — validated to have both `default`/`boot` bundles and to be the specific post-fix db from the documented Jun 20 `op-web-srv` notification-fd wedge fix). Verified with `s6-rc-db check` (exit 0). **Reboot succeeded.**

## Root causes found and fixed

- **`op-s6-systemctl`'s `S6D_RELOAD_SCRIPT` env var defaults to `/usr/local/sbin/op-s6-recompile-and-update`, checked *before* the correct repo-path fallback** (`crates/op-s6-systemctl/src/dbus.rs::run_artix_frontend_reload`, ~line 139). A stale installed copy at that path existed and was self-referentially broken: it computes `PROJECT_ROOT` from its own `$0`, so when run from `/usr/local/sbin/...` it resolves to `/usr` instead of the real repo root — explaining both `find: 'target/release': No such file or directory` and `install: cannot stat '/usr/deploy/s6/recompile-and-update.sh'`. **Fixed**: deleted `/usr/local/sbin/op-s6-recompile-and-update`, forcing fallback to the correct repo script. This is a workaround, not a permanent code fix — the daemon's env-var precedence logic itself is still there and would recreate this exact bug if that path ever gets reinstalled (e.g. by a future `recompile-and-update.sh` run, since `install_control_scripts()` writes to that same path).
- **`op-s6-systemctl`'s running process was orphaned** — a child of PID 1 directly, not tracked by any current s6 servicedir I could find (killed it by mistake chasing a bad CWD-based assumption about which servicedir owned it — no collateral damage, but had to manually relaunch `/usr/local/bin/op-s6-systemctl` via `nohup` since nothing auto-respawned it). **This should be looked into**: why isn't it under proper s6 supervision, and how was it originally started?
- **`gemma`'s `up` script silently swallows failures**: `/etc/s6/sv/gemma/up` (execline) does `foreground { sh /etc/s6/sv/gemma/shell_up }` then unconditionally `exit 0` — so even though `shell_up` has `set -eu` and would stop correctly on a real failure inside it, s6 never sees that failure; the oneshot always reports success. **Not yet fixed** — needs `foreground` to be replaced with something that propagates the real exit code (e.g. drop the trailing `exit 0` and let the script's own exit code from `shell_up` propagate, or explicit exit-code capture).
- **`xray` was crash-looping** because its config (`/etc/xray/config.json`, a symlink into `/dev/shm/xray-ghostbridge.json` — tmpfs, wiped every reboot) didn't exist post-reboot, since gemma's masked failure meant the regeneration chain (`op-gemma` → `op-identity-shuttle`) never actually completed at boot. **Fixed for this boot** by running both binaries manually as root; `xray` came up clean afterward — but see gemma bug above, this will recur on every reboot until that's fixed.
- **Port 443 conflict**: an Incus proxy device on the `caddy` container (`docker-port-0.0.0.0-443`, set up via `incus compose` importing Netmaker's own docker-compose bundle) was bound to host port 443, competing with `xray`. Per user direction, the `caddy` container was **deleted entirely** (xray already covers reverse-proxy duty; caddy was serving Netmaker's own dashboard/API/broker via SNI routing, now redundant). Confirmed caddy removed cleanly; `xray` restarted via D-Bus and came up stable.

## Current state — IN PROGRESS, left mid-operation

`sudo s6-apply` (the safe, documented, rollback-capable commit+install wrapper at `/usr/local/bin/s6-apply` — read its source, it really does sync→commit→live-install→bootdb-sync atomically with rollback on failure) was run to commit `ollama-srv` (needed for the LLM bring-up; it exists in `/etc/s6/sv/ollama-srv` and shows `active` in the working set, but predates the Jun 20 db currently live, so it's not running) and whatever else has accumulated in `/etc/s6/sv/` since Jun 20.

**It is currently stuck**, confirmed via full process tree (not guessed):
```
s6-apply(20410)
 └ s6-rc-set-install(20423)   [the "s6 live install -b" step]
    └ s6-rc(20424)  -u -- change ovsbr0-static ovsbr0-init opdbus op-web-srv op-projection op-openvswitch-daemon op-dbus-mirror op-dbus op-cognitive-mcp
       └ s6-svlisten(20588)   [waiting for a readiness notification]
          └ s6-ftrigrd(20592) [blocked reading the notification fifo]
```
This is the **exact documented `op-web-srv` wedge** from project memory: `op-web-srv` has `notification-fd=3` but its Rust binary never writes the readiness byte, so `s6-svlisten -U` blocks forever, holding the global s6-rc lock. The Jun 20 fix (removing `notification-fd` from `op-web-srv`/`op-assistant-grpc-srv`) was applied to the *bootdb-pointed-at* db, but this fresh commit compiles from the **current** `/etc/s6/sv/` source tree, which may have regressed/never had that fix applied to the actual source files (only to whatever db existed on Jun 20) — needs verification: check whether `/etc/s6/sv/op-web-srv/notification-fd` currently exists.

**Was about to (not yet done)**: write the readiness byte directly into `op-web-srv`'s open fd 3 (pid 20608, confirmed via `/proc/20608/fd/`) to unstick *this* transition non-destructively (no killing), then separately apply the permanent fix to `/etc/s6/sv/op-web-srv/notification-fd` (and `op-assistant-grpc-srv`) plus the git source at `deploy/s6/op-web-srv/` so future commits don't re-hit this.

**Do NOT kill `s6-apply` or its children carelessly** — per its own design, if it's killed mid-transition rather than allowed to complete or cleanly fail, its rollback logic (which only triggers on a clean non-zero exit, not a hang) won't run, and bootdb could be left pointing at whatever the live system was on before this run (which should still be the safe Jun 20 db at this point, since `s6 set commit` already completed but bootdb hasn't been repointed to the new db yet — so a reboot *right now*, before this resolves, would likely still boot into the Jun 20 db safely, but this hasn't been re-verified since the commit step ran).

## Still open / not yet done

- Fix `gemma`'s `up` script to not swallow failures (see above).
- Permanently fix `op-web-srv`/`op-assistant-grpc-srv` notification-fd in both `/etc/s6/sv/` and `deploy/s6/` git source, then verify a fresh `s6-apply` run doesn't wedge.
- Once the above is resolved: start `ollama-srv` via D-Bus `org.opdbus.v1.S6.Systemctl` `Start`, verify `ollama serve` comes up, verify `gemma4:12b` is reachable and zeroclaw's active provider (`ollama`) works end-to-end.
- `fsck` on `/dev/sda1` (FAT/EFI partition) — dmesg showed "not properly unmounted, some data may be corrupt" from the abrupt reboot during the outage. Not urgent, not yet done.
- Investigate why `op-s6-systemctl`'s process is orphaned/unsupervised rather than a normal s6 longrun — a "why" question, not yet answered.
- PR #15 (`feat/sled-source-port-salt`, security/correctness fixes triaged earlier) was sent to Ultraplan for cloud refinement, approved, and executed remotely as a PR — check GitHub for its actual landing state, not yet re-verified from this session.
- PR #14 (`plugin-capability`) was closed as superseded by #15 — done, no follow-up needed.

## Key files/paths for whoever picks this up

- `/usr/local/bin/s6-apply` — the safe wrapper, read it before running `s6 set commit`/`s6 live install` by hand ever again.
- `/etc/s6/sv/gemma/up` + `shell_up` — the failure-masking bug.
- `/etc/s6/sv/op-web-srv/notification-fd`, `/etc/s6/sv/op-assistant-grpc-srv/notification-fd` — check existence, should not exist.
- `deploy/s6/dbus-session/run` — already correctly edited this session to exec `busd` instead of `dbus-daemon` (busd installed at `/usr/local/bin/busd` v0.5.0) — **this change has never actually gone live yet**; it's sitting in a pending commit that hasn't successfully installed (blocked by all of the above). Once `s6-apply` succeeds cleanly, verify `dbus-session`'s live servicedir actually runs `busd`, not the old `dbus-daemon`.
- Project memory: `project_s6_commit_prune_hazard.md`, `artix-s6-layout.md` — both already updated with prior incidents; this handoff's findings should probably be folded into those after the dust settles.
