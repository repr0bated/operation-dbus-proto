# Full SDD workflow

## Configuration
- **Artifacts Path**: {@artifacts_path} → `.zenflow/tasks/{task_id}`

---

## Agent Instructions

---

## Workflow Steps

### [x] Step: Requirements
<!-- chat-id: 6da49e1a-c9f2-4ad7-b599-a2b2dc1a9234 -->

Create a Product Requirements Document (PRD) based on the feature description.

1. Review existing codebase to understand current architecture and patterns
2. Analyze the feature definition and identify unclear aspects
3. Ask the user for clarifications on aspects that significantly impact scope or user experience
4. Make reasonable decisions for minor details based on context and conventions
5. If user can't clarify, make a decision, state the assumption, and continue

Focus on **what** the feature should do and **why**, not **how** it should be built. Do not include technical implementation details, technology choices, or code-level decisions — those belong in the Technical Specification.

Save the PRD to `{@artifacts_path}/requirements.md`.

### [x] Step: Technical Specification
<!-- chat-id: 89f49b72-b153-411d-a0f8-478166aaee1c -->

Create a technical specification based on the PRD in `{@artifacts_path}/requirements.md`.

1. Review existing codebase architecture and identify reusable components
2. Define the implementation approach

Do not include implementation steps, phases, or task breakdowns — those belong in the Planning step.

Save to `{@artifacts_path}/spec.md` with:
- Technical context (language, dependencies)
- Implementation approach referencing existing code patterns
- Source code structure changes
- Data model / API / interface changes
- Verification approach using project lint/test commands

### [x] Step: Planning
<!-- chat-id: 087c081e-ae11-4d42-a9a3-59428841ea88 -->

Explored all 31 crates. Key findings:
- **Auto-generated SPEC format** (26 crates): sections are `## Quick Reference` → `### Source Structure` (file paths) + `### Key Dependencies` (toml block); and `## Module Structure` → `### Main Modules` (bare names). Binaries listed under `### Binaries`.
- **Rich/hand-written SPEC format** (5 crates): `op-chat`, `op-dbus-model`, `op-deployment`, `op-jsonrpc`, `op-ml` — use `##`/`###` section headings as feature/capability units.
- **Stub/patch files present** in: `op-core` (`lib.rs.patch`), `op-dbus-mirror` (`lib.rs.orig`), `op-chat` (`tool_loader.rs.stub`), `op-mcp` (`agents_server.rs.patch`, `mod.rs.patch`), `op-mcp-aggregator` (1 stub), `op-tools` (1 stub), `op-web` (5 `.fix`/`.patch` files).
- **Proto files**: `op-grpc-bridge` (5), `op-chat` (2), `op-mcp` (2), `op-cache` (1), `op-services` (1).
- **Frontend source**: `op-web/ui/src/` exists with `.tsx` files.
- `op-chat` DESIGN.md is 2740 lines; SPEC.md is 2297 lines — largest and most complex.

### [x] Step: Generate reports — Batch 1
<!-- chat-id: df06f52f-9413-4cb2-9ade-a148d169ea80 -->

Write `compare-<crate>.md` for 8 auto-generated SPEC crates.

**Crates**: `op-agents`, `op-snowball`, `op-cache`, `op-cognitive-mcp`, `op-core`, `op-dbus-mirror`, `op-dynamic-loader`, `op-execution-tracker`

**Per-crate process**:
1. Read `crates/<crate>/SPEC.md` — extract from `### Source Structure` the listed `.rs` file paths; extract from `### Main Modules` the bare module names; extract `### Key Dependencies` toml block for spec-deps; check `### Binaries` for binary entries.
2. `find crates/<crate>/src -name "*.rs"` — list actual source files; also note any `.rs.patch`, `.rs.stub`, `.rs.orig` alongside real `.rs` files (mark those modules ⚠️ Partial).
3. Read `crates/<crate>/src/lib.rs` (or `main.rs`) — collect `pub mod` / `mod` declarations.
4. Read `crates/<crate>/Cargo.toml` `[dependencies]` section — extract actual dep names.
5. Write `crates/<crate>/compare-<crate>.md` with all 5 required sections (Summary table, Module/File Comparison, Feature/Capability Comparison, Dependencies Comparison, Notes).

**Special notes for this batch**:
- `op-core`: `lib.rs.patch` exists alongside `lib.rs` → mark `lib` module as ⚠️ Partial.
- `op-dbus-mirror`: `lib.rs.orig` present → note in observations.
- `op-cache`: has a `proto/` subdirectory with 1 `.proto` file → note in Feature Comparison.

**Output**: 8 files — `crates/op-agents/compare-op-agents.md` … `crates/op-execution-tracker/compare-op-execution-tracker.md`

### [x] Step: Generate reports — Batch 2
<!-- chat-id: a7b350c2-943e-4bbd-a14d-7c65c20be68e -->

Write `compare-<crate>.md` for 8 auto-generated SPEC crates.

**Crates**: `op-gateway`, `op-grpc-bridge`, `op-http`, `op-identity`, `op-inspector`, `op-introspection`, `op-llm`, `op-mcp`

**Per-crate process**: same as Batch 1.

**Special notes**:
- `op-gateway`: also read `SECURITY-MODEL.md` — include its `##` headings as additional features in the Feature Comparison section.
- `op-grpc-bridge`: check `proto/` directory for 5 `.proto` files → each proto is a feature unit; include in Feature Comparison.
- `op-inspector`: also read `ADAPTER-WORKFLOW.md` — include its `##` headings as additional features.
- `op-mcp`: also read `README.md` and `docs/ARCHITECTURE.md`; `agents_server.rs.patch` and `mod.rs.patch` exist → mark `agents_server` and `mod` as ⚠️ Partial.

**Output**: 8 files — `crates/op-gateway/compare-op-gateway.md` … `crates/op-mcp/compare-op-mcp.md`

### [x] Step: Generate reports — Batch 3
<!-- chat-id: 55cfa1ed-db90-480f-86c0-0e7a9848a58f -->

Write `compare-<crate>.md` for 8 auto-generated SPEC crates.

**Crates**: `op-mcp-aggregator`, `op-mcp-proxy`, `op-network`, `op-plugins`, `op-services`, `op-state`, `op-state-store`, `op-tools`

**Per-crate process**: same as Batch 1.

**Special notes**:
- `op-mcp-aggregator`: also read `README.md` and `CLEANUP-CONTEXT-AWARE.md`; 1 stub file present → mark affected module ⚠️ Partial.
- `op-mcp-proxy`: no `lib.rs`, only `main.rs` → module declarations come from `main.rs`.
- `op-services`: has 1 `.proto` file in `proto/` → note in Feature Comparison.
- `op-tools`: 1 stub file present → mark affected module ⚠️ Partial.

**Output**: 8 files — `crates/op-mcp-aggregator/compare-op-mcp-aggregator.md` … `crates/op-tools/compare-op-tools.md`

### [x] Step: Generate reports — Batch 4 (auto-gen + rich SPEC)
<!-- chat-id: 419fb8fd-e2c4-4028-8049-ba7a10c00640 -->

Write `compare-<crate>.md` for 6 crates: 2 auto-generated + 4 rich SPEC.

**Crates**: `op-web`, `op-workflows`, `op-dbus-model`, `op-deployment`, `op-jsonrpc`, `op-ml`

**`op-web` and `op-workflows`** — auto-generated SPEC format (same as Batch 1).
- `op-web` additionally: scan `ui/src/` for `.tsx`/`.ts` files; list them in the Module Comparison as frontend modules; note the 5 `.fix`/`.patch` files (`chat_handler.rs.fix`, `main.rs.patch`, `orchestrator/mod.rs.patch`, `orchestrator/process.rs.patch`, `routes/mod.rs.patch`, `routes.rs.patch`) — mark those modules ⚠️ Partial.

**`op-dbus-model`, `op-deployment`, `op-jsonrpc`, `op-ml`** — rich SPEC format:
- Use `##` and `###` section headings from SPEC.md as the feature/capability units.
- For each feature heading, look for a corresponding `.rs` file or type definition in `src/`.
- If a source file name or type name clearly matches the section heading → ✅ Implemented; if a file exists but has a stub variant → ⚠️ Partial; if no match → ❌ Missing.
- In the Module/File Comparison table, use any explicit file paths mentioned in the spec text.

**Output**: 6 files — `crates/op-web/compare-op-web.md` … `crates/op-ml/compare-op-ml.md`

### [x] Step: Generate report — op-chat
<!-- chat-id: 4d16bd8a-2639-4468-ac6c-7e2182427943 -->

Write `crates/op-chat/compare-op-chat.md`. This crate has the largest spec (SPEC.md 2297 lines + DESIGN.md 2740 lines) and warrants its own step.

**Process**:
1. Read `crates/op-chat/SPEC.md` — extract `### Source Structure` file list, `### Main Modules`, `### Key Dependencies`; also collect all `##`-level SPEC sections as high-level features.
2. Read `crates/op-chat/DESIGN.md` — collect all `##`-level section headings as the feature/capability list (e.g. "Anti-Hallucination System", "Forced Tool Execution Architecture", "Agent Orchestration", "Workstack System", "Protocol Design", "Session Management", "Natural Language Administration", "Security Model", etc.).
3. `find crates/op-chat/src -name "*.rs"` — actual source files (already known: 31 `.rs` files). Note `tool_loader.rs.stub` alongside `tool_loader.rs` → mark `tool_loader` as ⚠️ Partial.
4. Check `proto/` for 2 `.proto` files → gRPC protocol implemented.
5. Read `crates/op-chat/Cargo.toml` for dep comparison.
6. For each DESIGN.md `##` feature, determine status by matching to source files:
   - Anti-Hallucination System → `forced_execution.rs`, `forced_tool_pipeline.rs` → ✅
   - Agent Orchestration → `orchestration/` directory (6 files) → ✅
   - Workstack → `orchestration/workstacks.rs`, `orchestration/workstack_executor.rs` → ✅
   - Session Management → `session.rs` → ✅
   - MCP Server → `mcp_server.rs` → ✅
   - Natural Language Admin → `nl_admin.rs` → ✅
   - Tool Execution → `tool_executor.rs`, `tool_orchestrator.rs`, `agent_tools.rs` → ✅
   - Skills → `orchestration/skills.rs` → ✅
   - … and so on for all headings.
7. Write the report with all 5 required sections.

**Output**: `crates/op-chat/compare-op-chat.md`

### [x] Step: Verification
<!-- chat-id: f1f80598-935d-48b7-8b4f-f98706e86275 -->

Confirm all 31 reports exist and are well-formed.

1. Run `ls crates/op-*/compare-*.md | wc -l` — must equal 31.
2. Run `git diff --name-only` — only new `compare-*.md` files; no SPEC.md, Cargo.toml, or `.rs` source files modified.
3. Spot-check 3 reports across different types (one auto-gen, one rich SPEC, one special) to confirm all 5 required sections are present.

### [x] Step: Rust schema repair investigation

Audit the focused Rust crates for remaining compile errors, stale schema-authority behavior, API mismatches, and formatting blockers.

1. Inspect the listed files and related callers using repository search.
2. Run targeted `cargo check -p <crate>` commands before editing where practical.
3. Identify concrete fixes that preserve the plugin as the single source of schema truth.

### [x] Step: Rust schema repair implementation

Implement targeted fixes in the focused files without changing the intended architecture.

1. Route schema resolution through shared registry paths where required.
2. Remove stale `SyncEngine` naming if encountered in the touched scope.
3. Fix high-signal code issues, warnings, and test breakages in touched files when practical.

### [x] Step: Rust targeted verification

Run targeted checks and tests for the modified crates.

1. Use `cargo check -p <crate>` for affected crates.
2. Run real targeted tests with valid names or looser filters so tests execute.
3. Record unrelated blockers without reverting user changes.

Executed targeted checks after installing the missing Rust toolchain and `protobuf-compiler`. Verified `op-state-store`, `op-grpc-bridge`, `op-state`, `op-cache`, and `op-network` with `cargo check -p ...`.

### [x] Step: Rust final verification and handoff

Finish by updating this plan and summarizing exact commands, results, and remaining blockers.

### [x] Step: Analyze refactored boot process

Review the refactored Rust code as the source of truth for early boot networking.

1. Ignore legacy deployment config files during the analysis.
2. Trace privacy fabric ownership across `src/main.rs`, `op-web`, `op-plugins`, and `op-grpc-bridge`.
3. Produce a written report covering `dinit`, `wg-quick`, `systemd-networkd`, OVS/OVSDB, Incus, DNS, and OpenClaw responsibilities.

Report saved to `.zenflow/tasks/new-task-7351/boot-process-report.md`.

### [x] Step: Implement refactored privacy boot fixes

Align the refactored bootstrap path with the required host/network ordering.

1. Ensure `wg-quick` is part of the early bootstrap path before attaching `wgcf` to `ovsbr0`.
2. Keep `ens3` standalone by default instead of auto-enslaving it to the bridge.
3. Ensure `grpc-bridge` is present on `ovsbr0` and brought up during host topology bootstrap.
4. Restore automatic `privacy_router` bootstrap inside `op-dbus`.

Report refresh completed on 2026-04-05:
- Re-scanned all `crates/op-*` directories, their `SPEC.md`/`DESIGN.md` files, and supplemental docs referenced by the task (`SECURITY-MODEL.md`, `ADAPTER-WORKFLOW.md`, `README.md`, `docs/ARCHITECTURE.md`, `CLEANUP-CONTEXT-AWARE.md` where present).
- Regenerated all 31 `compare-<crate>.md` reports under `crates/` to reflect the current source tree, explicit spec source references, proto files, partial patch/stub artifacts, and Cargo dependency deltas.
- Verified the report count is 31 and spot-checked representative reports (`op-core`, `op-dbus-model`, `op-chat`) for all required sections.
- Verification note: the repository already contains many unrelated tracked changes, so `git diff --name-only` does not isolate this task cleanly.

Detailed current-state spec expansion completed on 2026-04-05:
- Reworked the per-crate report format into a fuller current-system-state specification, retaining comparison context but making the implemented system the primary focus.
- Added per-crate current-state summaries, implementation overviews, grouped source inventories, capability tables, dependency surfaces, and operational notes.
- Filtered out vendored `node_modules` content from UI scans so `op-web` reflects repository-owned implementation rather than third-party package files.
- Re-verified representative detailed reports for `op-core`, `op-chat`, `op-mcp`, and `op-web`, and confirmed all 31 reports still exist.
