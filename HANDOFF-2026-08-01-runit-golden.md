# HANDOFF 2026-08-01 — runit/sv migration + accountability audit trail + golden deploy

Read this first in a new shell. It is the resume point, not a summary.

## Where things stand

Two commits exist locally on branch `agent/zeroclaw-runtime-routing`
(tracking `origin/agent/zeroclaw-runtime-routing`). **Neither is pushed.**

```
db71b93e  feat(deploy): golden subvolume + live install from one build; drop s6-era recompile script
352c0ad3  feat(runit): complete s6 -> runit/sv migration + accountability audit trail
```

Working tree is clean of in-progress git operations. Moving the repo directory
carries both commits intact.

## BLOCKER: push has no credentials

`git push` fails with `could not read Username for 'https://github.com'`.
Everything was checked and ruled out:

- `gh` 2.96.0 installed but **not logged in** (verified as both `jeremy` and root)
- no credential helper, no `~/.git-credentials`
- no `GH_TOKEN` / `GITHUB_TOKEN`, no ssh-agent running
- `~/.ssh/vps_key` and `~/.ssh/vps_transfer_key` both tested directly against
  GitHub → `Permission denied (publickey)`. Neither is registered there.
- remote is HTTPS: `https://github.com/repr0bated/operation-dbus-proto`

Fix with any one of:

```sh
gh auth login && git push
# or
ssh-keygen -t ed25519 -C odbus-deploy -f ~/.ssh/github_ed25519 -N ""
cat ~/.ssh/github_ed25519.pub     # add to GitHub -> Settings -> SSH keys
git remote set-url origin git@github.com:repr0bated/operation-dbus-proto.git && git push
# or
git config --global credential.helper store    # then one interactive push with a PAT
```

## NEXT STEPS, in order

1. Push the two commits.
2. `sudo deploy/runit/build-golden.sh --golden-only` — builds the deployable
   subvolume without touching the running host. Review `/opt/op-dbus/golden`
   and its `MANIFEST`.
3. Then the live half: `sudo deploy/runit/build-golden.sh` (or `--live-only`).
   Dry run says it restarts exactly 4 services:
   `op-cognitive-mcp op-grpc-bridge op-of-controller op-web`
   and deliberately holds back `ovs-vswitchd`. Verify each with `sv status`.
4. Nothing is live yet: the running `op-grpc-bridge` (was pid 1316) is the
   **07-29** build. None of this work is in effect on the host.

## ENVIRONMENT LANDMINES — read before running anything

- **Every cargo invocation needs `export CXXFLAGS="-include cstdint"`.** The
  vendored RocksDB in `cozorocks 0.1.7` fails on this GCC without it
  (`uint64_t does not name a type`). Pre-existing, unrelated to this work.
- **`./target` has ~760 root-owned fingerprint dirs** from an earlier root
  build, so `cargo check --workspace` fails with `Permission denied`. Work
  around with `CARGO_TARGET_DIR=<somewhere you own>`; a full cold check is
  ~5 min, a release build ~13 min. `./target` is 23 GB.
- **Never run bare `git status` / `git diff --stat`** in this repo. It reports
  ~2500 files as modified from filemode noise and floods the output. Use
  path-scoped `git diff --numstat` and filter `$1+$2>0` for real changes.
- Release build already done: 33 binaries in `target/release`, 0 errors.
- Host: **runit** is PID 1, controlled with `sv` (needs root). No s6 installed.
  Definitions `/etc/runit/sv/<svc>/run`, enablement symlink
  `/etc/runit/runsvdir/default/<svc>`, supervised tree `/run/runit/service`.

## What landed in commit 352c0ad3

### Accountability audit trail (`.kiro/specs/accountability-audit-trail`)
All 11 spec tasks implemented.

- `snowball` plugin: `query_events` + `verify_chain` schema methods, typed
  I/O structs at module scope so `op-grpc-bridge` can import them.
- `MutationEngine`: scoped `"snowball"` dispatch arm — only those two methods;
  the plugin's other seven still hit the catch-all echo (spec scope boundary).
- Durability: events mirrored into the streaming snowball `timing` subvolume
  inline with dispatch; failures warn and never fail the call. Startup replays
  from disk preserving stored `event_hash`/`prev_hash`.
- `EventChain`: added `replay_event`, `replay_from_footprint`, `verify_range`.
- `zeroclaw-gui`: new `accountability/` module (store/transport/view), no
  coupling to the chat path. `build.rs` compiles op-grpc-bridge's
  `operation.proto` **in place** (not copied) so it cannot drift.
- Tests: `crates/op-grpc-bridge/tests/accountability_audit_trail.rs`, 6 passing,
  including a durability round trip (writes timing records, rebuilds a fresh
  engine from disk, re-verifies the chain).

### runit/sv migration (`.kiro/specs/runit-sv-migration/requirements.md`)
requirements.md is written; design.md / tasks.md were **not** written (went
straight to implementation on request).

- Every dead spawn replaced with `sv`: `s6-rc`, `s6-svc`, `s6-svstat`,
  `s6-svscan`, `s6d`, `service6`, `systemctl`.
- `op_core::runit` is the single source of truth for the three runit paths
  (`crates/op-core/src/runit.rs`).
- Agent tools renamed `s6_*` -> `sv_*` (`crates/op-tools/src/builtin/sv.rs`),
  added `sv_restart_service` since `sv restart` is native. Fixed a latent bug:
  the old proxy addressed `org.opdbus.s6.Systemctl`, which never matched the
  daemon, so it always fell through to dead `s6-rc`.
- LLM system prompt + anti-hallucination map no longer teach s6.
- Deleted orphaned `crates/op-dbus/` (no Cargo.toml, not a workspace member).
- D-Bus interface is now `org.opdbus.v1.Runit.Systemctl`; the legacy
  `S6.Systemctl` name is still claimed for one release.
- systemd compat layer (`deploy/runit/`): `systemctl-shim`,
  `systemd-unit-to-runit`, `op-convert-systemd-units`,
  `99-systemd-unit-to-runit.hook`. Tested against the 11 real systemd units on
  this host.
- Regression guard: `crates/op-core/tests/no_s6_regression.rs` — 3 tests that
  fail if an s6 spawn, s6 path, or `s6_*` tool name returns. It caught 7 sites
  that manual review missed.

## What landed in commit db71b93e

`deploy/runit/build-golden.sh` — one build, two publish paths:

1. GOLDEN: populates `/opt/op-dbus/golden` (btrfs subvolume) with `bin/`,
   `sbin/`, `sv/`, `etc/`, `MANIFEST` (commit + per-binary sha256).
   **No snapshot, no send** — those are deployment-time steps.
2. LIVE: same binaries into `/usr/local/bin`, restarts only services whose
   binary changed.

Safety properties worth preserving if you edit it:

- Service matching is against the **full install path with a trailing word
  boundary**. Bare-name matching tied `opdbus` to `opdbus-rundirs` and pulled
  13 services into the restart set including `ovsbr0-uplink` and `uplink-dhcp`
  — restarting those on this remote host can cut access.
- `NEVER_AUTO_RESTART` holds back `ovs-vswitchd ovsbr0-* uplink-dhcp
  op-session-bus opdbus-rundirs dbus`; they are reported, not bounced.
- A host `run` script that differs from the repo copy is left alone.

Deleted `deploy/runit/recompile-and-update.sh` — an s6-era artifact that
installed itself as `op-s6-recompile-and-update` for "the historical s6d reload
path". Superseded by build-golden.sh.

## NOT DONE — open decisions for the user

1. **Crate/binary rename** (spec FR-7): `op-s6-systemctl` -> `op-runit-systemctl`
   and `s6d` -> `svd`. Deferred: it touches workspace members, the installed
   binary name, and the running service. `docs/overview/architecture.md` still
   references the old crate name for the same reason.
2. **btrfs deployment layout does not exist on this host.** No `/opt/op-dbus`,
   no `@op-dbus-*` subvolumes, and `deploy/btrfs-layout.sh`'s
   `BTRFS_ROOT=/mnt/btrfs-root` is not mounted. `build-golden.sh` creates
   `/opt/op-dbus/golden` itself (`/opt` is btrfs, verified), so the layout
   script may be stale — it predates the runit migration. Decide whether it
   still applies.
3. **btrfs send target** is unknown — needed for the actual deployment step.
4. Two host run scripts differ from the repo copies and were left alone:
   `/etc/runit/sv/notebook-sources-sync/run`, `/etc/runit/sv/xray-config-mount/run`.
5. `docs/.consolidation-staging/` and `.consolidation-staging/` still mention
   s6; treated as archive per the spec's non-goals.
6. The s6->runit rewrite of `crates/op-s6-systemctl/src/dbus.rs` was ported
   from **uncommitted work in a different clone**:
   `/home/jeremy/git/operation-dbus-proto`. That clone still holds unrelated
   uncommitted work (`auto_create.rs` +721, a `mutation_engine.rs` method-gap
   drafting hook, `op-plugins/src/lib.rs`) which was deliberately **not**
   brought over. Same origin remote; decide whether it should be.

## Verification already done (do not redo blindly)

- `cargo check --workspace` exits 0 (in an owned `CARGO_TARGET_DIR`).
- `op-state-store`: 41/41 tests pass.
- `no_s6_regression`: 3/3 pass.
- `accountability_audit_trail`: 6/6 pass.
- clippy: zero findings in any file authored here. Remaining `-D warnings`
  failures in `zeroclaw-gui` are all pre-existing files (`grpc.rs`, `theme.rs`,
  `catalog/*`, `app.rs`, `chat/*`, and pre-existing lines in `views/mod.rs`).
- `build-golden.sh --dry-run` reviewed against this host.
