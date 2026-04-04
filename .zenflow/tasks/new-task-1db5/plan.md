# Full SDD workflow

## Configuration
- **Artifacts Path**: {@artifacts_path} → `.zenflow/tasks/{task_id}`

---

## Agent Instructions

---

## Workflow Steps

### [x] Step: Requirements
<!-- chat-id: 02e6f6c6-4b19-4d9c-ba39-d2fd4b752de9 -->

Create a Product Requirements Document (PRD) based on the feature description.

1. Review existing codebase to understand current architecture and patterns
2. Analyze the feature definition and identify unclear aspects
3. Ask the user for clarifications on aspects that significantly impact scope or user experience
4. Make reasonable decisions for minor details based on context and conventions
5. If user can't clarify, make a decision, state the assumption, and continue

Focus on **what** the feature should do and **why**, not **how** it should be built. Do not include technical implementation details, technology choices, or code-level decisions — those belong in the Technical Specification.

Save the PRD to `{@artifacts_path}/requirements.md`.

### [x] Step: Technical Specification
<!-- chat-id: fa224f79-babd-4392-abf2-ebeba8fb3320 -->

Create a technical specification based on the PRD in `{@artifacts_path}/requirements.md`.

1. Review existing codebase architecture and identify reusable components
2. Define the implementation approach

Do not include implementation steps, phases, or task breakdowns — those belong in the Planning step.

Save to `{@artifacts_path}/spec.md` with:
- Technical context (language, dependencies)
- Implementation approach referencing existing code patterns
- Source code structure changes
- Data model / API / interface changes
- Verification approach using project lint/test commands

### [x] Step: Planning
<!-- chat-id: 250b256b-6a8f-414a-9faa-81277de33f8d -->

Create a detailed implementation plan based on `{@artifacts_path}/spec.md`.

1. Break down the work into concrete tasks
2. Each task should reference relevant contracts and include verification steps
3. Replace the Implementation step below with the planned tasks

Rule of thumb for step size: each step should represent a coherent unit of work (e.g., implement a component, add an API endpoint). Avoid steps that are too granular (single function) or too broad (entire feature).

Important: unit tests must be part of each implementation task, not separate tasks. Each task should implement the code and its tests together, if relevant.

If the feature is trivial and doesn't warrant full specification, update this workflow to remove unnecessary steps and explain the reasoning to the user.

Save to `{@artifacts_path}/plan.md`.

### [x] Step: Initialize script and setup environment
<!-- chat-id: d378b70c-87a6-4d19-aa09-642a3dd810d5 -->
- Create `scripts/generate_llm_specs.py`.
- Add required Python dependencies (`google-genai`, `python-dotenv`) to `pyproject.toml` or `scripts/requirements.txt`.
- Set up basic CLI argument parsing for `--crate` and `--dry-run` flags.
- [x] Verify script runs and accepts CLI arguments.

### [x] Step: Implement core prompt assembly and API logic
<!-- chat-id: 0d815ec0-2637-4402-8661-17772f07361c -->
- Implement logic to discover crates matching `op-*` in `crates/`.
- Implement context assembly reading `Cargo.toml`, `.rs` files in `src/`, and `AGENTS.md`.
- Integrate few-shot examples from `.kiro/specs/op-web` and `.kiro/specs/op-services`.
- Implement the async LLM API call using `google-genai` with Application Default Credentials (ADC) requesting `<requirements>`, `<design>`, and `<tasks>` XML tags.
- [x] Test with `--dry-run` on a sample crate (`op-core`) to verify prompt construction.

### [x] Step: Implement output parsing, writing and concurrency
<!-- chat-id: df0b3ee7-a83c-4ccc-b5be-e20798282dea -->
- Extract content from the generated XML tags.
- Write the extracted content to `.kiro/specs/<crate-name>/requirements.md`, `design.md`, and `tasks.md`.
- Implement highly parallelized generation using `asyncio` and exponential backoff for handling rate limits.
- [x] Run generation for a sample crate and manually verify output structure matches `.kiro/specs/op-web/`.

### [ ] Step: Clean up deprecated tooling
<!-- chat-id: b2a4a6e6-d83b-4867-b953-f17b6274e8ea -->
- Mark `crates/generate_specs.sh` for removal.
- Remove the `crates/SPECS/` directory.
- [ ] Ensure project builds and lints cleanly.
