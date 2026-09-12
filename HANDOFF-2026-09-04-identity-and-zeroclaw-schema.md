# Handoff — 2026-09-04 — identity guardrail, pairing removal, zeroclaw binary-backed schema

Branch: `identity-guardrail-and-zeroclaw-binary-schema` (9 commits, pushed to origin).
Base: `origin/main` = `9e9df97f`. Nothing merged to main.

Session started from a Devin session (`stingy-eyelash`, "Audit WireGuard Identity &
Metadata Injection") that hit a rate limit mid-flight. Its transcript is at
`~/.local/share/devin/cli/transcripts/stingy-eyelash.json`; its uncommitted output
(the decision record, CLAUDE.md correction, SIGNALS rows) is included in this branch.

---

## 1. State right now

| Thing | State |
|---|---|
| 9 commits | committed + **pushed** |
| `cargo check --workspace --all-targets` | **green** |
| Release binaries built | **yes** — `target/release/{op-grpc-bridge,opblob}`, 05:25 |
| Installed / deployed | **NO** — host still runs the Sep 3 04:15 bridge |
| Blob catalog resealed | **NO** — still `catalog_hash 350940c8…`, generation 37165 |
| In-flight uncommitted work | **yes, and UNVERIFIED** — see §5 |

**The deploy was prepared and never executed.** Everything below in §4 is ready to run.

---

## 2. What shipped in the 9 commits

**`47c7826f` — the big one: tched_router is no longer backed by a stub.**
The workspace resolved `zeroclaw` to `vendor/zeroclawlabs`, a 193-line stand-in whose
54 config types were **all empty structs**. `tched_router`'s 91 methods advertised real
names carrying no fields. It compiled, sealed and served for weeks. Both guards that
should have caught it failed silently: the drift test early-returned *by design* when
`$defs` was absent, and `toml::from_str::<Config>` against an empty struct with no
`deny_unknown_fields` accepts any input — so "validate against the REAL upstream type"
validated nothing.

Repointing at `/srv/git/zeroclaw` would not have fixed it: that checkout is **0.5.2**
while the deployed binary is **0.8.4**.

Now: no `zeroclaw` crate dependency at all. The surface is generated from
`zeroclaw config schema` — the official binary's own output, captured at
`schemas/zeroclaw/config.schema.json` (verified byte-identical, 1,345,220 bytes, to what
the running pid prints). Mirrors `emqx.rs`, which declares its own schema and drives the
shipped binary.

- **65 → 89 config methods.** +31 sections that exist in 0.8.4 and were invisible
  (`providers`, `sop`, `trust`, `escalation`, `risk_profiles`, `runtime_profiles`,
  `verifiable_intent`, `pacing`, `peer_groups`, the CLI-runner configs, `a2a`, `acp`,
  `wss`, file upload/download, `media_pipeline`, `image_gen`, the bundles).
  −7 that 0.8.4 removed, whose methods were **dead calls** (`agent`, `autonomy`,
  `extra_headers`, `identity`, `model_providers`, `swarms`, `workspace`).
- Config validation is now real (`jsonschema` against the official document).
- The generator **refuses to emit** from a too-small document — the silent-deletion
  failure mode is now a hard error.
- The drift test no longer early-returns.

**`d971284e` — identity guardrail (G-1).** `SessionIdentity` gains `principal_kind`
(`human`/`service`); the no-selector fallback requires a sled *positively* labelled
human, so an unlabelled sled fails closed. Explicit selectors still resolve anything.

> **This guardrail is INERT.** Nothing writes the field; all four live sleds are
> unlabelled. Arming it is **OD-35**. The commit reads like a fix — it is the
> precondition for one.

**`2ba0edfb` — pairing removed.** `POST /pair` minted a bearer never checked anywhere
(`lookup_paired_token`, zero callers), and `/admin/paircode/new` resolved its session
from **caller-supplied headers**, so naming the chatbot's session id would mint a code
bound to the service principal. BREAKING: the embedded UI's login stops working.

**`00196f21`** — MCP protocol-version negotiation (not authored here; carried so it
reaches the running bridge). **`d314c4fb`** — four examples frozen against moved APIs.
**`d6963fcc` / `72e83a61`** — 424 pre-existing dirty files (the op-blockchain →
op-snowball rename), committed separately so the tree is clean for the reseal guard.
Verified no key material in either.

---

## 3. Findings that matter more than the code

**No human can authenticate at all.** OIA1 has a validator (`op-grpc-bridge`) and
**no issuer**. `op-decoy-issuer` is installed on *this* host (`/usr/local/bin`, 6.7 MB)
with **no source in the tree** and **no service anywhere** — and it belongs on the Oracle
decoy, the only WG terminator. The tree contains a *patch* to `op-decoy-issuer.service`
but not the unit itself. This is why the chatbot kept becoming everyone's fallback
identity: it held the only working credential in the system. → **OD-34**.

**Live state corrections** (the tree and SIGNALS were both stale):
- `svc0` = **10.0.0.3** *is* the `:8090` tonic TLS door. The bridge binds it directly.
  The standing "nothing binds 10.200.0.2:8090" concern rests on a misreading — the
  dual-IP `3tched` port (10.0.0.2 HTTP + 10.200.0.2 gRPC) is the separate gRPC↔HTTP
  bridge. Nothing is supposed to bind 10.200.0.2:8090.
- Public `:443` is terminated by **`sni-demux.py` (Python)** sitting *in front of* xray,
  an undocumented hop in the identity trust path in a tree whose rule is "no new Python".
- All 4 sleds are anchored+active (spec says 3 are unanchored). So the fallback already
  returned `Ambiguous(4)`, not the chatbot — the bug needed the chatbot to be the *only*
  anchored sled. G-1 removes the dependence on that accident either way.
- Only `mail-3tched` and `xray` are RUNNING; two containers are named by session_id
  (`bea37ecb…` = chatbot).
- The `incus` state in SHM is **stale/fake** — one demo instance `privacy-user-123`
  while the host runs seven real ones.

**The recurring failure class** (now in SIGNALS): code that looks alive, compiles
nowhere, is exercised never. Three instances in one session — the zeroclaw stub, four
stale examples, and `crates/zeroclaw-gui` which is absent from `members` entirely so it
is never type-checked. Suggested: gate reseals on `--all-targets`, since
`reseal-plugins.sh` builds only two `--bin` targets and cannot surface this.

---

## 4. Deploy runbook (prepared, NOT run)

Release binaries are already built, so this is fast. Requires a clean tracked tree.

```bash
cd /srv/git/odbus
./deploy/reseal-plugins.sh        # clean-tree + origin/main checks, then seal+install+restart
```

It will: build (cached) → `opblob seal-shm` → install by **rename** (avoids ETXTBSY on
the running process) → `sv restart op-grpc-bridge`.

**Pre-deploy snapshot to verify against:**

| | value |
|---|---|
| catalog_hash | `350940c88276213af5dc8e32fa275ae6f9a6a3b02f5ffa6ae9aeb485b2aea526` |
| generation | 37165 |
| blobs | 68 |
| `tched_router` blob | 437,263 bytes @ `960c2e12` |

**Expected after:** catalog hash moves, `tched_router` blob grows to ~3 MB (measured:
2.56 MB embedded schema; `/dev/shm` is 16 GB at 1%, op-blob has no size cap, and
`build.rs` maps `object`→`google.protobuf.Struct` so proto descriptors do **not** grow).

**Rollback** (binaries already backed up):
```bash
sudo mv /usr/local/bin/op-grpc-bridge.bak-pre-zcschema-20260904T051652Z /usr/local/bin/op-grpc-bridge
sudo mv /usr/local/bin/opblob.bak-pre-zcschema-20260904T051652Z /usr/local/bin/opblob
sudo /usr/local/bin/opblob seal-shm && sudo sv restart op-grpc-bridge
```

**Watch for** (from SIGNALS): a rustls `CryptoProvider` panic loop on the new binary; a
plaintext `:8090` bind (the run script does set `ZEROCLAW_TLS_CERT_FILE`, so this should
be fine); and `init_audit_durability` replaying `timing/*.json` — currently clean, but a
prior incident hung a start at 3.46M files / ~10 GB RSS. Recovery there is a btrfs
**subvolume** rename, never `mkdir`.

**Unverified until deployed:** none of the 31 new methods has been *dispatched*. They
generate and compile; `read_section` reads the live config file, and sections absent from
`config.toml` are unexercised.

---

## 5. In-flight, uncommitted, UNVERIFIED

Started converting subprocess calls to D-Bus after the direction "this is operation
dbus. use dbus" and "nothing should call docker. this is an incus machine".

`crates/op-plugins/src/state_plugins/full_system.rs` + `crates/op-plugins/Cargo.toml`:

- Removed the `docker ps` subprocess. **Docker and podman are not installed** — the
  failure was swallowed by `if let Ok(..)`, so it reported an empty container list on a
  seven-container Incus host and looked like it worked. `DockerContainerInfo` removed.
- Removed the `/var/lib/lxc` scan (wrong tree; Incus stores under `/var/lib/incus`), and
  a comment saying "fall through to docker check" above a `return`.
- `capture_containers` now **reads the present state** from
  `/dev/shm/opdbus/state/incus.json` rather than collecting it. Per the correction
  received: *there is no projection — the current state IS the state.*
- Both `hostnamectl` calls → `nix::unistd::sethostname` + `/etc/hostname`.
  `hostnamectl` is **not installed** and neither `org.freedesktop.hostname1` nor
  `systemd1` is on this bus, so those calls could only ever fail. Added the nix
  `hostname` feature.

**This has NOT been compiled.** Build it before doing anything else with it.

### Not yet converted — the remaining subprocess calls

```
service_def.rs:447,503        Command::new("sv")        -> org.opdbus.v1.Runit.Systemctl
state_plugins/service.rs:119  Command::new("sv")        -> same
state_plugins/host_runtime.rs:352 Command::new("sv")    -> same
state_plugins/tched_router.rs:2128 Command::new("zeroclaw")
```

`emqx.rs` is the reference for the `sv` ones — it drives lifecycle over D-Bus at
`org.opdbus.v1.Runit.Systemctl`, path `/org/opdbus/v1/plugins/runit/systemctl`, with
`Status(service)->String(JSON)`, `Start|Stop|Restart(service)->(bool,String)`,
`IsActive(service)->String`. The runit service is the one component legitimately parsing
`sv` output, because it owns that boundary.

`tched_router.rs:2128` shells out to `zeroclaw config get/set/list/patch/init/migrate`.
Four of those six duplicate what the new native surface already does correctly. Context
given: this subprocess/polling pattern is what **instigated rebuilding the decoy** —
so it is a survivor of a purge already fought elsewhere. Note the tension in this
branch: the *schema* half of that plugin is now binary-backed the right way while the
*config get/set* half still shells out.

Also flagged and NOT actioned: **REST is deprecated** — `incus.rs` talks to the Incus
REST API over `/var/lib/incus/unix.socket`. That is not the sanctioned pattern either;
D-Bus is the only control plane.

---

## 6. Next actions, in order

1. **Build the in-flight §5 changes.** They are unverified.
2. **Deploy** (§4) — or decide to hold until §5 lands.
3. **OD-36 / OD-37** — small and independent: rotate the exposed chatbot key
   (private key + `mcp_token` leaked into a transcript, still live), and set
   `OP_IDENTITY_SESSION_ID` in run scripts (no script sets it today, so five services
   silently lose identity).
4. **OD-35** — arm the guardrail; it must land before OD-34 is useful.
5. **OD-34** — stand up the OIA1 issuer on the decoy. Long pole, needs off-host work,
   and needs the source recovered or rewritten first.
6. Finish the subprocess → D-Bus conversion (§5).

Full list: `WISHLIST.md` OD-34..OD-41. Decision record:
`.kiro/specs/control-plane-chatbot-identity/requirements.md`.

## 7. Notes for whoever picks this up

- Builds/tests go to a subagent (haiku); output floods context otherwise.
- The `op-dbus-unified` MCP server failed to connect all session
  (`UNKNOWN_CERTIFICATE_VERIFICATION_ERROR`), so every live-state claim here came from
  direct host inspection, not from plugin calls.
- `principal_kind` on the sled is a **different axis** from `SealedId.principal_kind`
  (always `wireguard-principal`). Sealing the sled value into SID1 would make
  `mcp_frontend` reject the envelope. OD-35 should consider renaming one of them.
