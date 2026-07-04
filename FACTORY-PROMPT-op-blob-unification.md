# Factory Mission: Unify operation-dbus-proto on the original op-blob design

## Context (read first, do not re-derive)

Two repos are involved:

- **`/home/jeremy/git/operation-dbus-proto/`** — the canonical workspace (you work HERE).
  `crates/op-blob` already exists and is wired in: `op-grpc-bridge/src/plugin_object_blob.rs`
  is a re-export shim over it, `grpc_server.rs::register_plugin_methods` persists sealed
  blobs via `op_blob::BlobStore`, and `cargo test -p op-blob -p op-grpc-bridge --lib`
  is green (30 + 7 tests).
- **`/home/jeremy/git/opdbus-blob/`** — reference repo (READ-ONLY for you). It holds the
  specs (`.kiro/specs/schemars-to-reflection-plugin-pipeline/`), the whitepaper
  (`schema-coupled-plugin-blob-reflection-whitepaper.md`), and the **original standalone
  `op-blob` prototype** at `opdbus-blob/op-blob/src/` whose design is the target.

The current `crates/op-blob` kept the workspace's *first-pass* descriptor shape
(`operation.plugin.v1.<Plugin>PluginMethods`, `google.protobuf.Struct`-typed I/O,
one service per plugin). The mission is to replace that with the original design's
**per-method typed descriptors** and unify the schemars adapter layer, so op-blob is
the single cohesive schema-pipeline crate.

Git state warning: the repo is mid-merge of `pr-17`. These paths still contain
conflict markers — **do not touch them**: `.gitmodules`, `operation-dashboard-ui-07`,
`crates/op-dbus-mirror/*`, `crates/op-openvswitch-daemon/src/grpc.rs`,
`crates/op-projection/src/lib.rs`, `crates/op-projection/src/schema_engine.rs`,
`crates/op-web/*`, `deploy/deploy.sh`, `deploy/s6/opdbus/run`. Everything you need
compiles without them. Do not commit; leave changes in the working tree.

## Non-negotiable rules

1. `op_state_store::PluginSchema` stays the single source of truth. Do NOT duplicate
   the type (the prototype's `schema.rs` port existed only because that repo lacked
   op-state-store). Instead `op-blob` re-exports it: `pub use op_state_store::{PluginSchema, MethodDecl, FieldSchema, FieldType, SideEffect};`
2. **No phantom reflection**: a service advertised via reflection must have a mounted,
   callable route. The two-layer strategy from the spec stands: build-time
   `Struct`-typed routes give Rust dispatch; runtime per-method typed descriptors give
   client-visible field types. Replace the *blob/reflection* descriptor source with the
   typed generator; only advertise `operation.method.*` services if you also mount
   matching routes (see Task 4), otherwise keep them in the blob + `file_by_filename`
   index without listing them in `ListServices`.
3. Blob sealing stays deterministic: same schema ⇒ same sealed bytes ⇒ same hash.
   No timestamps, sorted maps, methods sorted by name.
4. WireGuard keypair is the identity: public key only in blobs, private key never
   serialized, session id derived from the pubkey, lifespan `account-persistent`,
   no expiry fields ever.
5. Each migration step ends green: `cargo check -p <crate>` then the test gates below.

## Tasks (in order)

### Task 1 — Port the per-method typed descriptor generator

Source of truth: `/home/jeremy/git/opdbus-blob/op-blob/src/descriptor.rs`.

Create `crates/op-blob/src/descriptor.rs` implementing, per `MethodDecl`:

- file name `operation/method/<plugin>/<method_snake>.proto`
- package `operation.method.<plugin>.<method_snake>`
- messages `<MethodPascal>Input` / `<MethodPascal>Output` with **field-typed** members
  derived from `MethodDecl.args` / `MethodDecl.returns` JSON Schemas
  (string→string, integer→int64, number→double, boolean→bool, array→repeated,
  object-with-properties→nested message, enum/any/union→string). Resolve `$defs`/`$ref`.
  `returns: None` ⇒ empty Output message. **No `google.protobuf.Struct` anywhere.**
- service `<MethodPascal>Service` with rpc `<MethodPascal>`; grpc_path
  `/operation.method.<plugin>.<method_snake>.<MethodPascal>Service/<MethodPascal>`
- `descriptor_set_for_plugin(plugin_id, &PluginSchema) -> (Vec<u8> /*FileDescriptorSet*/, Vec<MethodDescriptor>)`
  where `MethodDescriptor` carries file_name, package, service_full, grpc_path,
  input_full, output_full, symbols.

You may re-implement with `prost-types` (workspace dep) instead of the prototype's
hand-encoded wire format — but the naming and typing contract above is exact.
Port the prototype's tests (`typed_descriptor_matches_spec_naming`, case conversions);
verify by decoding with `prost_types::FileDescriptorSet::decode`.

### Task 2 — Rework the blob model to the original manifest shape

Update `crates/op-blob/src/blob.rs`:

- `GrpcIdentity` becomes multi-service: `{ services: Vec<String>, files: Vec<String> }`
  (sorted). Keep a `legacy_service_name: Option<String>` only if a caller still needs
  the `operation.plugin.v1.<Plugin>PluginMethods` name — grep callers first
  (`dynamic_reflection.rs` uses `blob.grpc.service_name`; migrate it, see Task 4).
- `BlobMethod` gains `grpc_service`, `grpc_file`, `input_message`, `output_message`,
  `symbols: Vec<String>` (from Task 1's `MethodDescriptor`), keeping the existing
  subid / required_capability / side_effect / idempotent / args_schema / returns_schema.
- `blobify_plugin_schema*` populates `grpc.descriptor_set` from
  `descriptor_set_for_plugin` (per-method typed) instead of
  `synthesize_plugin_descriptor_set` (Struct-typed). Delete the old
  `synthesize_plugin_file_descriptor`/`schema_descriptor`/`json_schema_type_to_proto`
  Struct path once nothing references it.
- `sealed.rs` (zero-copy sectioned format) and `store.rs` (shm BlobStore) already match
  the original design — keep them; update their tests for the new manifest fields.

### Task 3 — Unify the schemars adapter into op-blob

- Move the adapter from `crates/op-plugins/src/state_plugins/schemars_adapter.rs`
  into `crates/op-blob/src/adapter.rs` (public: `plugin_schema_from_json`,
  `apply_state_defaults`, `schema_diffs` — make `schema_diffs` non-test-gated).
- Move `method_decl_from_schemars_with_output`, `AckOutput`, `EmptyInput` from
  `crates/op-plugins/src/state_plugins/plugin_scaffold_helpers.rs` into
  `crates/op-blob/src/adapter.rs`.
- In op-plugins, replace the moved code with re-exports
  (`pub use op_blob::adapter::...;`) so all ~70 plugin files compile **unchanged**.
  op-plugins gains an `op-blob` dependency (add to its Cargo.toml; note op-grpc-bridge
  already depends on both — no cycle: op-blob depends only on op-state-store).
- Reference for the adapter's expected behavior + tests:
  `/home/jeremy/git/opdbus-blob/op-plugins/src/state_plugins/schemars_adapter.rs`
  (same file, richer test set) — port `walks_array_of_objects_with_constraints`,
  `ingests_root_and_field_subids`, `reports_nested_mismatch`.

### Task 4 — Reflection catalog uses per-method services, no phantoms

Update `crates/op-grpc-bridge/src/dynamic_reflection.rs`:

- `rebuild_index` collects active services from `blob.grpc.services` (plural).
- Descriptor indexing decodes the blob's per-method `FileDescriptorSet` so
  `file_by_filename` / `file_containing_symbol` serve the typed descriptors.
- **Gate `ListServices`**: only include `operation.method.*` services when the bridge
  has mounted matching routes. Wire this to the existing per-method machinery
  (`plugin_grpc_gen.rs::PerMethodGrpcServices` / `per_plugin_reflection.rs`) — the
  runtime freeze path already builds typed per-method descriptors; converge them:
  the blob's descriptor set becomes the single typed-descriptor source consumed by
  both `PerMethodGrpcServices` registration and the reflection catalog. If full route
  mounting is out of reach in this pass, advertise the legacy
  `operation.plugin.v1.*PluginMethods` services (which ARE mounted via build.rs) in
  `ListServices` while still indexing the typed files for symbol/file lookups, and
  leave a `// TODO(unify-routes)` marker — do not fake it.
- Update `zeroclaw_object_blob.rs` test expectations to the new service naming.

### Task 5 — Port the opblob CLI

Create `crates/op-blob/src/bin/opblob.rs` from
`/home/jeremy/git/opdbus-blob/op-blob/src/bin/opblob.rs`, adapted:

- `inspect <file.blob>` — identity, session, D-Bus path, methods, decoded descriptors
- `catalog <dir>` / `store <dir>` — list active plugins + services from a BlobStore dir
- `btrfs-seal <image> <dir>` — seal a store into a btrfs image (module already exists)
- `keygen <keyfile>` — WG identity keypair (refuse overwrite; session is account-persistent)
- `demo-seal` may use the real zeroclaw schema
  (`op_plugins::state_plugins::zeroclaw::zeroclaw_plugin_schema()`) — that makes the
  bin depend on op-plugins; if that's too heavy, gate it behind a cargo feature `demo`.

### Task 6 — Retire superseded code

- Delete `crates/op-projection/src/blob.rs` ONLY IF `op-projection/src/lib.rs` no longer
  has conflict markers by the time you get here; otherwise leave it and note it.
- Remove any now-dead Struct-typed synthesis helpers in op-blob and the bridge shim.
- `crates/op-blob/README.md`: document the pipeline (copy the layer diagram from
  `/home/jeremy/git/opdbus-blob/op-blob/README.md`, adjusted for workspace reality).

## Acceptance gates (all must pass)

```sh
cargo check -p op-blob -p op-plugins -p op-grpc-bridge
cargo test  -p op-blob
cargo test  -p op-plugins --lib          # adapter re-export keeps plugins green
cargo test  -p op-grpc-bridge --lib      # incl. zeroclaw_object_blob + dynamic_reflection
cargo clippy -p op-blob -- -D warnings
```

Plus behavioral checks:

1. Decode a sealed zeroclaw blob's descriptor set: every method has
   `operation.method.zeroclaw.<method>.<Pascal>Service` with typed Input/Output
   messages; zero occurrences of `google.protobuf.Struct` in per-method files.
2. Seal twice from the same schema ⇒ byte-identical files.
3. `BlobRef` slices point inside the sealed buffer (zero-copy assertion exists in
   prototype tests — port it).
4. Blob with WG identity: private key base64 never appears in sealed bytes.
5. Reflection test proves: service listed ⇔ route mounted (or documented TODO fallback
   from Task 4).
6. btrfs seal test passes (mkfs.btrfs is installed; keep the `-f` flag — this host has
   a root-owned incus loop backing file that breaks mkfs's mount scan without it).

## Style

Match surrounding code. Comments only for constraints code can't express. Don't
reformat untouched code. Don't add dependencies beyond what's in workspace deps +
op-blob's existing x25519-dalek/rand_core.
