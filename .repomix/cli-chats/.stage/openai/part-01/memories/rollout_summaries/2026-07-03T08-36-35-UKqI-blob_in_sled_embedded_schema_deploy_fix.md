thread_id: 019f271f-5f4c-7491-817f-4be3878f2100
updated_at: 2026-07-03T09:36:48+00:00
rollout_path: /home/jeremy/.codex/sessions/2026/07/03/rollout-2026-07-03T04-36-35-019f271f-5f4c-7491-817f-4be3878f2100.jsonl
cwd: /home/jeremy/git/operation-dbus-proto
git_branch: main

# Blob architecture was embedded into the shared-memory sled and then deployed successfully after fixing a broken deploy script.

Rollout context: The user asked why blobs were not being written to SHM, clarified that the blob should include the schema, then asked to fix it. The work happened in `/home/jeremy/git/operation-dbus-proto` and centered on the SHM/blob path, projection writers, gRPC/web readers, and the deploy script.

## Task 1: Diagnose why blobs were not being written to shared memory
Outcome: partial

Preference signals:
- After the assistant initially traced logs, the user corrected the premise: "but it is supposed to be writing blobs instead of schema now. the blobs have the schema included" -> the user wants the blob architecture treated as the source of truth, not just the legacy schema JSON path.
- When the assistant proposed next steps, the user replied "yes" and later "fix it" -> in similar situations, the user expects direct implementation rather than extended discussion.

Key steps:
- The first runtime failure found in logs was `deploy/services.log: error returned from database: (code: 14) unable to open database file`; that was not the SHM write itself, but it blocked the startup path before projection/write logic could complete.
- The codebase still had two separate artifacts at first: `/dev/shm/live-schema.json` and `/dev/shm/plugin_schema.dat`, with the sled/blob depending on the schema JSON for its footprint hash.
- The assistant then traced the actual invocation paths for the blob writer (`write_sled_from_wg`, `write_sled_full`) and found it was called from `op-mcp`, `op-cognitive-mcp`, and `op-grpc-bridge`, but still derived its hash from the legacy schema JSON file.

Failures and how to do differently:
- Early investigation assumed the problem was "nothing is being written to SHM"; the evidence showed the more specific issue was that the runtime still had a split schema/blob design and the startup path was failing before the writer could run.
- A later deploy attempt failed because the installer tried to copy an s6 directory onto itself via a symlinked service directory. The deploy script needed a self-copy guard.

Reusable knowledge:
- `SchemaEngine::write_schemas_to_shm()` wrote to `/dev/shm/live-schema.json` and was still the legacy schema writer.
- `write_sled_from_wg()` and `write_sled_full()` were the actual blob writers and were the right hook for embedding schema into the blob.
- `op-services` startup failure (`code 14 unable to open database file`) was a real blocker, but it was not the blob-write root cause.

References:
- [1] `deploy/services.log`: `Error: error returned from database: (code: 14) unable to open database file`
- [2] `crates/op-projection/src/schema_engine.rs:129-152` legacy SHM JSON writer
- [3] `crates/op-identity/src/schema_bridge.rs:559-643` blob writer path and footprint generation
- [4] `crates/op-mcp/src/main.rs:102-114`, `crates/op-mcp/src/compact.rs:579-586`, `crates/op-cognitive-mcp/src/main.rs:102-114`, `crates/op-grpc-bridge/src/schema_engine.rs:439` invocation sites for the blob writer

## Task 2: Implement blob-in-sled architecture and deploy it
Outcome: success

Preference signals:
- The user requested "fix it" after being told the blob path still depended on the legacy schema file -> they wanted the architecture fixed, not just explained.
- The user then issued the exact deploy command `sudo ./deploy/deploy.sh --skip-network all` -> they prefer the agent to execute the concrete command when asked.

Key steps:
- Added a versioned embedded schema tail to `/dev/shm/plugin_schema.dat` while preserving the first 152 bytes as the canonical `IdentitySled` prefix.
- Added `SCHEMA_BLOB_MAGIC = OPBLOB01` and a length-prefixed embedded schema format in `crates/op-identity/src/schema_bridge.rs`.
- Made `write_sled()` preserve the embedded blob when the sled is rewritten, and added `read_schema_blob()` / `write_schema_blob()` helpers.
- Updated `crates/op-projection/src/schema_engine.rs` to write the embedded blob whenever the schema catalog is written.
- Updated `crates/op-grpc-bridge/src/grpc_server.rs` to read schema from the embedded blob first, then fall back to `/dev/shm/live-schema.json`.
- Updated `crates/op-cognitive-mcp/src/qdrant_shuttle.rs`, `crates/op-web/src/projection_client.rs`, `crates/op-web/src/handlers/schema.rs`, `crates/op-web/src/handlers/identity.rs`, `crates/op-web/src/handlers/zeroclaw.rs`, and `crates/op-web/src/main.rs` to prefer the embedded blob where applicable.
- Updated `crates/op-identity/src/anna_scribe.rs` and `crates/op-identity/src/bin/op-identity-sled.rs` so their footprint/diagnostic reads work with the embedded schema blob.
- Updated `deploy/deploy.sh` to install `op-identity-sled`, `op-grpc-bridge`, and `op-mcp-server`, and to avoid self-copying s6 directories when a service path is symlinked back into the repo.

Failures and how to do differently:
- Plain `rustfmt` failed because it defaulted to Rust 2015 on async-heavy files; rerunning with `rustfmt --edition 2021` fixed it.
- The first deploy attempt failed early because `/etc/s6/sv/gbr-warp` resolved to the same path as `deploy/s6/gbr-warp`; future deploy scripts should compare `readlink -f` and skip self-copies.
- `op-web-server` restarted with a warning because the script targets `op-web-server` while the live s6 service appears to be `op-web-srv`; that naming mismatch may need a follow-up cleanup.

Reusable knowledge:
- The blob format now is: 152-byte `IdentitySled` prefix + 20-byte blob header + embedded schema bytes.
- `write_sled()` now preserves the embedded schema blob, so WireGuard refreshes no longer destroy the embedded schema.
- The deploy script builds/installs the blob-owning binaries, so source changes actually reach `/usr/local/bin` and the s6 services.
- `op-identity-sled --path /dev/shm/plugin_schema.dat` now reports `schema_blob_bytes` and can confirm whether the embedded schema is present.

References:
- [1] `crates/op-identity/src/schema_bridge.rs:21-26` `SCHEMA_BLOB_MAGIC`, `SCHEMA_BLOB_VERSION`
- [2] `crates/op-identity/src/schema_bridge.rs:277-330` preserved blob write path and `write_schema_blob`
- [3] `crates/op-identity/src/schema_bridge.rs:352-390` `read_schema_blob`
- [4] `crates/op-identity/src/schema_bridge.rs:690-745` hash generation now uses `current_schema_catalog_hash()`
- [5] `crates/op-projection/src/schema_engine.rs:149-166` embedded blob write + blob-first footprint read
- [6] `crates/op-grpc-bridge/src/grpc_server.rs:99-143` blob-first schema discovery
- [7] `crates/op-cognitive-mcp/src/qdrant_shuttle.rs:424-431` blob-first schema reader
- [8] `crates/op-web/src/main.rs:30-45`, `crates/op-web/src/handlers/schema.rs:1-52`, `crates/op-web/src/handlers/identity.rs:1-40`, `crates/op-web/src/handlers/zeroclaw.rs:235-252`, `crates/op-web/src/projection_client.rs:1-40`
- [9] `deploy/deploy.sh:18-27` added missing binaries; `deploy/deploy.sh:90-98` self-copy guard using `readlink -f`
- [10] Verification outputs: `cargo check -p op-identity -p op-projection -p op-grpc-bridge -p op-cognitive-mcp -p op-web` passed; `git diff --check` passed; deploy completed successfully
- [11] Final live check: `/dev/shm/plugin_schema.dat 82013 bytes`, `/dev/shm/live-schema.json 81841 bytes`, and `op-identity-sled` reported `schema_blob_bytes: 81841` and `is_valid: true`
