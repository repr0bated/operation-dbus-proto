# Projection Troubleshooting Context

Date: 2026-04-19
Repo: /home/jeremy/git/operation-dbus-proto

## Summary

The new projection appeared not to load because the projection daemon had two likely failure modes:

1. Root projected paths were generated with a trailing slash, for example:
   /org/opdbus/projection/registry/

   D-Bus object paths cannot end in / except for the root object path itself, so root/container projection nodes could fail to publish.

2. RegistrySource::discover() used connect_with_retry(). That retry path waits through roughly 107 seconds of delays before returning an error. ProjectionEngine::refresh_all() runs sources serially, so a down or wrong registry endpoint blocks all supplemental sources behind it, including D-Bus and procfs projections.

## Changes Applied

### crates/op-projection/src/source.rs

Updated projection_path() so root/empty relative paths no longer produce trailing-slash object paths:

```rust
pub fn projection_path(namespace: &str, relative: &str) -> String {
    if relative == "/" || relative.is_empty() {
        format!("/org/opdbus/projection/{}", namespace)
    } else if relative.starts_with('/') {
        format!("/org/opdbus/projection/{}{}", namespace, relative)
    } else {
        format!("/org/opdbus/projection/{}/{}", namespace, relative)
    }
}
```

Added unit tests for:

- root relative path /
- empty relative path
- normal absolute relative path
- normal non-absolute relative path

### crates/op-projection/src/registry_source.rs

Changed registry discovery from startup retry behavior:

```rust
let mut client = self.connect_with_retry().await?;
```

to fail-fast behavior:

```rust
let mut client = self.connect().await?;
```

The watch path still has retry behavior, so registry reconnect is preserved without blocking every full refresh cycle.

## Verification Status

Passed:

```sh
CARGO_TARGET_DIR=/tmp/op-projection-target cargo test -p op-projection
```

Result:

```text
2 projection unit tests passed, plus op-projection bin tests and doc tests.
```

```sh
CARGO_TARGET_DIR=/tmp/op-projection-target cargo test -p op-projection
```

Also passed:

```sh
CARGO_TARGET_DIR=/tmp/op-projection-target cargo clippy -p op-projection -- -D warnings
```

Additional cleanup was required for clippy:

- Removed now-unused registry startup retry helper after discovery switched to fail-fast connect.
- Fixed a clippy `ptr_arg` warning in `crates/op-projection/src/dbus_source.rs`.
- Fixed two clippy warnings in the local `op-execution-tracker` dependency.

Workspace-wide formatting was attempted but remains blocked by unrelated pre-existing issues:

- trailing whitespace in `crates/crates/op-agents/src/agents/orchestration/memory.rs`
- missing modules referenced by other crates, including `dynamic_loader.rs` and `antigravity.rs`

The original repo target directory still appears to have ownership/permission problems. If target/ ownership is fixed:

```sh
cargo test -p op-projection
```

## Environment Notes

Codex could not see /usr/bin/doas:

```text
/usr/bin/bash: line 1: /usr/bin/doas: No such file or directory
```

The repo is mounted through SSHFS:

```text
jeremy@ssh.3tched.com:/home/jeremy/git on /home/jeremy/git type fuse.sshfs
```

Several repo files appeared as root:root locally, and normal apply_patch could not replace files under crates/op-projection/src. The final source edits were applied through the available elevated command in this environment after direct patch replacement failed.

## Operational Check

After building/deploying, check the projection service:

```sh
dinitctl status op-projection
```

Then inspect logs for:

- op-projection-daemon starting
- Registered primary source: registry
- Registered supplemental source: dbus-system
- Registered supplemental source: procfs
- no repeated long registry blocking before procfs/system bus projection appears

If registry is intentionally unavailable, supplemental projections should still publish after each refresh instead of waiting behind the registry retry sequence.
