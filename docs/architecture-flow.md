# Architecture Flow

End-to-end system architecture diagrams for operation-dbus-proto. Intended as the source of
truth for Lovable UI generation and onboarding.

---

## 1. Canonical State Mutation Path

```
External caller
  │
  ├── gRPC (op-grpc-bridge:50051)
  │     sync_engine.rs → ApplyContractMutation
  │
  └── JSON-RPC (op-jsonrpc:7020)
        → ApplyContractMutation

            │
            ▼
    D-Bus ingress
    org.opdbus.StateManager
    .ApplyContractMutation
            │
            ▼
    StateManager
    apply_state / apply_state_single_plugin
            │
            ▼
    SchemaEngine
    schema materialization + validation
    (plugin IS the schema — schema drives everything)
            │
            ▼
    Plugin diff/apply
            │
            ├── Persistent state  →  op-state-store (SQLite)
            │
            ├── Audit trail       →  BTRFS timing_subvol
            │                        (append-only, block chain)
            │
            └── DR state dump     →  BTRFS state_subvol
                                     current.json
                                     (only include_in_dr=true plugins)
```

---

## 2. Blockchain & Vector Pipeline

```
Plugin mutation completes
        │
        ▼
OptimizedBlockchain::add_footprint()
  ├── Writes PluginFootprint → BTRFS timing_subvol (synchronous)
  │
  └── try_send(EmbedRequest) → mpsc channel (non-blocking, drop if full)
              │
              ▼
      Embedding Worker (tokio task, background)
        ├── EmbeddingProvider::embed(text, Document)
        │     └── OpenClaw agent routing
        │           model = OPENCLAW_EMBEDDING_MODEL
        │           (default: openclaw:embedder-voyage4lite)
        │           → POST /v1/embeddings → Voyage API
        │           fallback: op-ml local ONNX
        │
        └── Qdrant::upsert_points()
              collection: op_footprints  (or plugin-specific)
              point_id:   block_hash
              vector:     1024-dim (voyage-4-lite)
              payload:    plugin_id, operation, timestamp, session_id
              endpoint:   10.149.181.190:6334 (gRPC)


Qdrant roles:
  ├── AI analysis     — semantic search over footprints & reasoning episodes
  ├── Disaster recovery — vector snapshot = point-in-time AI memory state
  └── Offsite backup  — btrfs send of vector storage to remote replica
```

---

## 3. Control-Plane Chatbot Reasoning Vectorization

```
Chatbot enters reasoning state
  (trigger: goal received / tool result / interrupt / replan)
        │
        ▼
  Reasoning Episode opens
  ┌─────────────────────────────────────────────────────┐
  │  episode_id (UUID v7)                               │
  │  goal_text, trigger, tools_consulted                │
  │  reasoning_summary (model-generated at close)       │
  │  outcome_class, confidence, plugin_id               │
  │  pii_flagged → redacts summary from vector input    │
  └─────────────────────────────────────────────────────┘
        │
        ▼
  Reasoning state exits
  (tool_call / response_emitted / direction_change / goal_achieved)
        │
        ├── 1. Write record → blockchain / event log  (synchronous)
        │
        └── 2. Enqueue embedding  (non-blocking)
                    │
                    ▼
            Embedding Worker (high priority)
              embed: reasoning_summary + goal_text + outcome_class
                     + tools_consulted  (no raw payloads, no PII)
                    │
                    ▼
            Qdrant upsert
              collection: ctl_plane_reasoning_episodes
              vector: 1024-dim voyage-4-lite
              payload: episode_id, started_at, ended_at, outcome_class,
                       trigger, exit_reason, plugin_id, conversation_id,
                       reasoning_summary, decision_output
                    │
                    ▼
            trace span: reasoning_episode.vectorized
```

---

## 4. Chatbot Accountability View

```
Human operator query: "why did the chatbot reconfigure the firewall at 3am?"
        │
        ▼
ChatbotAccountabilityService (gRPC)
  SearchEpisodes(query, filters{outcome_class, plugin_id, time_range})
        │
        ├── EmbeddingProvider::embed(query, Query intent)
        │         → OpenClaw → Voyage API (query input_type)
        │
        └── Qdrant::search(ctl_plane_reasoning_episodes, vector, filters)
                    │
                    ▼
            Scored results ranked by semantic similarity
            Each result:
              ├── reasoning_summary
              ├── decision_output
              ├── outcome_class
              ├── started_at / duration_ms
              └── tools_consulted

        Lovable UI renders results from JSON schema
        (no special UI code — schema-driven renderjson)
```

---

## 5. BTRFS Subvolume & Snapshot Architecture

```
/                           (BTRFS root, /dev/sda + /dev/sdb RAID-1)
├── timing_subvol/          audit ledger — append-only blockchain blocks
│     └── snapshots/        .send-state.json  (per-remote send tracking)
│
├── state_subvol/           DR state — current.json per include_in_dr plugin
│
└── vectors/                Qdrant storage volume
      └── storage/          point data, segments, WAL


Incremental send (BTRFS):
  snapshot N-1  ←── parent (pinned until all remotes confirm receipt)
  snapshot N    ←── current

  btrfs send -p <N-1> <N> | ssh remote btrfs receive <path>

  SendState tracks per-remote last_sent_snapshot
  Pruning NEVER deletes a pinned snapshot
  Pin released only after successful incremental send to ALL remotes


DR recovery order:
  1. Boot baseline Debian
  2. Apply state_subvol/current.json  (plugin schema reinstalls dependencies)
  3. Restore Qdrant vectors from snapshot
  4. Replay timing_subvol blocks from last DR checkpoint forward
```

---

## 6. Agent Orchestration (Post-Refactor)

```
BEFORE (static structs):
  50+ per-agent .rs files
  each with hardcoded SystemMessage, ModelConfig, ToolSet
  registered manually in agent_catalog.rs

AFTER (dynamic personas):
  config/agents/personas.yaml
    └── persona definitions: name, system_prompt, model, tools, tags

  PersonaAgent (generic handler)
    ├── loads persona from YAML at startup
    ├── routes tool calls via MCP
    └── returns structured output per plugin schema

  AgentCatalog
    └── built from YAML — no code changes to add/remove agents

  OpenClaw agent routing:
    model string = agent selector
    e.g. "openclaw:embedder-voyage4lite"
         "openclaw:reasoner-claude-sonnet"
         "openclaw:coder-deepseek"
```

---

## 7. Service & Port Summary

| Service | Host | Port | Protocol | Notes |
|---|---|---|---|---|
| op-dbus | op-dbus (host) | D-Bus session | D-Bus | StateManager, plugins |
| op-grpc-bridge | op-dbus | 50051 | gRPC/TLS | Primary mutation ingress |
| op-jsonrpc | op-dbus | 7020 | HTTP+JSON | Legacy / tooling |
| op-mcp | op-dbus | 3000 | HTTP | MCP tool server |
| op-cognitive-mcp | op-dbus | 3001 | HTTP | Cognitive tools, memory |
| op-web | op-web | 8080 | HTTP | UI frontend |
| OpenClaw | services container | 11434-ish | HTTP | LLM agent routing |
| NextDNS | services container | 53 | DNS | Internal resolver |
| Qdrant REST | qdrant container | 6333 | HTTP | Collection management |
| Qdrant gRPC | qdrant container | 6334 | gRPC | Vector ops (Rust client) |
| Xray | xray-server container | varies | proxy | Privacy ingress |

### Network segments

```
incusbr0 (10.149.181.0/24)  — internal Incus bridge
  ├── services  10.149.181.10   OpenClaw + NextDNS
  ├── qdrant    10.149.181.190  Qdrant vector DB (BTRFS-backed volume)
  └── xray-server              Privacy proxy

ovsbr0                        — OVS bridge (privacy + container networking)
  ├── wgcf                     Cloudflare WARP (obfuscation, no identity)
  ├── priv_wg / priv_xray / priv_warp
  └── ovsbr0-sock              Shared container socket port
```

---

## 8. Data Stores at a Glance

| Store | Location | What lives there | Durability |
|---|---|---|---|
| op-state-store | SQLite (op-dbus host) | Plugin state, cognitive memory, user memory | Persistent |
| BTRFS timing_subvol | /timing_subvol | Blockchain footprints (audit, immutable) | Persistent + replicated |
| BTRFS state_subvol | /state_subvol | DR current.json snapshots | Persistent + replicated |
| Qdrant | qdrant container | Vectors: footprints, reasoning episodes | Persistent + snapshotted |
| Embedding channel | in-process mpsc | In-flight embed requests | Best-effort (runtime only) |

---

## 9. Embedding Flow Detail

```
EmbedRequest {
  block_hash:      point ID in Qdrant
  embedding_text:  "plugin=firewall operation=set_rules ..."
  collection:      collection name
  payload:         JSON (plugin_id, op, ts, session_id, ...)
}

footprint_to_embedding_text():
  format: "plugin={plugin_id} operation={operation}
           actor={actor} outcome={outcome}
           summary={summary}"
  (no raw payloads — only metadata fields)

Channel: mpsc(1024) — try_send, silent drop on full
  rationale: embedding is runtime cognitive ability, not audit
  audit source of truth = BTRFS timing_subvol (never dropped)

Retry in worker: 5 attempts, 500ms base, exponential backoff
  worker logs warn on final failure, does not panic
```
