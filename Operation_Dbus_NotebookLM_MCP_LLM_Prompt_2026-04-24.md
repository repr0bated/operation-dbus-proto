# Operation Dbus NotebookLM MCP LLM Prompt
Date: 2026-04-24

You are implementing the NotebookLM MCP server for Operation Dbus. Use the proto definitions from https://github.com/repr0bated/operation-dbus-proto – do NOT invent messages.

Implement tonic gRPC server in Rust, integrating with existing CognitiveToolRegistry. Follow architecture: gRPC → D-Bus → StateManager → plugins. Do NOT detail plugin contracts – reference docs/schema/plugin-contracts.md.

Implement methods: AskQuestion, QueryNotebook, CreateNotebook, BatchCreateNotebooks, AddSource, AddFolder, ListNotebooks, GetNotebook, ListSources, GetSourceContent, GenerateDataTable, GetHealth, SetupAuth.

Preserve tool names from the three reference MCPs. Security: no shell=True, credentials 0600, retries with backoff.

Output: modified server.rs, tool_registry.rs, new grpc_service.rs.

Do not guess field names. If unclear, leave // TODO: check repo doc.

Phrase 'actionable tasks' must appear in your plan output.
