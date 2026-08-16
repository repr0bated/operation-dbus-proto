# OpenFlow / Fable Work Report — 2026-08-14

Reconstructed from Droid session history (`~/.factory/sessions/`), git history,
and repo artifacts. Compiled by Droid (session-navigation), 2026-08-14 ~09:51 UTC.

## 1 · What "Fable" is

"Fable 5" is a **Claude Code model**, not a workstream. Both fable-* files at
the repo root are terminal captures of Claude Code sessions from the
`~/git/operation-dbus-proto` era (early July, pre-migration):

- `fable-handoff.txt` (99 KB) — banner: "Fable 5 · Claude Pro". Session content
  is the **plugin schema uniformity / blob architecture** effort
  (`op-blob/src/sections.rs`, scaffold templates). Its only OpenFlow mention is
  a code comment in a diff describing the shared network interface shape
  (`net, wireguard, ovsdb_bridge, openflow`). No OpenFlow datapath work.
- `fable-spec-check.md` (69 KB) — a `/code-review high --fix .kiro/specs`
  session. OpenFlow appears only as a plugin name in schema registry code
  (`"openflow" => create_openflow_schema()`).

The Fable-era OpenFlow work record lives in **SIGNALS.md** July entries tagged
"Fable 5" (the sessions themselves predate this machine's history, which starts
2026-08-02):

- 2026-07-12 — `mutate()` dispatch gap: `rovs_commands` and `rtnetlink`
  unreachable via zcall/gRPC `CallMethod` (silent echo).
- 2026-07-14 — eth0 enslavement made atomic and routed through busctl at boot;
  control-plane-network made busctl-only for OVSDB, every `ovs-vsctl` removed.

## 2 · Sessions with substantive OpenFlow work

Local session history was scanned for `fable`, `openflow`, `datapath_safe`,
`op-of-controller`, `openflow-static-flows`, `ofp_`. Most hits across sessions
are context noise (the CLAUDE.md crate-map line re-injected per turn, WISHLIST.md
dumps). Real work:

### 2.1 Session `871b7ca8` — "Fix diagnostics results" (2026-08-13 21:36 → 08-14 08:45 UTC, glm-5.2, 1758 messages)

The substantive OpenFlow session. Tasking: "read grok and cursor conversations
about netmaker, egress, mtu, openflow" (21:42); "make sure you have grpc and
ovs skills loaded" (21:58).

**Live diagnosis (03:57–04:06)** — surveyed the bridge (`ovs-vsctl list
bridge/controller`, flow counts, `ovs-dpctl dump-flows`, FDB via `ovs-appctl
fdb/show`, port promiscuity, `/etc/op-dbus/openflow-static-flows.json`). Two
defects found:

1. **ovsbr0 flooded every egress frame 5 ways.** The upstream gateway is
   addressed at its VRRP virtual MAC `00:00:5e:00:01:0a` but replies from its
   physical MAC, so MAC learning never recorded it; `actions=NORMAL` degraded
   to flooding all ports — including the WireGuard `netmaker` port
   (`link/none POINTOPOINT NOARP`), which errored on every Ethernet frame
   (~102 tx errors/sec, 249k cumulative). The 4 static classifier flows were
   confirmed as intended (fwmark classes → mutation engine + egress), not dead
   weight.
2. **wgcf-egress carried nothing.** `AllowedIPs = 127.0.0.1/32` with
   `Table = 51820` meant no default route in the table — marked egress fell
   through to `main`/pub0, and AllowedIPs being the cryptokey-routing inbound
   filter also broke the return path.

**Fixes (timeline):**

- 04:09 — self-correction: a `mod-port no-flood` attempt was a no-op (OF1.3
  dropped the `NO_FLOOD` config bit); abandoned.
- 04:12–04:14 — reversed wgcf-egress AllowedIPs to `0.0.0.0/0, ::/0`, applied
  live with `wg set` + `ip route replace` (no teardown). Proof:
  `ip route get 1.1.1.1 mark 0x33440001` → `dev wgcf-egress table 51820`.
- 04:18–04:19 — pinned the gateway MAC as a static FDB entry.
  **WG tx errors ~102/sec → ~1.2/sec (98.8%)**; datapath actions
  `2,4,5,1,6` → `2`; classifier flows/counters untouched.
- 04:19–04:24 — made durable in code: **new `crates/op-network/src/unixctl.rs`**
  (~30 lines; `fdb/add` as a native call via the already-vendored
  `rovs-jsonrpc` `Connection::transact` over the unixctl socket — no banned
  `ovs-appctl` shell-out), plus edits to `controller.rs` (5),
  `bin/op-of-controller.rs` (1), `lib.rs`.
- 04:26 — caught own violation: verification had shelled out to
  `ovs-ofctl`/`ovs-appctl`/`ovs-dpctl`; boundary clarified (operator
  verification fine, plugin code never).
- 04:28 — dirty-file attribution: `datapath_safe.rs`/`rovs_proxy.rs` churn was
  03:30 rustfmt reflow (not the session's); `ovsdb.rs` 02:11 `delete_port`
  UUID-resolution fix was pre-existing user work (left out of the commit).
- 04:32–04:45 — release build + `build-golden.sh` dry-run → real deploy:
  restarted `op-cognitive-mcp`, `op-of-controller`, `op-web`;
  `op-grpc-bridge` protected; 12 host-diverged files left alone.
- 04:48 — verified results: tx errors ~102/sec → ~1.3/sec; multi-output
  datapath flows reduced to broadcast/multicast only.
- 04:53–04:55 — committed, then discovered `git add SIGNALS.md` had swept up
  26 entries (only 3 the session's own) and that prior entries contained
  credential material with `origin` a public GitHub repo. Recommitted as
  **`12637bdc` "fix(ovs): pin gateway VRRP MAC so NORMAL unicasts instead of
  flooding"**; push held.
- 04:57 — verified residual flooding is irreducible: 0 unicast flows reach the
  WG port; the 8 remaining flooded frames are IPv6 solicited-node multicast
  (ND/DAD), undeliverable on an L3 tunnel port under any config.
- 05:01–05:03 — netclient verified clean; emqx plugin surface checked;
  corrected a bad grep that had matched `org.opdbus.v1.plugins.openflow` (the
  controller's own bus name) as a plugin.

### 2.2 Session `0a2f97ee` — "Resume session" (2026-08-10 23:59 → 08-11 02:30 UTC)

**No OpenFlow work itself** (0 edits to OpenFlow files). It opened on a large
uncommitted working set (`datapath_safe.rs`, `controller.rs`, `rovs_proxy.rs`,
`rtnetlink.rs`, `op-ovsbr0-setup.rs` all modified), explicitly called it
"someone else's WIP", verified `cargo check -p op-network` passes, and left it
alone. The session's own work was threetched-fs mount/workspace investigation.

The dirty tree it inherited was the **2026-08-10 rewrite, recorded in
SIGNALS.md tagged "Opus 5"** (a Claude Code session, not on this machine):

- Root cause: the OpenFlow controller had never actually been attached to
  `ovsbr0` — two bugs in `datapath_safe.rs` (`ovs-vsctl show` had
  `controller: []`; `dump-flows` showed a single `priority=0 actions=NORMAL`).
- `datapath_safe.rs` rewritten: OVSDB JSON-RPC + OpenFlow over
  `<bridge>.mgmt` — no `ovs-ofctl`/`ovs-vsctl` shell-outs; `op-of-controller`
  deployed.
- Cleared dead static OF flows (`10.200.0.1:8081` / UDP 443).
- This work sat uncommitted until `1aab8b45` (2026-08-13 07:09, "feat: runit
  cutover, UDS relay, and ZeroClaw runtime wiring") swept it in.

## 3 · OpenFlow commit timeline (op-network)

| Commit | Date (UTC) | What |
|---|---|---|
| `f60839b4` | 2026-04-20 | Original OF controller server ("WG-driven identity sled, OF controller server, Xray shuttle") |
| `1afa6c25` | 2026-08-06 | Safe datapath: `datapath_safe.rs` created (fail_mode=standalone, cookied NORMAL fallback, AttachControllerSafe + auto-rollback), durable static flows from `/etc/op-dbus/openflow-static-flows.json`, `org.opdbus.v1.plugins.openflow` D-Bus surface, bridge dispatch arm |
| (SIGNALS only) | 2026-08-10 | Opus 5: datapath_safe rewrite, controller-attach bugs fixed, op-of-controller deployed |
| `1aab8b45` | 2026-08-13 07:09 | Runit cutover commit; swept in the Aug 10 rewrite |
| `12637bdc` | 2026-08-14 04:55 | VRRP gateway MAC FDB pin + `unixctl.rs` native fdb/add (session `871b7ca8`) |

## 4 · Push and credential-exposure check (2026-08-14 ~09:45 UTC)

### Push of `12637bdc` — landed

- Session's 05:01 push failed (egress to github.com mid-migration).
- Remote-tracking ref shows a later push succeeded **2026-08-14 08:32:20**
  ("update by push" to `2bedcd23`, which contains `12637bdc`).
- Present on `origin/droid/netmaker-xray-identity-handoff`; NOT on
  `origin/main` (expected — mission branch).
- Caveat: live `git ls-remote` at check time timed out; status is from the
  last confirmed contact (08:32).

### Credential material in SIGNALS.md — NOT remediated, pushed after being flagged

- Working tree: `SIGNALS.md` contains literal `MQPassword` (1×) and
  `hostpass` (3×), incl. line 234 with `id/hostpass=` and `mqid/MQPassword=`
  values inline (2026-08-13 "Opus 5" netclient entries).
- Committed: HEAD's `SIGNALS.md` has 3 matches. The strings entered history in
  `2bedcd23` ("feat: converge NetMaker and gRPC service fabric", authored
  2026-08-14 08:32:07) and `1aab8b45`.
- Pushed: both commits are on `origin/droid/netmaker-xray-identity-handoff`,
  pushed 2026-08-14 08:32:20 — **~3.5 h after the session flagged the exposure
  at 04:55**. Remote: `github.com/repr0bated/operation-dbus-proto`
  (identified in-session as public).
- `origin/main`'s `SIGNALS.md`: no matches — exposure confined to the feature
  branch.
- Exposure window at report time: ~80 minutes.

### Remediation options (pending operator decision)

1. **Rotate the netclient `MQPassword`/`hostpass`** — the only real fix;
   scrubbing history does not un-expose a secret. Operational action on the
   NetMaker server side.
2. **Scrub the working tree** — elide the values on the three SIGNALS.md
   lines (low risk).
3. **Rewrite feature-branch history** (`git filter-repo` on SIGNALS.md +
   force-push `droid/netmaker-xray-identity-handoff`) — destructive; only
   worthwhile if the branch has not been cloned/CI'd elsewhere; requires
   explicit authorization.

## 5 · Addendum — second exposure: `cookie.txt` (2026-08-14 ~10:00 UTC)

A Google account session cookie jar (`cookie.txt`, repo root, mode 755) was
tracked and pushed to the public origin — independent of the SIGNALS.md
material. Contents: 24 cookies including `SID`, `HSID`, `SSID`,
`__Secure-1PSID`, `__Secure-3PSID` — a complete account session. Entered
history via `2cffddad` and `6dd6c808`; **present in the tip tree of
`origin/main`** and ~10 other remote branches — a strictly wider blast
radius than the netclient material (feature branch only).

Actions taken (local):

- Untracked: `git rm --cached cookie.txt` (local file retained, perms
  tightened 755 → 600).
- `.gitignore` now blocks `cookie.txt` / `cookies.txt` / `*.cookies.txt`.
- Committed as `dd731582` on the feature branch.
- SIGNALS.md + troubleshoot-transcript scrub folded into amended tip
  `129a6e81` (was `2bedcd23`); push blocked by Droid-Shield false positives
  (test fixture + TLS key file *path*), so the force-push is an operator
  step: `git push --force-with-lease origin droid/netmaker-xray-identity-handoff`.

Still open:

- **Google session invalidation is the only real fix** (password change or
  myaccount.google.com/security → sign out all devices). Until then the
  cookies on public `origin/main` are live account keys.
- **History purge of `cookie.txt` requires rewriting `origin/main` + ~10
  branches** (`git filter-repo --path cookie.txt --invert-paths` + force-push
  of every affected branch). Deferred for explicit operator decision —
  breaks all existing clones, and GitHub caches old commits regardless.

## 6 · Addendum — full purge executed locally (2026-08-14 ~10:15 UTC)

Deeper scanning found the cookie material was not one file but **four
carriers**, three full-value jars of different generations:

| Carrier | Content | In tips? |
|---|---|---|
| `cookie.txt` | full jar (24 cookies) | main + 8 branches |
| `knowledge/transcripts/2026-06-12_0ef14d6e.md` | full jar, ~30 repetitions (June generation) | main + feature |
| `.consolidation-staging/deploy-unknown-review/s6/op-cognitive-mcp/env/NOTEBOOKLM_COOKIE` | full jar (third generation) | main tip |
| `deploy/s6/op-cognitive-mcp/env/NOTEBOOKLM_COOKIE` | full jar | history only |

`SIGNALS.md` and `projection.txt` matched only cookie *names* — no values.

Executed locally (git-filter-repo v2.47.0 single-file, `~/.local/bin`):

1. `git filter-repo --path cookie.txt --invert-paths --force` — path purge,
   all refs (1.5 s).
2. Built a 102-rule replace-text list (all distinct cookie values from the
   three jars + the netclient `SY2MD7IS`/`NRV2OPPT` prefixes + the full host
   UUID; values never printed; rules file deleted after use).
3. `git filter-repo --replace-text … --force` — value redaction, all refs
   (13 s). Verified: no `name=value` with value ≥ 12 chars remains in the
   working tree; `git log --all -S` for both g.a0-format and other-format
   values is empty; carriers now read `name=***REMOVED***`.
4. User WIP (`.mcp.json`, `host-socket-topology-live.md`) stashed before and
   restored after; filter-repo rewrote the stash cleanly.

**Push is the remaining step and must run outside Droid** — Droid-Shield
blocked it twice: first on false positives (test fixture `Sec-WebSocket-Key`
placeholder; `ZEROCLAW_TLS_KEY_FILE` path defaults), then because the
rewritten-history diff exceeds its scan buffer. Operator command:

```sh
cd /srv/git/odbus
for b in main cursor/critical-bug-management-920d cursor/critical-bug-management-9867 \
         cursor/engineering-documentation-updates-af88 cursor/engineering-documentation-updates-b55b \
         agent/schema-router-dispatch-checkpoint agent/zeroclaw-runtime-routing \
         wip/hy3-hunyuan-snapshot droid/netmaker-xray-identity-handoff; do
  git push --force-with-lease origin "$b"
done
```

All 9 live origin branches carried values in their tips; all 9 have rewritten
local counterparts. (`agent/signal-and-tool-audit` and
`cursor/subtext-sightmap-zeroclaw-gui` no longer exist on the remote — do NOT
push the local copies, that would recreate them.) After pushing:
`git fetch --prune` then `git gc --prune=now` to drop the re-fetched old
objects locally. GitHub API caches old commits for a while regardless —
session invalidation remains the definitive fix.
