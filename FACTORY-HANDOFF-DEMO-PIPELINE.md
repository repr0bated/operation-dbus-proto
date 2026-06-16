# Factory Handoff: GhostBridge Demo Pipeline / Vectorization / Accountability Loop

This handoff is for continuing work in `/home/jeremy/git/operation-dbus-proto`.

Jeremy is voice-typing. If a word looks wrong, infer from architecture context. Example: “fertilization” meant “vectorization.” “factory” means the next agent/session in Factory.ai.

## Goal

Get the whole GhostBridge pipeline working end-to-end enough for the system to be presentable:

- Chatbot at the top.
- Semantic evidence / Qdrant evidence pane at the bottom.
- Real data, not mock/demo-only strings.
- The chatbot must have the information it needs to answer successfully.
- A user must be able to confront the chatbot with live semantic evidence.
- The accountability chain must carry identity, trace, footprint, schema/subid, and vectorization metadata.

This is not just UI. The critical path is the data/control pipeline:

1. A request enters through the governed path.
2. Identity/trace/footprint are attached.
3. The action/session is recorded in canonical evidence.
4. Evidence is embedded/vectorized.
5. Qdrant can retrieve it semantically.
6. The chatbot can use or be confronted by that evidence.
7. UI shows chatbot top and evidence bottom.

## Non-Negotiable Architecture Rules

- There is only one live D-Bus bus: `org.opdbus.projection`.
- Plugin objects live under `/org/opdbus/v1/plugins/<name>`.
- Legacy `/opdbus/v1/...` paths are wrong. Fix stale strings when touched.
- Plugin IS schema. If a thing does not have validated plugin schema, it does not exist in the system.
- `plugin_schema_defs.rs` is an aggregator/formatter only. Do not move schema authority there if the plugin owns its schema in the `.rs` file. Jeremy explicitly corrected this.
- D-Bus is the control plane. D-Bus object existence is system existence.
- `uuid` is machine identity. Never replace it with `subid`.
- `subid` is the human operational taxonomy key and must be an OSCAL prop value, not remarks.
- `mut.*` records require `actor_id` and `capability_id`.
- `evt.*` records require `event_id` or `event_hash`.
- Compliance mappings belong in metadata arrays, not inside the `subid` string.
- Use Rust-first patterns. Avoid Python unless there is no reasonable shell/Rust path.
- Do not revert unrelated dirty worktree changes.

## Known Current State

Branch: `feat/sled-source-port-salt`

Dirty worktree exists. Do not revert user changes.

Important modified/untracked files from this work:

- Modified: `crates/op-cognitive-mcp/src/qdrant_shuttle.rs`
- Modified before/around this work: `crates/op-cognitive-mcp/src/notebooklm.rs`
- Modified before/around this work: `crates/op-plugins/src/state_plugins/zeroclaw.rs`
- Added/modified pitch files under `pitch/`
- Added this handoff file

## Important Claude Transcript Context

The “good original session” Jeremy wanted recovered is:

`~/.claude/projects/-home-jeremy-git-operation-dbus-proto/530852a9-02e8-4af8-bdf1-8d45b02180a7.jsonl`

The later recovery/frustration session is:

`~/.claude/projects/-home-jeremy-git-operation-dbus-proto/0ef14d6e-2b10-4e31-965d-7402203c5317.jsonl`

The Netmaker/socket/xray implementation attempt is:

`~/.claude/projects/-home-jeremy-git-operation-dbus-proto/3840b230-12e4-4f60-9e0c-56e670d22fd9.jsonl`

`netmaker.handoff` is an export of the Netmaker session.

The original smooth transcript included many design decisions, but some implementation claims were only talk. Verify before trusting.

## Live System Facts Already Verified

- Live D-Bus name: `org.opdbus.projection`
- `busctl --system tree org.opdbus.projection` shows plugin objects including:
  - `/org/opdbus/v1/plugins/cognitive_mcp`
  - `/org/opdbus/v1/plugins/ctl_plane_chatbot`
  - `/org/opdbus/v1/plugins/netmaker`
  - `/org/opdbus/v1/plugins/xray`
  - `/org/opdbus/v1/plugins/zeroclaw`
  - `/org/opdbus/v1/plugins/oscal_subid_registry`
- `op-cognitive-mcp` live at:
  - `10.220.35.1:3003`
  - `10.220.35.1:50052`
- `op-mcp-shim` at `/usr/local/bin/op-mcp-shim` worked against `http://10.220.35.1:50052` and returned 18 tools.
- `.mcp.json` points correctly at `10.220.35.1:50052`.
- Some other MCP configs previously had stale `100.90.37.1:50052`.

Incus state previously verified:

- `netmaker` container: STOPPED
- `netmaker-mq`: RUNNING
- `netmaker-ui`: RUNNING
- `wg-xray`: RUNNING
- `qdrant`: RUNNING
- `op-grpc-adapters`: RUNNING

Netmaker hardening pattern state:

- Host sockets exist under `/run/netmaker`.
- `netmaker-ui` and `netmaker-mq` have only loopback interfaces.
- Main `netmaker` API container was stopped, so `/run/netmaker/api.sock` was not actually serving.
- `wg-xray` config routes Netmaker domains via loopback TCP redirects, not direct Unix sockets.
- `wg-xray` config claimed/has device config for `/run/netmaker`, but live container did not show the mount during check.

## Voyage / MongoDB AI Endpoint State

Jeremy moved to the MongoDB version of Voyage due billing. Keys may look like `al-*`.

Live `op-cognitive-mcp` process had env:

- `COGNITIVE_MCP_DB_PATH=/var/lib/op-dbus/cognitive.db`
- `COGNITIVE_MCP_QDRANT_URL=http://127.0.0.1:6334`
- `COGNITIVE_MCP_VOYAGE_API_KEY` set

`~/.ssh/mongo-voyage` exists and contains MongoDB/Voyage credentials. Do not print secrets.

`rag_pipeline.rs` already had logic to:

- Fall back to `~/.ssh/mongo-voyage`
- Route `al-*` keys to `https://ai.mongodb.com/v1/embeddings`

Patch already made in `crates/op-cognitive-mcp/src/qdrant_shuttle.rs`:

- Added `DEFAULT_VOYAGE_MONGODB_API_URL = "https://ai.mongodb.com/v1/embeddings"`
- `VoyageClient::from_env` checks:
  - `COGNITIVE_MCP_VOYAGE_API_KEY`
  - `VOYAGE_API_KEY`
  - `voyage_key_from_file()`
- `COGNITIVE_MCP_VOYAGE_API_URL` still overrides.
- Default endpoint auto-selects MongoDB AI endpoint for `al-*` keys.
- Added key-file fallback reading `COGNITIVE_MCP_VOYAGE_KEY_FILE` or `~/.ssh/mongo-voyage`.
- Skips `mdb_sa_id_`, `mdb_sa_sk_`, comments; picks `al-*` or `pa-*`.
- `cargo check -p op-cognitive-mcp` passed after this patch, with only existing warnings.

Model decision:

- For now use `voyage-4` for document/query/code vectorization because Jeremy noted a large free-token difference.
- Keep vector metadata so future revectorization is possible.
- Do not hardwire assumptions that prevent later role/model-specific embeddings.

## Subid Taxonomy

Canonical format:

`<category>.<component-type>.<subject>.<verb>[.<facet>][@vN]`

Seven categories:

- `src`
- `prj`
- `sch`
- `mut`
- `obs`
- `evt`
- `exp`

Component types use OSCAL vocabulary:

- `software`
- `service`
- `network`
- `hardware`
- `process-procedure`
- `standard`
- `validation`
- `policy`
- `plan`
- `guidance`
- `physical`
- `this-system`
- `system`
- `interconnection`

Important examples:

- `src.network.ovsdb.monitor@v1`
- `prj.service.projected-object.publish@v1`
- `sch.standard.plugin-schema.resolve@v1`
- `mut.service.state-sync.apply-patch@v1`
- `exp.service.plugin-projection.render@v1`

## A.N.N.A. Scribe And “Friends” Plugins

Do not put all of this into one plugin.

Jeremy explicitly said:

- A.N.N.A. Scribe should probably be its own plugin.
- Her crew should each have their own plugin.

Expected plugin split:

- `anna_scribe`
  - Identity/session notary
  - Snowball trace
  - Footprint
  - `trace_id`
  - `subid` binding
- `semantic_vectors`
  - Voyage/Qdrant embedding/retrieval config only
  - Endpoint/model/key-source metadata
  - Vectorization/revectorization policy
- `ctl_plane_chatbot`
  - Reasoning/chatbot episode contract already exists
  - Do not overload it with embedding provider config
- `olivia_scal`
  - OSCAL/control mapping counsel
- `eugene_risk`
  - EU AI Act counsel
- `penny_privacy`
  - GDPR/privacy counsel
- `reggie_opa`
  - OPA/policy prosecutor

Search results:

- `zeroclaw.rs` already contains routing/subid material:
  - `oscal_subid_registry`
  - router emits provider/model/hint/candidate_subids/confidence/thinking_budget/reasoning_effort
  - OSCAL policy route
  - `model_transcript_mcp`
- `antigravity.rs` contains provider/router/model route schema:
  - embedding route for Vertex `text-embedding-005`
  - OSCAL/subid registry refs
  - compliance route
- Neither has first-class `anna_scribe` or the attorney plugins.
- Both files include some wrong legacy D-Bus paths like `/opdbus/v1/...`; update to `/org/opdbus/v1/...` when touched.

## Pipeline Blockers To Demoland

Major blocker: `op-web` chat bypasses the memory/evidence loop.

Current code facts:

- `crates/op-chat/src/memory_loop.rs` has `MemoryLoop`.
- `ChatActor` in `crates/op-chat/src/actor.rs` uses `MemoryLoop` if `container_id` is present.
- `ChatActor` injects `generate_system_prompt(session_memory.as_ref())`.
- `ChatActor` spawns post-turn persistence.
- `MemoryLoop::semantic_boost` is TODO.
- Qdrant upsert after turn is TODO.

But:

- User-facing `op-web` `/api/chat` in `crates/op-web/src/handlers/chat.rs` bypasses `ChatActor`.
- It builds fixed/custom system prompt and calls `state.chat_manager` directly.
- Therefore the web demo chatbot currently does not receive memory-loop/Qdrant/semantic evidence context.

UI:

- `AccountabilityPage.tsx` already has the desired rough shape: chatbot top and Qdrant bottom.
- But chat send is a stub simulating gRPC-Web roundtrip:
  - “Acknowledged ... Compliance trace appended”
- It is not actually calling `/api/chat`.
- Evidence bottom uses gRPC `EventChainService.SearchSemanticTrace`.

Server:

- `op-grpc-bridge/src/grpc_server.rs` has `SearchSemanticTrace` wired to `QdrantSemanticShuttle::search_semantic_trace`.
- This depends on Qdrant shuttle and Voyage config.
- `op-web/src/handlers/analytics.rs` semantic search endpoint is a stub returning empty results. UI may not use this, but it is a trap.

## Work Plan: Get End-To-End Pipeline Working

### 1. Confirm Current Build Health

Run:

```sh
cargo check -p op-cognitive-mcp
cargo check -p op-chat
cargo check -p op-web
cargo check -p op-grpc-bridge
```

Use focused checks. Full workspace can be slow/noisy.

### 2. Fix Chat Path To Use Memory Loop / Evidence Context

Find:

- `crates/op-web/src/handlers/chat.rs`
- `crates/op-chat/src/actor.rs`
- `crates/op-chat/src/memory_loop.rs`

Goal:

- `/api/chat` must either route through `ChatActor` or reuse the same `MemoryLoop` logic.
- Avoid creating a second memory path.
- Preserve existing API shape unless absolutely necessary.

Minimum acceptable behavior:

- Request has or derives:
  - `container_id`
  - `session_id`
  - `trace_id`
  - user identity / peer identity when available
- Before calling model:
  - retrieve recent/session memory
  - retrieve semantic evidence from Qdrant for user query
  - inject compact evidence into system prompt/context
- After model answer:
  - persist turn to canonical store
  - enqueue/upsert embedding into Qdrant
  - attach trace/footprint/subid metadata

Do not let UI-only state pretend this happened.

### 3. Implement `MemoryLoop::semantic_boost`

Current TODO must become real.

Expected behavior:

- Input: current user message/query, session/container context.
- Embed query using configured Voyage client.
- Query Qdrant for relevant prior blocks/transcripts/memory.
- Return small bounded evidence set suitable for prompt injection.
- Include metadata:
  - score
  - source
  - `trace_id`
  - `footprint`
  - `subid`
  - `event_id` or `event_hash`
  - timestamp
  - model used for embedding
  - vector schema version

Keep prompt evidence bounded. Do not dump huge raw chunks into model context.

### 4. Implement Post-Turn Qdrant Upsert

Current TODO must become real.

Each user/assistant turn or episode should create vectorizable evidence.

Point ID decision:

- Prefer deterministic ID based on content hash or event hash.
- If `episode_id` exists and is stable, use it as part of payload, but content/event hash avoids duplicate upserts.

Payload should include:

- `content`
- `content_hash`
- `session_id`
- `trace_id`
- `footprint`
- `subid`
- `category` (`evt`, `obs`, `mut`, `exp`, etc.)
- `source` (`chat_turn`, `event_chain`, `notebooklm`, etc.)
- `embedding_model` (`voyage-4` initially)
- `embedding_provider` (`mongodb_ai_voyage` or `voyage`)
- `embedding_endpoint`
- `schema_version`
- `created_at`

Do not store API keys in payload/logs.

### 5. Make Qdrant Shuttle Fully MongoDB/Voyage Compatible

Already patched `qdrant_shuttle.rs`; verify runtime uses it.

Check:

- Does `QdrantSemanticShuttle` call the patched `VoyageClient::from_env`?
- Does it use `voyage-4` or still `voyage-code-3`?
- Does endpoint default to MongoDB AI for `al-*`?
- Does it gracefully error when no key exists?

Need add or update tests if local pattern exists.

### 6. Wire Accountability Page Chat To Real API

Find UI:

- `crates/op-web/ui/src/.../AccountabilityPage.tsx`
- `crates/op-web/ui/src/grpc/client.ts`

Goal:

- Chatbox sends to real `/api/chat` or gRPC equivalent.
- Evidence pane calls real `SearchSemanticTrace`.
- When a chat answer arrives, evidence pane should refresh for:
  - query text
  - returned trace id
  - session id
  - footprint/subid if available

Minimum demo behavior:

- User asks question.
- Chatbot answers.
- Bottom pane shows relevant Qdrant semantic evidence from real collection.
- If evidence is empty, show “no evidence found” truthfully, not fake entries.

### 7. Make Semantic Search Endpoint Non-Stub Or Remove Trap

`crates/op-web/src/handlers/analytics.rs` semantic search returns empty results.

Options:

- Wire it to `QdrantSemanticShuttle` if HTTP endpoint is intended.
- Or ensure UI never uses it and mark it clearly not used.

Do not leave a route that silently returns empty evidence if a user-facing view may call it.

### 8. Add `semantic_vectors` Plugin

Jeremy said Voyage/vectorization obviously needs a plugin.

Do not combine with A.N.N.A.

Plugin should expose schema/config for:

- embedding provider
- endpoint
- model
- key source, but not actual key
- qdrant URL/collection
- vector dimensions
- vector schema version
- batch settings
- revectorization policy
- last successful embed timestamp
- last error summary
- supported sources
- D-Bus path for observation/config

Likely plugin path:

`/org/opdbus/v1/plugins/semantic_vectors`

Source file should follow existing state plugin patterns in:

`crates/op-plugins/src/state_plugins/`

Remember Jeremy’s correction:

- Schema belongs in the plugin `.rs` file if that is current repo pattern.
- `plugin_schema_defs.rs` is only aggregator/formatter.

### 9. Add `anna_scribe` Plugin

Separate plugin.

Should expose schema for:

- `session_id`
- `trace_id`
- `footprint`
- WireGuard pubkey identity anchor
- Snowball ledger head/current block
- subid binding
- HMAC/identity header status
- last notarized event
- authority references

Possible path:

`/org/opdbus/v1/plugins/anna_scribe`

Do not implement all attorneys here.

### 10. Add Attorney Plugins Later But Capture TODO

High but maybe not critical for first working pipeline:

- `olivia_scal`
- `eugene_risk`
- `penny_privacy`
- `reggie_opa`

For now, capture as explicit TODO/wishlist if not implementing.

### 11. Fix Stale D-Bus Paths

Search:

```sh
rg "/opdbus/v1" crates/op-plugins/src/state_plugins
```

Known stale files:

- `crates/op-plugins/src/state_plugins/zeroclaw.rs`
- `crates/op-plugins/src/state_plugins/antigravity.rs`

Replace with `/org/opdbus/v1` where these are D-Bus object paths.

### 12. Verify Live D-Bus Projection

After adding plugins/rebuilding/restarting relevant projection service, verify:

```sh
busctl --system tree org.opdbus.projection | rg '/org/opdbus/v1/plugins/(semantic_vectors|anna_scribe|cognitive_mcp|ctl_plane_chatbot|netmaker|xray|zeroclaw|oscal_subid_registry)'
```

If plugin object is missing, per Jeremy’s rule it does not exist yet.

### 13. Verify Qdrant

Need identify actual Qdrant endpoint/port mapping:

- live env said `http://127.0.0.1:6334`
- Qdrant container running

Check collections:

```sh
curl -s http://127.0.0.1:6334/collections
```

If only container-local, use the correct host/Incus proxy path.

Need know:

- collection name
- vector dimensions for `voyage-4`
- whether old `voyage-code-3` vectors exist
- whether mixed dimensions require separate collection

Do not mix incompatible vector dimensions in same Qdrant collection.

### 14. Verify End-To-End With One Real Event

Create or locate one real evidence item:

- event chain record
- chat transcript
- Netmaker plugin state observation
- Claude transcript excerpt

Pipeline test:

1. Embed/upsert it.
2. Search for it semantically.
3. Confirm payload includes trace/subid/source metadata.
4. Ask chatbot about it.
5. Confirm chatbot context includes retrieved evidence.
6. Confirm UI bottom pane displays same evidence.

### 15. Testing Guidance

Add focused tests around:

- Voyage key loading:
  - env key
  - `~/.ssh/mongo-voyage`
  - `al-*` endpoint selection
  - override endpoint
- Qdrant point payload shape
- semantic search returns expected payload
- `/api/chat` uses memory/evidence path
- UI no longer returns fake chat/evidence

Run:

```sh
cargo check -p op-cognitive-mcp
cargo check -p op-chat
cargo check -p op-web
cargo check -p op-grpc-bridge
```

Then targeted tests if available. Avoid full workspace test unless necessary.

## Current Pitch/Netmaker Side Task

Files created/sent:

- `pitch/GhostBridge-Netmaker-use-case-deck.html`
- `pitch/GhostBridge-Netmaker-use-case-deck.pdf`
- `pitch/GhostBridge-Netmaker-email-body.txt`

Correct framing:

- This is Jeremy’s Netmaker use case.
- Netmaker is part of the backbone of the PaaS.
- Not an ask. Not permission. Not “can you say no?”
- Jeremy will answer questions in the meeting.

Two emails were sent to `jeremy.alan.hobson@gmail.com`:

1. Earlier static brief with weaker “review/reaction” framing.
2. Replacement email with corrected subject:
   `GhostBridge x Netmaker: our platform use case`

The replacement supersedes the first.

## Suggested Next Agent Prompt

Use this prompt if starting a new Factory/Codex session:

```text
You are continuing work in /home/jeremy/git/operation-dbus-proto.

Read FACTORY-HANDOFF-DEMO-PIPELINE.md first. The goal is to get the GhostBridge end-to-end demo pipeline working: real chatbot top, real Qdrant semantic evidence bottom, real vectorization, trace/footprint/subid accountability, and plugin schema existence for the missing semantic/A.N.N.A. pieces.

Do not redesign the architecture. D-Bus is the control plane. The live bus is org.opdbus.projection and plugin paths are /org/opdbus/v1/plugins/<name>. Plugin IS schema. If a plugin object/schema is missing, create the plugin rather than pretending it exists. Keep A.N.N.A. Scribe separate from semantic_vectors and separate from the attorney plugins.

Start by verifying build health for op-cognitive-mcp, op-chat, op-web, and op-grpc-bridge. Then inspect op-web chat handling, op-chat MemoryLoop, QdrantSemanticShuttle, and AccountabilityPage. The likely main blocker is that /api/chat bypasses ChatActor/MemoryLoop and the UI chat is stubbed. Fix that path so the chatbot has semantic evidence and the evidence pane shows real Qdrant results.

Use voyage-4 for vectorization for now and preserve metadata for future revectorization. MongoDB AI Voyage keys may be in ~/.ssh/mongo-voyage and may look like al-*. Do not print secrets.

Make focused edits, use existing repo patterns, run targeted cargo checks, and do not revert unrelated dirty worktree changes.
```
