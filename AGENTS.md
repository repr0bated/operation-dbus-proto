# Repository Guidelines

## Project Structure & Module Organization
`src/` holds the root `op-dbus` binary and legacy glue modules. Most active Rust code lives in the workspace crates under `crates/crates/op-*`, including `op-core`, `op-plugins`, `op-state-store`, `op-workflows`, `op-web`, and `op-mcp`. Frontend code is split between the shared Vite app in `crates/src` and the production UI in `crates/op-web/ui`. Deployment assets live in `deploy/`; schemas, examples, and reference docs live in `schemas/`, `examples/`, and `docs/`. Python indexing utilities are in `openclaw-indexer/`.

## Engineering Principles
Prefer D-Bus-native integrations when the platform already exposes the capability there. Use gRPC for internal service-to-service communication where possible. Stick to the existing high-performance JSON path and avoid introducing alternate serialization approaches without a strong reason. Keep control-plane changes deterministic and schema-driven.

## Build, Test, and Development Commands
This repository targets Chimera Linux: use `doas` for elevated commands, `apk` for package management, and `dinitctl` for service checks on deployed hosts.

- `cargo build --workspace`: build all Rust crates.
- `cargo test --workspace`: run Rust unit tests across the workspace.
- `cargo fmt --all` and `cargo clippy --workspace -- -D warnings`: format and lint Rust changes.
- `cargo build -p op-web --release`: build the web server crate used in CI.
- `cd crates/op-web/ui && npm ci && npm run build:prod`: build the production UI.
- `cd crates && npm test`: run Vitest for the shared frontend app.
- `doas apk add rust cargo nodejs npm`: install common build dependencies on Chimera.
- `doas ./deploy/install.sh --dry-run --domain example.com`: preview a full install.
- `doas ./deploy/upgrade.sh` or `./deploy/deploy.sh`: update an installed host or run the dinit-focused deploy path.

## Coding Style & Naming Conventions
Rust uses edition 2021, `rustfmt`, 4-space indentation, `snake_case` for modules/functions, and `CamelCase` for types. Keep tests close to implementation with `#[cfg(test)]` where practical. TypeScript/React follows the existing ESLint setup, 2-space indentation, and `PascalCase` component names.

## Testing Guidelines
Add or update tests with every behavior change, especially in `op-state-store`, `op-plugins`, `op-workflows`, and root `src/` modules. Name tests after behavior, for example `rejects_invalid_schema`. For UI logic changes, add Vitest coverage; for visible UI changes, include screenshots in the PR.

## Commit & Pull Request Guidelines
Use short, imperative commit subjects, for example `add sqlite schema validation`. Keep each commit scoped to one logical change. PRs should list affected crates, exact verification commands, linked issues, and screenshots for frontend changes.

## Security & Configuration Tips
Start from `deploy/environment.default` for local or service configuration. Never commit live credentials, generated state from `storage/` or `snapshots/`, or build output from `target/`. Verify deployed services with `dinitctl status <service>` on Chimera hosts.
