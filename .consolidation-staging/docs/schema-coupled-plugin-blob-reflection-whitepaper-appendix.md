# Appendix: Blob Architecture Synthesis

## Definition of "Blob Architecture"

Core idea from direct Codex conversations & referenced by Claude sessions:

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

### Lifecycle

1. Plugin declares `schema()` + state from PluginSchema
2. "blobify" (bridge/projection side) assembles the full blob
3. Persist to /dev/shm; dynamic reflection catalog registers it
4. D-Bus + gRPC surface appears for clients "natively"

Replacement for monolithic `/dev/shm/live-schema.json` or whole projections.

## Concrete LLM Goal

Direct quotes from directive conversations:
- "first need to make sure that gemma is up zeroclaw ollama"
- "read codex conversation about blobs"
- "rewrite the plugin so the blob is complete and we can call it as if it were running native local"
- "would it be beneficial to include the model in the blob?"
- gemma4 is universal router + edges to local ollama inference.
- Inference "handed to the large-language-model plugin"

### Why This Matters for Zeroclaw/Gemma

Give a gemma4+zeroclaw deployable self-contained artifact. A "blob deploy" yields a package that exports full surfaces so consumer calls look 100% local/native without knowing it's a blob package.

## Current State (as of July 2026, operation-dbus-proto)

### Achieved Foundations

- Zeroclaw is registered full StatePlugin:
  - `crates/op-plugins/src/state_plugins/zeroclaw.rs:148`: LLM_PROVIDER=ollama, LLM_MODEL=gemma4 default.
  - Router config: gemma4 classifies EVERYTHING, emits tags/hints.
  - Providers: factory, ollama(gemma/gemma4), openrouter, gemini, antigravity, opencode/cli, etc. + oscal policy.
  - Schema dynamically generated via `schema_from_state` from live ZeroclawState (schema-as-code correct).
- SchemaEngine + /dev/shm projections authoritative, D-Bus objects live from schema only (no hard inline).
- op-grpc-bridge has zeroclaw special path + binary `op-grpc-bridge-zeroclaw`
- Live model pull success: `gemma4:12b` (gguf Q4, 7.5GB, vision+tools+thinking) exists under ollama.
- Ollama: supervised by s6 (root `ollama-srv`), user `ollama`, binary present. Endpoint expected `http://localhost:11434`.

### Not Yet Implemented

- Conceptual `PluginObjectBlob` + `blobify_plugin_schema()` + per-plugin sealed shm files + ActiveReflectionCatalog only exist as **chat sketches and snippets** in history (postcard roundtrip examples).
- No code integrated the binary sealing; reflection still treats some broader/monolithic aspects.
- ZeroclawPlugin `apply_state`/`calculate_diff` are stubs (schema-declared only, no real mutation yet).
- No orchestration that "completes" a blob containing model + config so external D-Bus/gRPC clients view it exactly as native.
- Incus + Xray + ghostbridge container.sock are transport pieces, not yet "blob packaged".
- No explicit "blob as the deployment" path for gemma4; everything still manual/piecemeal install.

### Why Not Implemented Yet as "The Deployment"

1. **Staging priorities** (rollouts coded in .codex/memories):
   - Sled identity + zero-copy shm source-of-truth.
   - Plugin refactor to explicit `schema()` + SchemaEngine projections.
   - Unix socket ownership canonical.
   - Zeroclaw absorbing LLM / op-llm retirement + 3-layer boundary clarification.
   - grpc-bridge-zeroclaw + projection_server deployment (script fixes ongoing).
   - Incus/qdrant shuttle wiring.
   - D-Bus first everywhere, no bypasses.
2. Blob vision surfaced late + conversation heavy. Directives ("read codex about blobs", "gemma up first") came *after* much runtime work.
3. Engineering caution (AGENTS): do not create unintended mutation overhead in hot paths.
4. Deploy fragility observed: symlinked s6 dirs caused cp issues in scripts; release builds heavy, needed single-job builds.
5. "Complete blob" requires solving the packaging + mount semantics + reflection reload (rethink monolithic) — not gated on core interop anymore but follow-on.
6. Model present (good!) but services are normal s6 rather than "blob subvol activated instance."

<!-- Extracted from /mnt/opt-inspect/home/git/operation-dbus-proto/docs/BLOB_ARCHITECTURE_SYNTHESIS.md on 2026-07-20 -->
