# Requirements: Plugin gRPC Proto Auto-Creator

## Problem

11 existing state plugins (antigravity, antigravity_chat, keyring, identity_sled, sess_decl,
mcp, xray, gcloud_adc, full_system, + 2 more) have no gRPC proto method definitions. Today,
closing this gap requires a human (or an ad-hoc agent session) to manually research each
plugin and hand-write its `.proto` file. This does not scale to future missing plugins.

## Goal

Build a **plugin auto-creator capability**: given the name of a plugin that has no gRPC
methods, the system automatically:

1. Researches what operations/fields that plugin should expose as gRPC methods.
2. Generates a production-ready, convention-compliant `.proto` file for it.
3. Makes the result available for integration (codegen) without further human authoring.

Success is **the auto-creator functioning**, not the 11 protos being hand-delivered. Once
built, running the auto-creator against any missing plugin (the current 11, or a future
one) should produce a usable `.proto` file automatically.

## Research Source

- Research must be performed via **NotebookLM** (an MCP-style knowledge source that the
  user will have running), not the existing `search-specialist` web-research agent.
- Because NotebookLM may not always be reachable (e.g., not started yet), the auto-creator
  must have a **deterministic fallback** so it still produces a usable draft proto from the
  plugin's existing state schema when NotebookLM is unavailable.

## Proto Conventions (mandatory, per gRPC best practices)

- No `bool success` fields — rely on gRPC status codes.
- No raw JSON string payloads — use `google.protobuf.Struct` for untyped/dynamic data.
- All enums start with `<ENUM_NAME>_UNSPECIFIED = 0`.
- Descriptive enum values (e.g., `STATUS_ACTIVE`, `STATE_RUNNING`).
- Request messages end in `Request`, responses end in `Response`.
- Streaming responses use `stream ResponseType` (e.g., for `Watch`).
- Each generated `.proto` file must be self-contained and independently compilable
  (proper `syntax`, `package`, and `import` statements).

## Out of Scope

- Manually authoring the 11 plugins' `.proto` files by hand as the primary deliverable.
- Modifying s6/service lifecycle or Xray config (unrelated to this task; governed by
  existing AGENTS.md policy).
- Running `npm run proto:gen` / TypeScript codegen verification against a full frontend
  build (no such script/path currently exists in this repository; codegen integration is
  addressed only to the extent the generated `.proto` files are structurally valid).

## Acceptance Criteria

- A plugin/module exists that, given a plugin name, produces a `.proto` file following the
  conventions above.
- The research step calls out to NotebookLM when available, and falls back to a schema-
  derived heuristic when it is not, without failing the overall generation.
- Running the auto-creator against each of the known missing plugins produces a valid,
  distinct `.proto` file per plugin.
- Existing `op-plugins` build/tests remain green after the change.
