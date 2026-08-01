# Operation Dbus NotebookLM MCP Server Technical Specification v0.2 (tonic)
Date: 2026-04-24

## Scope
No guessing on contracts. This spec references operation-dbus-proto repo structure only.

## Architecture Reference
- Follows docs/architecture/state-flow.md: gRPC → D-Bus → StateManager → plugins
- Plugin envelope types referenced by name only: stub, immutable, tunable, observed, meta, semantic_index, privacy_index (see docs/schema/plugin-contracts.md)

## Service Methods (placeholders - use actual proto fields)
- AskQuestion
- QueryNotebook
- CreateNotebook
- BatchCreateNotebooks
- AddSource
- AddFolder
- ListNotebooks
- GetNotebook
- ListSources
- GetSourceContent
- GenerateDataTable
- GetHealth
- SetupAuth

Do NOT invent request/response fields. Refer to https://github.com/repr0bated/operation-dbus-proto

## Traceability
R1-R3,R9-R11 → AskQuestion, QueryNotebook, ListNotebooks, GetHealth, SetupAuth
R4-R6 → CreateNotebook, BatchCreateNotebooks, AddFolder, AddSource, ListSources
R7,R12-R14 → GenerateDataTable, Gemini fallback, tool profiles

## Security
- credentials 0o600, data dir 0o700
- no shell=True, no eval
- exponential backoff retries
