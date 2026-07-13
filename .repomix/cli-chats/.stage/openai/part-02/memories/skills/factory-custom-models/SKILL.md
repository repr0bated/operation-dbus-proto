---
name: factory-custom-models
description: Use when the user wants models added or updated in Factory under `~/.factory/settings.json`, especially BYOK/customModels work like OpenRouter or xAI Grok families.
argument-hint: "[provider/model family or exact model ids]"
disable-model-invocation: true
user-invocable: false
allowed-tools:
  - Read
  - Grep
  - Bash
---

# Factory custom models

## When to use

Use this when the task is "add this model in Factory", "make all Grok models available", "set up BYOK in Factory", or similar and the real edit surface is `~/.factory/settings.json`.

Do not use this for:
- repo code changes
- Factory UI clicking instructions
- mission-only routing changes that do not touch `customModels`

## Inputs / context to gather

1. Confirm the target file is `~/.factory/settings.json`.
2. Inspect existing `customModels` entries for the provider shape already used on this machine.
3. Identify the key/model source the user pointed to.
   - For xAI/Grok on this machine, check `~/.local/share/opencode/auth.json`, `~/.local/state/opencode/model.json`, and `~/.cache/opencode/models.json`.
4. Decide whether the ask is one model or a whole family.
   - Requests like "all Grok models" mean enumerate the family, not just the first hit.

## Procedure

1. Back up the settings file before editing.
   - Example: `cp ~/.factory/settings.json ~/.factory/settings.json.bak-$(date +%Y%m%dT%H%M%S)`
2. Read the existing `customModels` array and copy the current field shape instead of inventing one.
3. Gather provider-specific facts from targeted local files.
   - xAI key: `~/.local/share/opencode/auth.json`
   - recent selections: `~/.local/state/opencode/model.json`
   - model family/capabilities: `~/.cache/opencode/models.json`
4. Add or update only the requested `customModels` entries.
   - Keep `provider: generic-chat-completion-api` when that is the established pattern.
   - Preserve sequential `index` values.
   - Use `noImageSupport: true` for text/chat-only entries.
5. If the request is a whole family, filter to the usable model class for Factory's text registry.
   - For Grok, include text-output models and exclude non-chat `grok-imagine-*` endpoints.
6. Validate immediately.
   - `jq empty ~/.factory/settings.json`
   - Use a root-bound `jq` query to prove the expected IDs exist.
7. Confirm file permissions remain restrictive if the file contains secrets.

## Efficiency plan

1. Do not start with recursive home-directory scans.
2. Read only `~/.factory/settings.json` and the targeted local provider files.
3. Reuse an existing model entry shape from `customModels` before searching wider.
4. If the user names a provider family, check the provider cache once and derive the full set in one pass.
5. Stop after `jq` validation and exact presence checks succeed.

## Pitfalls and fixes

- Symptom: huge noisy search output with no clear key source
  - Likely cause: broad `rg/find` over the whole home directory.
  - Fix: go straight to the known provider paths such as `~/.local/share/opencode/`, `~/.local/state/opencode/`, and `~/.cache/opencode/models.json`.

- Symptom: `jq` fails with `Cannot index string with string ("customModels")`
  - Likely cause: traversing from the wrong JSON node.
  - Fix: bind the root object first, for example `. as $root | any($root.customModels[]; .id == $id)`.

- Symptom: the user asks for "all" models but only one gets added
  - Likely cause: the request was interpreted too narrowly.
  - Fix: enumerate the provider family from the local model cache and add the full applicable set.

- Symptom: non-chat/image/video endpoints creep into Factory text-model registry
  - Likely cause: filtering only by provider name.
  - Fix: filter by usable text/chat capability and exclude endpoints like `grok-imagine-*` when `maxOutputTokens` is `0`.

## Verification checklist

- `~/.factory/settings.json` parses with `jq empty`.
- The intended `customModels` IDs are present via exact `jq` checks.
- No unintended extra provider-family entries were added.
- Backup file exists.
- Settings file permissions remain appropriate for local secret-bearing config.

## Minimal example

For Grok BYOK expansion on this machine:

```bash
jq empty ~/.factory/settings.json
jq -e '. as $root | any($root.customModels[]; .id == "custom:Grok-4.3-[xAI-BYOK]-0")' ~/.factory/settings.json
jq -r '.providers[]? | select(.id == "xai")' ~/.local/share/opencode/auth.json
```
