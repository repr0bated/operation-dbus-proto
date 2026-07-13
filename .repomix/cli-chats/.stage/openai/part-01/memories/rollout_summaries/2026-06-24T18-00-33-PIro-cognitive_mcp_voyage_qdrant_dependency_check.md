thread_id: 019efaca-778f-76f3-a721-762ec0bc505a
updated_at: 2026-06-24T21:22:17+00:00
rollout_path: /home/jeremy/.codex/sessions/2026/06/24/rollout-2026-06-24T14-00-33-019efaca-778f-76f3-a721-762ec0bc505a.jsonl
cwd: /home/jeremy/git/operation-dbus-proto
git_branch: feat/sled-source-port-salt

# Checked `op-cognitive-mcp` / Voyage embedding dependencies for startup loops and runtime prerequisites

Rollout context: the user asked whether there was a dependency loop between Voyage embedding setup and `cognitive-mcp` dependencies that could not be satisfied. The investigation focused on the Rust workspace, s6 service graph, live environment variables, and reachability of Qdrant/Voyage prerequisites.

## Task 1: Trace Voyage/Qdrant dependency loop for `op-cognitive-mcp`

Outcome: partial

Preference signals:

- The user asked: "check voyage embedding dependancies and cognative-mcp depends, is there a loop that cannot be satiffied?" -> future similar asks should first check the service graph and runtime prerequisites for cycles/unmet deps, not just code paths.

Key steps:

- Searched `crates/op-cognitive-mcp`, `crates/op-grpc-bridge`, `crates/op-plugins`, and `deploy` for Voyage/Qdrant/embedding references and s6 dependency definitions.
- Inspected the s6 layout under `deploy/s6` and `/run/s6-rc/servicedirs` to see what `op-cognitive-mcp` depends on and what depends on it.
- Read `CognitiveMcpServer::new()` and `RagPipeline::from_env()` / `QdrantSemanticShuttle::new()` to verify startup behavior.
- Checked live env vars for the running `op-cognitive-mcp` service, then probed Qdrant listeners and local health endpoints.

Failures and how to do differently:

- There was no hard dependency cycle in the s6 graph: `op-cognitive-mcp` depends on `dbus-session`, while other services depend on `op-cognitive-mcp`, not the reverse.
- Voyage is not a blocking startup dependency for the server process: `RagPipeline::from_env()` failure only logs a warning and the server continues without code-context tools.
- The likely unsatisfied runtime prerequisite was Qdrant reachability, not Voyage. `COGNITIVE_MCP_VOYAGE_API_KEY` was present in the live env, but no service was listening on localhost `6333` or `6334`, while the code defaults to `http://127.0.0.1:6334` unless overridden.
- `pgrep -f` is needed for long process names; plain `pgrep -af 'op-cognitive-mcp'` can miss the process name length limitation.

Reusable knowledge:

- `op-cognitive-mcp` startup is resilient: missing Voyage key or unavailable Qdrant does not prevent the service from coming up; it just disables optional retrieval features.
- The code paths split into two separate optional subsystems:
  - `RagPipeline::from_env()` for Voyage-based code retrieval and Qdrant embedding/search.
  - `QdrantSemanticShuttle::new()` for the accountability/semantic shuttle.
- The live service env showed `COGNITIVE_MCP_VOYAGE_API_KEY` configured, so the unresolved blocker was not Voyage credentials.
- Qdrant defaults were validated in code as `http://127.0.0.1:6334` (and `knowledge_plugin` also defaults to the same local endpoint), so a missing listener there is a strong signal for the failure mode.

References:

- `deploy/s6/op-cognitive-mcp/run`: `exec s6-envdir ./env /usr/local/bin/op-cognitive-mcp --db "$COGNITIVE_MCP_DB_PATH"`
- `deploy/s6/op-cognitive-mcp/dependencies.d/dbus-session`
- `crates/op-cognitive-mcp/src/server.rs`: `QdrantSemanticShuttle::new().await` and `RagPipeline::from_env()` are both optional; failures only warn and continue.
- `crates/op-cognitive-mcp/src/rag_pipeline.rs`: default Qdrant URL `http://127.0.0.1:6334`; `RagPipeline::from_env()` requires a Voyage key but only disables code tools if absent.
- `crates/op-cognitive-mcp/src/qdrant_shuttle.rs`: default Qdrant URL `http://127.0.0.1:6334`; construction does a `health_check()` and is separate from server liveness.
- Live env snapshot from `/run/service/op-cognitive-mcp/env` included `COGNITIVE_MCP_VOYAGE_API_KEY` and `COGNITIVE_MCP_QDRANT_URL=http://127.0.0.1:6334`.
- `ss -ltnp` / `curl` checks found no listeners on `127.0.0.1:6333` or `127.0.0.1:6334`.
- Live process evidence: `/usr/local/bin/op-cognitive-mcp --db /var/lib/op-dbus/cognitive.db` was running under s6 and serving `0.0.0.0:3003`.
