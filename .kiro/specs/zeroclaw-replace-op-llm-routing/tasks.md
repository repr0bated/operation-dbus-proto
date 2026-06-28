# Tasks: zeroclaw-replace-op-llm-routing

Implementation order for an agent. Each task is self-contained; complete and
verify before moving to the next.

---

## Task 1 — Pre-flight: confirm both crates compile

**Goal:** establish a clean baseline before any changes.

```bash
cargo check -p op-plugins 2>&1 | tail -5
cargo check -p op-llm 2>&1 | tail -5
```

Expected: both exit 0 with no errors.

If either fails, **stop**. Note the pre-existing errors as out-of-scope and
raise them separately. Do not proceed until the baseline is clean.

---

## Task 2 — Boundary audit: confirm Zeroclaw has no execution leakage

**Goal:** verify the architectural boundary is already clean (expected result).

```bash
# Should return zero matches
grep -n "reqwest\|Client::new\|Client::builder\|hyper\|TcpStream" \
  crates/op-plugins/src/state_plugins/zeroclaw.rs \
  crates/op-plugins/src/state_plugins/common/llm_projection.rs

# Should return zero matches
grep -n "op_llm::\|extern crate op_llm" \
  crates/op-plugins/src/state_plugins/zeroclaw.rs

# Should return exactly one match (op-llm/src/provider.rs)
grep -rn "^pub trait LlmProvider\|^trait LlmProvider" crates/ --include="*.rs"

# Should return at least one match in op-chat
grep -rn "op_llm::\|use op_llm" crates/op-chat/src/
```

**If the network-call or op_llm-import audits return matches**, perform the
cleanup described in spec.md §3.1 before continuing. Remove the offending
code; do not work around it.

**If all audits return the expected results**, proceed directly to Task 3.

---

## Task 3 — Add unit tests to `crates/op-llm/src/openclaw.rs`

**Goal:** add three pure-logic tests for `OpenClawProvider` (spec.md §3.3).

File: `crates/op-llm/src/openclaw.rs`

Append at the end of the file (inside a `#[cfg(test)]` block):

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

Check that `resolve_model`, `chat_url`, and `models_url` are either `pub(crate)`
or `pub` — if they are private, make them `pub(crate)` first.

Verify:
```bash
cargo check -p op-llm
cargo test -p op-llm -- openclaw --nocapture
```

---

## Task 4 — Add provider round-trip test to `crates/op-llm/src/provider.rs`

**Goal:** verify `ProviderType::from_str` handles all provider strings that
Zeroclaw declares in its route table (spec.md §3.4).

File: `crates/op-llm/src/provider.rs`

Before writing the test, check whether `"ollama"` is handled in the `from_str`
match:

```bash
grep -n '"ollama"' crates/op-llm/src/provider.rs
```

- If it is **not** handled: add `"ollama" => Ok(ProviderType::Custom("ollama".to_string()))` 
  to the match arm (after `"openrouter"` and similar OpenAI-compatible entries).
- If it **is** handled: use whatever variant it maps to in the test.

Append the test block:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_resolve_zeroclaw_declared_providers() {
        // Each entry: (input string, expected Display output)
        let cases: &[(&str, &str)] = &[
            ("factory", "factory"),
            ("gemini", "gemini"),
            ("anthropic", "anthropic"),
            ("openai", "openai"),
            ("openclaw", "openclaw"),
            ("openrouter", "openrouter"),
        ];
        for (input, expected_display) in cases {
            let got = input.parse::<ProviderType>()
                .unwrap_or_else(|e| panic!("failed to parse '{}': {}", input, e));
            assert_eq!(
                got.to_string(), *expected_display,
                "provider '{}' did not round-trip", input
            );
        }
    }
}
```

Verify:
```bash
cargo check -p op-llm
cargo test -p op-llm -- provider --nocapture
```

---

## Task 5 — Run all Zeroclaw schema tests

**Goal:** confirm existing schema tests remain green after Tasks 3–4.

```bash
cargo test -p op-plugins -- zeroclaw --nocapture
```

Expected: all three tests pass:
- `zeroclaw::tests::derived_schema_matches_hand_rolled`
- `zeroclaw::tests::all_subids_are_valid`
- `zeroclaw::tests::should_write_zeroclaw_schema_to_shm`

If any fail, investigate the cause before proceeding. The most likely cause is
a pre-existing issue unrelated to this feature — confirm by checking `git diff`.
Do not mask failures.

---

## Task 6 — Full workspace check and clippy

**Goal:** confirm no regressions across the workspace.

```bash
cargo check --workspace 2>&1 | tail -10
cargo clippy -p op-plugins -p op-llm -- -D warnings 2>&1 | tail -20
```

Fix any clippy warnings introduced by the new test code (e.g., dead-code,
unused-imports). Do not fix pre-existing warnings unrelated to this feature.

---

## Task 7 — Run all op-llm tests

**Goal:** confirm the full op-llm test suite passes with the new tests.

```bash
cargo test -p op-llm --all-targets -- --nocapture 2>&1 | tail -30
```

All tests should pass. Network-dependent tests may be skipped if the upstream
service is unavailable — that is expected in CI.

---

## Task 8 — Final verification checklist

Run each check and confirm the expected result:

```bash
# 1. No network client code in Zeroclaw
grep -c "reqwest\|Client::new" \
  crates/op-plugins/src/state_plugins/zeroclaw.rs
# Expected: 0

# 2. op-plugins does not depend on op-llm
grep "op-llm" crates/op-plugins/Cargo.toml
# Expected: no output (or only a dev-dependency if tests need it)

# 3. LlmProvider trait defined only once
grep -rn "^pub trait LlmProvider" crates/ --include="*.rs"
# Expected: exactly 1 match in crates/op-llm/src/provider.rs

# 4. compact-mcp still loopback-only
grep -rn "compact.mcp\|compact_mcp\|11436" crates/ --include="*.rs" | grep -v "127.0.0.1"
# Expected: no external bind address references

# 5. Both targeted crates compile
cargo check -p op-plugins && cargo check -p op-llm
# Expected: exit 0

# 6. Zeroclaw tests green
cargo test -p op-plugins -- zeroclaw
# Expected: 3 passed, 0 failed

# 7. op-llm tests green
cargo test -p op-llm
# Expected: all pass (network tests may be skipped)
```

---

## Done Criteria

The feature is complete when:

1. Tasks 1–8 all report the expected results.
2. No new binary, shim service, or crate has been created.
3. `plugin_schema_defs.rs` is unchanged.
4. `common/llm_projection.rs` is unchanged (unless audit revealed execution
   code that was removed).
5. `compact-mcp` and `cognitive-mcp` configurations are unchanged.
6. `git diff --stat` shows changes only in:
   - `crates/op-llm/src/openclaw.rs` (new test block)
   - `crates/op-llm/src/provider.rs` (new test block, optional `"ollama"` match arm)
   - Possibly `crates/op-plugins/src/state_plugins/zeroclaw.rs` if
     execution leakage was found and removed (expected: no change needed).
