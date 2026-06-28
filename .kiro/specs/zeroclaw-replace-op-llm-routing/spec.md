# Technical Spec: zeroclaw-replace-op-llm-routing

## Status: READY FOR IMPLEMENTATION

---

## 1. Scope Assessment (pre-work)

Before writing any code, the implementing agent **MUST** run:

```bash
cargo check -p op-plugins 2>&1 | head -40
cargo check -p op-llm 2>&1 | head -40
```

and confirm both compile cleanly. Any pre-existing errors must be noted and
treated as out of scope for this feature (do not fix unrelated breakage).

---

## 2. Boundary Audit

### 2.1 Scan Zeroclaw for execution leakage

```bash
grep -n "reqwest\|Client::new\|Client::builder\|hyper\|tokio::net\|TcpStream\|UnixStream" \
  crates/op-plugins/src/state_plugins/zeroclaw.rs \
  crates/op-plugins/src/state_plugins/common/llm_projection.rs
```

Expected result: **zero matches**. If matches are found, remove the network
client code and replace with the equivalent `op-llm` call site in `op-chat` or
`op-grpc-bridge`.

### 2.2 Scan Zeroclaw for op-llm import leakage

```bash
grep -n "op_llm\|crate::op_llm\|extern crate op_llm" \
  crates/op-plugins/src/state_plugins/zeroclaw.rs
```

Expected result: **zero matches**. `op-plugins` must not depend on `op-llm`.

### 2.3 Confirm op-chat uses op-llm

```bash
grep -rn "op_llm::\|use op_llm" crates/op-chat/src/
```

Expected result: at least one match for `LlmProvider` or `ChatManager`.

### 2.4 Confirm no duplicate LlmProvider trait

```bash
grep -rn "trait LlmProvider\|trait ChatProvider" crates/ --include="*.rs"
```

Expected result: exactly one match in `crates/op-llm/src/provider.rs`.

---

## 3. File Changes

### 3.1 `crates/op-plugins/src/state_plugins/zeroclaw.rs` — NO STRUCTURAL CHANGE EXPECTED

The current file is already a projection-only plugin. The audit in §2 should
confirm this. If the audit reveals execution code:

- Remove any `reqwest::Client` construction.
- Remove any HTTP calls to LLM providers.
- Remove any `op_llm` imports (add `op-llm` as a dev-dep only if needed for
  test assertions — never as a production dep of `op-plugins`).
- Replace removed logic with a comment pointing to the `op-llm` call site.

The `current_state()` function reads env vars at startup — this is permitted
bootstrap behavior. Do not change it.

The `write_schema_file_to()` function writes the tmpfs schema cache — this is
the correct projection cache write. Do not change it.

**If no execution leakage is found, no changes are made to `zeroclaw.rs`.**

### 3.2 `crates/op-plugins/src/state_plugins/common/llm_projection.rs` — NO CHANGE EXPECTED

This file contains only schema structs (`Provider`, `ModelRoute`, `Router`,
`LlmProjection`, etc.) with `serde` and `schemars` derives. It has no network
I/O. Confirm with the audit in §2.1.

### 3.3 `crates/op-llm/src/openclaw.rs` — ADD TESTS

Add unit tests for `OpenClawProvider` covering:

1. `should_resolve_model_falls_back_to_default` — `resolve_model("")` returns
   `DEFAULT_MODEL`.
2. `should_build_chat_url` — `chat_url()` returns the expected path suffix.
3. `should_build_models_url` — `models_url()` returns the expected path suffix.

These are pure-logic tests requiring no network. Add them as `#[cfg(test)]` at
the bottom of `openclaw.rs`.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_resolve_model_falls_back_to_default() {
        let p = OpenClawProvider::new(None, None);
        assert_eq!(p.resolve_model(""), DEFAULT_MODEL);
        assert_eq!(p.resolve_model("custom"), "custom");
    }

    #[test]
    fn should_build_chat_url() {
        let p = OpenClawProvider::new(Some("http://localhost:18789".into()), None);
        assert_eq!(p.chat_url(), "http://localhost:18789/v1/chat/completions");
    }

    #[test]
    fn should_build_models_url() {
        let p = OpenClawProvider::new(Some("http://localhost:18789".into()), None);
        assert_eq!(p.models_url(), "http://localhost:18789/v1/models");
    }
}
```

### 3.4 `crates/op-llm/src/provider.rs` — ADD ROUTE RESOLUTION TEST

Add a unit test verifying that `ProviderType::from_str` correctly resolves
the provider strings declared in Zeroclaw's route table:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_resolve_zeroclaw_declared_providers() {
        let cases = [
            ("factory", ProviderType::Factory),
            ("ollama", ProviderType::Custom("ollama".to_string())),  // adjust if ollama is added
            ("gemini", ProviderType::Gemini),
            ("anthropic", ProviderType::Anthropic),
            ("openai", ProviderType::OpenAI),
            ("openclaw", ProviderType::OpenClaw),
        ];
        for (input, expected) in &cases {
            let got = input.parse::<ProviderType>().unwrap_or_else(|e| {
                panic!("failed to parse provider '{}': {}", input, e)
            });
            assert_eq!(got.to_string(), expected.to_string(),
                "provider '{}' did not round-trip", input);
        }
    }
}
```

Note: if `"ollama"` is not currently handled by `ProviderType::from_str`, add
it as `ProviderType::Custom("ollama".to_string())` in the match arm for
OpenAI-compatible endpoints, or as a named variant if the team decides to
formalize it. Do not add a named variant without updating all exhaustive match
sites.

---

## 4. D-Bus Object Boundaries

### 4.1 Zeroclaw D-Bus object

- Object path: `/org/opdbus/v1/plugins/zeroclaw`
- Interface: `org.opdbus.v1`
- State fields exposed: all fields from `ZeroclawState` as serialized by
  `create_checkpoint()` → `simd_json::serde::to_owned_value(Self::current_state())`.
- Write path: only `apply_state()` (currently a stub; state is env-seeded).
- Read path: `create_checkpoint()` → checkpoint's `state_snapshot`.

No new D-Bus methods are added by this feature.

### 4.2 Consumer reads

Any consumer needing the active provider/model **SHALL** call the D-Bus object:

```
dbus-send --system --print-reply \
  --dest=org.opdbus.v1 \
  /org/opdbus/v1/plugins/zeroclaw \
  org.opdbus.v1.GetState
```

Or via `zbus` in Rust code. Do not read env vars or the tmpfs file for live
state.

---

## 5. Error Handling

### 5.1 op-llm execution errors

All `LlmProvider` implementations return `anyhow::Result<T>`. Callers in
`op-chat` wrap errors in their own `thiserror` types. No change to error types
is introduced by this feature.

### 5.2 Zeroclaw projection errors

`ZeroclawPlugin::write_schema_file_to()` returns `anyhow::Result<()>`.
If the tmpfs write fails (e.g., `/dev/shm` not available), the error is logged
and the plugin continues running. The schema file is a cache; its absence does
not prevent D-Bus operation.

---

## 6. OSCAL Subid Compliance

All existing `x-oscal-subid` annotations on `ZeroclawState`, `LlmTransport`,
`Provider`, `ModelRoute`, etc. **MUST** remain unchanged. The test
`all_subids_are_valid` verifies this.

If any field is added to `ZeroclawState` as part of this work, it **MUST**
carry an `x-oscal-subid` annotation following the format:
`<category>.service.zeroclaw.<subject>.<verb>@v1`.

---

## 7. tmpfs Schema Projection Cache

`/dev/shm/opdbus/schemas/zeroclaw.json` is written by:
1. `ZeroclawPlugin::write_schema_file()` — called at startup.
2. Tests call `write_schema_file_to(tmp_path)` for hermetic verification.

The `op-grpc-bridge` Axum host **reads** this file. It **never writes** it.
This contract is preserved by this spec.

---

## 8. Tests

### 8.1 op-plugins / Zeroclaw schema tests (existing, must stay green)

```bash
cargo test -p op-plugins -- zeroclaw --nocapture
```

Covers:
- `derived_schema_matches_hand_rolled` — schemars-derived schema matches golden.
- `all_subids_are_valid` — all `x-oscal-subid` annotations parse correctly.
- `should_write_zeroclaw_schema_to_shm` — tmpfs write is hermetic and round-trips.

### 8.2 op-llm provider and routing tests (new, §3.3 and §3.4)

```bash
cargo test -p op-llm -- --nocapture
```

Covers:
- `should_resolve_model_falls_back_to_default` (openclaw.rs)
- `should_build_chat_url` (openclaw.rs)
- `should_build_models_url` (openclaw.rs)
- `should_resolve_zeroclaw_declared_providers` (provider.rs)

### 8.3 Full workspace smoke check

```bash
cargo check --workspace
cargo clippy -p op-plugins -p op-llm -- -D warnings
```

---

## 9. What Intentionally Changes Nothing

The following concerns are explicitly out of scope and **must not be touched**:

- `compact-mcp` bind address and exposure — unchanged.
- `cognitive-mcp` gateway configuration — unchanged.
- `op-chat`'s `ChatManager` usage — unchanged.
- `op-grpc-bridge` schema-file read — unchanged.
- `plugin_schema_defs.rs` re-export of `zeroclaw_schema` — unchanged.
- `common/llm_projection.rs` struct definitions — unchanged unless audit in §2
  reveals execution code (expected: none).
- Any plugin other than `zeroclaw` — not touched.
