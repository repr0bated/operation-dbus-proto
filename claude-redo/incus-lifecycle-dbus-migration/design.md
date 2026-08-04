# incus-lifecycle-dbus-migration - Design

## Phase 0 findings (verified 2026-08-03)

The open question is resolved in source.

1. `SchemaBackedInterface::call` in
   `crates/op-grpc-bridge/src/schema_router.rs` is the generic
   `org.opdbus.v1.PluginV1.Call` implementation. It validates the declared
   method and arguments, reads `required_capability`, loads grants for the
   **process-global identity sled footprint**, and returns `AccessDenied` when
   the capability is absent.
2. That check is bus-agnostic. It does not inspect the D-Bus message sender,
   peer UID, or whether the interface was registered on the session or system
   bus. Dual registration alone would therefore reject the same call, or an
   unsafe blanket bypass would expose it to every caller allowed by the system
   bus policy.
3. `SideEffect` has only `Read` and `Mutation`; `SideEffect::Query` does not
   exist. Side-effect classification does not control authorization. Any
   non-null `required_capability` is checked, including for `Read` methods.
4. A null `required_capability` skips the grant check. That is not suitable for
   lifecycle mutation and is not required for the status query once the
   local-root system-bus route below exists.
5. `MutationEngine::dispatch_method_call` records the event and then invokes
   `dispatch_incus_method`. The current local draft already dispatches
   `start_instance`, `stop_instance`, and `restart_instance` natively through
   the Incus HTTP-over-UDS API, but it has no `get_instance_state` dispatcher.
6. The OpenFlow precedent was previously described incorrectly.
   `state_plugins/openflow.rs` is a system-bus **client proxy**. The actual
   peer-credentialed service registration lives in
   `crates/op-network/src/bin/op-of-controller.rs`.
7. `deploy/runit/3tched-incus-svcgen` was read end to end. It emits raw
   `incus` CLI calls directly from the `run`, `finish`, and `check` heredocs.

## Selected design: filtered local-root system-bus adapter

Do not dual-register the entire generic plugin catalog and do not add a
process-wide root bypass to `SchemaBackedInterface::call`.

Add a second, narrowly scoped registration in `op-grpc-bridge` with these
properties:

- it registers only the `incus` lifecycle route on the real system bus;
- it permits only `get_instance_state`, `start_instance`, `stop_instance`, and
  `restart_instance`;
- it reads the D-Bus message sender and resolves the sender's Unix UID through
  `org.freedesktop.DBus.GetConnectionUnixUser` (behind a testable authorizer);
- it accepts only UID 0 and records a fixed actor such as
  `dbus:system:runit:uid0` plus the schema-declared capability ID;
- after that peer-credential and method-allowlist gate, it reuses
  `MutationEngine::dispatch_method_call`, so native Incus dispatch and the
  immutable audit chain remain shared with gRPC/session-bus calls;
- the existing session-bus `SchemaBackedInterface` and its sled-footprint
  capability check remain byte-for-byte behaviorally unchanged.

The system-bus service may use the same canonical destination/path/interface
on that distinct bus, allowing runit scripts to use `busctl --system`, but it
must register a filtered interface rather than the full Incus method map.
System-bus policy currently allows local callers to send to `org.opdbus.*`, so
the in-service UID check is mandatory and must be covered by negative tests.

## FR-2: read-only `get_instance_state`

Add a read method to
`incus_schema()` (`crates/op-plugins/src/state_plugins/incus.rs`, alongside
the existing `methods.insert("start_instance", ...)` block at ~line 1718):

```rust
methods.insert(
    "get_instance_state".to_string(),
    super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
        GetInstanceStateInput,
        InstanceStateOutput,
    >(
        "get_instance_state",
        op_state_store::SideEffect::Read,
        false,
        "cap.service.incus.instance.read@v1",
        "obs.service.incus.instance.state@v1",
    ),
);
```

backed by a new `Self::get_instance_state(name) -> Result<InstanceStateOutput>`
that reuses the existing `incus_api_call("GET", "/1.0/instances/{name}/state", None)`
path already exercised by `Self::resolve` (~line 333) and
`set_instance_running` (~line 397) — no new HTTP surface, just exposing what
already exists as a queryable method instead of only an internal resolve
step. `InstanceStateOutput` must include the resolved name, raw Incus status,
and a derived `running` boolean so runit never parses documentation/example
state. The read capability remains enforced on the session bus. The selected
system-bus adapter authorizes the root-owned runit caller by peer credentials
and threads the same capability ID into the audit event.

## Script changes (FR-3, FR-4)

`run` (per container, e.g. `incus-ct-xray`):

```sh
NAME='xray'
BUSCTL_ARGS="--system"

incus_call() {  # $1=tool $2=json-args
    busctl $BUSCTL_ARGS call org.opdbus.v1.plugins /org/opdbus/v1/plugins/incus \
        org.opdbus.v1.PluginV1 Call ss "$1" "$2"
}

state() {
    incus_call get_instance_state "{\"name\":\"$NAME\"}" | ...  # parse "running"
}

if [ "$(state)" != running ]; then
    incus_call start_instance "{\"name\":\"$NAME\"}" || { echo "start failed for $NAME" >&2; exit 1; }
fi
# poll via get_instance_state instead of `incus list`, same timeout/backoff shape as today
```

`finish` mirrors this with `stop_instance`. `check` calls `get_instance_state`
once and greps the parsed `running` field. The existing timeout/retry
structure, `wait_dep` helper, and `trap`-based cleanup in the current scripts
are kept as-is — only the state-query and mutation primitives change, per
NFR-4 (behaviour-preserving).

## Generator update (FR-5)

`deploy/runit/3tched-incus-svcgen` currently emits the raw-CLI `run`/
`finish`/`check` bodies as inline heredocs/templates inside `gen_one`. The fix
is a template edit in that one file so every future
`3tched-incus-svcgen <name>` invocation produces the D-Bus version. Add an
explicit staging/output-root option so tests generate into `mktemp -d` rather
than writing scratch definitions into live `/etc/runit/sv`.

## Rollout order

1. Phase 0 investigation (complete) — read-only, no live changes.
2. FR-2 (`get_instance_state`) lands and is unit/isolated-dispatch tested —
   additive and safe to build alone. Live bridge verification waits for the
   post-outage network gate because the running bridge must not be restarted
   while `10.200.0.2` is absent.
3. FR-1 filtered local-root system-bus adapter lands and is verified against a
   disposable test container per NFR-1 — still no production container
   touched.
4. FR-5 (generator template) updated and used to generate/review all six
   repository definitions. Do not hand-copy definitions or binaries onto the
   running host.
5. Build once, review `sudo deploy/runit/build-golden.sh --dry-run`, then
   publish the reviewed golden/live artifact with
   `sudo deploy/runit/build-golden.sh`. The publisher installs definitions but
   does not auto-restart network-critical services.
6. FR-3 restarted **one container at a time** per NFR-2, cheapest/lowest-
   blast-radius first (suggested order: `cozo` or `qdrant` first, `xray`
   last, since it terminates public TLS/SNI).
7. FR-4 — verify every repository definition is byte-identical to the live
   installed file after rollout.

## Testing Strategy

- Unit test alongside `migration_methods_are_exposed_by_schema` (existing,
  `incus.rs`) extended to assert `get_instance_state` is present, tagged
  `SideEffect::Read`, and has typed output.
- Unit-test the local authorizer independently: UID 0 + allowed method passes;
  non-root, missing sender, credential lookup failure, and any method outside
  the four-method allowlist fail closed.
- Integration-test the system-bus adapter against an isolated test bus where
  possible, then use one disposable Incus container for the live lifecycle
  proof. Production containers remain untouched until the rollout gate.
