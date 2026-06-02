# Operation Dbus — NotebookLM MCP Requirements v1

## Baselines
- PleasePrompto/notebooklm-mcp: zero-hallucination, persistent Chrome auth, tool profiles (5/10/16)
- Pantheon-Security/notebooklm-mcp-secure: 48 tools, batch create, bulk upload, audio/video overviews, data tables JSON, Gemini fallback, 17 security layers
- julianoczkowski/notebooklm-mcp-2026: 9 tools, conversation_id, cookie auth, doctor diagnostics

## Requirements
**Core querying**
R1. Grounded query: ask_question(notebook_id, query) returns answer + citations, refuses if not in sources
R2. Conversation memory: support conversation_id for follow-ups
R3. Library management: list_notebooks, select_notebook, get_notebook, sync_library

**Notebook lifecycle**
R4. Programmatic creation: create_notebook(title, description) and batch_create_notebooks([])
R5. Bulk ingest: add_folder(path), add_source_url, add_source_text
R6. Source ops: list_sources, remove_source, get_source_content

**Advanced outputs**
R7. Structured extraction: generate_data_table(notebook_id, prompt) → JSON
R8. Audio/video overviews: generate_audio_overview hook

**Resilience & auth**
R9. Persistent auth: Chrome profile stored, never wipe on failed launch
R10. Session management: list_sessions, reset_session, close_session, get_health(deep_check=true)
R11. Quota awareness: get_quota, set_quota_tier (~50 queries/day free)
R12. Fallback path: gemini_query, deep_research when browser breaks

**Security & ops**
R13. Secure storage: credentials 0o600, data dir 0o700, no shell=True, no eval
R14. Tool profiles: minimal/standard/full
R15. Diagnostics: doctor, get_query_history

**Integration**
R16. Map to CognitiveToolRegistry: project:op-dbus → notebook ID, store→add_source_text, query→ask_question, list_namespaces→list_notebooks

## Phasing
Phase 1 (MVP): R1-R3 + R9-R11
Phase 2 (scale): R4-R6
Phase 3 (robust): R7, R12-R14
