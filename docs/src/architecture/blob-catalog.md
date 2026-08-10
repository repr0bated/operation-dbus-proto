# Sealed Blob Catalog

The sealed blob catalog is the runtime source of truth for active plugins. A
plugin is available to new catalog reads when its blob is present; removing it
through the catalog API deregisters it from those reads. Already-mounted D-Bus
objects remain active until their process restarts. Compiled typed gRPC routes
change only after a matching bridge binary is installed and restarted. Consumers
read the catalog instead of consulting the Rust registry or a generated
monolithic schema file.

## Runtime layout

The catalog lives on tmpfs:

```text
/dev/shm/opdbus/plugin-blobs/
├── <plugin-id>.<schema-hash-first-16-hex-characters>.blob
└── .manifest.json
```

Each blob contains the plugin schema, D-Bus identity, generated gRPC
descriptors, method metadata, and optional identity data. The schema hash is
deterministic for the canonical schema JSON. The filename therefore identifies
both the plugin and the schema version.

Individual blob and manifest replacements are rename-atomic:

1. A new blob is written to a temporary file and renamed into place.
2. Older hash versions of the same plugin are removed.
3. `.manifest.json` is written last using the same temporary-file-and-rename
   pattern.

An upsert or full reseal spans multiple filesystem operations and is not a
catalog-wide transaction. Manifest-aware consumers use `.manifest.json` as the
change marker; a consumer that scans blob files directly can observe an
in-progress reseal.

The manifest contains active plugin IDs and full schema hashes, an incrementing
`generation` derived from the previous parseable manifest, and the
`catalog_hash`. The generation restarts at `1` if the manifest is absent or
invalid. `catalog_hash` is computed once by `op-blob` from sorted plugin ID and
schema hash pairs. Consumers read the published value; they do not recompute
it. Catalog checks happen in response to arrivals rather than in polling loops.

## Resealing after schema changes

For a catalog-only reseal on the host, use the guarded workflow from the
repository root:

```bash
NO_RESTART=1 ./deploy/reseal-plugins.sh
```

The script:

1. Refuses tracked staged or unstaged changes. Untracked scratch files do not
   block it.
2. Fetches `origin/main` and requires `HEAD` to contain its current tip.
3. Builds release versions of `opblob` and `op-grpc-bridge`.
4. Seals schemas loaded from `DefaultPluginRegistry` into the SHM catalog.
   After sealing succeeds, the sweep retains only successfully sealed IDs. A
   registered plugin without a schema is reported as skipped and its previous
   blob, if any, is removed.
5. Leaves installed binaries and running services unchanged because
   `NO_RESTART=1` is set.

These preflight checks prevent a successful-looking reseal from publishing
schemas built from a dirty or behind-main checkout.

Dynamic gRPC reflection reloads the catalog on the next reflection request.
The D-Bus object tree and frozen typed gRPC routes do not reload dynamically;
for stale-catalog recovery where the installed bridge already matches the
schemas, activate the resealed catalog only after checking bridge status:

```bash
sudo sv status op-grpc-bridge
sudo sv restart op-grpc-bridge
sudo sv status op-grpc-bridge
```

For schema or dispatch-code changes, rebuild and publish the matching binary
through the canonical btrfs golden/live workflow. Suppress automatic restarts
so catalog and binary activation happen together in the final step:

```bash
CXXFLAGS="-include cstdint" cargo build --workspace --release
CXXFLAGS="-include cstdint" NO_RESTART=1 ./deploy/reseal-plugins.sh
sudo deploy/runit/build-golden.sh --dry-run
sudo deploy/runit/build-golden.sh --no-restart
sudo sv status op-grpc-bridge
sudo sv restart op-grpc-bridge
sudo sv status op-grpc-bridge
```

`--no-restart` leaves every affected service running its previous process.
Record the dry run's restart and held-back service lists, then schedule
deliberate restarts for services other than the bridge. The final bridge restart
above loads both the installed binary and the resealed catalog.

The default `reseal-plugins.sh` mode directly replaces two live binaries and
restarts the bridge. It does not update the golden release subvolume, so it is
not a release deployment path.

`--force` skips both repository checks:

```bash
NO_RESTART=1 ./deploy/reseal-plugins.sh --force
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

Common failures:

- `tracked files have uncommitted changes`: commit or stash tracked changes.
- `HEAD does not contain origin/main`: merge or rebase the fetched main branch.
- A plugin is reported as `skipped`: its registry entry returned no schema.
- A removed plugin disappears after resealing: this is expected full-catalog
  sweep behavior, not data loss from another state store.

Do not restore the retired `/dev/shm/opdbus/schemas` directory,
`live-schema.json`, or a separate schema manifest. Do not write blobs directly;
all catalog changes must go through `op-blob` so each blob replacement,
stale-version cleanup, and manifest publication follows the catalog lifecycle.
