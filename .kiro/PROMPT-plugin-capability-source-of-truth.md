# Kiro Spec Request: Plugin as Sole Source of Truth + Full Capability Model

Create a Kiro spec (requirements-first: `requirements.md` with EARS acceptance
criteria, then `design.md`, then `tasks.md`) under
`.kiro/specs/plugin-capability-source-of-truth/`. Match the format of the
existing spec `.kiro/specs/op-dbus-mirror-event-session-refactor/`.

This spec is the canonical, fully-fleshed-out definition of the plugin pipeline.
Do NOT write implementation code — produce the spec documents only.

## Core principle (non-negotiable)

The **plugin is the schema and the sole source of truth** — it is the only
reason any object exists, of all things. **No plugin → no schema → no object.**
Nothing is ever mounted, registered, routed, or exposed that does not originate
from a plugin definition. Every D-Bus object, every gRPC-exposed method, every
projected present-state entity traces back to exactly one plugin as its origin.

## Definition of a Capability (this is the heart of the spec)

A **capability** represents the COMPLETE, ENUMERABLE set of everything an object
can be used to achieve: **all methods, all functions, and any other
functionality** the object exposes. A capability is the object's full functional
surface — not a coarse flag. Concretely, for each plugin/object the capability
surface must declare, in a structured (machine-enumerable) form:
- **methods/functions**: name, argument schema, return schema, side-effect class
  (read-only vs mutation), idempotency, and the required authorization
  capability to invoke it;
- **properties/fields**: name, type, default, constraints, read-only, immutable;
- **signals/events** the object emits;
- **guarantees**: rollback / checkpoint / verification / atomic support;
- **classification**: subid taxonomy (src/prj/sch/mut/obs/evt/exp), tags, OSCAL.

This capability surface lives in the schema (single source) and is what the
bridge serves and gates on.

## Verified current state (the gap this spec must close)

1. **The schema has no methods.** All 68 plugins in `/dev/shm/live-schema.json`
   carry only: name, category, version, description, fields, dependencies,
   example, immutable_paths, tags, dialect, mutation_index, subids. There is no
   `methods`/`functions` key anywhere. The functional surface is absent from the
   single source.
2. **Methods are invoked through an opaque `handle_command(command: &str, args)`**
   on the plugin trait — stringly-typed, not enumerable. No structured method
   declaration exists.
3. **`PluginCapabilities` is 4 bools** (`supports_rollback`, `supports_checkpoints`,
   `supports_verification`, `atomic_operations`) and is defined TWICE
   (`op-state/src/plugin.rs` and `op-plugins/src/plugin.rs`). It is runtime
   guarantee metadata, NOT the functional surface, and it is duplicated.
4. **`capability_id` is plumbed but never enforced.** It threads through
   `DbusCallRequest` → `grpc_client` → `SchemaEngine.mutate(... capability_id ...)`
   but is `_capability_id` everywhere (ignored). No verify/check/grant exists.
5. **`SchemaBackedInterface.call` (in op-grpc-bridge/schema_router.rs)** validates
   the method against `route.methods`, which is extracted from a schema
   `"methods"` key that does not exist → always empty → validation always
   skipped; and it returns a stub `{"success": true}` instead of dispatching to
   the real execution path (`SchemaEngine.mutate`).
6. **Three registrars for `/org/opdbus/v1/plugins`**: op-projection (per-entity
   present-state), op-dbus-mirror (PluginInterface), op-grpc-bridge
   (SchemaBackedInterface). **Four crates request the bare bus name
   `org.opdbus.v1`**: op-state (dead — no s6 service), op-openvswitch-daemon,
   op-dbus-mirror, op-grpc-bridge. Only one process may own a well-known name.
7. **Runtime ≠ source.** Live owners: `org.opdbus.v1.plugins` (op-projection pid
   2194), `org.opdbus.v1.mirror` (op-dbus-mirror), `org.opdbus.v1.plugins.ovsdb`
   (op-openvswitch). An orphan service runs `/usr/local/bin/opdbus` that no crate
   defines. Source claims don't match live ownership — needs reconciliation.
8. **The per-entity projected tree is live-computed in op-projection**
   (procfs_reader, plugin_reader) and pushed to D-Bus; it is NOT in SHM. SHM
   holds schema only.

## Target architecture (decided — design to this)

- **Plugin trait declares its full capability surface** (methods/functions with
  arg+return schema + required-capability + side-effect class; properties;
  signals; guarantees; subids/tags). This replaces the opaque `handle_command`
  catch-all with an enumerable surface. The plugin definition IS the object.
- **Producer = op-projection (single producer, ONE place).** It emits the
  complete capability surface (methods + fields + signals + guarantees +
  classification) into SHM: per-plugin schema files under
  `/dev/shm/opdbus/schemas/`, the combined monolith `/dev/shm/live-schema.json`,
  and the atomic `.manifest.json` holding the single `catalog_hash` (leaf+fold,
  incremental, never recomputed by consumers). It ALSO writes present-state
  projections into SHM (Producer→SHM→Bridge), so present-state is in the
  authoritative SHM surface, not pushed peer-to-peer over D-Bus.
- **Bridge = op-grpc-bridge is the SOLE owner** of the canonical plugins bus
  name and the single registrar / projected tree. It reads SHM (capability
  schema + present-state), registers the entire D-Bus object tree from it,
  auto-exposes every object/method over gRPC, dispatches calls to the real
  `SchemaEngine.mutate` (no stub; `json_args` used), validates methods against
  the schema capability surface, and ENFORCES `capability_id` against the
  caller's identity (footprint/sessionid from the GhostbridgeInterceptor) at this
  single enforcement point before executing.
- **Trim the redundant registrars:** op-dbus-mirror drops plugin-object
  registration and its `org.opdbus.v1` name claim (keeps only ovsdb / nonnet /
  mirror-management). op-projection drops D-Bus object serving and becomes
  producer-only (→ SHM). Delete the dead op-state D-Bus name claim, the
  op-openvswitch-daemon bare-name claim, and the orphan `opdbus` service/binary.
- **Dedup `PluginCapabilities`** to one definition; fold its 4 guarantee bools
  into the richer per-method/per-object capability model.

## Plugin Autogeneration (built, but not fleshed out — spec it as a first-class stage)

The generate-a-plugin-on-missing path already EXISTS but is under-specified and
must become a complete lifecycle stage. What exists today:
- `AutoPlugin` + `AutoPlugin::create_from_requested_info(name, requested_info)`
  in `crates/op-plugins/src/auto_create.rs`.
- `query_elements_via_agent` spins up the **search-specialist** agent to research
  the plugin's fields/capabilities from the request.
- `default_registry.rs` already catches an unknown plugin and calls it:
  *"Unknown plugin '{}'; auto-creating review-required draft from requested info"*.
- `op-introspection` has `can_auto_generate(service_name)`.

What is missing (the spec must close this):
1. **It only produces draft-metadata, not the object's capability surface.**
   `build_auto_schema` emits fields ABOUT the draft (plugin_id, requested_info,
   status, pending_human_review, review_reason, recommended_fields, research,
   web_results_count, created_at) — NOT the enumerable methods/functions/
   properties/signals the object actually exposes. Autogeneration must synthesize
   the FULL capability surface defined above.
2. **It dead-ends as `draft_pending_review`** — never activates, never persists as
   a durable plugin definition, never flows through producer→SHM→bridge.
3. **No defined lifecycle.** Specify the complete state machine:
   missing object referenced → **Gemma researches the object's properties** →
   **full capability surface synthesized** (methods + args/returns + side-effect
   class + required-capability + properties + signals + guarantees + subid/tags)
   → human/agent review & approval → **persisted as the plugin definition (the
   sole source of truth)** → projected to SHM by the producer → registered and
   served live by the bridge. Include rejection, revision, and idempotent
   re-generation paths, and how a draft is quarantined (not served) until
   approved.

## Gemma owns object-property / capability-surface research

Gemma is the single schema-reasoning brain. Its responsibilities, which this spec
must reflect, are: subid classification (existing), tag routing (existing),
schema-driven UI generation across the 4 perspectives — Data/Numeric,
Spatial/Layout, User Flow, Context/Aesthetic (existing), and NOW **researching an
object's properties and synthesizing its full capability surface** for plugin
autogeneration (new). The autogeneration path must call **Gemma** for
object-property/capability research, replacing the generic
`create_agent("search-specialist")` seam in
`crates/op-plugins/src/auto_create.rs::query_elements_via_agent`. All schema
reasoning stays concentrated in Gemma (single source), never fanned out to a
generic agent. Note: Gemma is a plugin/StatePlugin itself, not a bespoke handler.

## Hard rules (must hold throughout the spec)

- ONE schema, ONE source of truth, computed in exactly ONE place; no derivation
  (e.g. catalog_hash) duplicated across files or recomputed by consumers.
- The ONLY valid path is `org.opdbus.v1.plugins`. New capabilities are plugins;
  never new `operation.<domain>.v1.*Service` protos; never raw ip:port.
- SHM is the authoritative present-state. Components READ it. NO polling loops,
  NO watchers; action is triggered by arrival (an inbound connection), not by a
  timer.
- Durability is the per-mutation immutable chain (the backup). No SQL anywhere
  (cozo→rocksdb, not sqlite); no btrfs-snapshot backups; no parallel persistence.
- Zero-trust: the identity footprint/sessionid (container id =
  Argon2(PSK, salt=WG-pubkey)) is the gate; capability enforcement augments it at
  the bridge. No container gets a NIC or IP; all container I/O over UDS.
- Subid taxonomy (src/prj/sch/mut/obs/evt/exp) classifies every object/capability.

## Spec deliverables

- `requirements.md`: Introduction, Glossary, and numbered Requirements each with
  a User Story and EARS acceptance criteria (WHEN/WHILE/IF/WHERE … THE … SHALL …).
  Cover: the plugin capability-surface declaration; the producer emitting it to
  SHM (schema + present-state + manifest); the bridge as sole owner/registrar/
  projected-tree; real dispatch to SchemaEngine.mutate; method validation from
  schema; capability_id enforcement; removal of duplicate registrars and dead
  name claims; PluginCapabilities dedup; reconciliation of runtime≠source.
- `design.md`: architecture, data flow (plugin → producer → SHM → bridge →
  gRPC/D-Bus), the capability schema format (JSON), trait changes, ownership
  handoff sequence on the live bus, and migration/rollout.
- `tasks.md`: incremental, checkbox implementation plan with `_Requirements: X.Y_`
  references, ordered so the workspace builds at each step.
