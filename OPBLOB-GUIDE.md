# OPBLOB01 — How to read the OP-DBUS sealed blobs

This is a short, copy-paste guide so **anyone, any organization, or any tool**
can access the OP-DBUS plugin data with the least possible setup. No Rust build,
no running daemon, no special privileges required beyond reading a file.

## What the data is

Every plugin (network, identity, chat, blockchain, …) is published as one
**sealed blob** in shared memory:

```
/dev/shm/opdbus/plugin-blobs/<plugin_id>.<hash16>.blob
```

The catalog also has `.manifest.json` with `catalog_hash`, `generation`, and the
list of `plugins`.

The blob is **self-describing**: it carries the schema JSON, the D-Bus identity,
the gRPC identity, and the protobuf descriptors needed to talk to the plugin —
all in one file. Read the blob, you have the whole plugin contract.

## The on-disk byte format (OPBLOB01)

```
offset  size  content
0       8     magic  "OPBLOB01"
8       2     format version (u16 LE) = 1
10      2     section count (u16 LE)
12      4     reserved
16      32    sha256 of the schema-json section (the schema hash)
48      16    reserved
64      24*n  section table: tag u32, reserved u32, offset u64, len u64
...           8-byte-aligned section payloads
```

Sections by `tag`:
- `1` = canonical `PluginSchema` JSON  ← the part you almost always want
- `2` = `BlobManifest` JSON
- `3` = protobuf `FileDescriptorSet`
- `4` = compliance / extra metadata JSON

The `<hash16>` in the filename is the first 16 hex chars of the tag-1 sha256.

## Parse it without Rust (Python example)

```python
import glob, struct, json

def read_plugin(plugin_id, dir="/dev/shm/opdbus/plugin-blobs"):
    path = sorted(glob.glob(f"{dir}/{plugin_id}.*.blob"))[0]
    b = open(path, "rb").read()
    assert b[:8] == b"OPBLOB01", "not an OPBLOB01 blob"
    version     = struct.unpack("<H", b[8:10])[0]
    n_sections  = struct.unpack("<H", b[10:12])[0]
    schema_hash = b[16:48].hex()
    off = 64
    for _ in range(n_sections):
        tag, _res, poff, plen = struct.unpack("<IIQQ", b[off:off+24])
        off += 24
        payload = b[poff: poff + plen]
        if tag == 1:                      # schema JSON
            return {"version": version, "schema_hash": schema_hash,
                    "schema": json.loads(payload)}
    raise KeyError("no schema section")

# Example: read the OpenFlow network plugin
of = read_plugin("openflow")
print(of["schema_hash"])
print(json.dumps(of["schema"], indent=2))
```

That is the entire contract. Point `dir` at the folder and loop over every
`*.blob` to load all 64 plugins as a single dataset.

## Using the Rust API (if you already have the workspace)

You do not need to parse bytes by hand — the crate hands you the decoded schema:

```rust
use op_blob::catalog::read_plugin_schema_shm;

let schema = read_plugin_schema_shm("openflow");   // Option<PluginSchema>
```

For all plugins at once, read the manifest ids and loop:

```rust
use op_blob::catalog::{read_manifest_plugin_ids_shm, read_plugin_schema_shm};

for id in read_manifest_plugin_ids_shm().unwrap() {
    let _ = read_plugin_schema_shm(&id);
}
```

For change detection, read the catalog hash and watch `generation` — never
re-hash the blobs yourself.

## Making the data widely usable

- **Human-readable export:** run the parser above and write one JSON file per
  plugin, or one combined `all-plugins.json`. Hand that to docs, audits, or a UI.
- **Cross-plugin connections:** each schema declares what it `requires`; load
  all 64 schemas and build a graph of those edges to see how plugins depend on
  each other.
- **UI rendering:** the `PluginSchema` fields (name, type, enum, default,
  required) are exactly what a form/field renderer consumes — drive your UI
  straight from the decoded schema.
- **Model access:** give a model the combined decoded schemas (not the raw
  bytes). One consolidated JSON is enough for it to answer questions about any
  plugin and find cross-connections.

## Companion droid

The `json-renderer` droid (`.factory/droids/json-renderer.md`) is the
batteries-included assistant for producing PluginSchema-conformant JSON from
these blobs. Load it when you want help rendering, validating, or exporting the
sealed-blob data for any consumer (D-Bus, gRPC, MCP, UI, or a document).

## Rules to respect

- The blob is the source of truth for present state. Do not edit blobs by hand;
  publish through the D-Bus control plane.
- Containers and external callers reach plugins only over Unix domain sockets /
  gRPC (TLS) — IP/port ACLs are not the access model.
- Treat the schema hash as immutable identity: a changed schema is a new blob,
  not an in-place edit.
