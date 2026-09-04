# Control-Plane Chatbot Identity and Delegation

**Status:** Decision record, 2026-09-04 (confirmed with Jeremy in session). Builds on
`standalone-emqx-identity-mcp/` FR-12..FR-14 (HOT five + `toolsets` projection — that
IS the chatbot's lazy-loading surface; the op-web compact meta-tools are the thing
FR-12.3 retires) and corrects the identity paragraph in `CLAUDE.md` (Argon2/PSK
sessionid, xray header injection) which describes a model that was repudiated on
2026-08-04 (`netmaker-xray-identity-handoff/` D-1..D-7).

## 1 · Identity — what was decided and what is live

| Fact | Decision | Implementation |
|---|---|---|
| WireGuard termination | Oracle decoy only. **No WG on the VPS/host.** The host never runs `wg show`. | `op-decoy-issuer` runs on the decoy; host has only WARP egress (`wgcf*`). |
| Key storage | WG public keys live in **Cozo**: humans in `human_principal`, sessions in `identity_sled`. | `op-cozo-store::HumanPrincipalRecord`, `/var/lib/op-dbus/identity-cozo` |
| `session_id` | `blake3::derive_key("op-identity session-id v1", pubkey)[..16]` → UUID. Equals the identity container name. | `op_identity::session::derive_session_id` |
| `principal_id` | `blake3::derive_key("op-identity human-principal v1", pubkey)[..16]` → UUID. Distinct context, cannot collide with a session id. | `op_identity::session::derive_principal_id` |
| `genesis` (arrival anchor / "activity id") | `blake3(pubkey ‖ chain_head ‖ head_ts ‖ catalog_hash ‖ arrival_ts)`, minted once per session, stamped on every snowball event. | `op_identity::session_genesis::mint_genesis` |
| Human credential | OIA1 (decoy-signed Ed25519, ≤900 s, nonce) carried **inner** as `x-oracle-identity-assertion-bin`. Xray is passthrough. | `op-grpc-bridge` is the sole validator |
| Local service credential | SID1 sealed envelope authored by MutationEngine, stored in the sled `sealed_id`, projected to `/dev/shm/opdbus/credentials/identity_sled.json` (root:secrets). | `op_identity::sealed_id`, `mcp_frontend::authenticate_sealed_id` |
| Retired | Argon2(PSK, salt=pubkey) sessionid (`derive_session_id_from_psk` reachable only via explicit `psk` arg), `X-Ghostbridge-Footprint` injection by xray, host `wg-chatbot`/`wg-decoy` tunnels (deleted `e4f656f6`), polling `decoy-identity-watcher.sh`. | Docs mentioning these are stale. |

## 2 · Two chatbots, two principals

The system has **two different things** that have both been called "the chatbot".
They must never share an identity.

### 2.1 Control-plane chatbot (service principal)

- The singleton daemon reasoning loop. Principal is server-side configuration
  (`OP_MCP_IDENTITY_*` on `op-grpc-bridge`), registered in `human_principal` with
  `display_alias = "control-plane-chatbot"`, sealed with SID1 at bridge start.
- Its **only** surface is the authenticated MCP door at `:8090/mcp`, presenting its
  SID1. It sees what FR-12/13/14 already define: the HOT five (`memory_recall`,
  `memory_store`, `workflow_query`, `workflow_run`, `toolsets`), then one selected
  WARM/COLD set at a time via `toolsets` → re-list. `/etc/opdbus/mcp-toolsets.json`
  (gen 3 live) and `/etc/opdbus/mcp-audience-policy.json` (pins
  `singleton_chatbot_principal_id = 87b0decc…`) are the mechanism. The op-web
  in-process compact meta-tools (`list_tools`/`search_tools`/`get_tool_schema`/
  `execute_tool` in `orchestrator/tools.rs`) are exactly what FR-12.3 retires — the
  chatbot loop must stop using them and go through the door like every other client.
- **Delegation = an `agents` toolset.** Add a WARM set (e.g. `agents`) whose typed tools
  are the `op-agents` D-Bus agents (`rust_pro`, `backend_architect`,
  `network_engineer`, `context_manager`, …). Each agent tool is one canonical
  `PluginService.CallMethod` dispatch to `op-agents`; the agent runs the underlying
  tools under **its own** principal with `parent_principal_id = chatbot` and
  `on_behalf_of = <human principal>` when the request originated from a UI session.
  Required skills and useful plugins are further WARM/COLD sets the chatbot may select.
  Promotion into a set is a reviewed manifest change with a generation bump (FR-14.7).
- **No direct execution.** `shell_exec`, `file_*`, raw `dbus_call`, `sv_*`, and every
  other direct-execution tool are in **no** set the chatbot may select and have no
  exact-`principal_id` grant row for it (FR-14.6: name-guessing is denied at call
  time). "Compact MCP" in conversation means this HOT+`toolsets` projection, not a
  generic execute escape hatch.
- **No internet in production.** Reasoning model defaults to local `op-gemma`.
  Remote providers are simply not declared in the production `tched_router` config.
- Must **never** be handed out as a fallback identity to anything else (see §4).

### 2.2 UI chat instance (human principal)

- The conversation that opens when a human logs in to the dashboard.
- Runs **as the human**: `principal_id` from the human's OIA1 (remote) or SID1 (locally
  registered human sled). `ChatService.Send` already dispatches `tched_router.Chat`
  with the caller's `GhostbridgeIdentity` — the UI must send that identity.
- `actor_id` on every mutation is the human's `principal_id`; when the human's chat
  invokes the control-plane chatbot, the resulting agent work carries
  `on_behalf_of = human`.
- The legacy `POST /api/chat` orchestrator path in op-web (random UUIDv4 session,
  free-text `user_id`, `op_tools` executed with op-web's ambient bus credentials)
  carries no identity and is **not** an acceptable production chat path.

## 3 · Model selection

Both the control-plane chatbot and UI chats select models **through `tched_router`**.
No `chat_manager.current_model()` process-global default.

- `tched_router` already exposes `ListModels`, `SetProvider`, `SetModel`,
  `SetSelection`, and purpose-scoped setters (`SetOvsRoutingModel`,
  `SetObfuscationModel`, `SetVectorizationModel`, `SetQdrantRetrievalModel`,
  `SetCozoRetrievalModel`).
- Add two purpose-scoped selections: **`SetControlPlaneModel`** (the chatbot's
  reasoning model; production default `gemma` local) and **`SetUiChatModel`** (default
  for new human conversations; a human may override per conversation with
  `provider`/`model` on `ChatService.Send`, constrained to routes the router declares).
- The UI exposes one picker per purpose, fed by `ListModels` / `GetModelRoutes`.
- Production config declares only local providers; the picker therefore shows only
  local models. No code path special-cases "offline".

## 4 · Guardrails (fail-closed)

- G-1 `resolve_identity_session(None)` / `configured_identity_session()` must **not**
  resolve to a service principal. The chatbot sled carries `principal_kind = service`;
  the single-current-session fallback applies to human sleds only. Today the chatbot is
  the only anchored sled, so op-web, `/admin/paircode/new`, and `/pair` all silently
  inherit its genesis. This is the "chatbot's tunnel became the host identity" bug from
  June in a new shape.
- G-2 `/pair` mints a bearer that is never checked (`lookup_paired_token` has no
  callers). **Resolved 2026-09-04 (Jeremy): do not build enforcement middleware for it.**
  The pairing path exists to serve zeroclaw-gui (`crates/zeroclaw-gui`, already out of the
  workspace; `crates/op-web/ui` = package `zeroclaw-gui-repo`, whose built `dist/` op-web
  still embeds and which still calls `paircode`). That whole display stack is being retired
  in favour of a json-render frontend (the json-render Vercel repo); the production frontend
  repo does not exist yet. `/pair` and `/admin/paircode/new` are therefore **deleted together
  with the display stack**, not guarded — writing token middleware would be new auth
  machinery on a path slated for removal. Interim mitigation already landed: both endpoints
  refuse a service principal (below), so the unchecked bearer can no longer be minted against
  the control-plane chatbot. The replacement frontend authenticates with OIA1 (remote human)
  or SID1 (locally registered human) like every other client — no third credential type.
- G-3 The control-plane chatbot has no direct-execution grants and no set containing
  a direct-execution tool. A test asserts a `tools/call` by the chatbot principal for
  any tool outside HOT ∪ its selected set (and for any `shell_*`/`file_*`/raw
  `dbus_call` name regardless of selection) is denied.
- G-4 `ctl_plane_chatbot` reasoning episodes carry `principal_id`, `session_id`,
  `session_genesis`, and `on_behalf_of` so an episode can name whose conversation it
  came from and who acted.
- G-5 The chatbot keypair exposed in the 2026-09-04 session
  (`/var/lib/opdbus-runtime/identities/chatbot/private.key`, `mcp_token`) is rotated:
  new keypair → new blake3 session/principal → `OP_MCP_IDENTITY_*` in the bridge run
  script → incus `user.opdbus.*` on the identity container → `capability-grants.json`
  row → identity dir.

## 5 · Implementation order

1. G-5 key rotation (operator action, needs `sudo sv restart op-grpc-bridge`).
2. G-1/G-2 fail-closed fallback + pairing (op-identity `session_projection`, op-web
   `pair.rs`); tests.
3. `tched_router` `SetControlPlaneModel` / `SetUiChatModel` + `ListModels` picker wiring;
   op-web chat reads the router selection instead of `chat_manager.current_model()`.
4. Chatbot loop moves onto the MCP door: `agents` WARM set in `mcp-toolsets.json`
   (gen bump) backed by typed `op-agents` dispatch tools; op-web orchestrator's
   in-process compact meta-tools and `op_tools` registry removed from the chatbot path;
   the loop authenticates with its SID1 (`op-identity-headers`-equivalent in-process)
   and drives HOT + `toolsets`; system prompt rewritten (no s6, no shell/file).
5. UI `ChatTransport` sends the human's SID1/OIA1 metadata on `ChatService.Send`;
   `ctl_plane_chatbot` episode schema gains identity fields (G-4).
6. CLAUDE.md identity paragraph corrected to §1 (done 2026-09-04); CLAUDE.md's
   "Compact mode (the 4 meta-tools) is in-process in op-web for the singleton
   control-plane chatbot" sentence becomes stale once step 4 lands and must be updated
   then.
