# Technical Specification: Codebase Specification Generation

## Technical Context
- **Language**: Python 3.13+ (ideal for async API interactions, complex text processing, and filesystem traversal).
- **Dependencies**: An LLM SDK like `anthropic` or `google-genai` for communicating with the model, `python-dotenv` for managing API keys, and `asyncio` / `aiohttp` for concurrent processing to speed up generation across ~30 crates.
- **Existing Patterns**: 
  - The project currently relies on `crates/generate_specs.sh`, which uses naive `grep`/`awk` to extract snippets into `crates/SPECS/*.md`. The new system will supersede this script.
  - High-quality, manual specifications reside in `.kiro/specs/op-web` and `.kiro/specs/op-services`. The new tooling will use these as few-shot examples (in-context learning) to guide the LLM's output.

## Implementation Approach
1. **Script Architecture**:
   - Create a Python script `scripts/generate_llm_specs.py`.
   - The script will automatically discover all crates within the `crates/` directory matching the pattern `op-*`.
   - For each target crate, the script will assemble a prompt context comprising:
     - `Cargo.toml` (for dependencies, version, and basic description).
     - The content of all `.rs` files within the crate's `src/` directory (combined into a single text block, leveraging modern large-context LLMs).
     - Global context from `AGENTS.md` to enforce project-specific architectural principles (D-Bus, gRPC, Rust 2021).
   - The script will query the LLM API, instructing it to act as a Senior Systems Architect. The prompt will explicitly request the generation of three distinct documents wrapped in XML tags: `<requirements>`, `<design>`, and `<tasks>`.
   - Upon receiving the response, the script will extract the content from the XML tags and write them to `.kiro/specs/<crate-name>/requirements.md`, `design.md`, and `tasks.md`.

2. **Template Adherence & Prompting Strategy**:
   - To guarantee structural parity with existing specs, the system prompt will include a strict schema or the actual markdown structure from `.kiro/specs/op-web/requirements.md` and `.kiro/specs/op-web/design.md`.
   - Ambiguities in the code will be explicitly addressed by instructing the model to document assumptions clearly under an "Assumptions" header.

3. **Concurrency & Scalability**:
   - The generation process will be highly parallelized using `asyncio` to process multiple crates simultaneously, while implementing exponential backoff to handle potential LLM API rate limits.

## Source Code Structure Changes
- **New Files**:
  - `scripts/generate_llm_specs.py` (The main generation script).
  - Updates to `pyproject.toml` or creation of `scripts/requirements.txt` to include necessary Python dependencies (e.g., `anthropic`, `python-dotenv`).
- **Generated Files**:
  - New directories and markdown files inside `.kiro/specs/` (e.g., `.kiro/specs/op-core/{requirements.md,design.md,tasks.md}`).
- **Deprecated/Removed Files**:
  - `crates/generate_specs.sh` and the entire `crates/SPECS/` directory will be marked for removal, as the new generated specs provide vastly superior, semantic insights.

## Data Model / API / Interface Changes
- N/A. This feature introduces internal developer tooling and documentation. No production data models, gRPC contracts, or D-Bus interfaces are altered.

## Verification Approach
- **Tooling Verification**: Use standard Python tooling for the script itself (e.g., `ruff check scripts/` or `black --check scripts/` if adopted).
- **Execution & Dry-run**:
  - The script must support a `--crate <name>` argument to test generation on a single crate (e.g., `python scripts/generate_llm_specs.py --crate op-core`).
  - Add a `--dry-run` flag to print the constructed LLM prompt to stdout without making the API call, verifying that the context assembly works correctly.
- **Output Validation**:
  - Generate the specs for a sample crate and manually verify that the output structure exactly matches `.kiro/specs/op-web/`.
  - Verify that the markdown files do not contain leftover XML tags or malformed markdown headers.