# Handoff — Baseline (T-00, Coordinator Agent)

## Scope Completed
WS0 baseline inventory of `op-llm` consumers and build state, prior to the
schemars-native single-projection refactor.

## `op_llm::` consumers (24 files)
- `op-web` (12): websocket.rs, system_prompt_loader.rs, state.rs, routes/llm.rs,
  routes/chat.rs, orchestrator/{process,tools,mod,parsing}.rs, chat_store.rs,
  handlers/{llm,chat}.rs
- `op-chat` (10): system_prompt.rs, tool_orchestrator.rs, memory_loop.rs,
  nl_admin.rs, main.rs, hybrid_executor.rs, forced_tool_pipeline.rs,
  chat_loop.rs, bin/list_tools_client.rs, actor.rs
- `op-mcp-proxy` (2): main.rs, http_server.rs

## `op-llm` Cargo.toml dependents (3)
- `crates/op-web/Cargo.toml`
- `crates/op-chat/Cargo.toml`
- `crates/op-mcp-proxy/Cargo.toml`

## Adapter files to absorb (T-41)
`crates/op-llm/src/{anthropic,gemini,gemini_cli,factory,openclaw,assistant,gcloud_adc}.rs`

## Build/Test baseline
- `cargo build --workspace`: **green** (0 errors, ~15 warnings, pre-existing).
- Full `cargo test --workspace` compile is slow on this host; per-phase crate
  gates (`cargo check -p <crate>` / `cargo test -p op-plugins -- <filter>`)
  are used as the working verification per `tasks.md`.

## Pre-existing state notes
- `common/llm_projection.rs` already contains the §2 schemars contract types
  (`ModelRoute` selector fields, `SelectorPolicy`, `SelectionInput/Output/Event`)
  but the `common` module is **not declared** in `state_plugins/mod.rs`, so it is
  currently an orphan (uncompiled). WS1 wires it in.
- `zeroclaw.rs` `ZeroclawState` is still untyped `simd_json::OwnedValue` fields,
  does not derive `JsonSchema`, lacks `selector_policy`, and uses the legacy
  `/opdbus/v1/...` transport path. WS1 retypes it and fixes the path.
- `zeroclaw_plugin_schema()` still calls `schema_from_state()`. WS1 replaces it
  with `plugin_schema_from_schemars::<ZeroclawState>()`.

## Verification Commands Run
```
rg 'op_llm::' crates -g '*.rs' -l
rg 'op-llm' crates -g 'Cargo.toml'
cargo build --workspace   # BUILD_EXIT=0
```

## Known Risks / Blocked Items
- `schemars` 0.8.22 (`preserve_order`) is the available version; the §8 converter
  must use `SchemaGenerator::into_root_schema_for::<T>()` and resolve
  `#/definitions/*` refs.

## Next-Agent Dependencies
- Schema Agent (WS1, T-10..T-13): wire `common` module, retype `ZeroclawState`,
  add `ZeroclawError`, add the schemars converter + golden test.
