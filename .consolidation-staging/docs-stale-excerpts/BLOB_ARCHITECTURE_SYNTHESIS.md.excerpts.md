# Dropped Excerpts from BLOB_ARCHITECTURE_SYNTHESIS.md

## Reason for exclusion

These sections contain BTRFS-specific packaging and deployment implementation details that are
deferred/planned work, not current architecture, and conflict with the `AGENTS.md`
zero-btrfs-overhead identity rule. They describe future tooling and workflows rather than
established patterns.

---

## BTRFS Dimension (excluded as implementation-specific packaging detail)

**BTRFS dimension** (explicit in later Claude directive):

- Blobs as (or wrapped inside) **btrfs subvolumes** for:
  - Complete deployment units ("blob is complete")
  - Mountable as device or bind (read heavy, avoid write mutation loops)
  - Snapshot, send/receive, isolation/hide design
  - "so you can either mount them add as device (i think that may erase, ...so prb mount)"
- Caution per AGENTS.md: "Zero-Btrfs Overhead" for identity/Xray paths. NVMe only for vectorized *footprint transport* (see `op-snowball` two/three subvol design).

---

## Concrete LLM Goal and Deployment Rationale (excluded as deployment planning, not runtime architecture)

**Concrete LLM goal** (direct quotes):

- "first need to make sure that gemma is up zeroclaw ollama"
- "read codex conversation about blobs"
- "rewrite the plugin so the blob is complete and we can call it as if it were running native local"
- "would it be beneficial to include the model in the blob?"
- gemma4 is universal router + edges to local ollama inference.
- Inference "handed to the large-language-model plugin"

**Why "blobs btrfs subvolumes" specially**:

- Give a gemma4+zeroclaw deployable self-contained artifact.
- A "blob deploy" yields a subvol that exports full surfaces so consumer calls look 100% local/native without knowing it's a blob package.

---

## Path to Fully Functional "Gemma4 Blob Deploy" using Zeroclaw/Ollama (excluded as planned implementation steps)

Goal state: `deploy create-zeroclaw-gemma-blob` produces a btrfs subvol containing:

- Projected sealed PluginObjectBlob(s) for zeroclaw (and dependents)
- Ollama config + bind or embedded model gemma4:12b manifest
- Minimal service activation (s6 or tiny unit)
- When activated/mounted the `/org/opdbus/v1/plugins/zeroclaw` (and full projections) appear, routes everything through the local gemma4 router/inference, clients see it as fully native local zeroclaw.

### Immediate practical steps (todo follow-on)

1. Port the sketched `PluginObjectBlob` + serde helpers (postcard + rkyv optional) into a small `op-blob` or inside `op-projection`.
2. Implement `blobify_plugin_schema()` (in grpc-bridge or dedicated crate) taking live schema + desc set.
3. Extend Zeroclaw + its schema to carry `blob_manifest` + `model_blob_ref`.
4. Enhance `deploy/btrfs-layout.sh` + add `deploy/deploy-blob.sh` (or entry in deploy.sh) for named subvol:
   - subvol create "@blob-zeroclaw-gemma4"
   - populate /config + copy projected blobs + symlink model store
   - register in modules or special blobs catalog
5. Add CLI or D-Bus mut in zeroclaw plugin to "materialize-blob" that packages current state + returns shm_path + btrfs root.
6. Make ollama start conditional/resilient under zeroclaw (respect the supplier locator).
7. docs/ + .kiro/spec for zeroclaw-blob-deploy (drive from existing `zeroclaw-absorbs-op-llm` pattern if residual).

Current zeroclaw already points the way (gemma4 local as canonical). Blob is the missing "complete deployment seal" for reliability/isolation/portability.

---

## Closing Recommendation (excluded as action items rather than architecture)

Next action recommendation: run `cargo check -p op-plugins -p op-projection -p op-grpc-bridge` then start `PluginObjectBlob` definition + `blobify` stub + update deploy script + test mount + local gemma4 query through projected surface.
