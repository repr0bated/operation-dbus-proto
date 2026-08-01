# HANDOFF 2026-08-01 — repo relocation + credentials + spec work

Read this first in a new shell. It is the resume point, not a summary.

## Where things stand

Branch `agent/zeroclaw-runtime-routing` is **fully pushed** — local HEAD
(`045d9701`) matches `origin/agent/zeroclaw-runtime-routing` exactly. The
credentials blocker noted in `HANDOFF-2026-08-01-runit-golden.md` is resolved
(see below) and is no longer accurate — that push went through.

## Repo location changed

This repo no longer lives at `/git/odbus` or `/home/jeremy/git-admin/odbus`.
Both were bind-mount aliases (defined in `/etc/fstab`) onto the real
directory `/home/admin/git/odbus`, which was only writable by `admin`.

**Canonical path now: `/srv/git/odbus`.**

- Real data is unchanged — still physically at `/home/admin/git` (btrfs
  subvol `@home`, `subvolid=257`). No bytes were copied; this was a bind-mount
  swap, not a data move.
- The two old aliases (`/git`, `/home/jeremy/git-admin`) were lazy-unmounted
  (`umount -l`) and their `/etc/fstab` lines removed.
- New `/etc/fstab` line: `/home/admin/git /srv/git none bind,nofail 0 0`.
- `/srv/git` also holds the sibling dirs that lived alongside `odbus`:
  `operation-dashboard-ui-07`, `cachyos-hyprland-noctalia`,
  `opdbus-golden-snapshot-20260721`, `repos-bulk`, `runit`, `zbus`, `zbusctl`,
  and a 134GB `vps-6aff90ab.vps.ovh.ca-snapshot` file (not a repo — a VPS
  backup, left in place, not yet relocated elsewhere).
- A genuinely separate, stale clone at `/home/jeremy/git/operation-dbus-proto`
  (last synced 2026-07-21, had uncommitted work: staged deletions of
  `op-openvswitch-daemon` s6 service files + `s6_systemctl.rs`, plus unstaged
  edits to `mutation_engine.rs`/`auto_create.rs`/`lib.rs`/etc.) was **deleted
  outright** per explicit instruction — that uncommitted work was not
  preserved. If anything from that line of work is missing, it'll need to be
  redone.
- **Not yet fixed**: `/home/admin/git` itself (the real backing directory) is
  still `admin:admin` owned, mode `755` — writable only by `admin`. Group-write
  access for `jeremy` (or a shared group) hasn't been set up yet, unlike the
  `secrets` group fix done for `.bash_secrets`. Anyone editing this repo as
  `jeremy` today is relying on `sudo`, not real group permissions.

## Credentials — now working, method changed from SSH to token

`gh` CLI is installed (2.96.0) and **not** logged in via `gh auth login` (that
flow hangs on an interactive device-code prompt in a non-interactive
session — don't use it here). Instead:

- `GH_TOKEN` lives in `/home/admin/.bash_secrets` (along with ~50 other
  credentials — AWS, Cloudflare, HF, OpenAI, etc.). That file was `600`,
  `admin`-only; it's now group-readable via a new `secrets` group
  (`admin`, `jeremy`, `node` are members) at `640`.
- Both `jeremy`'s `~/.bashrc` → `~/.profile` chain now source it:
  `[[ -r "/home/admin/.bash_secrets" ]] && source "/home/admin/.bash_secrets"`.
  Takes effect in *new* shells/logins only — a shell that predates the
  `usermod -aG secrets jeremy` won't see it until it restarts.
- `gh` auto-authenticates from `GH_TOKEN` in env — no `gh auth login` step
  needed. Confirmed: `gh api user` → `repr0bated`.
- `git` HTTPS auth goes through `~/.netrc` (`machine github.com`, login
  `repr0bated`, password = the token) — confirmed working via `git fetch`.
- `git config --global user.name/user.email` set to
  `Jeremy Hobson <jeremy.alan.hobson@gmail.com>`.
- **Incident**: while wiring up `.netrc` the first time, a shell-quoting bug
  on my end leaked the raw `GH_TOKEN` value into visible tool output twice.
  User was told to rotate/revoke that token (`ghp_t9vWzmOP...`) — **unclear
  if that's actually been done yet**. Check before trusting that token is
  still the live one; `/home/admin/.bash_secrets` may now hold a replacement.

## Two Kiro specs written this session — staged for review, nothing implemented

Both went through a spec → review → revise loop (not just one pass):

### `odbus/.specs/netclient-container-netns/`
Get `netclient` actually joined inside the `NetMaker` incus container
(currently loopback-only, no route out) using an OVS internal port — not a
veth — provisioned through the existing `rtnetlink`/`rovs_commands`
D-Bus surface. Phase 2 (separate, gated, NOT to run automatically after
phase 1) replaces xray's veth-backed NIC the same way.

Key resolved findings baked into the current spec:
- Gateway IP placement bug (nothing was answering ARP for the gateway) — fixed.
- WireGuard traffic must not use a raw SNAT egress — this deployment has a
  real decoy/obfuscation architecture (`openflow_obfuscation.rs`, xray's WARP
  egress at fwmark `0x51821`/table `51820`/`wgcf-egress`). netclient's egress
  now rides that same obfuscated path instead of a directly-attributable host
  IP. One unresolved item: **T-0** — confirm whether `wgcf-egress` lives in
  xray's netns or the host's netns (selects between two documented forwarding
  variants).
- Supervision: dropped an invented `ServiceController::IncusExec` design once
  it turned out `crates/op-grpc-adapters/src/adapters/netmaker.rs`
  (`NetmakerAdapter`, tonic service `op.adapters.v1.NetmakerService`) already
  does exactly this — join/leave/restart via local `netclient` + `sv restart
  netclient` — it's just not deployed *inside* the `NetMaker` container yet.
  Open question: does that container actually have runit/`sv` available?
  Never verified.

### `operation-dashboard-ui-07/.specs/netmaker-console/`
New dashboard page for the `netmaker` plugin. Retargeted mid-review from the
schema-driven 6-method surface (`operation.method.netmaker.*`, has a known
`AckOutput` type-mismatch bug — declares empty output, actually returns real
JSON) to the richer `op.adapters.v1.NetmakerService` — already fully
code-generated in this repo at `src/grpc/gen/adapters.client.ts`, no codegen
work needed. Adds hosts, live event streaming, and a CLI console (16
`netclient` subcommands) the old surface couldn't do at all. Explicitly
depends on the `netclient-container-netns` spec above actually deploying the
adapter inside the container — until then, mutation RPCs degrade to a
disabled/banner state in the UI rather than pretending to work.

## NEXT STEPS

1. Decide whether `/home/admin/git` needs real group-write perms (jeremy is
   currently sudo-only there) — same pattern as the `.bash_secrets` fix would
   work: `chgrp secrets /home/admin/git && chmod 775 /home/admin/git` (+
   `g+s` if new subdirs should inherit the group).
2. Confirm whether the leaked `GH_TOKEN` was actually rotated.
3. Review both specs before any implementation starts.
4. `HANDOFF-2026-08-01-runit-golden.md`'s own next-steps (golden subvolume
   build, live deploy of 4 services) are still outstanding and unrelated to
   this file — read it separately.
