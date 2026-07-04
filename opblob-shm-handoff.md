# OPBlob SHM Handoff

## What Changed

- Added real runtime materialization commands to `crates/op-blob/src/bin/opblob.rs`:
  - `opblob seal-shm`
  - `opblob seal-plugins <dir>`
- The commands discover canonical plugin schemas through `op_plugins::DefaultPluginRegistry::load_all_plugins()` using `op_state_store::MemoryStore`.
- The commands seal real `PluginSchema` objects into the SHM blob catalog at `/dev/shm/opdbus/plugin-blobs`.
- Added `op-plugins` and `tokio` dependencies to `crates/op-blob/Cargo.toml`.
- Fixed `crates/op-blob/src/blob.rs` so method manifest generation does not assume the `HashMap` key equals `MethodDecl.name`.
- Built and installed `/usr/local/bin/opblob`.
- Patched `/etc/s6/sv/op-dbus/run` to run `/usr/local/bin/opblob seal-shm` before starting `op-dbus`.
- Also added `OP_DBUS_GRPC_LISTEN="${OP_DBUS_GRPC_LISTEN:-10.200.0.1:50051}"` to `/etc/s6/sv/op-dbus/run`.

## Verified

- `cargo check -p op-blob` passes.
- `cargo build --release -p op-blob --bin opblob` passes.
- `/usr/local/bin/opblob seal-shm` succeeds.
- `/dev/shm/opdbus/plugin-blobs` contains 62 `.blob` files.
- `/usr/local/bin/opblob catalog /dev/shm/opdbus/plugin-blobs` parses the active catalog.
- `/usr/local/bin/opblob inspect /dev/shm/opdbus/plugin-blobs/zeroclaw.*.blob` parses and shows canonical D-Bus path `/org/opdbus/v1/plugins/zeroclaw`.
- `xray` blob exists.
- Installed binary hash matched release build:
  - `target/release/opblob`
  - `/usr/local/bin/opblob`

## Important Runtime State

- `/etc/s6/sv/op-dbus/run` has the blob hook and the bind fix.
- The blob hook was copied into `/run/service/op-dbus/run`.
- The bind fix was not copied into `/run/service/op-dbus/run` before interruption.
- `op-dbus` was failing because it tried to bind `10.200.0.2:50051`.
- Current host has `ovsbr0` as `10.200.0.1/30`; no `10.200.0.2` address exists.
- Manual run with blob hook disabled showed `Cannot assign requested address (os error 99)`.
- This failure is unrelated to `opblob`; `/usr/local/bin/opblob seal-shm` exits cleanly.

## Next Commands

```sh
sudo -n install -m 0755 /etc/s6/sv/op-dbus/run /run/service/op-dbus/run
sh -n /run/service/op-dbus/run
sudo -n s6-svc -r /run/service/op-dbus
sleep 2
sudo -n s6-svstat /run/service/op-dbus
find /dev/shm/opdbus/plugin-blobs -maxdepth 1 -type f -name '*.blob' | wc -l
/usr/local/bin/opblob inspect /dev/shm/opdbus/plugin-blobs/zeroclaw.*.blob | sed -n '1,20p'
```

## Do Not Use Yet

- Do not use `deploy/s6/opdbus/run` yet. It has merge conflict markers.
- Do not normalize `Cargo.lock` casually. The workspace already had broad dirty lockfile state.

## Repo Status Notes

- `crates/op-blob/` is untracked in git.
- `Cargo.toml` and `Cargo.lock` are dirty.
- `Cargo.lock` has broad dependency churn from the current workspace state.

## Architecture Status

- SHM blobstore is now real and populated.
- Projection has not been converted yet.
- Next code step is projection reading `/dev/shm/opdbus/plugin-blobs`, not `live-schema.json`.
