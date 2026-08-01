# Developer Workflow

## Lint and test

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
```

## Frontend

```bash
cd crates
npm run lint
npm run typecheck
npm test
```
