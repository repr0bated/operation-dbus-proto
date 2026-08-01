# PRD: Crate Implementation Comparison Reports

## Overview

Generate implementation comparison reports for each crate in the `crates/crates/` directory by analyzing their specification/design documents against their actual source code. Each report is saved as `compare-<crate-name>.md` inside the respective crate folder.

---

## Goals

- Provide a clear, per-crate status of how well the implementation matches the design intent captured in SPEC.md and DESIGN.md files.
- Identify gaps: modules, features, or capabilities described in specs that are missing or incomplete in the source.
- Surface implementation overage: code that exists but is not described in any spec (undocumented implementation).
- Enable the team to prioritize completion work and update stale specs.

---

## Scope

### Crates to Analyze

All subdirectories of `crates/crates/` that contain at least one spec or design markdown file:

| Crate | Spec Files |
|---|---|
| op-agents | SPEC.md |
| op-blockchain | SPEC.md |
| op-cache | SPEC.md |
| op-chat | SPEC.md, DESIGN.md |
| op-cognitive-mcp | SPEC.md |
| op-core | SPEC.md |
| op-dbus-mirror | SPEC.md |
| op-dbus-model | SPEC.md |
| op-deployment | SPEC.md |
| op-dynamic-loader | SPEC.md |
| op-execution-tracker | SPEC.md |
| op-gateway | SPEC.md, SECURITY-MODEL.md |
| op-grpc-bridge | SPEC.md |
| op-http | SPEC.md |
| op-identity | SPEC.md |
| op-inspector | SPEC.md, ADAPTER-WORKFLOW.md |
| op-introspection | SPEC.md |
| op-jsonrpc | SPEC.md |
| op-llm | SPEC.md |
| op-mcp | SPEC.md, docs/ARCHITECTURE.md |
| op-mcp-aggregator | SPEC.md, README.md, CLEANUP-CONTEXT-AWARE.md |
| op-mcp-proxy | SPEC.md |
| op-ml | SPEC.md |
| op-network | SPEC.md |
| op-plugins | SPEC.md |
| op-services | SPEC.md |
| op-state | SPEC.md |
| op-state-store | SPEC.md |
| op-tools | SPEC.md |
| op-web | SPEC.md |
| op-workflows | SPEC.md |

---

## What "Implemented" Means

For Rust crates, the implementation is the content of the `src/` directory. The following signals indicate implementation status:

- **Source file exists**: A `.rs` file corresponding to a module name from the spec
- **Module is declared**: The module is declared and `pub use`-d in `lib.rs` or `main.rs`
- **Key types/traits exist**: Structs, enums, traits, or functions named in the spec are present in source
- **Proto files exist**: For gRPC crates, `.proto` definitions in `proto/` indicate protocol design is implemented
- **Binaries exist**: Entries in `src/bin/` matching spec-described binaries

Non-Rust items (e.g., `op-web` with a Next.js/Vite UI) should also check for frontend source files in `ui/src/`.

---

## Report Format

Each `compare-<crate-name>.md` report must contain:

### 1. Header
- Crate name, report date, spec files analyzed

### 2. Summary Table
A quick status overview:
```
| Category | Count |
|---|---|
| Spec-described modules | N |
| Implemented modules | N |
| Missing modules | N |
| Extra (undocumented) modules | N |
| Overall completion | X% |
```

### 3. Module/File Comparison
For each module or component described in the spec:
- **Status**: Implemented / Partial / Missing
- **Spec description**: What the spec says it should do
- **Implementation notes**: What was found in source (file path, key types)

### 4. Feature/Capability Comparison
High-level features described in SPEC.md or DESIGN.md sections (e.g., "Anti-Hallucination System", "Session Management", "gRPC Agent Orchestration") with implementation status:
- **Implemented**: Clear evidence exists in source
- **Partial**: Some code exists but feature appears incomplete
- **Missing**: No corresponding implementation found

### 5. Dependencies Comparison
Dependencies listed in the spec vs what's in `Cargo.toml`:
- Extra dependencies (in Cargo.toml but not in spec — may be additions since spec was written)
- Missing dependencies (in spec but not in Cargo.toml)

### 6. Notes and Observations
- Anything noteworthy about the gap analysis
- Stale spec sections (if source has clearly moved beyond the spec)
- High-risk gaps (missing security, missing core types, etc.)

---

## Non-Goals

- Do not judge code quality or correctness — only presence vs absence.
- Do not rewrite or update the SPEC.md or DESIGN.md files.
- Do not analyze test coverage or test completeness beyond noting if tests exist.
- Do not analyze runtime behavior or correctness of implementations.

---

## Assumptions

1. SPEC.md files are the primary source of truth for what each crate is supposed to contain. DESIGN.md and other docs supplement the spec.
2. A module is considered "implemented" if its corresponding `.rs` file exists and is declared in `lib.rs` — even if internally incomplete.
3. The SPECS/ global directory contains higher-level cross-crate specs and should not generate individual crate reports, but may be referenced for context.
4. The report should reflect the state at the time of analysis — no assumptions about future implementation.

---

## Acceptance Criteria

- A `compare-<crate-name>.md` file exists in every crate folder that has a SPEC.md or DESIGN.md.
- Every report includes the summary table, module comparison, feature comparison, and dependency comparison sections.
- Reports are human-readable markdown with clear pass/fail/partial indicators.
- No existing source files or spec files are modified.
