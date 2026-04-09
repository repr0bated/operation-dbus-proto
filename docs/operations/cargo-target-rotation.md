# Cargo Target Rotation

This repository now has two target-cache retention paths that keep only the 3 newest build trees by default.

## Deploy Builds

`deploy/deploy.sh` uses rotating target caches automatically:

- system deploys: `/var/cache/op-dbus-build/build-*`
- user-mode deploys: `deploy/target-cache/build-*`

Retention is controlled with:

```bash
OP_DBUS_TARGET_RETENTION_COUNT=3
```

## Local Workspace Builds

Cargo does not support "keep the last N target directories" natively for plain `cargo build`. A repository cannot transparently replace the global `cargo` binary without shell or PATH changes, so the workspace provides a managed wrapper instead.

Use the managed wrapper directly:

```bash
./scripts/cargo-managed.sh build --workspace
./scripts/cargo-managed.sh check --workspace
./scripts/cargo-managed.sh test --workspace
```

The managed wrapper stores local build outputs in:

```bash
target-cache/build-*
```

You can override the local cache root or retention count:

```bash
OP_DBUS_LOCAL_TARGET_ROOT=/some/fast-disk/op-dbus-target-cache
OP_DBUS_TARGET_RETENTION_COUNT=3
```

Example:

```bash
OP_DBUS_TARGET_RETENTION_COUNT=2 ./scripts/cargo-managed.sh build -p op-web
```

The first managed run also removes any legacy flat Cargo cache layout inside the selected cache root.
