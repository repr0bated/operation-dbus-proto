# Session State

**Current Phase**: Investigation (threetched-fs + blob architecture)
**Current Stage**: Complete — pending optional follow-ups
**Last Checkpoint**: d2daef36 (uncommitted working tree, 153 files)
**Planning Docs**: `WISTLIST.md` (task board), `SIGNALS.md` (observations), `CLAUDE.md` (invariants)

---

## Session: 2026-08-10/11 — threetched-fs + blob architecture investigation

**Started**: 2026-08-10 ~23:00 | **Lines**: ~255

### What was accomplished

- **Diagnosed `--workspace live` error**: missing workspace, not permissions. `threetched-fs workspace list` returned `[]`; the `live` workspace was never created.
- **Verified mount lifecycle**: `threetched-fs mount` is foreground-blocking (no daemonization), confirmed with `timeout 4` (rc=124 at exactly 4s). SIGTERM without `--auto-unmount` leaves an ENOTCONN corpse requiring manual `umount`. Symlinks canonicalize through the mount (`findmnt` and `mountpoint` resolve to the real target; `umount` through a symlink works and cleanly stops the daemon).
- **Discovered views are frozen**: a view is pinned to `source_catalog_generation` (currently 544) and `source_catalog_hash` (`b6a55f82…`). Sealing a new blob bumps the generation but does NOT update any existing view or running mount. `threetched-fs capture` mints a new view; remounting picks it up.
- **Verified projection is lossless**: `threetched-fs inspect --plugin xray` returns 325,426 bytes of valid JSON (19 levels, 6,294 keys, no elision markers). `source_schema_hash` matches the sealed blob filename. `threetched-fs verify` passes.
- **Quantified UI-facing schema coverage gaps** across 68 plugin objects / 3,914 fields:
  - `display_name`: 86% missing (9/68 populated)
  - `category`: 64% uncategorized (44/68)
  - field `description`: 13% empty (547/3,914)
  - `constraints`: 93% absent (278/3,914)
  - `read_only_when`: 0% usage (0/3,914 — mechanism exists, entirely unused)
  - `relationships/dependencies`: 11 edges / 68 plugins
- **Decoded OPBLOB01 sealed format** from `crates/op-blob/src/blob.rs`: 64-byte header (magic, version, section count, schema sha256), 24-byte section table entries, 4 sections (SCHEMA_JSON, MANIFEST_JSON, DESCRIPTOR_SET, META_JSON). Verified self-consistency: recomputed sha256 of section 1 equals header hash equals filename hash16.
- **Discovered present state lives in a SECOND SHM source**: `/dev/shm/opdbus/state/<plugin>.json` (plain JSON, mode 644, world-readable, live-updating). The sealed blobs carry schema/manifest/descriptor/meta but NOT runtime values. State file content is byte-for-byte identical to what `threetched-fs inspect` returns as `value` and what `busctl GetAllProperties` returns (wrapped in an escaped D-Bus string).
- **Built and installed `/usr/local/bin/opblob`**: direct blob section reader with seal verification. Subcommands: `list`, `catalog`, `<plugin>` (schema), `<plugin> manifest`, `<plugin> meta`, `<plugin> descriptor`, `<plugin> state`, `<plugin> all`, `<plugin> sections`. Proven equivalent to `threetched-fs inspect --plugin xray` (value identical, hash identical, schema equal ignoring null-vs-omitted).
- **Found no host cron daemon**: all 6 cron processes are in containers (qdrant, NetMaker, cozo, mail-3tched, 2 UUID-named). `/etc/cron.d/3tched-disk-reclaim` is inert since Aug 6 (ran once at install, never again). The script also has a `journalctl: command not found` bug at line 62. `zeroclaw cron` is the only working scheduler on this host.
- **Surveyed competing blob-read CLIs**: `busctl` (498 hits/76 files), `zcall` (280/49), `zbusctl` (188/23), `opdbus` (146/29), `grpcurl` (143/34), direct `plugin-blobs` reads (103/48), `threetched-fs` (0/0). Root cause is discoverability, not drift — `threetched-fs` has zero mentions in the repo and lives in `/usr/local/sbin` (not on zeroclaw's scrubbed PATH).

### Installed artifacts (host-only, not in repo)

- `/usr/local/bin/opblob` — direct blob section reader with seal verification
- `/usr/local/libexec/3tched/threetchedfs-selftest` — drift gate + mount lifecycle test harness (inert, 2 harness bugs: fixed `sleep 3` too short after teardown; test 4 expectation backwards)

### Known issues found

- `threetched-fs tree` panics on SIGPIPE when piped to `head` (Rust panic, not clean exit)
- Null-serialization mismatch: blob serializer emits `example: null` and omits `display_name`/`org`; projection does the reverse. Content agrees but hash comparison is impossible.
- `/etc/cron.d/3tched-disk-reclaim` is inert (no host cron reads it); has `journalctl` bug at line 62
- Parts of the Rust tree declare `org.opdbus.PluginV1` (missing `.v1`) against the live `org.opdbus.v1.PluginV1` interface — cannot resolve (recorded in SIGNALS 2026-07-29)

---

## Pending Next Actions (all optional)

1. **Commit `opblob` into the tree** — `deploy/` or a crate binary would be more durable than a host-only script. Currently at `/usr/local/bin/opblob` (Python, ~230 lines).
2. **Document direct read paths in CLAUDE.md** — add a "reading plugin state" section with `opblob`, `threetched-fs inspect/tree`, and the state-dir shortcut. Root cause of the 6-CLI confusion was zero discoverability.
3. **Remove or fix `/usr/local/libexec/3tched/threetchedfs-selftest`** — installed but inert (nothing schedules it). Two harness bugs to fix if kept: `sleep 3` too short after teardown; test 4's corpse expectation is backwards (SIGTERM self-cleaned, not the expected ENOTCONN).
4. **Fix inert `/etc/cron.d/3tched-disk-reclaim`** — no host cron reads it. Convert to a runit sleep-loop service or install a host cron daemon. Also fix `journalctl: command not found` at line 62 (systemd leftover on a runit host).
5. **Fill schema coverage gaps** — `display_name` (86% missing), `category` (64% uncategorized), `read_only_when` (0% usage), `constraints` (7%). These land in `crates/op-plugins/src/state_plugins/*.rs` where the `<name>_schema()` functions live, not in UI code. Per CLAUDE.md, UI field renderers derive from PluginSchema, so filling these columns yields labels, grouping, tooltips, conditional disabling, and validation with no new UI code.

---

## Reference: key paths and commands

```sh
# Direct blob reads (installed this session)
opblob list                          # 67 plugins + schema hashes + sizes
opblob catalog                       # generation 544, catalog_hash b6a55f82…
opblob xray                          # canonical PluginSchema JSON
opblob xray manifest                  # D-Bus/gRPC identity + per-method manifest
opblob xray meta                      # json_schema, subids, immutable_paths
opblob xray descriptor                # protobuf FileDescriptorSet (binary)
opblob xray state                     # present state (from /dev/shm/opdbus/state/)
opblob xray all                       # schema + value, the full typed object
opblob xray sections                  # section table + seal verification

# Typed view filesystem (pre-existing)
threetched-fs capture                # mint a view from the live catalog
threetched-fs tree                    # enumerate the whole surface, no mount
threetched-fs inspect --plugin xray   # full typed object (schema + value)
threetched-fs views                   # list immutable content-addressed views
threetched-fs verify --view <hash>    # verify all content hashes

# Present state (no tool needed — plain JSON, mode 644)
cat /dev/shm/opdbus/state/xray.json | jq .

# Live D-Bus (for mutations only — reads should use opblob/threetched-fs)
busctl --address=unix:path=/run/opdbus/session-bus.sock \
  call org.opdbus.v1.plugins /org/opdbus/v1/plugins/xray \
  org.opdbus.v1.PluginV1 Call ss "<method>" '<json args>'
```

### Two SHM sources (the key architectural finding)

| | path | contents | freshness |
|---|---|---|---|
| sealed blobs | `/dev/shm/opdbus/plugin-blobs/<p>.<hash16>.blob` | schema, manifest, descriptor, meta (OPBLOB01 binary) | frozen at seal time |
| present state | `/dev/shm/opdbus/state/<p>.json` | runtime values (plain JSON, mode 644) | live, mutation-updated |
| catalog manifest | `/dev/shm/opdbus/plugin-blobs/.manifest.json` | generation, catalog_hash, plugin→hash map | bumped on each seal |
