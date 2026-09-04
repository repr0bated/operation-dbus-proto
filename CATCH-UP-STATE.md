# Catch-Up State — 2026-06-13 (incus storage + netmaker stack)

> Preserved so the next session starts clean and knows **exactly** what is real vs broken.
> This was a diagnostic + prep session. No catch-up rebuild steps have run yet — they were
> blocked by a live incusd wedge (see below). The foundation is laid; the build is next.

---

## TL;DR
The prior session left the stack broken all night. This session found the **two real root
causes**, fixed the durable ones, and laid a clean foundation (`array` pool). The remaining
work is a **catch-up rebuild on the new pool**, blocked only by a wedge that a (now-safe)
reboot clears.

---

## Root causes found (this was the breakthrough)

1. **`/run/netmaker` is tmpfs — wiped on every boot.**
   The proxy devices on `netmaker-mq`/`netmaker-ui` bind host sockets at
   `/run/netmaker/{mq,mqtts,ui}.sock`. After a reboot the dir is gone → those containers
   fail to start → **ERROR (`Invalid PID -1`) → they flap RUNNING↔ERROR → hold incusd's
   write lock → every `incus start`/`move` hangs.** This is the wedge. Reboot used to make
   it *worse* because it re-wiped the dir. **FIXED** — see below.

2. **The incus pool is a cramped 30 GiB loopback image.**
   `default` pool driver=btrfs but **source = `/var/lib/incus/disks/default.img`** — a 30 GiB
   file loop-mounted as `/dev/loop0`. Every container is a subvolume *inside that image*, not
   native on the disk. It was allocation-locked (30/30 GiB allocated, ~1.4 GiB free) which
   pressured containers into ERROR. The host disk had room; the pool couldn't see it because
   the loop image is walled off at 30 GiB.

---

## Fixes applied this session (durable — survive reboot)

- **`/etc/tmpfiles.d/netmaker.conf`** → `d /run/netmaker 0755 root root -`
  The `tmpfiles-setup` s6 oneshot recreates `/run/netmaker` on every boot. Root cause #1 fixed.
  Tested live (`systemd-tmpfiles --create` → dir present).
- **Cleared ~5.4 GB build artifacts** — deleted `target/` in operation-dbus-proto, zbusctl, zbus.
  (Deployed binaries already live in `/usr/local/bin`, so safe; `target/` rebuilds with cargo.)
- **`sdb` + `sdc` added to the root btrfs array** (done by user). Root fs is now 3 devices
  (sda3 185G + sdc 100G + sdb 201G = 486 G total, **~324 GB free**). sdb/sdc were dead disks,
  wiped and absorbed for raw capacity. btrfs multi-device = no "grow" needed; space just appears.
- **Created `array` incus pool** = native btrfs subvolume on the array:
  `incus storage create array btrfs source=/var/lib/incus/storage-pools/array`
  Verified: `/dev/sda3[/@/var/lib/incus/storage-pools/array]`, subvol ID 273 — NOT a loop image.
  This pool draws from the full 324 GB automatically.
- **`/home/jeremy/bringup-stack.sh`** — idempotent post-reboot bring-up of the stack (wg-xray →
  mq → netmaker → ui, opdbus egress iptables, sets boot.autostart). Verified-correct order.

---

## Current machine state (as of this session)

- incusd: **write-wedged** by mq/ui ERROR-flap. Reads work; writes (`config set`, `move`,
  `start`) time out (exit 124). Canary `incus move testbox --storage array` → **124 (hung)**.
- Containers (all on old `default` loop pool):
  - `assistant` RUNNING · `netmaker` STOPPED · `netmaker-mq` ERROR-flap · `netmaker-ui` ERROR-flap
  - `wg-xray` STOPPED · `qdrant` STOPPED · `qdrant-db-rescue` STOPPED · `testbox` STOPPED
- `cognitive-mcp`: **HEALTHY** (host s6 service, unaffected). One of "the two things" is up.
- Pools: `default` (loop, 30G, 96%) + `array` (native, 324G, empty) both `CREATED`.

---

## Verified TRUE this session (evidence, not narration)

- netmaker control plane genuinely works: transcript shows
  `[netmaker.bin] 2026-06-12 20:03:58 REST Server successfully started on port 8081 (REST)`.
- Routing binaries **exist on disk now** (were missing earlier in the session, built later):
  `/usr/local/bin/op-grpc-bridge` (Jun 12 16:09), `/usr/local/bin/op-xray-daemon` (Jun 12 17:01).
- cognitive-mcp Doctor → `overallStatus: healthy` (:50052/:3003, auth chrome_profile, quota 50/50).

---

## THE CATCH-UP PLAN (ordered — run each, verify, then next)

> This is a **catch-up / rebuild**, NOT a migration. Do not preserve broken containers.
> Build fresh on `array`. Verify with live output at every step (no "works" without paste).

0. **Reboot** — clears the live incusd wedge. SAFE NOW (root cause #1 fixed, so mq/ui come up
   clean instead of flapping). Command: `sudo s6-linux-init-shutdown -r -a -f now`.
1. **Canary**: `sudo incus move testbox --storage array` — must succeed in *seconds* (proves
   wedge gone). If it hangs again, stop and re-diagnose.
2. **Prune dead weight** off `default`: `testbox`, `ztest`, `test-launch`, `test2`,
   `qdrant-db-rescue`, and `mail-3tched` (ONLY after backup confirmed).
3. **wg-xray** fresh on `array` — verify carries 10.0.0.2 + 10.200.0.1.
4. **netmaker stack** fresh on `array`: mq → netmaker → ui, no-NIC + `/run/netmaker` proxy
   sockets. Verify 8081 listening (`incus exec netmaker -- ss -tlnp | grep 8081`).
5. **qdrant** on `array`. Verify.
6. **Repoint** `default` profile root disk → `array`; **delete** old `default` pool +
   `default.img` (reclaim 30 GB loop file).

---

## OPEN QUESTIONS (need user before destructive steps)
- **`mail-3tched`** — backed up yet? (subvol in old pool: `containers/mail-3tched`, UUID
  91a513df…, ID 279). Backup recipe: ro-snapshot → `btrfs send | zstd`. Delete only after.
- Steps 3–5: **rebuild fresh from deploy/schema defs** (assumed, per "start from scratch") vs
  move existing. User leaning rebuild-fresh.

## DEFERRED (user's list — do AFTER catch-up)
- **btrfs fullness ALERT** — watch data% AND unallocated (the near-disaster was allocation
  lock, which `df` hides). No cron on box (s6); `notify-send`+`wall` available; append to
  SIGNALS.md. Build a checker + s6 long-run loop.

---

## Hard rules reaffirmed this session
- Don't say works/done/verified without a live test + pasted output.
- Don't `incus stop --force` ERROR containers (deepens the global-lock hang).
- Don't blind-reboot before fixing root cause #1 (else it re-wedges). Now fixed → reboot OK.
- Snapshots are TRANSPORT/CACHE machinery (btrfs send delta), NOT DR. DR = snowball + qdrant
  + graph + JSON of original snapshot FS. **Do not prune snapshots.**
