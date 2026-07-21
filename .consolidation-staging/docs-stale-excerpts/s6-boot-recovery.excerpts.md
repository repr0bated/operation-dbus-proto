# Dropped Excerpts from s6-boot-recovery-gemma-ollama-handoff.md

Extracted: 2026-07-20

## PR References (Dropped - version-specific, no longer actionable)

```markdown
- PR #15 (`feat/sled-source-port-salt`, security/correctness fixes triaged earlier) was sent to Ultraplan for cloud refinement, approved, and executed remotely as a PR — check GitHub for its actual landing state, not yet re-verified from this session.
- PR #14 (`plugin-capability`) was closed as superseded by #15 — done, no follow-up needed.
```

**Reason**: PR numbers are ephemeral and version-specific. Current code state supersedes.

---

## Caddy Container Deletion (Dropped - obsolete infrastructure)

```markdown
**Port 443 conflict**: an Incus proxy device on the `caddy` container (`docker-port-0.0.0.0-443`, set up via `incus compose` importing Netmaker's own docker-compose bundle) was bound to host port 443, competing with `xray`. Per user direction, the `caddy` container was **deleted entirely** (xray already covers reverse-proxy duty; caddy was serving Netmaker's own dashboard/API/broker via SNI routing, now redundant). Confirmed caddy removed cleanly; `xray` restarted via D-Bus and came up stable.
```

**Reason**: Caddy infrastructure no longer exists; xray is the canonical reverse proxy. Conflict resolution is historical.

---

## Op-Openvswitch-Daemon References (Dropped - deprecated daemon)

From the s6-rc change line:
```markdown
└ s6-rc(20424) -u -- change ovsbr0-static ovsbr0-init opdbus op-web-srv op-projection op-openvswitch-daemon op-dbus-mirror op-dbus op-cognitive-mcp
```

**Reason**: `op-openvswitch-daemon` has been removed from the tree. OpenVSwitch is now managed via native OVSDB JSON-RPC through the rovs plugins. See `CLAUDE.md` Host tooling section.

---

## Ollama-Srv Bring-Up Status (Dropped - task-specific context)

```markdown
Once the above is resolved: start `ollama-srv` via D-Bus `org.opdbus.v1.S6.Systemctl` `Start`, verify `ollama serve` comes up, verify `gemma4:12b` is reachable and zeroclaw's active provider (`ollama`) works end-to-end.
```

**Reason**: Task-specific next-action for that session. Ollama bring-up is operational procedure, not recovery documentation.

---

## Orphaned Op-S6-Systemctl Process Investigation (Dropped - unresolved investigation)

```markdown
**`op-s6-systemctl`'s running process was orphaned** — a child of PID 1 directly, not tracked by any current s6 servicedir I could find (killed it by mistake chasing a bad CWD-based assumption about which servicedir owned it — no collateral damage, but had to manually relaunch `/usr/local/bin/op-s6-systemctl` via `nohup` since nothing auto-respawned it). **This should be looked into**: why isn't it under proper s6 supervision, and how was it originally started?

...

- Investigate why `op-s6-systemctl`'s process is orphaned/unsupervised rather than a normal s6 longrun — a "why" question, not yet answered.
```

**Reason**: Unresolved investigation item from that session. If this is still an issue, it belongs in WISHLIST.md or a dedicated supervision audit, not recovery documentation.

---

## Fsck Reminder (Dropped - routine maintenance, not recovery)

```markdown
- `fsck` on `/dev/sda1` (FAT/EFI partition) — dmesg showed "not properly unmounted, some data may be corrupt" from the abrupt reboot during the outage. Not urgent, not yet done.
```

**Reason**: Routine filesystem maintenance reminder from that session. Not part of the recovery procedure itself.

---

## Busd Migration Status (Dropped - deployment status, not recovery)

```markdown
`deploy/s6/dbus-session/run` — already correctly edited this session to exec `busd` instead of `dbus-daemon` (busd installed at `/usr/local/bin/busd` v0.5.0) — **this change has never actually gone live yet**; it's sitting in a pending commit that hasn't successfully installed (blocked by all of the above). Once `s6-apply` succeeds cleanly, verify `dbus-session`'s live servicedir actually runs `busd`, not the old `dbus-daemon`.
```

**Reason**: Deployment status from that session. The busd migration is either complete or documented elsewhere; recovery docs should be implementation-agnostic.

---

## Raw S6 Command Examples (Modified, not dropped - replaced with service6 policy reference)

Original text contained raw `s6-rc-db check`, `s6 set commit`, `s6 live install` commands.

**Action**: Kept `s6-rc-db check` in recovery procedure (emergency console recovery context where raw commands are permitted per AGENTS.md). Added explicit reference to agent host-service policy requiring `sudo service6 ...` for normal operations and noting raw commands are reserved for boot and console recovery.
