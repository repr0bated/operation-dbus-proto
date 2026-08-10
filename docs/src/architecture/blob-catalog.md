# Sealed Blob Catalog

The sealed blob catalog is the runtime source of truth for active plugins. A
plugin exists when its blob is present in the catalog; removing that blob
deregisters the plugin. Consumers read the catalog instead of consulting the
Rust registry or a generated monolithic schema file.

## Runtime layout

The catalog lives on tmpfs:

```text
/dev/shm/opdbus/plugin-blobs/
├── <plugin-id>.<schema-hash-first-16-hex-characters>.blob
└── .manifest.json
```

Each blob contains the plugin schema, D-Bus identity, generated gRPC
descriptors, method metadata, and optional identity data. Blob bytes are
deterministic for the same schema. The filename therefore identifies both the
plugin and the schema version.

Catalog mutations are atomic:

1. A new blob is written to a temporary file and renamed into place.
2. Older hash versions of the same plugin are removed.
3. `.manifest.json` is written last using the same temporary-file-and-rename
   pattern.

The manifest is the commit point for consumers. It contains the active plugin
IDs and full schema hashes, a monotonic `generation`, and the `catalog_hash`.
`catalog_hash` is computed once by `op-blob` from sorted plugin ID and schema
hash pairs. Consumers read the published value; they do not recompute it.
Catalog checks happen in response to arrivals rather than in polling loops.

## Resealing after schema changes

On the host, use the guarded workflow from the repository root:

```bash
./deploy/reseal-plugins.sh
```

The script:

1. Refuses tracked staged or unstaged changes. Untracked scratch files do not
   block it.
2. Fetches `origin/main` and requires `HEAD` to contain its current tip.
3. Builds release versions of `opblob` and `op-grpc-bridge`.
4. Seals schemas loaded from `DefaultPluginRegistry` into the SHM catalog.
   Plugins absent from the registry are swept; registered plugins without a
   schema are reported as skipped.
5. Installs both binaries with rename-based replacement, restarts
   `op-grpc-bridge` through runit, and reports service status.

These preflight checks prevent a successful-looking reseal from publishing
schemas built from a dirty or behind-main checkout.

Use `NO_RESTART=1` when only the catalog should change:

```bash
NO_RESTART=1 ./deploy/reseal-plugins.sh
```

This mode does **not** install the newly built binaries or restart the bridge.
D-Bus and dynamic reflection surfaces resynchronize on the next arrival, but
frozen per-method gRPC service descriptors retain their old method signatures
until `op-grpc-bridge` is replaced and restarted.

`--force` skips both repository checks:

```bash
./deploy/reseal-plugins.sh --force
```

Use it only when intentionally testing a non-main or dirty checkout; it removes
the protection against publishing stale or uncommitted schemas.

## Verification and troubleshooting

List the plugin IDs and gRPC services currently represented by the catalog:

```bash
sudo /usr/local/bin/opblob catalog /dev/shm/opdbus/plugin-blobs
```

Inspect one blob's schema identity, D-Bus path, methods, capabilities, subids,
and descriptors:

```bash
sudo /usr/local/bin/opblob inspect \
  /dev/shm/opdbus/plugin-blobs/<plugin-id>.<schema-hash16>.blob
```

After a normal reseal, verify that the bridge remained up:

```bash
sudo sv status op-grpc-bridge
```

Common failures:

- `tracked files have uncommitted changes`: commit or stash tracked changes.
- `HEAD does not contain origin/main`: merge or rebase the fetched main branch.
- A plugin is reported as `skipped`: its registry entry returned no schema.
- A removed plugin disappears after resealing: this is expected full-catalog
  sweep behavior, not data loss from another state store.

Do not restore the retired `/dev/shm/opdbus/schemas` directory,
`live-schema.json`, or a separate schema manifest. Do not write blobs directly;
all catalog changes must go through `op-blob` so blob replacement, stale-version
cleanup, and manifest publication remain one operation.
