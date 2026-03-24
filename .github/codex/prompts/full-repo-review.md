Perform a full code review of the entire checked-out repository.

Do not limit your inspection to the current diff or recently changed files. Review the repository broadly enough to catch cross-cutting bugs, hidden regressions, unsafe deploy behavior, schema or API contract drift, permission flaws, test gaps, and integration failures.

Expected review behavior:
- Inspect the full repository, with explicit attention to root `src/`, workspace crates under `crates/`, deployment code under `deploy/`, and GitHub workflows under `.github/`.
- Treat this as a code review, not a style pass. Prioritize correctness, security, reliability, data-loss risk, concurrency issues, auth or privilege mistakes, CI or deploy regressions, and missing tests for risky behavior.
- Findings must come first and be ordered by severity.
- Every finding must include precise file paths and 1-based line references whenever you can support them from the checked-out code.
- Explain the concrete failure mode and why it matters.
- If you do not find a material issue, say so explicitly and then list residual risks or testing gaps.
- Keep the response suitable for posting directly as a GitHub PR comment in Markdown.
