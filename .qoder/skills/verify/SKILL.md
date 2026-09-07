---
name: verify
description: Run the full cargo pre-push gate (fmt, clippy, tests) before reporting work done. Use when asked to verify a change, check the build, or as a final step before committing non-trivial edits. CI does NOT gate fmt/clippy/test on this repo, so this is the only check.
---

Run the workspace-wide pre-push gate from the repo root. Stop on the first failure and report the failing output verbatim; do not continue past a red step.

## Steps (in order)

1. **Format check**
   ```bash
   cargo fmt --all -- --check
   ```
   If this fails, list the files that need formatting and offer to run `cargo fmt --all` to fix them.

2. **Clippy**
   ```bash
   cargo clippy --workspace --all-targets --all-features -- -D warnings
   ```
   Treat warnings as errors (`-D warnings` is the CI-equivalent gate this repo lacks).

3. **Tests**
   ```bash
   cargo test --workspace --all-targets --all-features
   ```
   Report failing test names and their stderr.

4. **Release build (optional, only if the user asks for a release-ready check)**
   ```bash
   CXXFLAGS="-include cstdint" cargo build --workspace --release
   ```
   The `CXXFLAGS` is required — vendored RocksDB in `cozorocks` fails on modern GCC without it.
   If verifying `op-web` specifically, the embedded UI must be built first:
   ```bash
   (cd crates/op-web/ui && npx vite build)
   ```
   `op-web` release builds panic without `crates/op-web/ui/dist/index.html`.

## Reporting

- If all three steps pass: one-line "verify: green" summary with the test count.
- If any step fails: name the step, quote the failing output, and stop. Do not claim the change works until the gate is green.
- Do not auto-fix clippy or test failures — report them for the user to decide on.
