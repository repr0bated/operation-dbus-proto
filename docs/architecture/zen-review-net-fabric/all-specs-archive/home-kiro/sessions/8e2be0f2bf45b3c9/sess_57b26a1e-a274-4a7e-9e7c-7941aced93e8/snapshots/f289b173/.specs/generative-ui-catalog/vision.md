# Vision — LLM-Generated UI Gallery

**Captured:** 2026-08-02, from session `cli_54b2fac7-7380-4ce8-bada-0c5cf4cba778_Xx4nlUYF`
**Status:** Owner's vision, verbatim intent preserved. Supersedes `requirements.md` where they conflict (see §7).
**Owner:** Jeremy (human, authoritative on intent)

---

## 1. The objective function

The model is not asked to make a *nice* UI, or a *novel* UI. It is asked to make the data reachable:

> "Make this set of data as accessible as possible to as many people, industries, and causes as possible."

That single sentence is the whole brief. Accessibility and reach are the goal; novelty is only a side effect of searching for reach. This matters because it changes what a "good" element is — an element that exposes the data to an audience that previously could not use it beats an element that looks better to an operator who already understands it.

## 2. The brief is deliberately starved

The model gets three things and nothing else:

1. The json-render.dev API and its documentation
2. How to access the data
3. The one-line objective from §1

Explicitly withheld: **any UI noun.** No "card," no "navigation," no "dashboard," no "table," no "panel." The prompt must not name a UI pattern, a layout, or a component. If the vocabulary is supplied, the model reproduces the vocabulary; the point is to find out what it builds when it has to invent the vocabulary itself.

The chat interface exists on top of this for the cases where something specific is needed — the operator can add context, run semantic searches, or ask for a particular surface. But the default path is the starved brief, because that is the path that produces things a human would not have specified.

## 3. Design-time generation, run-time determinism

This is the point most easily misread, and it was misread once in the source conversation.

**Not this:** the UI morphs with the data, rendering differently on every request, generated on the fly.

**This:** the LLM is a *UI designer*. It works offline and produces hundreds or thousands of **fixed** elements. Each element, once minted, is deterministic — the same data flows to the same place every time it renders. Known field goes here, known field goes there. Theme, color, and CSS may vary; the data handling does not.

What that means concretely:

- Generation cost is paid once, at design time, not per request. Once an element is rendered and approved, it is no longer a model problem — it is a static asset.
- No inference latency in the render path, no nondeterminism in front of an operator, no per-request token spend.
- Variety comes from the *number* of fixed elements and from which one is selected, never from re-generating one.
- An element can be reviewed, diffed, and approved, because it does not change after approval.

## 4. Gallery, then catalog — two stages with a gate

```
mint  →  GALLERY  →  (promotion)  →  CATALOG  →  rendered to site
         candidate                   approved
```

- **Gallery** is the raw output pool. Everything the model produces lands here. Unapproved. Nothing in the gallery reaches the site.
- **Catalog** is the approved set. Promotion from gallery to catalog is the human gate, and only catalog elements render to the site.

The existing `requirements.md` treats these as one 200-element rolling set. They are two sets with a review boundary between them.

## 5. Anti-convergence: memory is wiped

After each render, the model's memory is wiped. Each mint starts from zero.

The reason: a model that remembers its previous 40 elements converges — it starts refining a house style instead of exploring. Wiping forces each attempt to re-derive an approach from the data alone. This is a stronger mechanism than rejecting outputs that resemble existing gallery entries, because it prevents the convergence rather than filtering it after the fact.

## 6. Modes

### 6.1 Deliberation mode

A long-running mode with a time budget rather than a count budget:

> "I want it to have time to think and examine the data. Let it run for three hours and produce 100 or 10 — but it is really good, and cross examine."

Characteristics:
- Time-boxed (hours), not output-boxed.
- Yield count is an *outcome*, not a target. Ten excellent elements is a success; a hundred mediocre ones is not.
- The model is expected to cross-examine the data — look at it from multiple angles, compare blobs, test its own assumptions before committing to an element.

### 6.2 Hint seasoning

Into an otherwise unchanged starved brief, inject a single word or short phrase — industry, demographic, or domain. Not a description, not a requirement. A hint.

The worked example: **the single word "compliance."** Nothing else changes. The model decides what that implies for how the data should be reached.

This is the steering mechanism that preserves §2: it biases the exploration without supplying UI vocabulary.

## 7. Where this conflicts with the current spec

| Current `requirements.md` / `spec.md` | This vision |
|---|---|
| `spec.md` §3 mint scaffold prescribes `CONTEXT / DATA SHAPE / SIGNAL TO CONVEY / ACTION TO AFFORD / CONSTRAINTS / TRADE-OFF VECTOR` | Three-line starved brief. The scaffold supplies exactly the framing §2 forbids — "signal to convey" and "action to afford" are UI thinking handed to the model |
| US-3: one rolling gallery of exactly 200 | Two stages: unbounded-ish gallery, curated catalog, human promotion gate (§4) |
| US-1: "same surface may resolve to different elements over time" | Correct in effect, but the mechanism is *selection among fixed elements*, not regeneration (§3) |
| Rejection rule: reject if element resembles an existing gallery entry | Replaced/augmented by memory wipe (§5) — prevent convergence rather than filter it |
| Objective framed as novelty / exploring the constraint space | Objective is accessibility and reach (§1); novelty is instrumental |
| No mode concept | Deliberation mode and hint seasoning (§6) |
| No MCP / cross-blob concept | MCP as one data pool for cross-connections (§8) |

## 8. MCP as one data pool

The reason all blob schemas are served over MCP is to present them to the model as a **single pool** rather than per-plugin silos. That lets the model make cross-connections between blobs — join data across plugins that were never designed to be viewed together — which is where the "things humans wouldn't think of" come from.

Consequence: the model's data access must not be scoped per-plugin. It needs the whole pool, or it cannot find the cross-connections.

## 9. The staging table

Schema is dumped and sealed by us, so we have complete control over what the model and the renderer see. The open design question from the conversation, recorded as-is:

> "Maybe a staging table for UI that sits between the blobs and the UI."

Hard constraint on that table: **it must still present as schema.** If it is going to be rendered, the renderer needs schema, not rows. So the staging layer's output contract is JSON Schema, whatever its internal storage.

Supporting fact: the data is already verified at seal time. The renderer does not need to re-validate, which removes a whole class of runtime work from the render path.

## 10. What we actually have to work with (measured 2026-08-02)

The vision depends on the model being able to see field names and types. Measured against the two candidate sources:

**Proto layer — `proto/plugin_methods/`, 437 methods:**

| Output shape | Count | Renderable? |
|---|---|---|
| `Output {}` — empty message | 286 | No — nothing to lay out |
| exactly 1 field, `google.protobuf.Value` | 48 | No — opaque |
| exactly 1 field, `repeated google.protobuf.Value` | 42 | No — opaque |
| exactly 1 field, concrete scalar (`string`/`int64`/`bool`/`repeated string`) | 30 | Thin |
| 2+ typed fields | 31 | Yes |

So roughly **61 of 437 methods (14%)** carry anything a designer could deterministically lay out. 286 carry nothing at all, and 90 carry one opaque `Value`.

**Rust/JSON-Schema layer — `crates/op-plugins/src/state_plugins/`:** the same methods are fully typed. `netmaker list_networks` is `ListNetworksOutput {}` in proto, but `NetworkSummary { netid, address_range, address_range6, default_mtu, default_keepalive, ... }` with doc comments in Rust, carrying `JsonSchema` derive.

**Implication for the vision:** the protos are the wrong input for the designer. A `google.protobuf.Value` tells the model nothing to design against, and an empty message tells it less. The `<name>_schema()` JSON Schema from the sealed plugin blobs — served via MCP per §8 — is the only source with the field names, types, and descriptions the starved brief depends on. The staging table (§9) sits on that source, not on the protos.

A note already recorded in the Rust source confirms this was understood there:

> a type that degrades to `google.protobuf.Value` in the generated proto leaves callers with no schema.

## 11. What this means, in one paragraph

We are not building a UI that an AI redraws on demand. We are hiring an AI as a UI designer, giving it a deliberately impoverished brief so it cannot fall back on convention, pointing it at one unified pool of verified schema, wiping its memory between attempts so it cannot converge on a style, letting it deliberate for hours when we want quality over volume, and seasoning it with a single word when we want to bias it toward a domain. It produces a large body of fixed, deterministic elements. Humans promote the good ones from gallery to catalog. From that point on the elements are static assets with no model in the render path — which is what makes the whole thing affordable and reviewable.

---

## 12. Open questions carried forward

- Staging table: materialized rows that project back to JSON Schema, or a schema view over the sealed blobs directly?
- Promotion criteria for gallery → catalog. Human judgment only, or assisted by measured reach?
- How is "accessible to many industries and causes" evaluated? Without a measure, the objective in §1 is unfalsifiable.
- Does memory wipe (§5) apply within a deliberation run (§6.1), or only between runs? A three-hour cross-examination arguably requires memory *within* the run.
- Element selection: which catalog element serves a given surface, and who decides?
