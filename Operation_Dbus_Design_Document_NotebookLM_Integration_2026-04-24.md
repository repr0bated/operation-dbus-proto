# Operation Dbus Design Document - NotebookLM Integration
Date: 2026-04-24

## Architecture
Rust server → CognitiveToolRegistry → NotebookLM bridge + Gemini fallback
- gRPC/JSON-RPC ingress → D-Bus → StateManager → plugins (per state-flow.md)
- Cognitive memory acts as cache; NotebookLM is source of truth

## Components
- tool_registry.rs: registers 16 core tools
- resources.rs: sled-backed CognitiveMemoryStore
- trait_agent_executor.rs: logging and observability
- register_notebooklm_tools(): bridge with retries and session rotation

## Namespace Mapping
project:op-dbus-core → NotebookLM notebook ID
project:op-dbus-bindings → notebook ID
Typed tools prevent agents guessing namespaces

## Tool Definitions (16 core)
1. ask_question 2. query_notebook 3. list_notebooks 4. select_notebook 5. get_notebook
6. create_notebook 7. batch_create_notebooks 8. add_source_url 9. add_source_text 10. add_folder
11. list_sources 12. remove_source 13. get_source_content 14. generate_data_table 15. get_health 16. doctor

## Phased Implementation
MVP: grounded queries + conversation_id + persistence
Scale: bulk create + add_folder for 97 repos
Robust: data tables, Gemini fallback, tool profiles, quotas
