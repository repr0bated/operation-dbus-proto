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

## 8. MCP as one pooled data surface

The reason all blob schemas are pooled over MCP is so the designer sees **one data pool**, not 65 per-plugin silos. That is what lets it find joins nobody specified — the source of "things humans wouldn't think of."

MCP remains **enhancement, not core data**: core is the schema contract (deterministic, hash-pinned, what elements bind to); MCP is *discovery across the pool* — what is even worth designing. In a long run, most cycles go here: search the pool, find candidate cross-plugin joins, pull the relevant schemas, verify against live reads.

Real joins in this system that are invisible plugin-by-plugin:

- **One packet path, five plugins.** `netmaker.NodeSummary.egress_ranges` → `rtnetlink` routes/rules → `openflow` flows on `ovsbr0` → `wgcf` underlay → `xray`. A human builds five pages because there are five plugins. The data is one story.
- **One container, four plugins.** `incus.IncusInstance` ↔ `procfs` processes ↔ `netmaker` node ↔ `btrfs` subvolume.
- **One identity, four plugins.** `identity_sled` ↔ `keypair` ↔ `wireguard` ↔ `netmaker` host pubkey.

What MCP serves today is `refresh_blob_vectors` / `search_blob_vectors` — Qdrant semantic search over blob content. That is discovery-by-meaning, not schema retrieval; there is no `get_blob_schema` yet. `refresh_blob_vectors` needs the same reseal trigger as the projection, or the pool goes stale.

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

`json_render` already has the vocabulary for this — `export_json_schema`, `get_component_schema`, `get_spec_schema`, `list_components`, `list_renderers`, `validate_spec`, `build_prompt_surface` (15 sealed methods). The table should serve *that* shape. `build_prompt_surface` is also how the full json-render.dev API — which lives in a blob — gets into the system prompt, rather than parsing 934 generated messages.

**Supporting fact:** the data is already verified at seal time. The seal *is* the verification — content-addressed hash per plugin plus a catalog hash over the set. A renderer re-validating would be redundant work.

**The hash is provenance, not a cache key.** You do not consult it to decide whether to re-render (§5 says re-run deliberately). You record it so you know what a gallery entry was derived from, and which catalog entries are pinned to a schema that has since moved. A hash change invalidates *provenance on existing entries*, per plugin — not the pipeline.

## 10. Hints — the reach dial

Into an otherwise unchanged starved brief, inject a single word. Not a description, not a requirement. A hint.

Two **separate** dimensions:

- **Industry / cause** — "clinics," "rural," "logistics," "auditors." Worked example from the conversation: the single word **"compliance."**
- **Demographic** — who is reading. "Someone who's never seen a network diagram," "low vision," "a night-shift operator," "a board member."

Kept separate they compose: demographic × industry × plugin. Collapsed into one tag list they become soup.

This is the axis that actually implements §1 — "as many industries and causes as possible" is not a property of the schema, it is the hint dimension. It is also the cheapest multiplier available: hints × repeated wiped samples.

Because a hint is not a specification, the same hint re-run after a wipe still yields an independent draw, so it composes with §5 rather than collapsing it into determinism.

**Guard rail:** a demographic hint shapes *presentation* — density, sequencing, vocabulary, contrast, what leads. It must **never** gate what data exists. If a hint starts deciding who may see which fields, access control has leaked into the design layer; that belongs in capabilities where it is already enforced (`cap.network.netmaker.*`).

**Coverage map:** hints × plugins is a grid you can inspect — which cells have promoted elements and which are empty. That turns §1 from an aspiration into something with visible gaps, and tells you where to aim the next runs instead of sampling blindly.

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

**Blob / JSON-Schema layer — the same methods, fully typed.** `netmaker list_networks` is `ListNetworksOutput {}` in proto, but in the sealed blob it is `NetworkSummary { netid, address_range, address_range6, default_mtu, default_keepalive, … }` with per-field doc comments carried into `description`.

Verified in the blob: `$defs`/`definitions` present, `oneOf` unions preserved, nested type names intact (`NicDevice`, `DiskDevice`, `ProxyDevice`, `NamedDevice`). **The information is not lost — it is lost only in the JSON-Schema→proto step.**

Two facts that follow:

- **The UI already generates from blob descriptors, not `plugin_methods.proto`.** Filename `netmaker__operation_method_netmaker_list_nodes.ts` → package `operation.method.netmaker.list_nodes`, which is the blob's naming. So the UI is on the blob pipeline already — it is just consuming the *degraded descriptor* rather than the *raw schema sitting beside it in the same blob*.
- **The schema is standard.** JSON Schema via schemars (Draft 7 / 2020-12), protobuf + reflection for transport, schema-as-source-of-truth with codegen downstream, content-addressed immutable artifacts. The one genuinely unusual choice is FNV-hashed field numbers instead of sequential-with-`reserved` — deliberate, since it buys the field identity stability §3 depends on.

**Why the degradation is unavoidable:** JSON Schema is strictly more expressive than protobuf — recursion, `oneOf` unions, arbitrary nesting, constraints, descriptions. `json_schema_type_to_proto` collapsing objects to `Struct`/`Value` is a lossy projection, not a bug in the schema. Every project generating proto from JSON Schema hits this wall. Which is exactly why the staging table (§9) is the right call: the standard artifact reaches the designer unflattened, and protobuf keeps only the job it is good at — the wire contract.

The Rust source already records the same conclusion:

> a type that degrades to `google.protobuf.Value` in the generated proto leaves callers with no schema.

## 14. What this means, in one paragraph

We are not building a UI that an AI redraws on demand. We are hiring an AI as a UI designer, giving it a deliberately impoverished brief so it cannot fall back on convention, pointing it at one pooled surface of already-verified schema so it can find joins spanning plugins, wiping its memory between runs so that repeated runs over the same schema yield independent draws rather than a converging house style, letting it deliberate for hours and cross-examine against live data, and seasoning it with a single industry or demographic word to aim it. It emits a large body of fixed, deterministic elements. Humans promote the good ones from gallery to catalog, and every entry carries the schema hash, mode, hint, and reasoning that produced it. From promotion onward the elements are static assets with no model in the render path — which is what makes the whole thing affordable, reviewable, and safe to pin. The single binding constraint on all of it is how much the schema actually says.

---

## 15. Open questions carried forward

- **Per-plugin invalidation vs cross-plugin elements — unresolved tension.** Per-plugin re-render scope was accepted on the grounds that defined elements have no cross-plugin composition to invalidate. But §8 makes cross-plugin joins the most valuable class of element, and such an element is pinned to *several* schema hashes. Those two positions conflict and need reconciling — probably a composite provenance key rather than a single `schema_hash`.
- Staging table substrate: Cozo (already the declared durable registry, 7 sealed methods) or a plain table alongside the blobs?
- Does json-render.dev consume JSON Schema directly, or want its own spec shape? This single fact decides whether the table is a pass-through or needs a projection. Unverified — could not reach the network in the source session.
- Add `get_blob_schema(plugin_id)` to MCP beside `refresh_blob_vectors` / `search_blob_vectors`? Small additive change; makes schema consumable by agents and the UI alike.
- Promotion criteria for gallery → catalog: human judgment only, or assisted by the coverage map (§10)?
- Element selection at runtime: which catalog element serves a given surface, and who decides?
- Does the gallery ever evict, given §5 says it accumulates? If it grows without bound, the coverage map becomes the only navigational tool over it.
