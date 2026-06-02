# Operation Dbus - Robustness Recommendations

Based on Codex scaffold with single cognitive_memory tool.

1. Durable memory layer
- Replace Arc<CognitiveMemoryStore> with sled or SQLite in resources.rs
- Add versioning per store operation
- Add namespace_kind: "project" → auto-maps to NotebookLM notebook ID

2. Harden NotebookLM bridge
- Retry + session rotation
- Citation passthrough: return [{text, source, page}]
- Bulk ingest: notebook_add_sources_bulk(files[])

3. Namespace design
- Register typed tools: dbus_query_core, dbus_query_bindings, etc.
- Hardcode namespace per tool

4. Observability
- Log namespace, agent_id, latency in trait_agent_executor.rs
- Expose stats via sse.rs or Prometheus
