# Requirements — Model-Agnostic Generative UI Gallery

## Architecture Decisions

### AD-1: Follow Vercel json-render spec format
The spec format is `{ root, elements }` with components, props, children, actions,
visibility, state bindings (`$state`, `$cond`, `$bindState`), and watchers — matching
upstream json-render as closely as possible. The spec is pure JSON, no TypeScript.

### AD-2: No TypeScript
The renderer is Rust/egui (`interpret.rs`). The catalog is defined in Rust.
No Node.js, no React, no JS framework. TypeScript is not used anywhere in the
render pipeline. The LLM generates pure JSON specs; Rust interprets them.

### AD-3: D-Bus first for state
The state model is the D-Bus projected tree. `$state` paths resolve against
D-Bus object properties. Live updates come via `PropertiesChanged` signals.
Plugin state, procfs values, child objects — all addressable as D-Bus properties
mapped to JSON pointer paths.

### AD-4: Components are singular atomic units
Each catalog component does exactly one thing. No compound widgets.
Label, pill, button, gauge, repeat, stack — each is one line, one binding.
The LLM composes pages from these atoms.

### AD-5: LLM is the autonomous compositor
The LLM observes the full picture (all components + all plugin state) and
assembles the UI itself. No user intervention for the primary flow. On runtime
state changes, the LLM adapts. Gallery = bench for unused components. Catalog =
active rendered UI.

### AD-6: Catalog vs Gallery
- **Catalog** = the active rendered UI, composed by the LLM
- **Gallery** = bench of unused/new components with a promote button
- Components move between catalog (active) and gallery (bench)
- LLM can retire unused components from catalog → gallery
- Operator can promote from gallery → catalog

---

## Functional Requirements

### FR-1: Atomic component separation
Each component in the catalog is a singular unit with one responsibility.
Components are separated from schema into individual items (Gemini code exists
for this). Each renders one datum or one layout primitive.

### FR-2: json-render spec compatibility
Generated specs must follow the Vercel json-render format:
```json
{
  "root": "<element-id>",
  "elements": {
    "<id>": {
      "type": "<CatalogComponent>",
      "props": { ... },
      "children": ["<child-id>", ...],
      "visible": [...],
      "watch": { ... }
    }
  }
}
```

### FR-3: $state expression system
Props can bind to live state via expressions:
- `{ "$state": "/path/to/value" }` — read from D-Bus state
- `{ "$cond": <condition>, "$then": <value>, "$else": <value> }` — conditional
- `{ "$template": "Memory: ${/procfs/memory/MemTotal} kB" }` — interpolation
- `{ "$bindState": "/path" }` — two-way binding

State paths resolve against the D-Bus projected tree.

### FR-4: Streamable individual values
Every plugin field is individually addressable and streamable. For procfs,
each value (MemTotal, MemFree, loadavg, uptime) is a separate bindable datum
that can land in whatever component the LLM places it in. D-Bus
`PropertiesChanged` signals deliver live updates.

### FR-5: Autonomous LLM composition
The LLM:
- Sees all available atomic components (the catalog definition)
- Sees all live plugin state (the D-Bus tree / sealed blobs)
- Composes pages/views from components + state bindings
- Adapts on significant runtime state changes
- Can retire unused components to gallery
- Runs without operator trigger

### FR-6: Gallery as component bench
Unused/new components live in the gallery (last menu item). Each has a
"promote" button that sends it to the active catalog. The LLM-generated novelty
specs land here first before being promoted to active rendering.

### FR-7: Repeat kind for collections
The `repeat` kind (already committed, `d8f2b3e1`) iterates over arrays.
Child collections aggregate at seal/mutation time into the parent state via
the `DispatchOutcome.signal` → `persist_*_mutation` →
`publish_plugin_projection_from_cache` path.

### FR-8: Slash-agnostic bind paths
Bind paths work with or without a leading `/`. The interpreter normalizes
them. This is critical for LLM generation quality — the model never trips
on JSON pointer syntax.

---

## Non-Functional Requirements

### NFR-1: No per-plugin UI code
One spec works for all plugins. The only plugin-specific work is defining
the state struct. Everything else — components, bindings, rendering — is generic.

### NFR-2: Spec validation at admission
Generated specs must pass structural validation before admission:
- Root exists in elements map
- All component types exist in catalog
- All children resolve (no dangling refs)
- No cycles in the tree
- Bind paths are syntactically valid

### NFR-3: 200-slot gallery limit
The gallery holds up to 200 novelty specs. Stable-core elements (~40) are
never displaced. FIFO retirement for oldest novelty when full.

---

## Research Findings (2026-08-19)

### Vercel json-render (upstream)
- Framework-agnostic: React, Vue, Svelte, Solid, React Native, Ink (terminal),
  Next.js, PDF, email, video, 3D — all from the same spec format
- Catalog = Zod-typed component definitions with prop schemas
- Renderer walks the spec tree, maps types to implementations
- `$state`/`$cond`/`$template`/`$bindState` expression system for dynamic props
- `visible` for conditional rendering
- `watch` for state-reactive actions
- `createSpecStreamCompiler` for progressive rendering during LLM generation
- Actions system including built-in `setState`

### Google A2UI
- Protocol-level: agent sends declarative component descriptions over JSONL
- Client renders with native widgets (React, Flutter, whatever)
- Agent never executes anything on client — describes intent only
- More transport-focused, less renderer-focused

### Key insight: Constrained > Unconstrained generation
- Constrained: AI selects from registered components (our approach)
- Unconstrained: AI generates raw HTML/CSS/JS (Google Stitch, Claude Artifacts)
- Constrained is safer, more consistent, composable
- The catalog IS the guardrail — the LLM can only use what's registered

### Applicable to our Rust/egui/D-Bus stack
- The spec format is pure JSON — renderer-independent
- Our `interpret.rs` IS a renderer, same role as `@json-render/react`
- The gap: adding `$state`/`$cond`/`watch` expression resolution to the interpreter
- D-Bus `PropertiesChanged` signals = the live state update transport
- No TypeScript needed anywhere — Rust catalog, Rust renderer, JSON specs, D-Bus state
