# Tasks — Auto-Generated GUI from Blob Catalog

## Phase 1: Foundation — role-map and TypeScript types

- [ ] **1.1** Create `src/lib/subid-ui.ts` — TypeScript mirror of `op-state-store/src/subid_ui.rs`.
  Export `UiRole` as a string union type (all 17 role strings from `UiRole::as_str()`). Export
  `UiSubidProjection` interface matching the Rust struct. No logic — types only.

- [ ] **1.2** Create `src/json-render/catalog/role-map.ts` — the `ROLE_MAP` object mapping every
  `UiRole` to a `RoleMapping` (component name, optional staticProps, useRepeat flag, omit flag).
  See design.md Layer 1 for the full table. Export `ROLE_MAP` and `RoleMapping`.

- [ ] **1.3** Write unit tests for `role-map.ts` in `src/test/role-map.test.ts`:
  - Every UiRole has an entry in ROLE_MAP
  - Omitted roles have `omit: true` and no component
  - Component names in ROLE_MAP are a subset of `CATALOG_COMPONENTS` (imported from catalog.ts)

---

## Phase 2: Spec generator

- [ ] **2.1** Create `src/json-render/spec-gen/generate-plugin-page.ts`.
  Implement `generatePluginPageSpec(pluginId, displayName, projections)` per the design.
  - Produces a flat `{ root, elements }` spec
  - Element IDs use the `<pluginId>--<fieldId>--<role>` convention
  - All `$state` paths start with `/plugins/<pluginId>/`
  - Uses `ROLE_MAP` exclusively for component name resolution
  - Handles multi-surface plugins: when `ui_surfaces.is_authoritative()` accepts a surface list and
    scopes the generated elements

- [ ] **2.2** Create `src/json-render/spec-gen/validate-spec.ts`.
  Client-side fast validator: checks `type` against catalog component names, required props present,
  `$state` paths start with `/`. Reads catalog component names from `CATALOG_COMPONENTS`. Returns
  `{ valid: boolean; errors: string[] }`.

- [ ] **2.3** Write unit tests in `src/test/generate-plugin-page.test.ts`:
  - Given a minimal `UiSubidProjection[]` with known roles, the output spec has the expected
    elements and type values
  - All generated element types are in `CATALOG_COMPONENTS`
  - All `$state` paths start with `/plugins/`
  - validate-spec returns valid for the generated output
  - A projection with only omitted roles produces a spec with just the outer card (no crash)

---

## Phase 3: Dynamic page spec hook

- [ ] **3.1** Create `src/json-render/spec-gen/use-plugin-page-spec.ts`.
  Hook `usePluginPageSpec(route)` that:
  1. Returns the static spec from `PAGE_SPECS` if the route is in the static map
  2. Otherwise reads `useEventStore` schemas/schemaHashes and `useBlobCatalog` to find the plugin
     that owns the route via its `ui_surfaces`
  3. Calls `generatePluginPageSpec` and memoizes by `(route, schemaHash)`
  4. Returns an `emptyState` spec if no plugin claims the route

  The hook calls `GET /api/ui-model/plugin-schema/:plugin` to get `UiSubidProjection[]`. This
  endpoint already returns `ui_projection` — use it.

- [ ] **3.2** Modify `src/json-render/runtime/PageSpecOutlet.tsx`.
  Replace `pageSpecFor(route)` with `usePluginPageSpec(route)`. Wrap in a loading state for the
  async schema fetch. The shell chrome stays visible; only the content region shows loading.

---

## Phase 4: Dynamic nav

- [ ] **4.1** Create `src/json-render/navigation/use-dynamic-nav.ts`.
  Hook `useDynamicNav()` that:
  1. Starts with `NAV_MANIFEST`
  2. Reads `useBlobCatalog` plugins list
  3. For each plugin, calls `GET /api/ui-model/plugin-schema/:plugin` to get `ui_surfaces`
  4. For each authoritative surface route, adds a `NavItem` to a "Plugins" section (or the
     plugin's `x-oscal-category` as the section name if available)
  5. Returns the merged list

  Deduplicates: if a surface route already exists in NAV_MANIFEST (e.g. `/antigravity/chat`),
  the static entry wins.

- [ ] **4.2** Modify `src/json-render/shell/shellSpec.ts`.
  Refactor from a module-level `const shellSpec: Spec` to `export function buildShellSpec(navItems: NavItem[]): Spec`.
  The function signature is the only change; the generated spec structure stays the same except
  the nav items are driven by the parameter.
  Keep the existing `shellSpec` export as `buildShellSpec(NAV_MANIFEST)` for backward compatibility
  and for tests.

- [ ] **4.3** Modify `src/json-render/runtime/JsonRenderProvider.tsx` (or the component that
  renders the shell spec) to call `buildShellSpec(useDynamicNav())` and re-build the shell spec
  when `catalogHash` changes.

---

## Phase 5: Antigravity wiring

- [ ] **5.1** Verify `antigravity` is in `STREAM_PLUGIN_IDS` (it is — this is a verification
  task, not implementation). Run `npm test src/test/stream-plugins.test.ts` (or write it if
  absent) to confirm the catalog type `antigravity` is registered.

- [ ] **5.2** Confirm `/antigravity/chat` static spec takes priority in `usePluginPageSpec`.
  Write a test: given a route `/antigravity/chat`, `usePluginPageSpec` returns `chatSpec`
  regardless of what the blob declares.

- [ ] **5.3** Manually verify (or write a test): with the live system running, navigating to
  `/antigravity` renders a card with the antigravity plugin's observable fields, each bound to
  `/plugins/antigravity/<field>` in the live state store.

---

## Phase 6: op-gallery-gen plugin page mode (Rust side)

- [ ] **6.1** Add `TargetPluginContext` struct to `context.rs` (design.md Layer 5). Add optional
  `target_plugin: Option<TargetPluginContext>` field to `GenerationContext`. Update
  `build_system_message` to prepend the role-to-component table when `target_plugin` is set.

- [ ] **6.2** Add the role table as a static string constant in `context.rs`:
  ```rust
  pub const ROLE_COMPONENT_TABLE: &str = "..."; // markdown table from REQ-1.1
  ```
  The table is a constant, not generated, so it cannot drift from the catalog at runtime.
  A test asserts every component name in the table appears in the catalog's prompt.

- [ ] **6.3** Add `plugin_page` command to `op-gallery-gen`'s HTTP handler (in `ui_model.rs`
  or a new handler):
  `POST /gallery-gen/generate-plugin-page { plugin_id: string }` — assembles a
  `TargetPluginContext` for the named plugin (reads from blob catalog), runs one inference
  turn, validates, and returns the spec or errors. Does not write to gallery — the operator
  promotes it manually.

- [ ] **6.4** Write a Rust test in `context.rs` that the system message for a plugin-scoped run
  contains the role table and the plugin's schema but NOT the schemas of all other plugins.

---

## Phase 7: Schema export and CI sync check

- [ ] **7.1** Verify `scripts/export-catalog-schema.mts` exists and runs as part of `npm run build`.
  If not, create it: it calls `appCatalog.jsonSchema({ strict: true })` and writes
  `schemas/json-render/catalog.schema.json`, then updates `catalog.manifest.json` with the
  sha256 of both files.

- [ ] **7.2** Write a CI check (can be a vitest test in `src/test/`) that reads
  `STREAM_PLUGIN_IDS` from `stream-plugins.ts` and the plugin list from
  `/dev/shm/opdbus/plugin-blobs/.manifest.json` (or a fixture copy of it). Fails if any sealed
  plugin is absent from `STREAM_PLUGIN_IDS`. This is REQ-8.2.

  When the live blob catalog is not available (CI without a running daemon), the test skips with
  a clear "blob catalog not available" message — it does not fail.

---

## Phase 8: End-to-end smoke test

- [ ] **8.1** With the live system running, navigate through the following and verify each renders
  without a blank or error state:
  - `/` (overview — static spec)
  - `/antigravity/chat` (static chatSpec, live stream)
  - `/antigravity` (generated from blob, live state values)
  - `/plugins` (stream grid — static spec)
  - Any plugin that has `ui_surfaces` in the blob (e.g. `/tched_router` if declared)

- [ ] **8.2** Stop the StateSync stream (disconnect the backend) and verify:
  - The shell chrome remains visible
  - The `healthPill` shows offline
  - Content regions show a "connecting…" or "offline" state, not a crash

- [ ] **8.3** Run `npm test` across the full test suite. All existing tests must pass. The three
  new test files (role-map, generate-plugin-page, use-plugin-page-spec) must all pass.

---

## Ordering notes

Phases 1–2 are purely additive (new files, no existing code changed). They can be done and tested
independently of the running system.

Phase 3 changes `PageSpecOutlet.tsx`, which affects how pages render. Do this when Phases 1–2 are
green.

Phase 4 changes shell spec assembly. Do this after Phase 3 is stable.

Phases 5 and 7 are verification/test tasks that can be interleaved.

Phase 6 (Rust) is independent of the TypeScript phases and can proceed in parallel.

Phase 8 is integration and happens last.
