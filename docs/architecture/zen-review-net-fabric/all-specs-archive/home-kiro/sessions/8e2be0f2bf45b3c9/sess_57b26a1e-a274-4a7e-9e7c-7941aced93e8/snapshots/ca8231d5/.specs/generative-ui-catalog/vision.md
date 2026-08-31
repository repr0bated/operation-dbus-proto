# Vision — LLM-Generated UI Gallery

**Captured:** 2026-08-02, from session `cli_54b2fac7-7380-4ce8-bada-0c5cf4cba778_Xx4nlUYF` (20:45–21:20 UTC)
**Status:** Owner's vision. Supersedes `requirements.md` / `spec.md` where they conflict (see §12).
**Owner:** Jeremy — authoritative on intent.

---

## 1. The objective function

The model is not asked for a *nice* UI, or a *novel* UI. It is asked to make the data reachable:

> "Make this set of data as accessible as possible to as many people, industries, and causes as possible."

That sentence is the whole brief. Accessibility and reach are the goal; novelty is only a by-product of searching for reach. An element that exposes the data to an audience that previously could not use it beats an element that looks better to an operator who already understood it.

## 2. The brief is deliberately starved

The model receives three things:

1. The json-render.dev API and its documentation
2. How to access the data
3. The one-line objective from §1

Roughly three lines. Explicitly withheld: **every UI noun.** No "card," no "navigation," no "dashboard," no "table," no "panel." Naming a pattern hands the model the conventional answer and it stops deriving. Give it capability + data + intent only, and it must reason from the data's actual structure.

The chat interface sits on top for the cases where something specific is needed — add context, run a semantic search, ask for a particular surface. But the default path is the starved brief, because that is the path that produces what a human would not have specified.

**Direct consequence — the schema is the entire design vocabulary.** Field names and descriptions are the only domain words in the prompt. `is_egress_gateway`, `egress_ranges`, `address_range`, `nat_type` are what the model reasons over. schemars carries Rust doc comments through into JSON Schema `description`, so per-field comments are payload, not source noise:

```rust
/// CIDR ranges this node egresses for
/// Egress NAT mode: direct_nat or virtual_nat
```

Under this prompt design those descriptions do real work — they are the semantic hints that let a model infer "this is a network reachability concept" without anyone saying so.

## 3. Design-time generation, run-time determinism

This was misread once in the source conversation and corrected.

**Not:** the UI morphs with the data, rendering differently every request, generated on the fly.

**Actually:** the LLM is a **UI designer**. It works at design time and emits hundreds or thousands of **fixed** elements — many different ways to sort, group, display, and mutate the same data. Once emitted, each element is deterministic: a known field lands in a known slot, every time. Colors and CSS may vary freely; the data path through an element does not.

The model's job is therefore *bigger*, not smaller — it is design, not runtime templating.

**No inference in the request path at all.** Runtime is: pick element → bind fields → render.

What makes fixed bindings safe is field-level identity stability, which is exactly what the FNV-hashed field numbers buy — a field keeps its identity even when Rust struct order shifts.

## 4. The vocabulary ceiling (the load-bearing constraint)

Because no UI vocabulary is supplied, **schema richness sets a hard ceiling on everything downstream.**

A designer given `NodeSummary { id, network, address, connected, status, is_egress_gateway, egress_ranges }` can invent many elements — status tiles, egress-topology views, connectivity tables, per-network rollups, alerting surfaces.

A designer given `google.protobuf.Value` can invent exactly one: dump the JSON.

So `google.protobuf.Value` is not merely weak typing — it is an **empty vocabulary entry**. The model has no words for that data. Each occurrence *caps how many elements can ever exist* for it.

The ceiling binds in three places at once:

| Capped thing | Why |
|---|---|
| Catalog size | No field names → nothing to design against |
| Cross-connection discovery (§8) | Semantic embedding has nothing to embed; the join becomes undiscoverable |
| Cross-examination (§7) | You cannot confirm or refute a hypothesis against an opaque blob |

This is what makes the typed-output work directly load-bearing for the gallery rather than a code-quality nit. It is also why the empty outputs were *fatal* rather than untidy.

## 5. Memory wipe is the sampling mechanism

Memory is wiped after each render. This was initially misread as a cost to mitigate — "cache the render so the model never pays cold start." That is wrong and would have defeated the purpose.

The wipe is **how you sample.** Each run is an independent draw from the design space over the same schema. Retained memory anchors the model on its previous designs and it converges — variations on its first idea instead of genuinely different framings.

Therefore:

- You **want** to run the same schema many times. N wipes over one schema yields N independent derivations.
- Two runs over identical input can legitimately produce completely different framings, and **both belong in the gallery.**
- Caching a render to avoid re-running would suppress exactly the diversity being generated.
- The gallery is an **accumulating pool of independent samples**, not a queue to drain.

**Critical boundary:** memory is wiped *between* renders and must **accumulate within** one. A long run needs full working memory across its own exploration — hypothesise, query, discard, refine. The wipe is the boundary between samples, not a constraint inside a sample.

## 6. Gallery, then catalog — two stages with a human gate

```
mint → GALLERY → (human promotion) → CATALOG → rendered to site
       accumulating                   approved
       sample pool                    for schema X
```

- **Gallery** — every sample lands here. Unapproved. Nothing in the gallery reaches the site.
- **Catalog** — the approved set. Only catalog elements render to the site.
- **Promotion** is a human decision.

Already scaffolded: `/gallery` and `/catalog` routes are registered in the built bundle, and `.specs/generative-ui-catalog/` exists. It mirrors the discipline already written down for the auto-creator plugin — *propose for human review, never auto-seal into the live catalog.*

**Provenance across the gate is the real risk.** A rendering is designed against a specific schema hash, and that hash moves. The catalog hash moved three times in one session (`5f2beaca` → `94ef11da` → `1f5c4056`); netmaker's own schema went `1bad12a2` → `3345fce4`.

- Gallery entries must carry the `schema_hash` they were drawn against, or you can promote something already stale.
- Catalog entries stay **pinned** to that hash, so a reseal mechanically identifies which approved elements are now bound to a schema that no longer exists.

That is the difference between "approved" and "approved **for schema X**." Without the pin, a human signature is applied to a moving target — which is precisely the failure that hit the dashboard's netmaker clients: approved-looking code bound to a schema that had moved on, silently returning nothing.

## 7. One mode

There is **one mode**: deliberation.

> "I want it to have time to think and examine the data. Let it run for three hours and produce 100 or 10 — but it is really good, and cross examine."

- **Time-boxed, not output-boxed.** Hours, not a target count.
- **Yield is an outcome, not a goal.** Ten excellent elements is success; a hundred mediocre ones is not.
- **Cross-examination is required.** The model looks at the data from multiple angles, compares blobs, and tests its own assumptions before committing to an element.

**Cross-examination needs live data, not just schema.** To test whether a hypothesised connection holds, the run must call the plugin read methods (`list_networks`, `list_nodes`, `get_node`) and inspect actual values — real nodes, real ranges, real instances. It can then check whether egress ranges genuinely correspond to routes found elsewhere, and drop the idea if they do not. This is the sharper reason the typing work matters: an opaque `Value` can neither confirm nor refute anything.

**Why fewer-and-deeper is right, beyond quality:** the gate in §6 is a person's attention. Ten well-reasoned candidates is tractable; a thousand is a queue nobody drains, and the gate silently degrades into rubber-stamping.

Hints (§10) are **dials on this one mode**, not additional modes.

## 8. MCP — one unified pool so discoveries can be made across blobs

MCP was introduced for one reason: **to present the data as a single unified source — a pool in which cross-references can be made, so that discoveries can be made across blobs.**

The unit of that pool is the **blob**. Each sealed plugin blob is self-contained and says nothing about any other, so a relationship that spans two blobs is not merely undocumented — it is unrepresentable in either one. Pooling is what turns such a relationship into something that can be found at all. Without the pool there is no place where a cross-blob fact could exist; with it, discovery becomes a search rather than an act of authorship.

That is why "things humans wouldn't think of" is a realistic expectation rather than optimism. A human designs per blob because that is how the artifacts are organised; the interesting relationships are the ones that cut across that organisation, and they are invisible from inside any single blob.

**Measured pool:** 65 sealed blobs in `/dev/shm/opdbus/plugin-blobs/`, content-addressed by hash (`netmaker.3345fce4814b458a.blob`).

MCP remains **enhancement, not core data**: core is the schema contract (deterministic, hash-pinned, what elements bind to); MCP is *discovery across the pool* — what is even worth designing. In a long run, most cycles go here: search the pool, find candidate cross-blob references, pull the relevant schemas, test them against live reads.

Real cross-blob relationships in this system, invisible blob-by-blob:

- **One packet path, five blobs.** `netmaker.NodeSummary.egress_ranges` → `rtnetlink` routes/rules → `openflow` flows on `ovsbr0` → `wgcf` underlay → `xray`. A human builds five pages because there are five plugins. The data is one story.
- **One container, four blobs.** `incus.IncusInstance` ↔ `procfs` processes ↔ `netmaker` node ↔ `btrfs` subvolume.
- **One identity, four blobs.** `identity_sled` ↔ `keypair` ↔ `wireguard` ↔ `netmaker` host pubkey.

**The pool is larger than the UI can currently see.** 65 blobs exist; the UI's proto surface covers 54 plugins. Twelve blobs have no representation in it at all:

```
adc  antigravity  antigravity_chat  compact_mcp  full_system  gcloud_adc
identity_sled  keyring  mcp  privacy_routes  sess_decl  xray
```

Two of those are ones already identified as needed — **`antigravity`**, the UI-rendering plugin whose schema lives in a blob, and **`identity_sled`**, which the UI needs to read. `xray` and `privacy_routes` are load-bearing in the packet path above. So the designer searching the pool can reach material the UI has no binding for, which is a gap to close deliberately rather than a discrepancy to reconcile.

What MCP serves today is `refresh_blob_vectors` / `search_blob_vectors` — Qdrant semantic search over blob content. That is discovery-by-meaning, not schema retrieval; there is no `get_blob_schema` yet. `refresh_blob_vectors` needs the same reseal trigger as the projection, or the pool goes stale and cross-blob discovery runs against schemas that have moved.

## 9. The staging table

Schema is dumped and sealed by us, so we control the whole pipeline (schemars → PluginSchema → opblob → SHM). Nothing external requires the *descriptive* schema to be protobuf-shaped.

> "Maybe a staging table for UI that sits between the blobs and the UI."

**Why it resolves the tension:** blobs are sealed and immutable; the UI needs something queryable, denormalised, and cheap to read cold. Different jobs. Trying to make one artifact do both is what produced the protoc namespace-collision fight.

**Hard constraints:**

- **It must still present as schema.** A table of rows is not renderable. The payload is real JSON Schema with `$defs` and nesting intact; the relations are the index. A field row cannot be `(message, name, "string")` — it must carry the full JSON Schema fragment so the model sees types, formats, enums, descriptions, defaults, units.
  - Query path (relations) → *which* schema for this surface
  - Payload (JSON Schema) → *how* to render it
- **Strictly derived, never authored.** One-way: seal → project → serve. Anything writing UI-specific facts directly into the table creates a second source of truth and reproduces the netmaker plugin/adapter duplication already called out.
- **Trigger is reseal.** The catalog hash is the natural signal — no polling.
- **`describe_relation` is not enough.** Cozo describing its own table shape tells the model about the *table*, not about netmaker. The renderable schema is the payload, not the relation metadata.
- **It does not have to be pre-resolved — a set is enough.** The earlier framing assumed a choice between raw schema and a pre-resolved projection with refs inlined. That choice is unnecessary. The table can serve the **set** of schema documents a method needs — the transitive closure of its `$defs` — handed over together. A stateless model with a three-line prompt then has everything in front of it without resolving `$ref` against anything remote, and without a second flattened representation existing at all.

  That matters because a pre-resolved projection is a derived artifact that can drift from the blob. A set has nothing to drift: every member is the raw schema, unmodified, straight from the seal. It removes the drift risk and the resolution burden at the same time, which is why it is preferred over inlining.

`json_render` already has the vocabulary for this — `export_json_schema`, `get_component_schema`, `get_spec_schema`, `list_components`, `list_renderers`, `build_prompt_surface` (15 sealed methods, `validate_spec` among them). The table should serve *that* shape. `build_prompt_surface` is also how the full json-render.dev API — which lives in a blob — gets into the system prompt, rather than parsing 934 generated messages.

**There is no validation on the read path — and validation happens on mutation.** That is the precise statement, and the split matters.

**Reads.** Validation exists to detect disagreement between two representations of the same thing. On the read side there is one source of truth: the blobs. The audit trail, a compliance view, an operator view, and whatever we look at ourselves all derive from the same sealed blobs. Nothing is reconciled against anything, so there is nothing to validate.

The seal is not a validation pass either; it is **identity**. A content-addressed hash per plugin plus a catalog hash over the set says "this is exactly what was sealed" — a statement about which bytes these are, not a judgement about whether they are correct.

**Writes.** Validation lives at the **mutation boundary**, and only there. A mutation updates the D-Bus tree, and that is the single point where correctness is enforced. Everything downstream — seal, projection, staging table, element, render — is a read of something that was already validated when it was written.

Consequences worth stating plainly:

- The render path performs no validation. Not deferred, not cached — absent.
- The UI never re-checks a schema it was handed, because there is no competing schema it could disagree with.
- An element that affords an action must **not** embed validation logic. Duplicating the mutation-side rules client-side creates a second source of truth for validity — the same failure mode as authoring into the staging table. The element submits; the mutation boundary decides.
- Authorization is a separate axis from validation and is already enforced by capabilities (`cap.network.netmaker.*`). An action allowlist in an element is authorization scoping, not validation.
- `validate_spec` is a design-time convenience for the designer and the promotion gate, never a runtime gate in front of an operator.
- This is a large part of why the pipeline is cheap: correctness is enforced once at write, and everything after it is a read.

It also explains why the compliance hint in §10 works at all. **Compliance is not a separate dataset or a separate pipeline** — it is another lens over the same blobs. That is what makes a single word a sufficient input: nothing has to be sourced, joined, or certified differently to produce a compliance framing, because it is the same data seen differently. The same holds for an audit view. If any of these needed their own source, the one-word hint would be meaningless and the gallery model would collapse.

**The hash is provenance, not a cache key.** You do not consult it to decide whether to re-render (§5 says re-run deliberately). You record it so you know what a gallery entry was derived from, and which catalog entries are pinned to a schema that has since moved. A hash change invalidates *provenance on existing entries*, per plugin — not the pipeline.

## 9.1 The state model — there is only one state

> "The desired state is the state. If you don't like it, mutate it."
> "Only one state."

Not "desired and actual happen to agree." There is **one** state, singular. No second copy of it exists anywhere — no desired-versus-actual, no mirror, no snapshot, no cache, no per-client view. Change happens exactly one way: **mutation.**

This is why §9 puts validation at the mutation boundary — the mutation is the *only* state transition, so it is the only place where correctness can be enforced or where anything could be wrong.

**"State sync" is a category error here,** and so is "state mirror." Both presuppose two things that could disagree. With one state there is nothing to sync and nothing to mirror; a `GetState` is simply a read of the state, not a snapshot to be compared against something else. Names carrying `Mirror`, `Sync`, or `Snapshot` describe an architecture this system does not have — which is why `snapshot.state` was removed rather than fixed, and why the projection was removed on purpose so that mutation updates the tree directly.

What that removes from the UI, which matters because a designer will otherwise reach for it:

- No drift indicators, no "out of date" badges, no diff between desired and actual — none of those have a referent.
- No "last synced" timestamps, no refresh-to-reconcile, no polling. Updates arrive from mutation events, per the reactive principle already established in this workspace.
- No optimistic-update-then-reconcile dance. An action submits a mutation; the resulting state *is* the state. There is no provisional local belief to later correct.
- No history-of-state to diff against, and no state versioning. Blobs are versioned by hash because a *schema* is an artifact; state is not an artifact and has no versions.

**On the client store.** A client-side store is a transport buffer holding the current state for the render pass — it is not a second state and must never accumulate its own truth. The moment it retains a value the mutation stream has moved past, a second state exists and every consequence above is reintroduced.

This is a place where the starved brief (§2) does concrete protective work. A model trained on conventional dashboards will reach for refresh buttons, sync status, and staleness warnings by reflex, because those words come bundled with the UI vocabulary it knows. Withholding that vocabulary is what stops it inventing affordances for a condition this system cannot enter.

**Useful as a review signal:** if a promoted element contains a drift, sync, staleness, snapshot, or refresh affordance, that is evidence the brief leaked vocabulary — a defect in the prompt, not just in the element.

### 9.1.1 Strictly enforced

This is an invariant, not a preference. It is not traded off for convenience, performance, or offline behaviour, and there is no "mostly one state" configuration.

**Prohibited outright:**

| Prohibited | Why it violates the invariant |
|---|---|
| Any second copy of state — mirror, snapshot, replica, cache, materialised view | Creates a thing that can disagree with the state |
| A client store that retains a value past the mutation stream | Becomes a second state with its own lifetime |
| Optimistic local updates | A provisional state exists before the mutation resolves |
| Polling, refresh, or reconcile paths | Only meaningful if two states could diverge |
| Desired/actual pairs, drift detection, staleness metadata | Presupposes the distinction this model does not have |
| Any write that does not go through the mutation boundary | Bypasses the only validated transition (§9) |
| Authoring into the staging table rather than deriving it | Second source of truth (§9) |

**Enforcement points:**

- **Mutation boundary** — the only permitted state transition, and where validation lives. A write that arrives any other way is rejected, not merged.
- **Projection direction** — one way, seal → project → serve. The staging table is derived, never authored.
- **Capabilities** — `cap.*` gates who may mutate. Separate axis from validation, equally non-negotiable.
- **Promotion gate** — an element carrying a prohibited affordance from the table above does not get promoted. This is where the review signal becomes an enforcement action rather than an observation.

**Consequence for the designer:** the prohibitions are not communicated to the model as rules, because §2 forbids handing it UI vocabulary — naming "refresh" to prohibit it would teach it the concept. The invariant is enforced at the gate instead. A model that has never been given the word will rarely reach for the affordance; when it does, the element is rejected and the prompt is examined for the leak.

## 9.2 Categorisation — the subid is the category axis

Every method carries a subid, and the coverage is total: **437 of 437.** There are no gaps, so the subid namespace is a complete index over the surface rather than a convention that mostly holds.

**Grammar:** `<kind>.<domain>.<subdomain>.<resource>.<verb>@v<n>`

```
obs.network.netmaker.networks.list@v1     capability: cap.network.netmaker.networks.list@v1
mut.standard.oscal.subid.register@v1      capability: oscal.invoke
```

Capability is a separate string, not derived from the subid — the authorization axis of §9.1.1, kept independent.

### Tier 1 — kind, and why it matters more than it looks

| kind | methods |
|---|---|
| `mut` | 256 |
| `obs` | 181 |

This is exactly the read/write split that §9 turns on. **The subid already declares which side of the mutation boundary a method sits on**, which means an element's affordances are derivable from subids alone with no extra metadata:

- `obs.*` → binds to display. No validation, no mutation, one state read (§9.1).
- `mut.*` → binds to an action that crosses the validated boundary. The element submits; it carries no rules (§9).

That the split is 256 mutations to 181 observations is worth noting on its own — this surface is action-heavy, not report-heavy, so a designer given the whole pool will find more to *do* than to *show*.

### Tier 2 — domain, which crosses plugin boundaries

| domain | methods | plugins |
|---|---|---|
| `service` | 215 | 24 |
| `software` | 104 | 11 |
| `network` | 55 | 11 |
| `data` | 17 | 2 |
| `storage` | 15 | 1 |
| `memory` | 10 | 1 |
| `container` | 7 | 1 |
| `standard` | 5 | 1 |
| `security` | 4 | 1 |
| `agent` | 3 | 1 |
| `hardware` | 2 | 1 |

12 domains over 54 plugins.

**This resolves the open tension in §15.** The per-plugin versus cross-plugin conflict dissolves, because the domain *is already* the cross-plugin category. `network` is 55 methods spanning eleven plugins — `netmaker`, `rtnetlink`, `openflow`, `openflow_obfuscation`, `ovsdb_bridge`, `rovs_commands`, `wireguard`, `wg_opdbus`, `proxy_server`, `ghostbridge`, `net`. That is precisely the five-plugin packet path §8 describes, and the taxonomy asserts the grouping rather than leaving it to be discovered.

So the two mechanisms divide cleanly:

- **Within-domain joins come free from the subid.** No embedding search needed; the category already says these belong together.
- **Cross-domain joins still need MCP discovery (§8).** The container example — `incus` ↔ `procfs` ↔ `netmaker` ↔ `btrfs` — spans `service`, `service`, `network`, `storage`. No taxonomy asserts that; pooled embeddings are what surface it.

Element provenance therefore pins to a **domain** and the set of schema hashes it covers, not to a single plugin.

### Remapping — and why plugin-local organisation does not apply

Categories and subids can be freely remapped for the UI. Which plugin a method happens to live in is an implementation detail of the backend; it carries no information about how the data should be reached, so it must not constrain how the UI groups things. The subid is the stable identifier; a category is a *view* over subids.

**This already has a home.** `oscal_subid_registry` exposes exactly the operations a remap needs:

| method | subid |
|---|---|
| `register` | `mut.standard.oscal.subid.register@v1` |
| `lookup` | `obs.standard.oscal.subid.lookup@v1` |
| `resolve` | `obs.standard.oscal.subid.resolve@v1` |
| `list_by_category` | `obs.standard.oscal.category.list@v1` |
| `export` | `obs.standard.oscal.export@v1` |

`list_by_category` means category is already a first-class concept with a registry behind it. So a UI remap is a **registry entry, not UI code** — which keeps it derived rather than authored, consistent with §9, and keeps the mapping in one place instead of scattered across elements.

### Compliance is not hypothetical

That registry is **OSCAL** — the NIST compliance standard. So the one-word compliance hint of §10 has an actual control-mapping surface behind it: a compliance lens is a subid→control mapping resolved through the registry, over the same blobs. This is the concrete instance of the §9 claim that compliance is a lens rather than a separate source, and it means the hint has somewhere real to land rather than relying on the model's general notion of the word.

### Consequence for the coverage map

The coverage grid in §10 should be **hints × domain**, not hints × plugin — 12 legible cells rather than 54 incidental ones.

One caution on skew: `service` (215) and `software` (104) are 73% of all methods. A domain-level grid will be dominated by two cells, so those two need subdomain granularity to be informative, while the thin tail (`hardware` 2, `agent` 3, `security` 4, `standard` 5) is where reach gaps will actually be visible.

### MCP — the same taxonomy over one unified source

MCP was implemented so the data can be **presented as one unified data source**, so that cross-connections can be made across it. That is its purpose — not a tool registry, not a convenience API. The unification *is* the feature, because a join only becomes findable once the things being joined sit in one addressable space.

The subid taxonomy spans that unified source, so everything in §9.2 applies to it and not just to the 437 plugin protos. Measured across the codebase:

| kind | occurrences | meaning |
|---|---|---|
| `obs` | 530 | observation — read |
| `mut` | 517 | mutation — the validated write boundary (§9) |
| `exp` | 197 | exposition / render |
| `src` | 63 | source of record, indexing, identity linkage |

(`cap` — 131 — is the separate authorization namespace of §9.1.1, not a subid kind.)

**The plugin protos expose only two of the four kinds.** All 437 are `obs` or `mut`. Everything in `exp` and `src` is invisible from the UI's proto surface, which means the surface the UI currently sees is a strict subset of what the taxonomy describes.

**`exp` matters most, and it revises §4.** There are 191 distinct `exp` subids and they are **field-level render identities**:

```
exp.network.wireguard.peer.allowed-ips.render@v1
exp.network.wireguard.interface.port.render@v1
exp.agent.persona.catalog.render@v1
exp.service.antigravity.auth.provider@v1
```

Spread: `service` 88, `software` 82, `network` 11, `agent` 7, `standard` 2, and a `ui` domain with 1.

This is a presentation taxonomy at field granularity that already exists. A designer binding against `exp` subids gets **named renderable fields even where the corresponding proto output is `google.protobuf.Value`** — so the vocabulary ceiling of §4 is not as low as the proto measurement in §13 alone suggests. The 14% figure is the ceiling *of the proto surface*; the unified source is richer. How much richer is unmeasured and worth establishing, because it changes the priority of the proto typing work relative to simply exposing `exp` to the designer.

**`src`** — 56 distinct — carries source-of-record and linkage identities (`src.software.workspace.index@v1`, `src.software.user-container-memory.identity-link@v1`), and reaches domains absent from the plugin surface entirely: `policy`, `process-procedure`, `hardware`, `standard`.

**The registry is reachable from the same place.** The MCP server surface already includes the compliance graph, the subid registry, and the audit log. So the remap mechanism of §9.2 lives where the designer is already searching — a category remap and a discovery query are the same address space, not two systems to bridge.

**Unification is what makes the cross-kind story visible.** A single network narrative spans three plugins and three kinds — `exp.network.wireguard.peers.render`, `obs.network.netmaker.networks.list`, `mut.network.rtnetlink.*`. Looked at per-plugin, or per-kind, or through the protos alone, it is three unrelated fragments. Pooled and categorised by domain, it is one story with a display side, a read side, and a write side.

**Provenance hook.** `IdentitySled` (152 bytes, `op-identity/src/schema_bridge.rs`) carries `vector_id` — the Qdrant UUID of the last reasoning episode — with the stated invariant that every vectorized episode is traceable to the sled. Alongside `mutation_index` (monotonic mutation counter) and `hashed_footprint` (Blake3 over canonicalized schema state), that gives the reasoning trail of §11 an existing anchor rather than a new one to invent.

## 10. Hints — the reach dial

Into an otherwise unchanged starved brief, inject a single word. Not a description, not a requirement. A hint.

Two **separate** dimensions:

- **Industry / cause** — "clinics," "rural," "logistics," "auditors." Worked example from the conversation: the single word **"compliance."**
- **Demographic** — who is reading. "Someone who's never seen a network diagram," "low vision," "a night-shift operator," "a board member."

Kept separate they compose: demographic × industry × plugin. Collapsed into one tag list they become soup.

This is the axis that actually implements §1 — "as many industries and causes as possible" is not a property of the schema, it is the hint dimension. It is also the cheapest multiplier available: hints × repeated wiped samples.

Because a hint is not a specification, the same hint re-run after a wipe still yields an independent draw, so it composes with §5 rather than collapsing it into determinism.

**Guard rail:** a demographic hint shapes *presentation* — density, sequencing, vocabulary, contrast, what leads. It must **never** gate what data exists. If a hint starts deciding who may see which fields, access control has leaked into the design layer; that belongs in capabilities where it is already enforced (`cap.network.netmaker.*`).

**Coverage map:** hints × domain (§9.2) is a grid you can inspect — which cells have promoted elements and which are empty. That turns §1 from an aspiration into something with visible gaps, and tells you where to aim the next runs instead of sampling blindly.

## 11. Provenance record

Every gallery entry carries:

| Field | Purpose |
|---|---|
| `schema_hash` | what it was drawn against; pins the binding |
| `mode` | sets the reviewer's posture |
| `hint.industry`, `hint.demographic` | explains why the framing looks the way it does |
| reasoning trail | which joins it found, what it verified against live data, what it rejected |

Without the hint recorded, a reviewer sees an odd framing with no idea what produced it. Without the reasoning trail, a three-hour candidate cannot be judged on whether its reasoning holds — and that is the only basis on which it *should* be judged.

## 12. Where this conflicts with the current spec

| Current `requirements.md` / `spec.md` | This vision |
|---|---|
| `spec.md` §3 mint scaffold prescribes `CONTEXT / DATA SHAPE / SIGNAL TO CONVEY / ACTION TO AFFORD / CONSTRAINTS / TRADE-OFF VECTOR` | Three-line starved brief (§2). The scaffold supplies exactly the framing that is forbidden — "signal to convey" and "action to afford" are UI thinking handed to the model |
| US-3: one rolling gallery of exactly 200, evict oldest/lowest-scoring | Two stages with a human gate (§6). Gallery **accumulates** samples (§5); eviction would discard independent draws |
| US-1: "same surface may resolve to different elements over time" | True in effect, but the mechanism is *selection among fixed elements* (§3), not regeneration |
| Rejection rule: reject if it resembles an existing gallery entry | Replaced by the memory wipe (§5). Two identical inputs producing different framings is the *goal*; near-duplicates are a sampling outcome, not a defect to filter |
| Objective framed as novelty / exploring the constraint space | Objective is accessibility and reach (§1); novelty is instrumental |
| No mode concept | One deliberation mode (§7) |
| No MCP / cross-blob concept | MCP as the pooled cross-connection surface (§8) |
| No hint concept | Hints are the reach dial and the coverage measure (§10) |
| No provenance concept | Provenance record is what makes the gate real (§11) |
| US-5 / §5 Observability: unknown DSL nodes emit runtime diagnostics; diagnostics counted per element and used for eviction | No read-path validation (§9). Runtime diagnostics indicate a genuine bug, not a routine check — and cannot be an eviction signal for a set that accumulates |
| `spec.md` §1: `ConfirmSpec` / action allowlist framed as interpreter-side safety | Authorization, not validation. Capabilities already enforce access; the mutation boundary validates the payload. An element must not carry mutation rules (§9) |
| US-5 / NFR-2: interpreter executes against "live Zustand state"; `useEventStore.getState()` for high-frequency updates | Compatible only if the store is a transport buffer that retains nothing past the mutation stream. Any retained value is a second state and is prohibited (§9.1.1) |
| US-4: `Snapshot` frame type; "reconnect resumes from cursor without full resnapshot" | `Snapshot` is a prohibited name and concept (§9.1). Reconnect reads the state; there is no snapshot to resume against |
| NFR-1..5 assume a runtime interpreter is the whole system | Still valid for the render path, but silent on design-time, which is where this vision lives |

## 13. What we actually have to work with (measured 2026-08-02)

The vision depends on the model seeing field names, types, and descriptions. Measured against both candidate sources.

**Proto layer — `proto/plugin_methods/`, 437 methods:**

| Output shape | Count | Designable against? |
|---|---|---|
| `Output {}` — empty message | 286 | No — nothing to bind |
| 1 field, `google.protobuf.Value` | 48 | No — empty vocabulary entry |
| 1 field, `repeated google.protobuf.Value` | 42 | No — same |
| 1 field, concrete scalar | 30 | Thin |
| 2+ typed fields | 31 | Yes |

Roughly **61 of 437 (14%)** carry anything a designer could work from. 286 carry nothing; 90 carry one opaque `Value`.

**This is the ceiling of the proto surface only.** §9.2 measures 191 field-level `exp.*` render subids that the protos do not expose — so the unified MCP source describes named fields in places where the proto says `Value`. The 14% bounds what the UI can see today, not what the taxonomy knows.

**Blob / JSON-Schema layer — the same methods, fully typed.** `netmaker list_networks` is `ListNetworksOutput {}` in proto, but in the sealed blob it is `NetworkSummary { netid, address_range, address_range6, default_mtu, default_keepalive, … }` with per-field doc comments carried into `description`.

Verified in the blob: `$defs`/`definitions` present, `oneOf` unions preserved, nested type names intact (`NicDevice`, `DiskDevice`, `ProxyDevice`, `NamedDevice`). **The information is not lost — it is lost only in the JSON-Schema→proto step.**

Two facts that follow:

- **The UI already generates from blob descriptors, not `plugin_methods.proto`.** Filename `netmaker__operation_method_netmaker_list_nodes.ts` → package `operation.method.netmaker.list_nodes`, which is the blob's naming. So the UI is on the blob pipeline already — it is just consuming the *degraded descriptor* rather than the *raw schema sitting beside it in the same blob*.
- **The schema is standard.** JSON Schema via schemars (Draft 7 / 2020-12), protobuf + reflection for transport, schema-as-source-of-truth with codegen downstream, content-addressed immutable artifacts. The one genuinely unusual choice is FNV-hashed field numbers instead of sequential-with-`reserved` — deliberate, since it buys the field identity stability §3 depends on.

**Why the degradation is unavoidable:** JSON Schema is strictly more expressive than protobuf — recursion, `oneOf` unions, arbitrary nesting, constraints, descriptions. `json_schema_type_to_proto` collapsing objects to `Struct`/`Value` is a lossy projection, not a bug in the schema. Every project generating proto from JSON Schema hits this wall. Which is exactly why the staging table (§9) is the right call: the standard artifact reaches the designer unflattened, and protobuf keeps only the job it is good at — the wire contract.

The Rust source already records the same conclusion:

> a type that degrades to `google.protobuf.Value` in the generated proto leaves callers with no schema.

## 14. What this means, in one paragraph

We are not building a UI that an AI redraws on demand. We are hiring an AI as a UI designer, giving it a deliberately impoverished brief so it cannot fall back on convention, pointing it at one unified pool of 65 sealed blobs so it can find references that cross blob boundaries and could not exist inside any single one, wiping its memory between runs so that repeated runs over the same schema yield independent draws rather than a converging house style, letting it deliberate for hours and cross-examine against live data, and seasoning it with a single industry or demographic word to aim it. It emits a large body of fixed, deterministic elements. Humans promote the good ones from gallery to catalog, and every entry carries the schema hash, mode, hint, and reasoning that produced it. From promotion onward the elements are static assets with no model and no read-path validation — there is nothing to validate on a read, because the audit view, the compliance view and the operator view all come from the same blobs; validation happens once, at the mutation boundary. The single binding constraint on all of it is how much the schema actually says.

---

## 15. Open questions carried forward

- **How much of the `Value` gap do `exp.*` subids already cover?** 191 field-level render subids exist (§9.2) against 90 methods whose only output is an opaque `Value` (§13). If the overlap is high, exposing `exp` to the designer is cheaper and faster than typing protos; if low, the typing work stays primary. This is measurable and currently unmeasured — it is the highest-value question on the list.
- **Should `exp` and `src` be projected to the UI at all,** or does the UI stay on `obs`/`mut` with `exp` consumed only at design time by the designer?
- **The twelve unbound blobs (§8).** `antigravity` and `identity_sled` are known to be needed and have no proto representation. Is the fix to project them, or does the designer reach them only through the pool? `xray` and `privacy_routes` sit in the packet path, so leaving them unbound caps the most valuable cross-blob element class.

- **Per-plugin invalidation vs cross-plugin elements — resolved by §9.2.** Provenance pins to a subid domain plus the set of schema hashes it covers, not to a single plugin. Recorded here for the trail; no longer open.
- Staging table substrate: Cozo (already the declared durable registry, 7 sealed methods) or a plain table alongside the blobs?
- Does json-render.dev consume JSON Schema directly? If it does, the table is a pass-through of the schema set (§9) and no projection exists at all. Unverified — could not reach the network in the source session.
- Add `get_blob_schema(plugin_id)` to MCP beside `refresh_blob_vectors` / `search_blob_vectors`? Small additive change; makes schema consumable by agents and the UI alike.
- Promotion criteria for gallery → catalog: human judgment only, or assisted by the coverage map (§10)?
- Element selection at runtime: which catalog element serves a given surface, and who decides?
- Does the gallery ever evict, given §5 says it accumulates? If it grows without bound, the coverage map becomes the only navigational tool over it.
