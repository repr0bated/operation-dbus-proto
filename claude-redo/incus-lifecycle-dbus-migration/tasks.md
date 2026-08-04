# incus-lifecycle-dbus-migration - Implementation Tasks

## Safety gate after the network outage

- [x] **S-0 — Re-read current host state without lifecycle mutations.**
  - `xray` and `NetMaker` are running but have only loopback interfaces.
  - `incus-ct-xray` and `incus-ct-netmaker` report down.
  - `op-grpc-bridge` is still running from before the fallback, but its
    configured `10.200.0.2` bind address is currently absent.
- [ ] **S-1 — Hold production rollout until network recovery.**
  - Code, unit tests, and isolated-bus tests are allowed.
  - Do not restart `op-grpc-bridge`, any production container lifecycle
    service, OVS, uplink, DHCP, or the session bus until the separate
    `netclient-container-netns` recovery gate passes.

## Phase 0: Investigation (read-only, no live changes)

- [x] 0.1 Locate the generic `PluginV1::Call` dispatch implementation.
      - `SchemaBackedInterface::call` in
        `crates/op-grpc-bridge/src/schema_router.rs` performs lookup,
        validation, grant enforcement, and dispatch.
- [x] 0.2 Determine whether the check is bus-aware.
      - It is bus-agnostic and reads grants for the process-global identity
        sled footprint. It does not resolve the message sender or UID.
- [x] 0.3 Confirm read-method behavior.
      - `SideEffect::Query` does not exist; the enum is `Read|Mutation`.
      - Any non-null `required_capability` is enforced for either side-effect
        value; null skips the grant check.
- [x] 0.4 Read `deploy/runit/3tched-incus-svcgen` end to end.
      - Raw `incus` calls are emitted directly in `gen_one`'s heredocs.
- [x] 0.5 Update design.md and select the evidence-backed design.
      - Selected: a filtered, UID-0-only system-bus adapter for four Incus
        lifecycle/status methods. Generic dual registration and a shared
        root bypass are rejected.

## Phase 1: `get_instance_state` (FR-2) — additive, safe to ship alone

- [ ] 1.1 Add `GetInstanceStateInput { name: String }` and reuse
      or replace `InstanceRunningOutput` with a typed `InstanceStateOutput`
      containing `{name, status, running}` in `incus.rs`.
- [ ] 1.2 Implement `Self::get_instance_state` using the existing
      `incus_api_call("GET", "/1.0/instances/{name}/state", None)` path.
- [ ] 1.3 Register the method in `incus_schema()` as `SideEffect::Read` with
      `cap.service.incus.instance.read@v1` / `obs.service.incus.instance.state@v1`.
- [ ] 1.4 Add it to `dispatch_incus_method`; extend schema/output tests to
      cover `SideEffect::Read`, the capability/subid, and typed output.
- [ ] 1.5 `cargo check -p op-plugins` and `cargo test -p op-plugins incus`.
- [ ] 1.6 After Phase 2 and S-1 clear, verify the session route with an
      identity holding `cap.service.incus.instance.read@v1` and the system
      route from an uncredentialed UID-0 shell. A non-root system-bus caller
      must be denied.

## Phase 2: Root/boot-time auth fix (FR-1)

- [ ] 2.1 Implement a filtered Incus lifecycle interface on the real system
      bus. Expose only `get_instance_state`, `start_instance`,
      `stop_instance`, and `restart_instance`.
- [ ] 2.2 Resolve the D-Bus sender UID through
      `GetConnectionUnixUser`; require UID 0 and fail closed on missing sender
      or credential lookup failure. Keep the session-bus path unchanged.
- [ ] 2.3 Reuse `MutationEngine::dispatch_method_call` after authorization,
      recording actor `dbus:system:runit:uid0` and the schema capability ID.
- [ ] 2.4 Add isolated tests for root/allowed, non-root, missing sender,
      lookup failure, disallowed method, and unchanged session-bus grants.
- [ ] 2.5 Create one disposable test container (not any of the six
      production containers) for verification after S-1 clears.
- [ ] 2.6 From a root shell with no WG/session-bus identity, verify
      `start_instance`/`stop_instance`/`restart_instance` succeed against the
      test container and Incus API state reflects the change.
- [ ] 2.7 Verify NFR-3: an existing WG-identified session that previously
      lacked `cap.service.incus.instance.*` still cannot call these methods.
- [ ] 2.8 Delete the disposable test container through the approved lifecycle
      path.

## Phase 3: Generator template (FR-5)

- [ ] 3.1 Update `deploy/runit/3tched-incus-svcgen`'s `run`/`finish`/`check`
      templates per design.md's script sketch. Add an explicit staging/output
      root so generator tests never write under live `/etc/runit/sv`.
- [ ] 3.2 Generate into a `mktemp -d` staging root and confirm the emitted
      `run` contains `busctl` calls, not `incus start`/`incus stop`/
      `incus list`.
- [ ] 3.3 Remove the isolated temporary staging directory; no live service was
      created or enabled.

## Phase 4: Roll out to live services, one at a time (FR-3)

- [ ] 4.0 Generate and review repository definitions for all six containers,
      build once, review `sudo deploy/runit/build-golden.sh --dry-run`, then
      publish with `sudo deploy/runit/build-golden.sh`. Do not hand-copy files
      into `/etc/runit/sv`; publication installs them without automatically
      restarting the production containers.

For each container in this order — `cozo`, `qdrant`, `mail-3tched`,
`netmaker`, `assistant`, `xray` (cheapest blast radius first, `xray` last
since it terminates public TLS/SNI):

- [ ] 4.x.1 Verify the already-published live definition matches the reviewed
      repository definition before restarting that container.
- [ ] 4.x.2 `sudo sv restart incus-ct-<name>`.
- [ ] 4.x.3 Verify live: `sudo sv status incus-ct-<name>`, `incus list` shows
      `RUNNING`, and the container's actual service still responds (for
      `xray`: a real TLS/SNI request through it; for `mail-3tched`: SMTP/IMAP
      port reachability; etc.).
- [ ] 4.x.4 Only proceed to the next container after 4.x.3 passes.

- [ ] 4.1 cozo
- [ ] 4.2 qdrant
- [ ] 4.3 mail-3tched
- [ ] 4.4 netmaker
- [ ] 4.5 assistant
- [ ] 4.6 xray

## Phase 5: Repo parity (FR-4)

- [ ] 5.1 Confirm all six reviewed
      `deploy/runit/incus-ct-<name>/{run,finish,check,log/run}` sets are tracked.
- [ ] 5.2 `diff` each repo file against its live counterpart — must be empty.
- [ ] 5.3 Confirm the stale busctl-based
      `deploy/runit/incus-ct-netmaker/run` was replaced by the generated,
      reviewed post-migration definition before publication.

## Dependencies

```
Phase 0 → Phase 1 (independent of Phase 2, ship first)
       → Phase 2 → Phase 3 → Phase 4 → Phase 5
```

Phase 1 has no dependency on Phase 2 and should land first regardless of how
long Phase 2's investigation/implementation takes.

All live verification and Phase 4 additionally depend on S-1. Source work does
not.

## Notes on risk

Phase 4 is the only phase that touches production containers, and it touches
all six, including `xray` (public-facing) and `mail-3tched` (mail service).
NFR-1 and NFR-2 (from requirements.md) govern it: one container at a time,
disposable test container for Phase 2's own verification, live checks before
moving to the next, `xray` last.
