# Blob Architecture: Full History Synthesis + Deployment Analysis (2026)

Gathered from:
- ~/.codex/history.jsonl + /memories/rollout_* + raw_memories.md (detailed threads)
- ~/.claude/history.jsonl (explicit directive prompts tying gemma/zeroclaw/ollama to newly mentioned "blob architecture")
- ~/.factory/history.json (zeroclaw missions, refactors)
- Project source (AGENTS.md, crates/*, deploy/*, docs/*)
- Incidental zeroed in clean/archive notebooks

## Definition of "Blob Architecture" (user+agent co-evolution)

Core idea from direct Codex convos & referenced by Claude sessions:

> "is there a way to make schema like blobs so a proto could be a blob ?"
> "can the dbus object + reflection proto be a blob ?"
> "we are going to be blobbing everything. the sled, compliance blobs"
> "no more monolithic live schema ... individual blobs"
> "blob as sealed projection artifact" + "shm_path with hash in filename" + "ActiveReflectionCatalog tied to blob lifetime"
> "blobify_plugin_schema in bridge, not plugin"

**Blob** = self-contained, sealed, schema-derived, zero-copy/loadable runtime artifact that carries:

- schema_json + schema_hash (authoritative contract)
- dbus_identity + path
- grpc_file_descriptor_set (for dynamic reflection)
- blob_method_manifest + richer metadata (footprint policy, subids, OSCAL etc)
- (later) state snapshots, model pointers, providers config

Serialization target: rkyv (ideal zero-copy) / postcard (light) or bincode. Written to /dev/shm/... .hash.blob (mmap friendly).

**Lifecycle**:
1. Plugin declares `schema()` + state from PluginSchema
2.  "blobify" (bridge/projection side) assembles the full blob
3. Persist to /dev/shm; dynamic reflection catalog registers it
4. D-Bus + gRPC surface appears for clients "natively"

Replacement for monolithic `/dev/shm/live-schema.json` or whole projections.

**BTRFS dimension** (explicit in later Claude directive):

- Blobs as (or wrapped inside) **btrfs subvolumes** for:
  - Complete deployment units ("blob is complete")
  - Mountable as device or bind (read heavy, avoid write mutation loops)
  - Snapshot, send/receive, isolation/hide design
  - "so you can either mount them add as device (i think that may erase, ...so prb mount)"
- Caution per AGENTS.md: "Zero-Btrfs Overhead" for identity/Xray paths. NVMe only for vectorized *footprint transport* (see `op-snowball` two/three subvol design).

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

## Current State (as of July 2026, operation-dbus-proto)

### Achieved foundations (good progress)
- Zeroclaw is registered full StatePlugin:
  - `crates/op-plugins/src/state_plugins/zeroclaw.rs:148` : LLM_PROVIDER=ollama, LLM_MODEL=gemma4 default.
  - Router config: gemma4 classifies EVERYTHING, emits tags/hints.
  - Providers: factory, ollama(gemma/gemma4), openrouter, gemini, antigravity, opencode/cli, etc. + oscal policy.
  - Schema dynamically generated via `schema_from_state` from live ZeroclawState (schema-as-code correct).
- SchemaEngine + /dev/shm projections authoritative, D-Bus objects live from schema only (no hard inline).
- op-grpc-bridge has zeroclaw special path + binary `op-grpc-bridge-zeroclaw`
- BTRFS primitives used: `op-snowball` implements timed/vector/state subvolumes.
- deploy/btrfs-layout.sh + module subvols (agents, mcp etc) + s6 services.
- Live model pull success: `gemma4:12b` (gguf Q4, 7.5GB, vision+tools+thinking) exists under ollama.
- Ollama: supervised by s6 (root `ollama-srv`), user `ollama`, binary present. Endpoint expected `http://localhost:11434`.

### Not yet = full "Blob deployment"
- Conceptual `PluginObjectBlob` + `blobify_plugin_schema()` + per-plugin sealed shm files + ActiveReflectionCatalog only exist as **chat sketches and snippets** in history (postcard roundtrip examples).
- No code integrated the binary sealing; reflection still treats some broader/monolithic aspects.
- ZeroclawPlugin `apply_state`/`calculate_diff` are stubs (schema-declared only, no real mutation yet).
- No btrfs-backed "zeroclaw-gemma-blob" creation or lifecycle (deploy.sh not aware of blob units).
- No orchestration that "completes" a blob containing model + config in subvol so external D-Bus/gRPC clients view it exactly as native.
- Incus + Xray + ghostbridge container.sock are transport pieces, not yet "blob packaged".
- BTRFS deploy tooling for complete *runnable* artifacts is partial (modules not the sealed reflection+LLM blobs).
- No explicit "blob as the deployment" path for gemma4 ; everything still manual/piecemeal install.

### WHY not implemented yet as "the deployment"
1. **Staging priorities** (rollouts coded in .codex/memories):
   - Sled identity + zero-copy shm source-of-truth.
   - Plugin refactor to explicit `schema()` + SchemaEngine projections.
   - Unix socket ownership canonical.
   - Zeroclaw absorbing LLM / op-llm retirement + 3-layer boundary clarification.
   - grpc-bridge-zeroclaw + projection_server deployment (script fixes ongoing).
   - Incus/qdrant shuttle wiring.
   - D-Bus first everywhere, no bypasses.
2. Blob vision surfaced late + conversation heavy. Directives ("read codex about blobs", "gemma up first") came *after* much runtime work.
3. Engineering caution (AGENTS): do not create unintended Btrfs mutation overhead in hot paths. Subvols proven only in snowball footprint first.
4. Deploy fragility observed: symlinked s6 dirs caused cp issues in scripts; release builds heavy, needed single-job builds.
5. "Complete blob" requires solving the packaging + mount semantics + reflection reload (rethink monolithic) — not gated on core interop anymore but follow-on.
6. Model present (good!) but services are normal s6 rather than "blob subvol activated instance."

## Path to Fully Functional "Gemma4 Blob Deploy" using Zeroclaw/Ollama

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

This analysis from exhaustive reading of ~2.8M lines of agent chat histories + source. Ready to code the missing blob pieces.

Next action recommendation: run `cargo check -p op-plugins -p op-projection -p op-grpc-bridge` then start `PluginObjectBlob` definition + `blobify` stub + update deploy script + test mount + local gemma4 query through projected surface.
