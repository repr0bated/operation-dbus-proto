thread_id: 019ef94b-af38-77f0-9343-95210573d1d6
updated_at: 2026-06-24T11:50:12+00:00
rollout_path: /home/jeremy/.codex/sessions/2026/06/24/rollout-2026-06-24T07-02-27-019ef94b-af38-77f0-9343-95210573d1d6.jsonl
cwd: /home/jeremy/git/operation-dbus-proto
git_branch: feat/sled-source-port-salt

# Fixed the shared unix-socket ownership bug and verified the canonical qdrant-facing bridge, but Qdrant semantic search still failed because the shuttle was not configured.

Rollout context: The user was debugging the post-refactor D-Bus / gRPC bridge and insisted that the system be checked on both the session and system buses. The key practical goal became: find where the shared unix socket is projected in the D-Bus tree, make `createunixsocket` work without clobbering the serving socket, then test the qdrant endpoint through the canonical backplane rather than direct container access.

## Task 1: Inspect buses, projections, and qdrant reachability

Outcome: partial

Preference signals:

- The user said, “there was a refactor, so check both session and system busses” -> future runs should inspect both buses explicitly after backplane refactors instead of assuming the old bus is still authoritative.
- The user said, “look for sockets in dbus tree” and “there are no proxy devices. the unix socket is craeated with the plugin method create_unix_socket” -> future runs should search the projected D-Bus tree for socket-related objects/methods rather than assuming an Incus proxy path.

Key steps:

- Verified that the system bus only exposed `org.opdbus.v1.mirror` and `org.opdbus.v1.plugins.ovsdb`, not the plugin projection tree the user expected.
- Confirmed `/dev/shm/opdbus/projections/knowledge.json` existed but still did not contain real qdrant-derived content; it was only a wrapper with empty `data`.
- Confirmed the qdrant container itself was healthy when accessed inside the container: it listened on `127.0.0.1:6333` and `127.0.0.1:6334`, and `/collections` returned collections including `repomix_rag`, `repos_lsp_*`, and `ctl_plane_reasoning_episodes`.
- Confirmed host endpoints were closed or timed out: `127.0.0.1:6333`, `127.0.0.1:6334`, and the direct container IP path were not usable from the host.

Failures and how to do differently:

- The qdrant semantic RPC still returned `FailedPrecondition: Qdrant Semantic Shuttle is not configured; check Voyage and Qdrant settings` over both TCP and Unix-socket bridge paths. That means the service is up, but the semantic shuttle isn’t configured/linked, so direct qdrant semantic verification is blocked until environment wiring is fixed.
- A lot of the bridge state is split across `op-dbus` and `op-grpc-bridge-zeroclaw`; future debugging should always identify which process owns which listener before attributing the failure to the wrong binary.

Reusable knowledge:

- The live qdrant container is healthy inside the container namespace, but host reachability depends on the bridge, not on raw `localhost:6333`.
- The qdrant semantic search endpoint is `operation.v1.EventChainService/SearchSemanticTrace`, not `PluginService`.
- The qdrant shuttle requires `COGNITIVE_MCP_QDRANT_URL`, `COGNITIVE_MCP_QDRANT_COLLECTION`, `COGNITIVE_MCP_USER_MEMORY_COLLECTION`, `COGNITIVE_MCP_SCHEMA_SLED_PATH`, and Voyage credentials to initialize.

References:

- [1] `busctl --system list` showed only `org.opdbus.v1.mirror` and `org.opdbus.v1.plugins.ovsdb`.
- [2] `incus exec qdrant -- sh -lc 'curl -fsS --max-time 3 http://127.0.0.1:6333/collections'` returned a healthy collection list.
- [3] `grpcurl ... operation.v1.EventChainService/SearchSemanticTrace` returned `FailedPrecondition: Qdrant Semantic Shuttle is not configured; check Voyage and Qdrant settings`.

## Task 2: Fix unix_socket ownership and make the canonical bridge authoritative

Outcome: success

Preference signals:

- The user explicitly corrected the model with “there are no proxy devices. the unix socket is craeated with the plugin method create_unix_socket” -> future runs should treat the unix socket as plugin-managed state, not an Incus proxy.
- The user’s “fix” after the failed qdrant test indicates they want the current live failure resolved end-to-end, not just source-level cleanup.

Key steps:

- Rebuilt and installed both live binaries: `op-dbus` and `op-grpc-bridge-zeroclaw`.
- Patched `crates/op-plugins/src/state_plugins/unix_socket.rs` so `ensure_bound` no longer unlinks an existing `/run/ghostbridge/container.sock`; it now treats an existing shared socket as externally owned transport and only registers metadata.
- Patched `crates/op-grpc-bridge/src/mutation_engine.rs` comment to match the new non-destructive behavior.
- Verified the live shape after restart:
  - `op-dbus` owns `10.200.0.1:50051`
  - `op-grpc-bridge-zeroclaw` owns `0.0.0.0:8090`
  - only `op-grpc-bridge-zeroclaw` owns `/run/ghostbridge/container.sock`
- Confirmed the Unix socket now serves reflection and the full operation service set.
- Exercised `operation.v1.PluginService/CallMethod` through the canonical bridge with `createunixsocket` for `qdrant` and verified that `/dev/shm/opdbus/projections/unix_socket.json` contains the expected state:
  - `name: qdrant`
  - `path: /run/ghostbridge/container.sock`
  - `ports: [6333, 6334]`
  - `protocol: grpc`

Failures and how to do differently:

- The protobuf JSON response for the `createunixsocket` call rendered the `ports` list as `[null, null]` even though the persisted projection file had correct numeric ports. That suggests a serialization/rendering mismatch in the response path, not in the persisted fold.
- The qdrant semantic RPC still failed with `FailedPrecondition`, so the transport fix did not by itself wire the semantic shuttle.

Reusable knowledge:

- `UnixSocketPlugin::ensure_bound` must not blindly unlink a socket that is already the shared transport.
- The canonical socket path is `/run/ghostbridge/container.sock`.
- The operation service surface on the Unix socket includes `grpc.reflection.v1.ServerReflection`, `operation.v1.PluginService`, `operation.v1.EventChainService`, `operation.v1.RuntimeMirror`, `operation.v1.StateSync`, `operation.v1.DbusPassthrough`, `operation.v1.OvsdbMirror`, `operation.v1.RegistrationService`, `operation.mail.v1.MailService`, `operation.privacy.v1.PrivacyNetworkService`, `operation.registry.v1.ComponentRegistry`, `op_cache.*`, `op_chat.chat.ChatService`, and `zeroclaw.ZeroclawService`.

References:

- [1] `crates/op-plugins/src/state_plugins/unix_socket.rs` now registers on an existing shared transport instead of removing it.
- [2] `cargo check -p op-plugins`, `cargo check -p op-grpc-bridge`, and `cargo check -p op-web --bin op-dbus` all passed after the patch.
- [3] `cargo build --release -p op-web --bin op-dbus -p op-grpc-bridge --bin op-grpc-bridge-zeroclaw` completed successfully.
- [4] `sudo ss -lxnp | rg '/run/ghostbridge/container.sock|zeroclaw'` showed only `op-grpc-bridge-zeroclaw` bound to the shared socket after restart.
- [5] `grpcurl ... operation.v1.PluginService/CallMethod` for `createunixsocket` returned success and the projection file contained the correct qdrant socket registration.
