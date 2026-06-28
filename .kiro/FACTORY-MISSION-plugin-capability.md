# Factory Mission: Implement the Plugin Capability Source-of-Truth Spec (TDD-orchestrated)

## Mission
Implement the spec at `.kiro/specs/plugin-capability-source-of-truth/`
(`requirements.md` = 13 requirements + 5 NFRs, `design.md`, `tasks.md` = 57
tasks). This consolidates the plugin pipeline so that **the plugin is the sole
source of truth for every object's existence**, defines a **capability as the
complete enumerable method/function/property/signal surface of an object**, and
makes the gRPC bridge the single owner/registrar/projected-tree fed by
`Producer → SHM → Bridge`.

`tasks.md` is the authoritative work breakdown and is already ordered so the
workspace builds at each step. Every task carries `_Requirements: X.Y_` — honor
that traceability.

## Method: TDD-orchestrated
Use the enabled skills: `tdd-orchestrator`, `test-automator`,
`spec-to-code-compliance`, `rust-pro`, `rust-async-patterns`,
`systems-programming-rust-project`, `orchestrate-batch-refactor`.

For each requirement group:
1. **RED** — write tests directly from that requirement's EARS acceptance
   criteria (WHEN/WHILE/IF/WHERE … SHALL …). One test per criterion where
   feasible. Tests must fail first.
2. **GREEN** — implement the minimum to pass.
3. **REFACTOR** — clean up; keep the workspace compiling.
4. **VERIFY** — `spec-to-code-compliance`: confirm every acceptance criterion in
   the group is satisfied before moving on.

## Orchestration (workstreams)
Decompose `tasks.md` into these dependency-ordered workstreams; parallelize
within a workstream, gate between them on a green build:

- **WS1 — Capability model & dedup** (R1, R2): plugin trait declares the full
  capability surface (methods/functions w/ arg+return schema, side-effect class,
  required-capability, properties, signals, guarantees); collapse the duplicate
  `PluginCapabilities` (op-state vs op-plugins) to one.
- **WS2 — Producer → SHM** (R3, R9, R10, R11): op-projection emits the complete
  capability schema + present-state into SHM (per-plugin files + monolith +
  atomic manifest with the single catalog_hash); subid taxonomy on every
  capability; serialization completeness.
- **WS3 — Bridge as sole owner/registrar/projected-tree** (R4, R5, R6): bridge
  owns the plugins bus name, registers the tree from SHM, real dispatch through
  `SchemaEngine.mutate` (NO stub, `json_args` used), method validation against
  the schema capability surface.
- **WS4 — Capability enforcement** (R7): enforce `capability_id` against the
  caller identity (footprint/sessionid from GhostbridgeInterceptor) at the
  bridge — the single enforcement point.
- **WS5 — Trim redundant registrars & dead claims** (R8): op-dbus-mirror drops
  plugin-object registration + its `org.opdbus.v1` name claim (keeps ovsdb/
  nonnet/mirror-mgmt); op-projection drops D-Bus object serving (producer-only);
  delete the dead op-state name claim, op-openvswitch bare-name claim, and the
  orphan `opdbus` service/binary.
- **WS6 — Autogeneration + Gemma** (R12, R13): the missing-plugin lifecycle
  (research → synthesize full capability surface → review → persist as the plugin
  → project → serve; with quarantine until approved), and route object-property/
  capability research through **Gemma** (replace the
  `create_agent("search-specialist")` seam in
  `auto_create.rs::query_elements_via_agent`). Gemma is a plugin/StatePlugin.

## Hard rules (do not violate — these are why prior attempts failed)
- **NO stubs, NO placeholders, NO "for now"/"in a full implementation" comments.**
  A registered object that returns fake success or validates against an empty set
  is a defect. The recurring failure mode here is inert code that *looks* wired.
- **One schema, one source, computed in exactly ONE place.** Never recompute the
  catalog_hash in a consumer; consumers read the manifest hash.
- **Only valid path is `org.opdbus.v1.plugins`.** No new
  `operation.<domain>.v1.*Service` protos; no raw ip:port.
- **SHM is authoritative present-state; READ it. NO polling loops, NO watchers.**
  Action is triggered by arrival, not a timer.
- **Durability is the mutation chain.** No SQL (cozo→rocksdb); no btrfs-snapshot
  backups; no parallel persistence.
- **Zero-trust:** identity footprint/sessionid is the gate; capability
  enforcement augments it. No container NIC/IP; container I/O over UDS.

## Live-system safety (critical)
Running services exist: op-projection (owns `org.opdbus.v1.plugins`, pid ~2194,
also the schema producer), op-dbus-mirror, op-grpc-bridge. **Make code + test
changes only. Do NOT restart, redeploy, `s6` commit, or kill any running
service.** The bus-ownership cutover (WS5) is a deliberate, separate deploy step
the human will run via `deploy/s6/recompile-and-update.sh`. Implement the code so
the cutover is possible; do not perform it.

## Definition of done
- `cargo build --workspace --release` clean — **zero errors, zero warnings.**
- All tests written from the acceptance criteria pass (`cargo test --workspace`).
- Every requirement verified by `spec-to-code-compliance`; check off the
  corresponding boxes in `tasks.md`.
- No constructed-but-unused modules; no stub handlers; no second identity
  mechanism; both schema access paths (per-plugin file + combined monolith) work.
- A short `IMPLEMENTATION-NOTES.md` summarizing what changed per workstream and
  any deviations from the spec (with justification).

## Working mode
- Work in a git worktree/branch off the current branch; do not commit to a shared
  branch without the human's review.
- Commit per workstream with messages citing the requirements (e.g.
  `feat(bridge): sole owner + real dispatch [R4,R5,R6]`).
