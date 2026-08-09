# Spec — Model-Agnostic Generative UI Gallery

## Summary

Replace the hardcoded `op-gemma` gallery generators with a model-agnostic inference loop driven through the Antigravity chat interface. Any loaded inference model — local or remote, any provider — reads the sealed plugin blob catalog, the json-render.dev documentation, and a single universal prompt, then generates json-render specs rendered by the existing DSL interpreter. The operator interacts through the Antigravity chat UI to provide additional context per run. Each generation run is stateless — memory resets completely between runs.

## Problem

The current `ui_gallery.rs` in `op-gemma` contains 40 hand-coded Rust generator functions that produce canned specs seeded with schema data. This:

- Ties the gallery to a specific model (`gemma_brain`) despite the system being model-agnostic
- Produces predictable, stereotyped output rather than novel lenses over the data
- Cannot incorporate operator guidance or additional context
- Has no data discovery layer — each generator only sees the schemas it was coded to use
- Cannot cross-reference plugins or discover relationships between schema slices

## Solution

A standalone generation harness with three progressive data tiers, an Antigravity chat interface for operator interaction, and strict run isolation:

**Baseline (always present):**
- Sealed plugin blobs (`/dev/shm/opdbus/plugin-blobs/`) — the full `PluginSchema` JSON for all 64 plugins
- Instructions: how to access the blob data (read API, section layout, field meanings)
- json-render.dev documentation (component catalog, spec grammar, action vocabulary)
- The prompt: "make this dataset as accessible to as many people, industries, causes as possible"

**Optional tier 1 — MCP as unified data source:**
- Operator toggles on MCP integration
- Exposes cross-blob discovery through MCP tool calls — the model can query across plugins, find relationships, search by field type or subid category
- Enables the model to find schema slices it wouldn't otherwise combine

**Optional tier 2 — Qdrant semantic search:**
- Operator toggles on Qdrant
- Vectorized blob views land in domain framings (privacy-ops, network-eng, compliance, accessibility)
- The model gets semantic context: "these three plugins together serve a privacy workflow" rather than raw schema alone

**Chat interface:**
- Antigravity chat UI is the operator surface
- Operator can provide additional guidance before or during generation ("focus on network observability", "make something for a compliance auditor", "cross the WireGuard and OSCAL plugins")
- Each run is fully siloed — no persistent memory, no carryover between sessions

## Output

The generation loop produces json-render.dev specs conforming to the existing `SpecContract`:
```json
{
  "root": "<element-id>",
  "elements": {
    "<id>": { "type": "<catalog-component>", "props": {...}, "children": [...] }
  }
}
```

Specs are admitted to the 200-slot rolling gallery through `CatalogStore::admit()` with the existing tier/versioning/retirement system unchanged.

## Scope

**In scope:**
- Generation harness replacing `op-gemma/src/ui_gallery.rs` generator functions
- Antigravity chat integration as the operator interface
- Three-tier data access (blobs baseline, +MCP, +Qdrant)
- Run isolation (stateless per session)
- Model-agnostic inference (any provider/model via ZeroClaw routing)

**Out of scope:**
- Changing the DSL interpreter (`interpret.rs`)
- Changing the catalog store invariants (200 slots, stable core, immutable versioning)
- Changing the sealed blob format or `PluginSchema` structure
- Persistent memory across runs (explicitly excluded)
- Specific model selection logic (that's ZeroClaw's domain)

## Acceptance Criteria

1. Gallery generation works with any inference model routed through ZeroClaw — no gemma-specific code paths
2. Baseline mode produces valid specs using only blobs + json-render docs + the universal prompt
3. MCP toggle adds cross-blob discovery without breaking baseline
4. Qdrant toggle adds semantic context without breaking baseline or MCP
5. Antigravity chat UI accepts operator guidance and passes it to the generation context
6. Each run starts with zero memory — no state leaks between sessions
7. Generated specs pass the existing `SpecContract` validation and render without `RenderError` in the interpreter
8. The stable-core 40 elements are never displaced by generated novelty specs
