thread_id: 019f28df-7d3a-7283-9260-c7671e8ea78d
updated_at: 2026-07-03T16:49:54+00:00
rollout_path: /home/jeremy/.codex/sessions/2026/07/03/rollout-2026-07-03T12-46-03-019f28df-7d3a-7283-9260-c7671e8ea78d.jsonl
cwd: /home/jeremy/git/operation-dbus-proto
git_branch: main

# Added xAI BYOK Grok models to Factory using the local opencode key source

Rollout context: The user was working in `/home/jeremy/git/operation-dbus-proto` and asked to add Grok as BYOK in Factory, then expanded the request to make all Grok models available. The agent used local Factory/opencode config files rather than asking the user to paste a secret.

## Task 1: Add Grok as BYOK in Factory

Outcome: success

Preference signals:
- The user first said: "add grok as byok in factory" -> they want the Factory config updated directly, narrowly, and in the Factory config location rather than discussed abstractly.
- When asked about the key source, the user said: "you can get key in opencode" / "i want all grok models avail" -> use local opencode auth/state as the BYOK source and broaden the registry to cover available Grok chat models without more prompting.

Key steps:
- Confirmed Factory registry lives in `~/.factory/settings.json` and that `customModels` is the relevant place for BYOK custom models.
- Read opencode auth/state under `~/.local/share/opencode/auth.json`, `~/.local/share/opencode/account.json`, and `~/.local/state/opencode/model.json` to identify the active `xai` BYOK key and recent Grok model IDs.
- Added a Factory custom model entry for `grok-4.3` using `baseUrl: https://api.x.ai/v1`, `provider: generic-chat-completion-api`, and the local `xai` API key.
- Backed up `~/.factory/settings.json` before editing and kept file permissions at `600`.
- Validated with `jq empty` and a root-bound existence check for the inserted model entry.

Failures and how to do differently:
- A broad recursive search for opencode config was too noisy and produced huge irrelevant output; the agent then pivoted to targeted reads under `~/.config/opencode/`, `~/.local/share/opencode/`, and `~/.local/state/opencode/`.
- The first attempt only added one Grok model; the user then clarified they wanted all Grok models available, so the agent had to expand scope.

Reusable knowledge:
- In this environment, Factory’s editable registry is `~/.factory/settings.json`, and BYOK chat models are stored in `customModels`.
- opencode stores the active `xai` API key in `~/.local/share/opencode/auth.json` and recent model selection in `~/.local/state/opencode/model.json`.
- For Factory custom chat models, the working pattern was `baseUrl: https://api.x.ai/v1`, `provider: generic-chat-completion-api`, `noImageSupport: true`, and sequential `index` values.
- Grok image/video endpoints (`grok-imagine-*`) are not chat models and were intentionally excluded from Factory’s `customModels` list.

References:
- `~/.factory/settings.json`
- Backup created: `~/.factory/settings.json.bak-20260703T164824`, then `~/.factory/settings.json.bak-20260703T164920`
- Validated model IDs present: `grok-4.3`, `grok-4.20-multi-agent-0309`, `grok-4.20-0309-non-reasoning`, `grok-4.20-0309-reasoning`, `grok-build-0.1`
- Final custom IDs used:
  - `custom:Grok-4.3-[xAI-BYOK]-0`
  - `custom:Grok-4.20-Multi-Agent-0309-[xAI-BYOK]-0`
  - `custom:Grok-4.20-0309-Non-Reasoning-[xAI-BYOK]-0`
  - `custom:Grok-4.20-0309-Reasoning-[xAI-BYOK]-0`
  - `custom:Grok-Build-0.1-[xAI-BYOK]-0`
- Validation commands used:
  - `jq empty /home/jeremy/.factory/settings.json`
  - `jq -e '. as $root | any($root.customModels[]; .id == "custom:Grok-4.3-[xAI-BYOK]-0" ...)' /home/jeremy/.factory/settings.json`
  - `stat -c '%a %n' /home/jeremy/.factory/settings.json ...`

## Task 2: Expand Factory to all Grok chat models

Outcome: success

Preference signals:
- The user said: "i want all grok models avail" -> the default should be to enumerate all usable Grok chat models, not just a single model, and to exclude non-chat endpoints unless the user explicitly asks otherwise.

Key steps:
- Queried opencode’s cached model metadata in `~/.cache/opencode/models.json` to discover all direct `xai` Grok models with text output.
- Filtered to chat-capable models only; intentionally excluded `grok-imagine-image`, `grok-imagine-image-quality`, and `grok-imagine-video` because they have non-text outputs and `maxOutputTokens: 0`.
- Updated `~/.factory/settings.json` to include all direct xAI Grok chat models from opencode with the same BYOK key and matching OpenAI-compatible config.
- Verified the Factory registry now contains exactly the wanted set and no extras/missing entries.

Reusable knowledge:
- The direct xAI Grok models present in the cache and added to Factory were:
  - `grok-4.3`
  - `grok-4.20-multi-agent-0309`
  - `grok-4.20-0309-non-reasoning`
  - `grok-4.20-0309-reasoning`
  - `grok-build-0.1`
- The resulting Factory entries all use the same xAI BYOK key, `generic-chat-completion-api`, `https://api.x.ai/v1`, and `noImageSupport: true`.
- The final registry check showed `wanted`, `present`, `missing`, and `extra` all matched with `missing: []` and `extra: []`.

Failures and how to do differently:
- The agent initially hesitated about whether to include Grok Imagine models; the user’s wording was about all Grok models, but the final implementation used a sensible chat-model-only interpretation for Factory’s text model registry.
- A prior wide `rg/find` scan across the home directory was too noisy; future similar tasks should go straight to opencode’s known config/state files and model cache.

References:
- `~/.cache/opencode/models.json`
- Final verification output confirmed these `xai` Grok models were present in Factory and matched opencode:
  - `grok-4.20-0309-non-reasoning`
  - `grok-4.20-0309-reasoning`
  - `grok-4.20-multi-agent-0309`
  - `grok-4.3`
  - `grok-build-0.1`
- Excluded non-chat Grok endpoints for reference:
  - `grok-imagine-image-quality`
  - `grok-imagine-video`
  - `grok-imagine-image`
