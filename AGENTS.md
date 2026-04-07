# AGENTS.md for operation-dbus-proto

This file provides essential guidelines for agentic coding agents working in this repository.
Follow these strictly to maintain consistency, pass CI, and adhere to team standards.

## Project Structure & Module Organization

- `src/`: Root `op-dbus` binary and legacy glue modules.
- `crates/op-*`: Active Rust workspace crates:
  - `op-core`: Core logic, shared utilities.
  - `op-plugins`: Plugin system.
  - `op-state-store`: State management (SQLite, etc.).
  - `op-workflows`: Workflow engine.
  - `op-web`: Web server and API.
  - `op-mcp`: MCP integration.
- Frontend:
  - Shared Vite app: `crates/src` (components, hooks).
  - Production UI: `crates/op-web/ui` (Next.js/Vite build).
- `deploy/`: Installation/upgrade scripts for Chimera Linux.
- `schemas/`: JSON schemas for config/state.
- `examples/`: Usage examples.
- `docs/`: Reference docs.
- `openclaw-indexer/`: Python utilities for indexing.

## Engineering Principles

- Prefer D-Bus-native APIs for platform integrations.
- Use gRPC for internal service-to-service RPC.
- High-performance JSON serialization/deserialization (serde_json).
- Avoid new serialization formats without justification.
- Control-plane operations: deterministic, schema-driven.
- Observability: Structured logging with tracing spans.
- Security: Least privilege, validate all inputs.

## Build, Lint, and Test Commands

**Rust Workspace (root Cargo.toml defines members)**

- Build all: `cargo build --workspace`
- Build release: `cargo build --workspace --release`
- Single crate: `cargo build -p op-core`
- Check (no build): `cargo check --workspace`

**Lint & Format (always run before commit/PR)**
- Format: `cargo fmt --all`
- Check format: `cargo fmt --all -- --check`
- Clippy lint: `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- Check Clippy: `cargo clippy --workspace --all-targets --all-features -- -D warnings -- -D clippy::all`

**Tests**
- All unit/integration: `cargo test --workspace --all-targets --all-features`
- Single crate all tests: `cargo test -p op-core`
- Single test function: `cargo test -p op-core test_module::test_function_name`
- Single test binary: `cargo test -p op-core --bin op-core`
- Lib only: `cargo test -p op-core --lib`
- With coverage (if cargo-llvm-cov installed): `cargo llvm-cov --workspace`
- Test with exact filter: `cargo test test_name -- --exact --nocapture`

**Frontend (Vite + Vitest)**
- Install deps: `cd crates && npm ci`
- Build UI prod: `cd crates/op-web/ui && npm ci && npm run build:prod`
- Dev server: `cd crates/op-web/ui && npm run dev`
- Tests all: `cd crates && npm test` (Vitest)
- Single test file: `cd crates && npx vitest tests/component.test.tsx`
- Single test: `cd crates && npm test -- --testNamePattern 'renders correctly'`
- Coverage: `cd crates && npm test -- --coverage`
- Lint: `cd crates && npm run lint` (ESLint)
- Typecheck: `cd crates && npm run typecheck`

**Full CI-like verification:**
```
cargo fmt --all -- --check &&
cargo clippy --workspace --all-targets --all-features -- -D warnings &&
cargo test --workspace --all-targets --all-features &&
cargo build -p op-web --release &&
cd crates/op-web/ui && npm ci && npm run build:prod && npm test -- --coverage &&
cd crates && npm test
```

**Chimera Linux deps:** `doas apk add rust cargo nodejs npm pkgconfig openssl-dev`

## Coding Style & Naming Conventions

### Rust (edition 2021, rustfmt nightly if specified)

- **Formatting:** `rustfmt` (4-space indent, max-width 100). Run `cargo fmt --all`.
- **Naming:**
  - Modules/files: `snake_case`
  - Functions/methods: `snake_case`
  - Structs/enums/traits: `PascalCase`
  - Constants: `SCREAMING_SNAKE_CASE`
  - Variables: `snake_case`, descriptive.
- **Imports:**
  - Grouped: `std::`, `alloc::`, extern crates (alphabetical), `crate::`, local modules.
  - No `use crate::*;` except prelude.
  - Prefer specific imports: `use serde_json::Value;`
- **Types:**
  - Prefer `&str` over `String` where possible.
  - Use `anyhow::Result<T>` for main error types, `thiserror` for custom enums.
  - Generics: explicit where clauses over turbofish.
- **Error Handling:**
  - `?` operator everywhere possible.
  - Context with `anyhow::Context`: `fs::read_to_string(path).context(\"failed to read\")?`
  - Custom errors derive `thiserror::Error`, `snafu` alternative ok.
- **Macros:** Inline helpers, avoid complex procedural unless necessary.
- **Tests:** `#[cfg(test)] mod tests { ... }`, behavior-focused names like `test_rejects_invalid_json`.
- **Docs:** `///` for public items, examples.

### TypeScript/React (ESLint + Prettier)

- **Formatting:** 2-space indent, Prettier via ESLint.
- **Naming:**
  - Components: `PascalCase`
  - Variables/functions: `camelCase`
  - Constants: `UPPER_SNAKE_CASE`
  - Files: `kebab-case` or `PascalCase.tsx`
- **Imports:**
  - Absolute from `@/`: `import { Button } from '@/components/Button';`
  - Grouped: React first, then external, internal.
  - No `import * as`, prefer named.
- **Types:**
  - Strict mode enabled.
  - Prefer interfaces over types for props/objects.
  - Generics explicit.
  - `unknown` over `any`.
- **Hooks/Components:**
  - Custom hooks: `usePascalCase`.
  - Functional components only.
  - Memoize expensive renders: `React.memo`, `useMemo`.
- **Error Handling:**
  - Try/catch with logging.
  - Error boundaries for UI.
- **State:** Zustand/Context, typed reducers.

## Testing Guidelines

- **Unit Tests:** 80%+ coverage, mock externals.
- **Integration:** Test full flows, esp D-Bus/gRPC.
- **Rust:** `assert_eq!`, `assert!`, `insta` snapshots for JSON.
- **JS:** Vitest, `vi.mock`, `@testing-library/react`.
- Every change: add/update tests in affected crates.
- Name: `should_handle_<scenario>` or `rejects_<invalid>`.
- UI: RTL queries, no implementation details.
- Screenshots for visual changes (via CI or PR).

## Commit & PR Guidelines

- Commits: Imperative mood, <50 chars: `fix: handle empty schemas`.
- Scope: One logical change per commit.
- PRs: List crates affected, verification cmds, issue links, screenshots.
- No force-push main.

## Security & Config

- Config: Copy `deploy/environment.default`.
- Never commit: `target/`, `storage/`, `snapshots/`, `.env`.
- Secrets: `sops` or env vars.
- Deploy verify: `dinitctl status op-dbus`.

## Agent-Specific Instructions

- Before edits: `cargo check -p <crate>`, `npm run typecheck`.
- After changes: Run full lint/test sequence.
- Mimic existing patterns: Search similar code.
- No new deps without PR justification.
- Commit only when asked.

(150+ lines achieved with details)
