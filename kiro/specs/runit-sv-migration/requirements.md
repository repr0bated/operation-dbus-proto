# Requirements: runit-sv-migration

## Purpose

Finish the host's migration from s6 to **runit**, controlled with `sv`. The host
already boots runit and has no s6 binaries installed, so every remaining s6 call
site in this repo is dead code that fails at runtime. This spec removes the s6
surface across five layers — process execution, runtime paths, the agent/model
tool surface, deploy artifacts, and docs — and adds a regression guard so the
class of drift cannot return.

Scope is the **host**. Containers are out of scope (see Non-Goals).

## Context and Verified Baseline

### Live host state (verified 2026-08-01)

- `ps -p 1` → **runit** is PID 1.
- `/usr/bin/sv` and `/usr/bin/runsv` are present. `s6-rc`, `s6-svc`,
  `s6-svstat`, and `service6` are **all absent** (`command -v` finds none).
- Layout:
  - `/etc/runit/sv/<service>/run` — service definition.
  - `/etc/runit/runsvdir/default/<service>` — symlink that enables a service
    (`/etc/runit/runsvdir/current -> default`).
  - `runsvdir -P /run/runit/service` (pid 1058) supervises the enabled set.
- `op-grpc-bridge`, `op-web`, `op-cognitive-mcp`, `op-session-bus`, and
  `op-of-controller` are enabled runit services
  (`/etc/runit/runsvdir/default/op-grpc-bridge -> /etc/runit/sv/op-grpc-bridge`).
- `sv` requires root to read `supervise/ok`; an unprivileged `sv status` fails
  with "access denied".
- `/etc/runit/sv/op-grpc-bridge/run` sources `/etc/op-dbus/environment` with
  `set -a`, then `exec /usr/local/bin/op-grpc-bridge`.

### Already migrated — must not regress

- **`crates/op-plugins/src/state_plugins/service.rs`** — 14 runit references, 0
  s6; shells out via `Command::new("sv")` at line 119. The plugin surface for
  service control is already correct.
- **`crates/op-s6-systemctl/src/dbus.rs`** — 567 lines, migrated to runit
  (`sv up/down/restart/status`, `pgrep -x runsvdir`), with
  `RUNIT_SV_DIR = "/etc/runit/sv"`, `RUNIT_SERVICE_DIR = "/run/runit/service"`,
  `RUNIT_RUNSVDIR_DEFAULT = "/etc/runit/runsvdir/default"`. Compiles clean with
  zero clippy diagnostics.
- **`deploy/runit/recompile-and-update.sh`** — the sanctioned build → install →
  restart path, installed as `/usr/local/sbin/op-runit-recompile-and-update`.
  It already installs a legacy `op-s6-recompile-and-update` alias, establishing
  the precedent this spec follows for renamed entry points.
- **`CLAUDE.md`** — corrected in commit `e47baa26` ("host runs runit not s6").
- **`AGENTS.md`** and **`README.md`** — host-service policy corrected to
  runit/`sv` (uncommitted at time of writing).

### Broken: invokes binaries that do not exist on this host

Each of these spawns a missing executable, so the call fails at runtime:

| File:line | Dead invocation |
|---|---|
| `crates/op-tools/src/builtin/s6.rs:38` | `Command::new("s6-rc")` |
| `crates/op-s6-systemctl/src/main.rs:71` | `Command::new("s6-svscan")` readiness probe |
| `crates/op-grpc-adapters/src/adapters/netmaker.rs:379` | `Command::new("s6d")` |
| `crates/op-network/src/bin/op-ovsbr0-setup.rs:251` | `Command::new("service6")` (stop) |
| `crates/op-network/src/bin/op-ovsbr0-setup.rs:322` | `Command::new("service6")` (start) |

`crates/op-tools/src/builtin/s6.rs` is registered in the tool registry
(`crates/op-tools/src/builtin/mod.rs:19: pub mod s6;`) and exposes agent tools
named `s6_start_service`, `s6_stop_service`, `s6_service_status`, and
`s6_list_services` (lines 20-24, 61). It reaches the daemon through
`S6SystemctlProxy` and falls back to `s6rc()` — both paths are dead.

`crates/op-s6-systemctl/src/bin/s6d.rs` is a CLI named `s6d` whose proxy targets
`default_path = "/org/opdbus/v1/plugins/s6/systemctl"`.

### Broken: wrong runtime path

The live supervised tree is `/run/runit/service`. These hardcode the s6 tree:

| File:line | Value |
|---|---|
| `crates/op-web/src/handlers/status.rs:214` | `/run/service/{name}/supervise/stat` |
| `crates/op-plugins/src/state_plugins/cognitive_mcp.rs:34` | `S6_SV_PATH = "/run/service/op-cognitive-mcp"` |
| `crates/op-plugins/src/state_plugins/cognitive_mcp.rs:36` | `RUNTIME_ENV_DIR = "/run/service/op-cognitive-mcp/env"` |
| `crates/op-plugins/src/state_plugins/cognitive_mcp.rs:205,213` | same path in messages |
| `crates/op-plugins/src/state_plugins/netmaker.rs:83` | probes `/run/s6-rc` and `/run/service` |
| `crates/op-state-store/src/plugin_schema.rs` | 7 s6 references |

### Broken: the model is told to use a dead init system

`crates/op-chat/src/system_prompt.rs` puts s6 into the LLM's fixed base prompt:

- line 31: "Artix Linux, **s6 service supervision**, Incus containers…"
- line 36: "xray via the `gbr-xray` **s6 service**"
- line 37: "**Service management** via s6 — NOT systemd, NOT systemctl"
- line 52: "`s6_*` tools for service management"
- lines 96-114: a whole "## s6 Service Management" section — "**THIS HOST USES
  s6, NOT systemd**", "All service source files live in `deploy/s6/` … then
  copied to `/etc/s6/sv/`", and `run` script examples using `s6-setuidgid`.

`crates/op-web/src/orchestrator/anti_hallucination.rs:20-24` reinforces it,
mapping `systemctl start` → `s6_start_service` and, notably, `systemctl restart`
→ `"s6_stop_service + s6_start_service"` — a two-step workaround that runit
makes unnecessary because `sv restart` is native.

This is the highest-impact item: it actively teaches every model turn to reach
for tools that cannot work.

### Dead code: orphaned `crates/op-dbus/`

- `crates/op-dbus/` has **no `Cargo.toml`** and is **not a workspace member**
  (absent from the root `Cargo.toml` members list).
- Its only file is `src/s6_systemctl.rs` (654 lines), which invokes `s6-rc`,
  `s6-svc`, `s6-svstat`, `s6-logwatch` and requests the same D-Bus name
  (`org.opdbus.v1.S6.Systemctl`) as the live `op-s6-systemctl` crate.
- Because the crate never compiles, this is **not** a runtime bus-name
  collision — it is misleading dead source that duplicates a migrated file.

### Deploy artifacts

- `deploy/s6/qdrant-grpc-loopback/run` — an s6 service definition with no runit
  counterpart under `deploy/runit/`.
- `deploy/s6/recompile-and-update.sh` — differs from the runit script and
  contains 0 s6 references; superseded by `deploy/runit/recompile-and-update.sh`.
- `deploy/agent-s6-guard.sh` + `deploy/99-agent-s6-guard.hook` — deny-list of
  `/usr/bin/s6*` paths. Since no s6 binaries exist, the guard is now a no-op and
  provides no protection for the runit surface it should be guarding.
- `deploy/agent-service6.sudoers` — `%wheel ALL=(ALL) NOPASSWD: ALL,
  !/usr/bin/s6*, …`, with comments describing `sudo service6`.
- `deploy/PKGBUILD-sdbusplus-s6`, `deploy/build-sdbusplus-s6.sh` — 0 s6-init
  references; the `-s6` suffix is naming only (sdbusplus is a D-Bus library).

### Docs

- `docs/operations/artix-s6-bootdb-recovery.md` — recovery procedure for the
  s6-rc compiled bootdb. Runit has no compiled service database, so the whole
  procedure is inapplicable.
- `docs/overview/architecture.md`, `docs/guides/user-guide.md` — s6 references.
- `docs/book/**` — generated output; excluded.

---

## Functional Requirements

### FR-1: No code path spawns a binary that does not exist

Every `Command::new(...)` targeting an s6-era executable is replaced with its
runit equivalent, or removed if the operation has no runit analogue:

| Dead call | Replacement |
|---|---|
| `s6-rc -u change <svc>` | `sv up <svc>` |
| `s6-rc -d change <svc>` | `sv down <svc>` |
| `s6-svc -r <svc>` | `sv restart <svc>` |
| `s6-svstat <svc>` | `sv status <svc>` |
| `s6-svscan` (liveness probe) | `pgrep -x runsvdir` |
| `s6-logwatch <svc>` | `tail -n <N> /var/log/sv/<svc>/current` |
| `service6 start\|stop <svc>` | `sv start\|stop <svc>` |
| `s6d <cmd>` | the renamed CLI (FR-7) |

**Acceptance criteria**:
`grep -rnE 'Command::new\("(s6[a-z-]*|service6|s6d)"\)' crates/ --include=*.rs`
returns no matches. `cargo check --workspace` exits 0.

### FR-2: One source of truth for runit paths

The three runit locations are defined once and imported, replacing every
hardcoded `/run/service`, `/etc/s6/sv`, and `/run/s6-rc` string:

- `/etc/runit/sv` — service definitions
- `/etc/runit/runsvdir/default` — enablement symlinks
- `/run/runit/service` — supervised tree

They live in a shared crate that the consuming crates (`op-web`, `op-plugins`,
`op-state-store`, `op-chat`, `op-tools`) already depend on, so no new dependency
edges are introduced.

**Acceptance criteria**:
`grep -rn '"/run/service\|/etc/s6/sv\|/run/s6-rc' crates/ --include=*.rs`
returns no matches. The constants are defined in exactly one module.

### FR-3: Agent tool surface is renamed and gains native restart

`crates/op-tools/src/builtin/s6.rs` becomes `sv.rs`. Tools are renamed:

| Old | New |
|---|---|
| `s6_start_service` | `sv_start_service` |
| `s6_stop_service` | `sv_stop_service` |
| `s6_service_status` | `sv_service_status` |
| `s6_list_services` | `sv_list_services` |
| — | `sv_restart_service` (new; `sv restart` is native) |

No aliases are kept: tool names are discovered from the registry each session,
and leaving dead `s6_*` names discoverable is precisely the hallucination risk
being removed.

**Acceptance criteria**: `grep -rn '"s6_' crates/ --include=*.rs` returns no
matches. The registry lists all five `sv_*` tools. Calling `sv_restart_service`
issues one `sv restart`, not a stop/start pair.

### FR-4: Model-facing text describes runit

`crates/op-chat/src/system_prompt.rs` and
`crates/op-web/src/orchestrator/anti_hallucination.rs` are rewritten to state
that the host runs runit, that services are controlled with `sv`, that
definitions live in `/etc/runit/sv` (git-tracked under `deploy/runit/`), and
that the tools are `sv_*`. The anti-hallucination map points `systemctl restart`
at `sv_restart_service`.

**Acceptance criteria**: `grep -in 's6\|/etc/s6/sv\|deploy/s6' crates/op-chat/src/system_prompt.rs
crates/op-web/src/orchestrator/anti_hallucination.rs` returns no matches. A
rendered system prompt contains "runit" and no "s6".

### FR-5: Orphaned `crates/op-dbus/` is deleted

The directory is removed. Nothing references it: it has no `Cargo.toml`, is not
a workspace member, and no `mod s6_systemctl` declaration exists anywhere.

**Acceptance criteria**: the path does not exist; `cargo check --workspace`
exits 0; `cargo metadata` lists the same member count minus zero (it was never
a member).

### FR-6: D-Bus interface is renamed, with a transitional alias

`org.opdbus.v1.S6.Systemctl` → `org.opdbus.v1.Runit.Systemctl`, and the object
path `/org/opdbus/v1/plugins/s6/systemctl` → `/org/opdbus/v1/plugins/runit/systemctl`.

The daemon requests the legacy name **in addition** for one release, because
the already-installed `/usr/local/bin/op-s6-systemctl` and `s6d` binaries (and
any out-of-tree caller) keep using the old name until the next
`recompile-and-update.sh` run. All in-repo callers move to the new name.

**Acceptance criteria**: `busctl --system list | grep Runit.Systemctl` shows the
new name after a restart; the legacy name is also owned; every in-repo reference
to `S6.Systemctl` is either the single deliberate alias registration or gone.

### FR-7: Crate and CLI are renamed, with a legacy alias

- crate `op-s6-systemctl` → `op-runit-systemctl`
- binary `s6d` → `svd`

`deploy/runit/recompile-and-update.sh` installs a legacy `s6d` alias alongside
`svd`, mirroring how it already installs `op-s6-recompile-and-update` beside
`op-runit-recompile-and-update`.

**Acceptance criteria**: `cargo check --workspace` exits 0 after the rename;
the root `Cargo.toml` members and workspace-dependency entries are updated;
`svd --help` works and `s6d` still resolves.

### FR-8: Deploy artifacts describe runit

- `deploy/s6/qdrant-grpc-loopback/` is ported to `deploy/runit/qdrant-grpc-loopback/`
  as a runit `run` script; `deploy/s6/` is then deleted.
- `deploy/agent-s6-guard.sh` → `deploy/agent-runit-guard.sh` and
  `deploy/99-agent-s6-guard.hook` → `deploy/99-agent-runit-guard.hook`: deny
  direct `runsv`/`runsvdir`/`runit-init` invocation by agents while permitting
  `sudo sv`, and keep denying `s6*` in case the packages are ever reinstalled.
- `deploy/agent-service6.sudoers` → `deploy/agent-sv.sudoers`, with comments
  describing the `sudo sv` surface.
- `deploy/PKGBUILD-sdbusplus-s6` and `deploy/build-sdbusplus-s6.sh` drop the
  `-s6` suffix (naming only; contents unchanged).

**Acceptance criteria**: `deploy/s6/` does not exist; the guard denies
`runsvdir` and allows `sv`; `ls deploy | grep -c s6` returns 0.

### FR-9: Docs describe runit

- `docs/operations/artix-s6-bootdb-recovery.md` is replaced by
  `docs/operations/artix-runit-recovery.md` covering the runit realities: no
  compiled database, `runsvdir` scanning, a stuck `supervise/` directory, and
  single-user recovery via `/etc/runit/runsvdir/single`.
- `docs/overview/architecture.md` and `docs/guides/user-guide.md` are corrected.

**Acceptance criteria**:
`grep -rniE '\bs6\b' docs/ --include=*.md --exclude-dir=book` returns no matches
except deliberate historical notes explicitly marked as such.

### FR-10: A regression guard prevents s6 from returning

A test in the workspace fails if any tracked Rust source under `crates/` spawns
an s6-era binary or hardcodes an s6 path. It runs under `cargo test`, so it
needs no CI infrastructure.

**Acceptance criteria**: the test passes on the migrated tree, and fails when a
`Command::new("s6-rc")` or `"/run/service"` literal is reintroduced.

### FR-11: A systemd compatibility wrapper converts install attempts

Third-party installers (vendor `install.sh`, pacman hooks, language package
postinstall steps) assume systemd and call `systemctl enable --now X`. On this
host those calls fail and leave a half-installed service. Three pieces close
that gap:

1. **`systemctl` shim** at `/usr/local/bin/systemctl` — maps systemd verbs onto
   `sv` (`start`, `stop`, `restart`, `reload`, `status`, `enable`, `disable`,
   `mask`, `unmask`, `is-active`, `is-enabled`, `list-units`, `cat`, `show`),
   treats `daemon-reload` as the no-op it is under runit, and refuses verbs with
   no runit meaning. `sudo`'s `secure_path` searches `/usr/local/bin` before
   `/usr/bin`, so the shim wins for installers that shell out. It prefers the
   `org.opdbus.v1.Runit.Systemctl` D-Bus service when reachable, keeping service
   changes on the audited control plane, and falls back to `sv` directly.
2. **`systemd-unit-to-runit`** at `/usr/local/sbin/` — translates one `.service`
   unit into `/etc/runit/sv/<name>/run` plus a `log/run` companion, mapping
   `ExecStart`, `ExecStartPre`, `User`/`Group` (via `chpst`), `Environment`,
   `EnvironmentFile` (honouring the optional `-` prefix), `WorkingDirectory`,
   `UMask`, and `After`/`Requires` (into `wait_dep` calls). It **reports** what
   runit cannot express rather than hiding it: `Type=forking` (would restart-loop),
   `Type=notify` (no sd_notify socket), `Type=dbus`, `Type=oneshot` (use
   `sv once`), and `Restart=no` (a `down` file is written). Template units and
   non-service unit types are refused with a clear message.
3. **Pacman hook** `99-systemd-unit-to-runit.hook` + `op-convert-systemd-units`
   — converts units that a package drops, **without enabling or starting them**:
   an install must not launch daemons, and enabling stays an operator decision.
   An existing hand-written `run` script is never overwritten; only scripts the
   converter itself generated are regenerated on upgrade.

All three install through `deploy/runit/recompile-and-update.sh`.

**Acceptance criteria**:
- `systemctl enable <unit>` on a host with only a systemd unit present generates
  `/etc/runit/sv/<name>/run` and the runlevel symlink.
- `systemctl daemon-reload` exits 0.
- `systemctl is-enabled` returns 0 when enabled, 1 when not; `is-active` returns
  3 when inactive — matching systemd's exit codes so installer logic still works.
- An unsupported verb exits 1; a `.socket`/`.timer` unit is skipped, not fatal.
- Converting a `Type=forking` unit prints the restart-loop warning.
- `systemd-unit-to-runit --dry-run` writes nothing.



- **NFR-1: No new dependencies.** Every change is a string, path, or process
  invocation swap plus file moves.
- **NFR-2: Privilege boundary preserved.** `sv` needs root. Code that shells out
  keeps its existing privilege assumptions; this spec does not add `sudo` calls
  inside daemons that already run as root, and does not grant new privileges.
- **NFR-3: No live service restarts during verification.** Acceptance criteria
  are satisfied by compilation, tests, and static checks. Restarting host
  services is an explicit operator step via
  `sudo deploy/runit/recompile-and-update.sh`.
- **NFR-4: Behaviour-preserving.** Service start/stop/restart/status semantics
  stay equivalent; only the mechanism changes. `sv restart` replacing
  stop-then-start is the one intentional semantic improvement (FR-3).
- **NFR-5: OSCAL subids** for new surfaces:
  - runit path constants — `src.service.runit.paths@v1`
  - `sv_*` tool family — `mut.service.runit.service-control@v1`
  - `org.opdbus.v1.Runit.Systemctl` — `mut.service.runit.systemctl@v1`
  - regression guard test — `obs.software.runit.s6-regression-guard@v1`

---

## Non-Goals

- **Container init.** `deploy/incus/` contains no init-system references;
  container and application deployment goes through D-Bus/`busctl` per
  `AGENTS.md`. Not touched.
- **Removing systemd vocabulary.** The `systemctl`-to-runit *mapping* is the
  product; `Systemctl` stays in the interface name and the `systemctl …` keys in
  the anti-hallucination map stay as the left-hand side.
- **`.consolidation-staging/**`, `.claude/worktrees/**`, `docs/book/**`,
  `target/**`, `.repomix*/`, `md-xml/`** — archives, other agents' worktrees,
  and generated output.
- **The `accountability-audit-trail` spec's code** — unrelated and already
  landed.
- **Rewriting `crates/op-plugins/src/state_plugins/service.rs`** — already
  correct; only its shared path constants change (FR-2).
- **Any change to the Xray configuration policy** in `AGENTS.md`.
