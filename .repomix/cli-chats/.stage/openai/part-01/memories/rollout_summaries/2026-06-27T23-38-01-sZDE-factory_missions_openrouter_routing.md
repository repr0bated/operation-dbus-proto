thread_id: 019f0b72-7fd4-7de2-b90e-a31be8ed412e
updated_at: 2026-06-27T23:39:25+00:00
rollout_path: /home/jeremy/.codex/sessions/2026/06/27/rollout-2026-06-27T19-38-01-019f0b72-7fd4-7de2-b90e-a31be8ed412e.jsonl
cwd: /home/jeremy/git/operation-dbus-proto
git_branch: feat/sled-source-port-salt

# Factory Missions OpenRouter routing was configured in `~/.factory/settings.json` after discovering the existing OpenRouter custom model catalog and a mission-local model-settings file.

Rollout context: the user wanted Factory Missions to use OpenRouter models for the multi-agent execution layer. The agent inspected local Factory config under `/home/jeremy/.factory` and the repo working directory `/home/jeremy/git/operation-dbus-proto`, then updated global Factory settings rather than touching repo code.

## Task 1: Inspect Factory config and identify the correct Missions keys
Outcome: success

Preference signals:
- The user explicitly asked to “follow this to set up openrouter for my mission” and referenced the Missions-specific keys, implying they care about configuring Missions orchestration separately from standard chat and do not want a generic model switch.
- The user’s text said, “You do not need to toggle an enable missions flag inside extraArgs,” which suggests future agents should not invent or require an `enableMissions`-style flag when the request is specifically about model routing.

Key steps:
- Read `~/.factory/settings.json` and found `customModels` already populated with multiple OpenRouter entries.
- Searched for mission-related config and found a mission workspace under `~/.factory/missions/7167cd9e-6b37-4177-852f-0a5f8fa3fc37/` with `model-settings.json`.
- Confirmed that the mission-local file already uses model IDs, not full model objects: `workerModel`, `validationWorkerModel`, `workerReasoningEffort`, `validationWorkerReasoningEffort`, `skipScrutiny`, `skipUserTesting`.

Failures and how to do differently:
- `~/.factory/memories.md` did not exist, so there was no local preferences file to consult.
- A broad `rg` over `~/.factory` produced an enormous truncated output; future similar searches should be narrowed to the specific config paths first.

Reusable knowledge:
- The existing Factory model catalog stores OpenRouter models in `~/.factory/settings.json` under `customModels`, with stable IDs like `custom:GPT-OSS-120B-[OpenRouter]-0`.
- Mission-local settings use IDs, which is a strong hint that global Missions keys should also reference the same `custom:*` IDs rather than embedding full model objects.

References:
- `~/.factory/settings.json` contained `customModels` with OpenRouter entries.
- `~/.factory/missions/7167cd9e-6b37-4177-852f-0a5f8fa3fc37/model-settings.json` showed:
  - `workerModel": "custom:North-Mini-Code-(free)-[OpenRouter]-0"`
  - `validationWorkerModel": "custom:Poolside-Laguna-M.1-(free)-[OpenRouter]-0"`

## Task 2: Patch global Factory settings for Missions OpenRouter routing
Outcome: success

Preference signals:
- The user asked for the setup to apply to “my mission,” which suggests future actions should prioritize mission orchestration settings, not just chat defaults.

Key steps:
- Backed up `~/.factory/settings.json` to `~/.factory/settings.json.bak-20260627193857` before editing.
- Added top-level keys to `~/.factory/settings.json`:
  - `missionOrchestratorModel": "custom:GPT-OSS-120B-[OpenRouter]-0"`
  - `missionModelSettings.workerModel": "custom:North-Mini-Code-(free)-[OpenRouter]-0"`
  - `missionModelSettings.validationWorkerModel": "custom:Poolside-Laguna-M.1-(free)-[OpenRouter]-0"`
  - plus the reasoning-effort and skip flags already used in mission-local settings.
- Validated with `jq empty` and a scoped `jq` query confirming the referenced IDs exist in `customModels`.
- Confirmed file mode remained `600`.

Failures and how to do differently:
- The first `jq` presence check used the wrong scope and failed with `Cannot index string with string ("customModels")`; the fix was to bind the root object explicitly with `. as $root | ...`.

Reusable knowledge:
- On this machine, `~/.factory/settings.json` is the correct place for global Missions model routing keys.
- The OpenRouter-backed IDs that validated successfully were:
  - `custom:GPT-OSS-120B-[OpenRouter]-0`
  - `custom:North-Mini-Code-(free)-[OpenRouter]-0`
  - `custom:Poolside-Laguna-M.1-(free)-[OpenRouter]-0`
- Validation pattern that worked: `jq empty` for syntax plus a root-bound `jq` query to verify referenced IDs resolve against `.customModels[]`.

References:
- Backup file: `/home/jeremy/.factory/settings.json.bak-20260627193857`
- Validation command that succeeded:
  - `jq '. as $root | {missionOrchestratorModel, missionModelSettings, referencedModelsPresent: ([.missionOrchestratorModel, .missionModelSettings.workerModel, .missionModelSettings.validationWorkerModel] as $ids | all($ids[]; . as $id | any($root.customModels[]; .id == $id)))}' /home/jeremy/.factory/settings.json`
- Successful validation output included `referencedModelsPresent: true`.

