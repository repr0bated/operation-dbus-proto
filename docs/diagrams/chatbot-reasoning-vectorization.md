# Chatbot Reasoning Vectorization

```mermaid
---
config:
  theme: neo-dark
---
sequenceDiagram
    participant User
    participant ChatBot as ctl-plane-chatbot
    participant ReasoningMgr as ReasoningStateManager
    participant EventLog as EventChain / BTRFS
    participant EmbedQueue as EmbeddingQueue<br/>(mpsc 1024, try_send)
    participant VoyageAPI as Voyage API
    participant Qdrant

    User->>ChatBot: Start conversation
    ChatBot->>ReasoningMgr: Begin reasoning episode
    ReasoningMgr->>ReasoningMgr: Track episode state<br/>(episode_id, goal, tools_consulted)

    ChatBot->>ChatBot: Process input, generate reasoning

    ChatBot->>ReasoningMgr: Close episode with results
    ReasoningMgr->>ReasoningMgr: Materialize EpisodeRecord<br/>(schema-conformant, privacy-filtered)

    ReasoningMgr->>EventLog: Write episode record (synchronous)<br/>→ BTRFS timing_subvol block
    ReasoningMgr->>EmbedQueue: try_send embed request<br/>(non-blocking, silent drop if full)

    Note over EmbedQueue: Embedding is runtime cognitive ability<br/>not audit — BTRFS is the source of truth

    EmbedQueue->>EmbedQueue: Apply privacy filters<br/>Build embedding text<br/>(no raw payloads, metadata only)
    EmbedQueue->>VoyageAPI: POST /v1/embeddings<br/>(input_type=document)
    VoyageAPI-->>EmbedQueue: Return vector (1024-dim voyage-4-lite)
    EmbedQueue->>Qdrant: Upsert to ctl_plane_reasoning_episodes<br/>(5 retries, 500ms base, exponential backoff)

    Note over User,Qdrant: Human operator accountability search

    participant HumanOp as Human Operator
    HumanOp->>VoyageAPI: Embed search query<br/>(input_type=query)
    VoyageAPI-->>HumanOp: Return query vector
    HumanOp->>Qdrant: Semantic search with filters<br/>(outcome_class, plugin_id, time_range)
    Qdrant-->>HumanOp: Scored episodes<br/>(reasoning_summary, decision_output,<br/>tools_consulted — schema-approved fields only)
```

## Key facts for models

- **No D-Bus watcher** in this path — reasoning episodes write directly to EventChain/BTRFS then enqueue to embedding worker
- **No SQLite** — episode records go to BTRFS timing_subvol (audit) and Qdrant (vector search)
- **EmbeddingQueue is lossy by design** — `try_send`, drop on full; BTRFS is the durable audit trail
- **Embedding text** is metadata only: `plugin=X operation=Y actor=Z outcome=W summary=...` — no raw payloads
- **PII flag** → redacts summary from vector input before enqueueing
- **Qdrant collection**: `ctl_plane_reasoning_episodes`, 1024-dim, voyage-4-lite
- **Retry**: 5 attempts, 500ms base, exponential backoff in worker; logs warn on final failure, does not panic
