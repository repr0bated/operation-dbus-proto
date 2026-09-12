# Technical Specification: Crate Implementation Comparison Reports

## Overview

This spec describes how to generate `compare-<crate-name>.md` files for each crate in `crates/op-*` by comparing the crate's spec/design documents against its actual source code.

---

## Technical Context

- **Language**: Analysis is performed by an agent (no new Rust or TypeScript code is produced)
- **Input sources**: SPEC.md, DESIGN.md, and other markdown docs per crate; `src/` directory tree; `Cargo.toml`
- **Output**: One `compare-<crate-name>.md` markdown file per crate, written directly into the crate folder
- **Crate root**: `crates/` (e.g. `crates/op-core/`, `crates/op-chat/`)
- **No existing code is modified** — only new report files are created

---

## SPEC.md File Taxonomy

The SPEC.md files in this repo fall into two categories:

### Auto-Generated SPECs (majority of crates)

These follow a consistent machine-generated format with named sections:

```
## Source Structure
# lists exact file paths like: op-core/src/types.rs

## Main Modules
# bare module names: types, error, security, ...

## Key Dependencies
# Cargo.toml dependency block verbatim

## Binaries
# [[bin]] entries verbatim or "# No binaries"
```

Crates with auto-generated SPEC: `op-agents`, `op-snowball`, `op-cache`, `op-cognitive-mcp`, `op-core`, `op-dbus-mirror`, `op-dynamic-loader`, `op-execution-tracker`, `op-gateway`, `op-grpc-bridge`, `op-http`, `op-identity`, `op-inspector`, `op-introspection`, `op-llm`, `op-mcp`, `op-mcp-aggregator`, `op-mcp-proxy`, `op-network`, `op-plugins`, `op-services`, `op-state`, `op-state-store`, `op-tools`, `op-web`, `op-workflows`

### Hand-Written / Rich SPECs (feature-heavy crates)

These use human-authored section headings that map to architectural features, not just files:

- `op-chat`: SPEC.md + DESIGN.md — 2000+ lines describing Anti-Hallucination System, gRPC Orchestration, Session Management, Workstack, Skills, MCP Server, etc.
- `op-ml`: SPEC.md — detailed ML pipeline spec
- `op-jsonrpc`: SPEC.md — JSON-RPC protocol spec with type definitions
- `op-dbus-model`: SPEC.md — D-Bus interface and type system spec
- `op-deployment`: SPEC.md — deployment lifecycle and state machine spec

---

## Data Extraction Approach

### 1. Spec-Described Modules

For **auto-generated SPECs**:
- Parse "Source Structure" section: collect all file paths listed (e.g. `op-core/src/types.rs`)
- Parse "Main Modules" section: collect bare module names (e.g. `types`, `error`)
- Parse "Binaries" section: collect binary names

For **rich SPECs**:
- Use `##` and `###` section headings as the unit of feature/capability description
- Identify explicit module/file references (e.g. code blocks or paths like `src/foo.rs`)

### 2. Actual Implementation (Source)

For every crate, run a file-system scan:

```
find crates/<crate-name>/src -name "*.rs"
```

Additionally:
- Read `lib.rs` or `main.rs` to collect `pub mod`, `mod`, and `pub use` declarations
- Check `proto/` directory for `.proto` files (gRPC crates)
- For `op-web`, also check `ui/src/` for frontend source files

### 3. Dependency Comparison

- **Spec side**: parse the "Key Dependencies" code block in SPEC.md — extract crate names (left side of `=` in `toml`, strip whitespace and version suffix)
- **Actual side**: read `Cargo.toml` `[dependencies]` and `[dev-dependencies]` sections — extract crate names

### 4. Feature/Capability Status Heuristic

To determine implementation status of a feature (from a rich SPEC):

| Status | Criterion |
|---|---|
| **Implemented** | A `.rs` file exists whose name or content clearly corresponds to the feature; or the feature's primary type/trait is declared in any source file |
| **Partial** | A source file exists for the feature but with stub content (e.g. `todo!()`, empty structs, or a `.stub` extension variant present) |
| **Missing** | No source file or type definition matches the feature name; or only a `.rs.patch`/`.rs.stub`/`.rs.copied` file exists without the real `.rs` |

Special files to note: `.rs.patch`, `.rs.stub`, `.rs.copied` files alongside a `.rs` file indicate the module exists but is in transition — mark as **Partial**.

---

## Report Structure (per crate)

Each `compare-<crate-name>.md` follows this exact structure:

```markdown
# compare-<crate-name>

**Date**: YYYY-MM-DD  
**Spec files analyzed**: SPEC.md [, DESIGN.md, ...]

---

## Summary

| Category | Count |
|---|---|
| Spec-described modules | N |
| Implemented modules | N |
| Missing modules | N |
| Extra (undocumented) modules | N |
| Overall completion | X% |

---

## Module / File Comparison

| Module | Status | Spec Description | Implementation Notes |
|---|---|---|---|
| `module_name` | ✅ Implemented / ⚠️ Partial / ❌ Missing | ... | `src/module_name.rs` — key types found / not found |

---

## Feature / Capability Comparison

| Feature | Status | Notes |
|---|---|---|
| Feature Name | ✅ Implemented / ⚠️ Partial / ❌ Missing | Brief evidence or absence note |

---

## Dependencies Comparison

### In Spec but Missing from Cargo.toml
- `crate_name` — referenced in spec but not present in Cargo.toml

### In Cargo.toml but Not in Spec
- `crate_name` — added since spec was written or undocumented

---

## Notes and Observations

- High-risk gaps or security-critical missing items
- Stale spec sections
- Any unusual file patterns (stub files, patches, copied sources)
```

---

## Status Indicators

| Indicator | Meaning |
|---|---|
| ✅ Implemented | Source file exists and is referenced in lib.rs/main.rs |
| ⚠️ Partial | File exists but has stub/patch variants, or module undeclared |
| ❌ Missing | No corresponding source file found |

---

## Crate Coverage

All 31 crates listed in `requirements.md` receive a report. Crates with supplementary docs (DESIGN.md, SECURITY-MODEL.md, ADAPTER-WORKFLOW.md, CLEANUP-CONTEXT-AWARE.md, README.md, docs/ARCHITECTURE.md) — these are included as additional spec sources in the header but the primary comparison basis remains SPEC.md.

### Special Cases

- **`op-web`**: Also check `ui/src/` for Next.js/Vite frontend source files
- **`op-chat`**: DESIGN.md is 80KB — use its `##` section headings as feature list
- **`op-mcp`**: `docs/ARCHITECTURE.md` supplements SPEC.md for feature comparison
- **`op-inspector`**: `ADAPTER-WORKFLOW.md` describes the adapter workflow feature
- **`op-gateway`**: `SECURITY-MODEL.md` describes security features to verify
- Crates with `proto/` directories: verify `.proto` file presence as a proxy for "gRPC protocol implemented"

---

## Output File Location

Each report is saved at:

```
crates/<crate-name>/compare-<crate-name>.md
```

Example: `crates/op-core/compare-op-core.md`

No existing files (SPEC.md, DESIGN.md, Cargo.toml, source .rs files) are modified.

---

## Verification

Since this is a documentation analysis task (no compilable code produced), verification consists of:

1. Confirm all 31 `compare-*.md` files exist in their respective crate directories
2. Confirm each report contains all required sections (Summary, Module Comparison, Feature Comparison, Dependencies Comparison, Notes)
3. Confirm no spec or source files were modified (`git diff --name-only` should show only new `compare-*.md` files)
