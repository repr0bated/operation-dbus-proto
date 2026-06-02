# Operation Dbus NotebookLM MCP Actionable Tasks
Date: 2026-04-24

## Phase 1 – gRPC Ingress (MVP)
Task 1.1: Add tonic server to server.rs — action: create grpc_service.rs, wire to CognitiveToolService — verify: cargo check
Task 1.2: Wire tool_registry to gRPC — action: map AskQuestion to existing MemoryTool — verify: unit test
Task 1.3: Implement conversation_id support — action: modify query to accept conversation_id per julianoczkowski — verify: follow-up query retains context
Task 1.4: Health/auth tools — action: implement get_health, setup_auth — verify: deep_check returns status

## Phase 2 – NotebookLM Bridge (Scale)
Task 2.1: CreateNotebook/Batch — action: implement create_notebook, batch_create_notebooks — verify: 97 notebooks created
Task 2.2: AddFolder bulk ingest — action: implement add_folder — verify: repomix splits ingested
Task 2.3: Source ops — action: list_sources, remove_source, get_source_content — verify: pruning works

## Phase 3 – Persistence & Robustness
Task 3.1: Migrate to sled — action: replace Arc with sled in resources.rs — verify: restart retains namespaces
Task 3.2: Secure storage — action: set 0o600 credentials, 0o700 data dir — verify: permission check
Task 3.3: Tool profiles — action: implement minimal/standard/full — verify: token count reduces
Task 3.4: Data tables + Gemini fallback — action: generate_data_table, gemini_query — verify: fallback triggers on browser fail
Task 3.5: Doctor diagnostics — action: implement doctor command — verify: outputs auth status and quota
