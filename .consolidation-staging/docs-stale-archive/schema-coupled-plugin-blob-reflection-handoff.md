# Schema-Coupled Plugin Blob Reflection Handoff

Date: 2026-06-30
Repo: `operation-dbus-proto`

## Current Direction

The bridge has moved from a monolithic live schema mindset toward individual plugin object blobs.

The settled model is:

`PluginSchema` -> `PluginObjectBlob` -> active reflection catalog -> mounted tonic reflection service

Key idea:

- the schema is still the seed and source of truth
- the blob is the frozen runtime object for one plugin
- the caller sees only the relevant projection
- tonic reflection is static at build time, so the bridge now owns the mutable active catalog

## What Was Added

### Generic blob support

File:

- [crates/op-grpc-bridge/src/plugin_object_blob.rs](/home/jeremy/git/operation-dbus-proto/crates/op-grpc-bridge/src/plugin_object_blob.rs)

Contains:

- `PluginObjectBlob`
- `DbusObjectIdentity`
- `GrpcObjectIdentity`
- `BlobMethod`
- `blobify_plugin_schema(...)`
- schema-derived `FileDescriptorSet` synthesis from `PluginSchema.methods`
- deterministic FNV-1a field number policy with protobuf reserved range skip
- canonical schema hashing helpers
- per-object shm naming helpers

### Zeroclaw consumer

File:

- [crates/op-grpc-bridge/src/zeroclaw_object_blob.rs](/home/jeremy/git/operation-dbus-proto/crates/op-grpc-bridge/src/zeroclaw_object_blob.rs)

Purpose:

- first plugin-specific consumer of the generic blob helper
- proves the shape for freezing D-Bus identity, gRPC identity, schema, and method metadata together

### Active reflection catalog

File:

- [crates/op-grpc-bridge/src/dynamic_reflection.rs](/home/jeremy/git/operation-dbus-proto/crates/op-grpc-bridge/src/dynamic_reflection.rs)

Purpose:

- maintain the active plugin blob set
- advertise only active service names
- answer reflection lookups from the active catalog

### Bridge wiring

File:

- [crates/op-grpc-bridge/src/grpc_server.rs](/home/jeremy/git/operation-dbus-proto/crates/op-grpc-bridge/src/grpc_server.rs)

Changes:

- `register_plugin_methods(...)` now freezes a `PluginObjectBlob`
- the blob is inserted into `active_reflection`
- v1 reflection is now built from `DynamicReflectionService`
- the bridge no longer relies on the old monolithic live schema for reflection identity

### Whitepaper

File:

- [docs/schema-coupled-plugin-blob-reflection-whitepaper.md](/home/jeremy/git/operation-dbus-proto/docs/schema-coupled-plugin-blob-reflection-whitepaper.md)

Contains:

- architecture summary
- blob model
- active reflection model
- D-Bus / gRPC / schema coupling
- risks and tests

## Verification

Passed:

- `cargo check -p op-grpc-bridge`
- `cargo test -p op-grpc-bridge dynamic_reflection --lib`
- `cargo test -p op-grpc-bridge zeroclaw_object_blob --lib`

Known warning:

- pre-existing `to_snake_case` dead code warning in `crates/op-grpc-bridge/src/proto_gen.rs`

## Important Constraints

- Do not reintroduce D-Bus passthrough as the public path for plugin methods.
- Do not treat the monolithic live schema as the runtime source of truth.
- Keep reflection aligned to mounted callable services only.
- Keep per-object blobs as the unit of active registration and removal.
- The blob can now synthesize richer typed descriptors from schema methods, but the generated tonic route service still uses `google.protobuf.Struct` request/response bodies until `build.rs` is upgraded to generate matching typed messages.

## Next Work

1. Move additional plugins onto `PluginObjectBlob` consumers one by one.
2. Upgrade `crates/op-grpc-bridge/build.rs` so generated tonic services use the same schema-derived request/response messages synthesized by the blob.
3. Replace any remaining live-schema-driven reflection paths with blob-driven registration.
4. Expand the active reflection catalog tests to cover non-Zeroclaw plugins.
5. Decide whether the per-object shm storage should stay JSON for now or move to a packed binary envelope.
6. Remove the old dead experimental reflection helpers once the blob path fully replaces them.

## Notes For Resume

The most important files to inspect first on resume are:

- [crates/op-grpc-bridge/src/dynamic_reflection.rs](/home/jeremy/git/operation-dbus-proto/crates/op-grpc-bridge/src/dynamic_reflection.rs)
- [crates/op-grpc-bridge/src/plugin_object_blob.rs](/home/jeremy/git/operation-dbus-proto/crates/op-grpc-bridge/src/plugin_object_blob.rs)
- [crates/op-grpc-bridge/src/grpc_server.rs](/home/jeremy/git/operation-dbus-proto/crates/op-grpc-bridge/src/grpc_server.rs)
