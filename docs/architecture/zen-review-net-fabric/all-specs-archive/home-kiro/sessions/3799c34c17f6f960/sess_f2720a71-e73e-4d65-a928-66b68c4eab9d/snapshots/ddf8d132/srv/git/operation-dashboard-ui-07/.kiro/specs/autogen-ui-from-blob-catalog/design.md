# Design — Auto-Generated GUI from Blob Catalog

## Overview

Three catalogs and one stream produce the entire GUI. None of them alone is sufficient:

```
Component catalog (catalog.ts)   →  paint vocabulary (what the renderer can draw)
Blob catalog (plugin-blobs/)     →  typed world (what exists, what its fields mean)
StateSync stream                 →  live binding (what the values are right now)
                    ↓
  role → component mapping       →  the missing step (how to translate schema roles
                                    into catalog component names)
                    ↓
         generated Spec           →  { root, elements } admitted by CatalogGuard
                    ↓
         Renderer                 →  draws the page
```

The key design insight: **a Spec is just a named tree of catalog types with $state bindings**. The
generator does not invent UI; it asks "for each field the blob declares, which catalog component
expresses this role?" and emits the corresponding element with its state path.

---

## Layer 1: Role-to-Component Map

File: `src/json-render/catalog/role-map.ts`

This is the sole point of contact between `UiRole` (semantics) and catalog component names
(syntax). It is a plain object, not a class, so it can be imported by both the runtime page
generator and by tests.

```ts
import type { UiRole } from "@/lib/subid-ui"; // mirrors op-state-store/subid_ui.rs

export interface RoleMapping {
  /** Primary catalog component type for this role. */
  component: string;
  /** Props to merge in (static, not state-bound). */
  staticProps?: Record<string, unknown>;
  /** When true, the mapped element needs a `repeat` block. */
  useRepeat?: boolean;
  /** When true, the role produces no rendered element and is silently omitted. */
  omit?: boolean;
}

export const ROLE_MAP: Record<UiRole, RoleMapping> = {
  "surface":             { omit: true },        // becomes a route, not an element
  "display-value":       { component: "stateValue" },
  "state-flag":          { component: "statusDot" },
  "collection-view":     { component: "streamObject", useRepeat: true },
  "record-view":         { component: "card" },
  "value-list":          { component: "container", useRepeat: true },
  "binary-control":      { component: "pill", staticProps: { tone: null } },   // read-only
  "text-control":        { component: "stateValue" },                           // read-only
  "numeric-control":     { component: "statCard", staticProps: { sub: null, variant: null, tone: null } },
  "multi-choice":        { component: "stateValue", staticProps: { format: "raw" } },
  "editable-collection": { component: "streamObject", useRepeat: true },
  "record-editor":       { component: "card" },                                 // read-only
  "structured-control":  { component: "card" },                                 // read-only
  "validation-carrier":  { omit: true },
  "hydration-source":    { omit: true },
  "trigger-binding":     { omit: true },
  "repeat-binding":      { omit: true },
};
```

No other file may contain a string like `"stateValue"` derived from a `UiRole`. All other files
import `ROLE_MAP`.

---

## Layer 2: Plugin Page Spec Generator

File: `src/json-render/spec-gen/generate-plugin-page.ts`

```ts
import type { Spec } from "@json-render/core";
import type { UiSubidProjection } from "@/lib/subid-ui";
import { ROLE_MAP } from "@/json-render/catalog/role-map";

/**
 * Generate a json-render Spec for a plugin page.
 *
 * The spec binds every projected field to /plugins/<pluginId>/<fieldName>
 * via $state. Only fields whose role has a non-omit mapping appear in the
 * spec. The outer container is a card with the plugin name as title.
 *
 * The spec is flat { root, elements }. Children are element id strings.
 * CatalogGuard validates the result before it is ever rendered.
 */
export function generatePluginPageSpec(
  pluginId: string,
  displayName: string,
  projections: UiSubidProjection[],
): Spec;
```

### Element ID convention

Element IDs are deterministic: `<pluginId>--<fieldId>--<role>`. This means:
- Two runs on the same schema produce identical specs (important for signature dedup in the gallery)
- CatalogGuard can reference elements by ID in error messages

### State path convention

All live bindings use the uiStore path convention:
- Single field: `{ $state: "/plugins/<pluginId>/<fieldName>" }`
- Collection member (repeat item): `{ $item: "<fieldName>" }`

### Generated spec shape (example: `tched_router` plugin)

```json
{
  "root": "tched_router--page",
  "elements": {
    "tched_router--page": {
      "type": "card",
      "props": { "title": "Tched Router", "subtitle": null, "tone": null, "className": null },
      "children": [
        "tched_router--selected_provider--display-value",
        "tched_router--selected_model--display-value",
        "tched_router--providers--collection-view"
      ]
    },
    "tched_router--selected_provider--display-value": {
      "type": "stateValue",
      "props": {
        "path": "/plugins/tched_router/selected_provider",
        "label": "selected provider",
        "format": null
      }
    },
    "tched_router--selected_model--display-value": {
      "type": "stateValue",
      "props": {
        "path": "/plugins/tched_router/selected_model",
        "label": "selected model",
        "format": null
      }
    },
    "tched_router--providers--collection-view": {
      "type": "streamObject",
      "props": {
        "pluginId": "tched_router",
        "member": "providers",
        "className": null
      }
    }
  }
}
```

### Multi-surface plugins

When `ui_surfaces.is_authoritative()` is true, the generator produces a spec per surface, scoped
to the fields named in `surface.schema`. The root for each surface is
`<pluginId>--<surfacePath.replace('/', '-')}--page`.

---

## Layer 3: Dynamic Page Spec Store

File: `src/json-render/spec-gen/use-plugin-page-spec.ts`

```ts
/**
 * Returns the Spec for a route.
 *
 * Lookup order:
 * 1. Static PAGE_SPECS (chat, overview, catalog, gallery, generate, plugins, network)
 * 2. Promoted spec in the gallery store for (route + schemaHash)
 * 3. Freshly generated spec from the blob + role map
 * 4. emptyState spec if no blob data is available for the route
 *
 * The generated spec is memoized by (route, schemaHash). A new schemaHash
 * invalidates the memo.
 */
export function usePluginPageSpec(route: string): Spec | null;
```

The hook reads `useEventStore` for:
- `schemas` — to find which plugin owns this route
- `schemaHashes` — the cache key for memoization
- `latestState` — not used for spec generation (generation is schema-only)

The hook reads `useBlobCatalog` for:
- `plugins` — the authoritative list of sealed plugin IDs

---

## Layer 4: Dynamic Nav Assembly

File: `src/json-render/navigation/use-dynamic-nav.ts`

```ts
/**
 * Returns NAV_MANIFEST enriched with entries derived from blob ui_surfaces.
 *
 * Plugin entries are added to a synthetic "Plugins" section (or per-category
 * sections if the plugin declares x-oscal-category). Static entries are
 * unchanged.
 *
 * This list is passed to the shell spec builder, not directly to nav components.
 */
export function useDynamicNav(): NavItem[];
```

### Shell spec assembly

`shellSpec.ts` is refactored to be a function `buildShellSpec(navItems: NavItem[]): Spec` rather
than a module-level constant. The `PageSpecOutlet` passes the dynamic nav to it.

This function is called once when the blobCatalog resolves and re-called when `catalogHash` changes.
The resulting spec is stored in React state and passed to the shell `Renderer`.

---

## Layer 5: op-gallery-gen — Plugin Page Generation Mode

The `GenerationContext` in `context.rs` gains a new optional field:

```rust
pub struct GenerationContext {
    // … existing fields …

    /// When set, generation is scoped to this plugin.
    /// The context includes only this plugin's schema and projections.
    pub target_plugin: Option<TargetPluginContext>,
}

pub struct TargetPluginContext {
    pub plugin_id: String,
    pub projections: Vec<UiSubidProjection>,   // from project_schema_ui
    pub ui_surfaces: UiSurfaceProjection,
}
```

The system message for a plugin-scoped run prepends the role-to-component table from REQ-1.1
(copied as a markdown table into the context). The user message is:

> "Generate a complete page spec for plugin `<plugin_id>`. Use only catalog components.
> Bind all `display-value` fields to `/plugins/<plugin_id>/<fieldName>` via `$state`.
> Every `collection-view` field must use `streamObject`. Omit roles marked omit in the
> role table above."

The JSONL patch output is assembled by the existing `spec_stream::assemble` and validated by the
existing `CatalogGuard`. Nothing in the Rust pipeline changes for this mode.

---

## Data flow summary

```
useBlobCatalog
    │  (GET /api/ui-model/plugins + StateSync catalogHash)
    ▼
plugin IDs (authoritative, merged HTTP + stream)
    │
    ▼  GET /api/ui-model/plugin-schema/:id
PluginSchema + ui_projection (UiSubidProjection[]) + schemaHash
    │
    ▼  generatePluginPageSpec(pluginId, displayName, projections)
Spec { root, elements } — all $state paths at /plugins/<id>/…
    │
    ▼  CatalogGuard.validate(spec)  (client-side replica for fast feedback)
validated Spec  →  Renderer
    │
    ▼  StateSync stream updates /plugins/<id>/* in uiStore
live values appear in rendered elements
```

---

## Files to create

| File | Purpose |
|---|---|
| `src/json-render/catalog/role-map.ts` | UiRole → catalog component mapping |
| `src/json-render/spec-gen/generate-plugin-page.ts` | Spec generator from blob projections |
| `src/json-render/spec-gen/use-plugin-page-spec.ts` | Dynamic page spec hook with memoization |
| `src/json-render/navigation/use-dynamic-nav.ts` | Nav enriched with blob ui_surfaces |
| `src/lib/subid-ui.ts` | TypeScript mirror of op-state-store/subid_ui.rs UiRole types |

## Files to modify

| File | Change |
|---|---|
| `src/json-render/shell/shellSpec.ts` | Refactor from constant to `buildShellSpec(navItems)` |
| `src/json-render/pages/index.ts` | Replace static `PAGE_SPECS` map with dynamic lookup |
| `src/json-render/runtime/PageSpecOutlet.tsx` | Use `usePluginPageSpec` instead of `pageSpecFor` |
| `src/json-render/navigation/manifest.ts` | No change to SECTION_ORDER or NAV_MANIFEST; used by `useDynamicNav` |
| `src/json-render/catalog/catalog.ts` | No new components; `STREAM_PLUGIN_IDS` kept in sync |
| `src/json-render/catalog/stream-plugins.ts` | Add any newly sealed plugins |

## Files unchanged (deliberately)

| File | Why unchanged |
|---|---|
| `src/json-render/catalog/catalog.ts` | The catalog is the contract; generated specs must fit it |
| `src/json-render/pages/chat.pagespec.ts` | Hand-written specs take priority over generated |
| `src/json-render/pages/overview.pagespec.ts` | Same |
| `src/json-render/shell/ShellRenderer.tsx` | Shell rendering is already correct |
| `op-gallery-gen/src/spec_stream.rs` | JSONL patch assembler is unchanged |
| `op-gallery-gen/src/catalog_guard.rs` | Admission gate is unchanged |

---

## Client-side CatalogGuard (TypeScript)

To give fast feedback before shipping a generated spec to the Renderer, a lightweight TypeScript
`validateSpec(spec, catalogSchema)` function is added in:

`src/json-render/spec-gen/validate-spec.ts`

It reads `catalog.schema.json` (already available at `schemas/json-render/`) and checks:
- Every element `type` is in the catalog
- Every required prop is present
- Every `$state` path starts with `/`

This is not a replacement for the Rust CatalogGuard (which is authoritative for gallery admission);
it is a client-side fast path for the dynamic page generator.

---

## Antigravity plugin — concrete wiring

`antigravity` declares `ui_surfaces` with four routes. The generator for `/antigravity`:

1. Reads `UiSubidProjection[]` for `antigravity` from `GET /api/ui-model/plugin-schema/antigravity`
2. Filters to `surface`, `display-value`, `state-flag` roles (the index page)
3. Generates a spec with:
   - `card` root titled "Antigravity"
   - `stateValue` for each `display-value` field (e.g. `selected_provider`, `selected_model`,
     `provider_route`)
   - `statusDot` for `obs` boolean fields (e.g. `vertex_auth.enabled`)
   - Row of `navItem`-like elements (rendered as `badge` or `text`) linking to the four sub-surfaces

For `/antigravity/chat`, the static `chatSpec` takes priority (REQ-5.2). The dynamic generator is
never called for that route.

---

## Chat stream binding (distinct from StateSync)

The antigravity chat is a second stream (`op_chat.chat.ChatService.Send`), not StateSync. Tokens
land in the uiStore at `/streams/<streamId>/messages`. The `chatSpec` uses this path directly:

```json
{
  "type": "container",
  "repeat": { "statePath": "/streams/stream_antigravity_01/messages", "key": "id" },
  "children": ["chat-message"]
}
```

The dynamic page generator MUST NOT attempt to generate a chat spec from blob projections. Chat
is the exception: it requires its own catalog types (`antigravityChatContainer`, `chatMessage`)
and its own stream path. The static spec handles it entirely.

---

## Error states

| Condition | Rendered output |
|---|---|
| Identity not established | `healthPill` in topbar shows "offline"; contentRegion shows connecting state |
| Plugin present in blob but not in STREAM_PLUGIN_IDS | Spec for that plugin falls back to raw `streamObject` for the whole plugin |
| Generated spec fails CatalogGuard | `emptyState` with error message + "retry" action |
| Route matches no plugin surface | `emptyState` with "no spec for this route" |
| schemaHash changed during render | stale spec is replaced; loading indicator during re-generation |
