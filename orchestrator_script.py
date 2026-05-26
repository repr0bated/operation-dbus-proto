"""
Rust Crate Audit Orchestrator
Runs N specialist LLM agents in parallel per crate, validates citations,
emits structured JSON findings, and produces a zipped markdown report.

Usage:
  python3 orchestrator_script.py [--crate NAME] [--resume] [--force] [--rpm N]
"""

import os
import re
import sys
import glob
import json
import time
import zipfile
import argparse
import threading
from concurrent.futures import ThreadPoolExecutor, as_completed
from openai import OpenAI, RateLimitError

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------

ROOT           = os.getcwd()
CRATES_DIR     = os.path.join(ROOT, 'crates')
AUDIT_OUTPUT_DIR = os.path.join(ROOT, 'audits')

LLM_BASE_URL   = os.getenv("LLM_BASE_URL", "http://127.0.0.1:11435/v1")
LLM_MODEL      = os.getenv("LLM_MODEL",    "gemini-2.5-pro")

MAX_FILE_BYTES    = 200_000      # truncate any single file over 200 KB
MAX_CONTEXT_CHARS = 2_800_000   # ~700 K tokens; trim payload if exceeded
MAX_WORKERS       = 16
SKIP_CRATES       = {"op-core"}

_client = OpenAI(api_key="mcp-api", base_url=LLM_BASE_URL)

# ---------------------------------------------------------------------------
# Qdrant RAG — semantic retrieval from 97-repo vector store
# ---------------------------------------------------------------------------

import urllib.request
import urllib.error

VOYAGE_API_KEY    = os.getenv("VOYAGE_API_KEY", "")
VOYAGE_MODEL      = os.getenv("VOYAGE_MODEL", "voyage-4-lite")
VOYAGE_EMBED_URL  = "https://api.voyageai.com/v1/embeddings"
QDRANT_REST_URL   = os.getenv("QDRANT_URL", "http://127.0.0.1:6333")
QDRANT_COLLECTION = os.getenv("QDRANT_COLLECTION", "repomix_rag")
RAG_LIMIT         = int(os.getenv("RAG_LIMIT", "8"))   # chunks per role query

# Per-role semantic queries — mapped to the intent of each worker role
_ROLE_RAG_QUERIES = {
    "01_Architecture_Module_Map":          "Rust crate architecture module hierarchy entry points lib.rs",
    "02_Public_API_Surface_And_Dead_Code": "Rust public API surface dead code unused exports pub use glob",
    "03_Dependency_Feature_Audit":         "Cargo.toml dependency features sled cozo storage backend",
    "04_Error_Handling_Panic_Safety":      "Rust unwrap expect panic Result error handling propagation",
    "05_Type_System_Trait_Coherence":      "Rust trait implementation coherence orphan rule type safety",
    "06_Async_Concurrency_Model":          "Rust async tokio spawn concurrency race condition blocking",
    "07_Security_Unsafe_Surface":          "Rust unsafe block command injection process spawn security vulnerability",
    "08_Performance_Allocation_And_Memory":"Rust memory allocation mmap performance hot path simd",
    "09_Serialization_Data_Integrity":     "Rust serde serialization deserialization JSON validation",
    "10_Testing_Coverage":                 "Rust unit test integration test coverage property testing",
    "11_Logging_Observability":            "Rust tracing logging metrics observability structured logs",
    "12_Configuration_Secrets":            "Rust configuration secrets environment variables hardcoded credentials",
    "13_Build_Codegen":                    "Rust build.rs codegen proc macro tonic prost protobuf",
    "14_FFI_Platform_Bindings":            "Rust FFI C bindings unsafe extern platform specific",
    "15_Dependency_Supply_Chain":          "Rust supply chain dependency audit crates.io yanked",
    "16_Improvement_Suggestions":          "Rust architecture improvement refactoring ergonomics observability",
    "17_DBus_IPC_Attack_Surface":          "D-Bus IPC system bus authentication authorization security",
    "18_Schema_OSCAL_Compliance":          "schema as code protobuf OSCAL FedRAMP NIST 800-53 compliance control mapping",
    "19_Risk_Register_Tech_Debt":          "Rust security vulnerability risk register critical high severity",
}

def _voyage_embed(text: str) -> list[float] | None:
    """Embed query text via Voyage AI. Returns None if key missing or error."""
    if not VOYAGE_API_KEY:
        return None
    payload = json.dumps({
        "input": [text],
        "model": VOYAGE_MODEL,
        "input_type": "query",
    }).encode()
    req = urllib.request.Request(
        VOYAGE_EMBED_URL,
        data=payload,
        headers={
            "Authorization": f"Bearer {VOYAGE_API_KEY}",
            "Content-Type": "application/json",
        },
        method="POST",
    )
    try:
        with urllib.request.urlopen(req, timeout=15) as resp:
            body = json.loads(resp.read())
        return body["data"][0]["embedding"]
    except Exception as e:
        print(f"  [rag] Voyage embed error: {e}")
        return None

def _qdrant_search(vector: list[float], limit: int) -> list[dict]:
    """Run cosine similarity search against repomix_rag collection."""
    payload = json.dumps({
        "vector": vector,
        "limit": limit,
        "with_payload": True,
        "with_vector": False,
    }).encode()
    req = urllib.request.Request(
        f"{QDRANT_REST_URL}/collections/{QDRANT_COLLECTION}/points/search",
        data=payload,
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    try:
        with urllib.request.urlopen(req, timeout=10) as resp:
            body = json.loads(resp.read())
        return body.get("result", [])
    except Exception as e:
        print(f"  [rag] Qdrant search error: {e}")
        return []

def fetch_rag_context(role_name: str) -> str:
    """
    Retrieve semantically relevant chunks from the 97-repo vector store
    and format them as a reference block for the LLM prompt.
    Returns empty string if RAG is unavailable.
    """
    # Strip leading numeric prefix if present (e.g. "07_Security..." → lookup key)
    bare = re.sub(r'^\d+_', '', role_name)
    # Try exact match first, then prefix match
    query = _ROLE_RAG_QUERIES.get(role_name) or next(
        (v for k, v in _ROLE_RAG_QUERIES.items() if k.endswith(bare)), None
    )
    if not query:
        return ""

    vector = _voyage_embed(query)
    if vector is None:
        return ""

    hits = _qdrant_search(vector, RAG_LIMIT)
    if not hits:
        return ""

    parts = [
        "## Reference: Semantically Similar Code from 97-Repo Corpus\n"
        "The following excerpts are from related open-source repositories "
        "(mullvadvpn-app, omicron, dropshot, serde, systemd, dbus-broker, "
        "automated-security-helper, fedramp-automation, opa, protovalidate, and others). "
        "Use them as reference patterns — note how production codebases handle schema-as-code, "
        "OSCAL compliance, and the specific pattern being audited. "
        "⚠ STRICT RULE: Do NOT produce any file:line citation from this Reference section. "
        "ALL citations in your output must refer exclusively to files in the FILES section below.\n"
    ]
    for i, hit in enumerate(hits, 1):
        p = hit.get("payload", {})
        repo      = p.get("repo", "unknown")
        filepath  = p.get("file_path", "")
        content   = p.get("content", "").strip()
        score     = hit.get("score", 0.0)
        line_s    = p.get("line_start", "")
        parts.append(
            f"### [{i}] {repo} — {filepath}"
            + (f" (lines {line_s}–{p.get('line_end','')})" if line_s else "")
            + f"  score={score:.3f}\n```\n{content[:1500]}\n```\n"
        )
    return "\n".join(parts)


def fetch_best_practice(snippet: str, limit: int = 3) -> list[dict]:
    """
    Embed a specific code snippet and retrieve the closest corpus examples.
    Used by the best-practice compliance role to find how the same pattern
    is handled in production-quality repos.
    """
    vector = _voyage_embed(snippet)
    if vector is None:
        return []
    return _qdrant_search(vector, limit)


# ---------------------------------------------------------------------------
# Best-practice pattern extraction (static grep pass)
# ---------------------------------------------------------------------------

# Each entry: (pattern_name, regex, context_lines)
_BP_PATTERNS = [
    ("unsafe_block",          re.compile(r'unsafe\s*\{'),                          2),
    ("simd_json_from_str",    re.compile(r'simd_json::from_str'),                  2),
    ("unwrap_on_lock",        re.compile(r'\.(read|write)\(\)\s*\.unwrap\(\)'),    2),
    ("command_new",           re.compile(r'Command::new\s*\('),                    3),
    ("format_json_manual",    re.compile(r'format!\s*\(\s*r?".*\{.*\}.*"'),        2),
    ("static_mut",            re.compile(r'static\s+mut\s+'),                      2),
    ("unwrap_expect",         re.compile(r'\.(unwrap|expect)\s*\('),               1),
    ("std_fs_in_async",       re.compile(r'std::fs::|tokio::fs::'),                2),
    ("process_exit",          re.compile(r'std::process::exit|process::exit'),     2),
    ("spawn_blocking",        re.compile(r'spawn_blocking\s*\('),                  2),
]

def extract_code_patterns(file_contents: dict) -> list[dict]:
    """
    Grep all crate files for known anti-patterns. Returns a list of
    {pattern, file, line, snippet} dicts, capped at 5 instances per pattern
    to avoid flooding the prompt.
    """
    found = []
    per_pattern_cap = 5
    counts: dict[str, int] = {}

    for rel_path, content in file_contents.items():
        if not rel_path.endswith('.rs'):
            continue
        lines = content.splitlines()
        for pattern_name, regex, ctx in _BP_PATTERNS:
            if counts.get(pattern_name, 0) >= per_pattern_cap:
                continue
            for lineno, line in enumerate(lines, 1):
                if counts.get(pattern_name, 0) >= per_pattern_cap:
                    break
                if regex.search(line):
                    start = max(0, lineno - 2)
                    end   = min(len(lines), lineno + ctx)
                    snippet = "\n".join(lines[start:end])
                    found.append({
                        "pattern": pattern_name,
                        "file":    rel_path,
                        "line":    lineno,
                        "snippet": snippet,
                    })
                    counts[pattern_name] = counts.get(pattern_name, 0) + 1
    return found


def run_best_practice_role(crate_name: str, file_contents: dict,
                            audit_dir: str, resume: bool) -> None:
    """
    Role 00: Best Practice Compliance.
    Extracts specific anti-patterns from the crate source, retrieves how
    the same pattern is handled in the 97-repo corpus via vector search,
    then asks the LLM to produce a compliance gap table.
    """
    output_path = os.path.join(audit_dir, "00_Best_Practice_Compliance.md")
    if resume and os.path.exists(output_path) and os.path.getsize(output_path) > 0:
        print("  [skip] Best_Practice_Compliance (--resume)")
        return

    print("  [run] Best_Practice_Compliance (pattern extract + vector lookup)...")

    patterns_found = extract_code_patterns(file_contents)
    if not patterns_found:
        with open(output_path, 'w') as f:
            f.write("# Best Practice Compliance\n\nNo known anti-patterns detected.\n")
        return

    # For each found pattern, retrieve best-practice examples from corpus
    comparisons = []
    for hit in patterns_found:
        corpus_hits = fetch_best_practice(hit["snippet"], limit=2)
        corpus_block = ""
        for ch in corpus_hits:
            p = ch.get("payload", {})
            corpus_block += (
                f"  **{p.get('repo','?')}** `{p.get('file_path','')}`"
                f" (score={ch.get('score',0):.3f}):\n"
                f"  ```\n  {p.get('content','').strip()[:600]}\n  ```\n"
            )
        comparisons.append({**hit, "corpus": corpus_block or "  (no corpus matches)"})

    # Build a structured prompt for the LLM
    blocks = []
    for c in comparisons:
        blocks.append(
            f"### Pattern: `{c['pattern']}` — `{c['file']}:{c['line']}`\n"
            f"**Code in crate:**\n```rust\n{c['snippet']}\n```\n"
            f"**Corpus best practice examples:**\n{c['corpus']}"
        )

    bp_prompt = (
        "ROLE: Best Practice Compliance\n\n"
        "Below are specific code patterns extracted from the crate, paired with "
        "semantically similar implementations retrieved from a 97-repository "
        "production corpus (mullvadvpn-app, omicron, dropshot, serde, systemd, "
        "dbus-broker, automated-security-helper, and others).\n\n"
        "For EACH pattern:\n"
        "1. Assess whether the crate implementation follows the corpus best practice\n"
        "2. If it deviates, describe the gap concisely\n"
        "3. Rate the gap: Compliant | Minor Gap | Major Gap | Critical Gap\n\n"
        "Output a markdown table: "
        "| Pattern | File:Line | Crate Approach | Corpus Best Practice | Gap | Rating |\n\n"
        "Then list actionable recommendations for each Major/Critical gap.\n\n"
        + "\n\n".join(blocks)
    )

    output_md = call_llm_agent(SYSTEM_PROMPT, bp_prompt, crate_name, {})
    os.makedirs(audit_dir, exist_ok=True)
    with open(output_path, 'w', encoding='utf-8') as f:
        f.write(output_md)
    print("  [done] Best_Practice_Compliance ✓")


# ---------------------------------------------------------------------------
# Global thread-safe rate limiter (token bucket)
# ---------------------------------------------------------------------------

_rl_lock     = threading.Lock()
_rl_tokens   = 0.0
_rl_capacity = 0.0
_rl_last     = time.monotonic()

def _rate_init(rpm: int) -> None:
    global _rl_tokens, _rl_capacity, _rl_last
    _rl_capacity = float(rpm)
    _rl_tokens   = float(rpm)
    _rl_last     = time.monotonic()

def _rate_acquire() -> None:
    """Block the calling thread until a token is available."""
    global _rl_tokens, _rl_last
    while True:
        with _rl_lock:
            now     = time.monotonic()
            elapsed = now - _rl_last
            _rl_tokens = min(_rl_capacity, _rl_tokens + elapsed * _rl_capacity / 60.0)
            _rl_last   = now
            if _rl_tokens >= 1.0:
                _rl_tokens -= 1.0
                return
            wait = (1.0 - _rl_tokens) * 60.0 / _rl_capacity
        time.sleep(min(wait, 1.0))

# ---------------------------------------------------------------------------
# LLM call with retry + rate limiting
# ---------------------------------------------------------------------------

def call_llm_agent(system_prompt: str, role_prompt: str,
                   crate_name: str, file_contents: dict) -> str:
    file_block = _build_file_block(file_contents)
    user_msg = (
        f"ROLE: {role_prompt}\n\n"
        f"CRATE: {crate_name}\n\n"
        f"FILES:\n{file_block}\n\n"
        "Produce ONE markdown file, no preamble. "
        "Cite every finding as file:line using ONLY files listed in the FILES section above — "
        "NEVER cite anything from the Reference corpus. "
        "Only mark something Critical if it is exploitable given the files shown — "
        "do not speculate about threat models not evidenced in the code."
    )
    messages = [
        {"role": "system", "content": system_prompt},
        {"role": "user",   "content": user_msg},
    ]

    backoff = 15
    while True:
        _rate_acquire()
        try:
            resp = _client.chat.completions.create(model=LLM_MODEL, messages=messages)
            return resp.choices[0].message.content or ""
        except RateLimitError as e:
            print(f"  [{crate_name}] rate limit — waiting {backoff}s")
            time.sleep(backoff)
            backoff = min(backoff * 2, 240)
        except Exception as e:
            msg = str(e).lower()
            if any(x in msg for x in ("resource exhausted", "quota", "429")):
                print(f"  [{crate_name}] resource exhausted — waiting {backoff}s")
                time.sleep(backoff)
                backoff = min(backoff * 2, 240)
            else:
                print(f"  [{crate_name}] LLM error: {e}")
                return f"# Error\n\n```\n{e}\n```"

def _build_file_block(file_contents: dict) -> str:
    """Assemble file content, truncating oversized files and total payload."""
    parts = []
    total = 0
    for path, content in file_contents.items():
        if len(content) > MAX_FILE_BYTES:
            content = content[:MAX_FILE_BYTES] + f"\n\n[TRUNCATED: file exceeds {MAX_FILE_BYTES} bytes]"
        chunk = f"--- {path} ---\n{content}"
        if total + len(chunk) > MAX_CONTEXT_CHARS:
            parts.append(f"--- [PAYLOAD LIMIT REACHED: remaining files omitted] ---")
            break
        parts.append(chunk)
        total += len(chunk)
    return "\n".join(parts)

# ---------------------------------------------------------------------------
# Citation validator
# ---------------------------------------------------------------------------

_CITATION_RE = re.compile(r'`?(crates/[\w\-/]+\.rs):(\d+)`?')

def validate_citations(output_md: str) -> list[dict]:
    """
    Extract file:line citations from LLM output and verify they exist on disk.
    Returns a list of {citation, valid, reason} dicts.
    """
    results = []
    for m in _CITATION_RE.finditer(output_md):
        rel_path, lineno = m.group(1), int(m.group(2))
        abs_path = os.path.join(ROOT, rel_path)
        if not os.path.exists(abs_path):
            results.append({"citation": f"{rel_path}:{lineno}", "valid": False,
                            "reason": "file not found"})
            continue
        with open(abs_path, encoding="utf-8", errors="replace") as f:
            line_count = sum(1 for _ in f)
        if lineno > line_count:
            results.append({"citation": f"{rel_path}:{lineno}", "valid": False,
                            "reason": f"file has {line_count} lines"})
        else:
            results.append({"citation": f"{rel_path}:{lineno}", "valid": True,
                            "reason": "ok"})
    return results

# ---------------------------------------------------------------------------
# Structured findings extractor (from risk register markdown table)
# ---------------------------------------------------------------------------

_ROW_RE = re.compile(
    r'\|\s*\*{0,2}(Critical|High|Med(?:ium)?|Low)\*{0,2}\s*\|([^|]+)\|([^|]+)\|([^|]+)\|',
    re.IGNORECASE
)

def extract_findings(risk_md: str, crate_name: str) -> list[dict]:
    findings = []
    for m in _ROW_RE.finditer(risk_md):
        severity, issue, evidence, rec = [x.strip() for x in m.groups()]
        findings.append({
            "crate":          crate_name,
            "severity":       severity.capitalize(),
            "issue":          re.sub(r'\*+', '', issue).strip(),
            "evidence":       evidence.strip(),
            "recommendation": re.sub(r'\*+', '', rec).strip(),
        })
    return findings

# ---------------------------------------------------------------------------
# Agent prompts
# ---------------------------------------------------------------------------

SYSTEM_PROMPT = (
    "You are a Senior Rust Systems Architect performing a production security and quality audit. "
    "Audit ONLY the files provided in the FILES section — do not speculate about code you cannot see. "
    "Mark a finding Critical only if it is directly exploitable given the provided source. "
    "CITATION RULE: Every file:line citation MUST refer to a file listed in the FILES section. "
    "NEVER cite files or line numbers from the Reference corpus — those are background examples only. "
    "SCHEMA-AS-CODE: This codebase follows a schema-as-code discipline using Protocol Buffers and OSCAL. "
    "Flag any place where data contracts are expressed as ad-hoc structs or strings rather than versioned schemas. "
    "Produce ONE markdown document, no preamble."
)

WORKER_PROMPTS = [
    ("01_Architecture_Module_Map", (
        "ROLE: Architecture & Module Map\n"
        "- List total .rs files, top-level modules, bin targets\n"
        "- Identify entry points (lib.rs, main.rs, bin/*)\n"
        "- Note module hierarchy (mod declarations)\n"
        "- Output sections: Overview, Module Tree, Entry Points, Notes\n"
        "Cite: crates/{CRATE}/src/lib.rs:1"
    )),
    ("02_Public_API_Surface_And_Dead_Code", (
        "ROLE: Public API Surface & Dead Code\n"
        "- Enumerate every `pub` item (fn/struct/enum/trait/const/static/mod/type/use)\n"
        "- Count totals, list top 10 most impactful with file:line\n"
        "- Flag glob re-exports (pub use *)\n"
        "- Flag pub fields on structs that should be private\n"
        "DEAD CODE:\n"
        "- List every `#[allow(dead_code)]` attribute with file:line — these suppress warnings, not fix the problem\n"
        "- Find functions/structs/enums defined but never referenced within the provided files\n"
        "- Find `use` imports that are unused (look for compiler hint patterns: prefixed with _)\n"
        "- Find `mod` declarations where the module file appears empty or contains only TODOs\n"
        "- Output a Dead Code table: Item | Type | file:line | Recommendation (remove/expose/test)"
    )),
    ("03_Dependency_Feature_Audit", (
        "ROLE: Dependencies & Feature Inventory\n"
        "- List EVERY direct dependency from Cargo.toml with its version and enabled features\n"
        "- For each dep, state which features are explicitly enabled vs pulled in by default\n"
        "- Flag yanked, unpinned (*), or CVE-adjacent crates\n"
        "- Flag anyhow, thiserror, tokio — note which tokio features are enabled\n"
        "- List the crate's own [features] section in full, or state 'none defined'\n"
        "- For each feature: list what it gates (which deps/code blocks use cfg(feature=...))\n"
        "SCHEMA-AS-CODE DEPENDENCIES:\n"
        "- Check for prost, tonic, prost-build, tonic-build (Protocol Buffer schema-as-code)\n"
        "- Check for protovalidate, protoc-gen-validate (schema field validation)\n"
        "- Check for schemars, jsonschema, openapiv3 (JSON/OpenAPI schema generation)\n"
        "- Check for oscal-rs, fedramp, or any OSCAL-related crate\n"
        "- Flag any crate that handles structured data but has NO schema dependency — "
        "ad-hoc struct definitions are a schema-as-code gap\n"
        "STORAGE BACKEND CHECK:\n"
        "- Search all src files for: sqlx, rusqlite, sqlite, SqlitePool, SqliteConnection, diesel\n"
        "- Search for: sled, DbInstance, cozo, CozoDB, op-cozo-store\n"
        "- Search for: redis, memcached, op-cache\n"
        "- Produce a Storage Backend table: Backend | Found at file:line | Role (KV/Graph/Cache/Queue)\n"
        "- Flag if SQLite/sqlx is used for graph or knowledge storage (architectural violation if CozoDB/sled is mandated)\n"
        "- Flag if sled/cozo is absent where it is expected per the architecture"
    )),
    ("04_Data_Structures_State_Model", (
        "ROLE: Data Structures\n"
        "- Count Arc, Rc, RefCell, RwLock, Mutex, OnceCell per file\n"
        "- Count .clone() calls — flag counts > 20 in a single file\n"
        "- Flag large structs (> 5 public fields)\n"
        "- Note any globally mutable state (static mut, lazy_static)"
    )),
    ("05_Error_Handling_Result_Patterns", (
        "ROLE: Error Handling\n"
        "- Count .unwrap(), .expect(), .unwrap_or(), ? operator\n"
        "- Count todo!(), unimplemented!(), panic!()\n"
        "- List first 5 .unwrap() sites with file:line + 1-line context\n"
        "- Flag .unwrap() on RwLock/Mutex (lock poisoning risk)\n"
        "- Recommend Result vs panic at each site"
    )),
    ("06_Async_Concurrency_Model", (
        "ROLE: Async & Concurrency\n"
        "- Count async fn, tokio::spawn, spawn_blocking\n"
        "- Flag std::fs, std::process::Command::output() inside async fns (blocks reactor)\n"
        "- Flag missing .await or dropped JoinHandles\n"
        "- Check Send/Sync bounds on public async traits"
    )),
    ("07_Security_Unsafe_Surface", (
        "ROLE: Security & Unsafe\n"
        "- List ALL `unsafe {` blocks with file:line + 1-line context\n"
        "- Flag any missing `// SAFETY:` comment\n"
        "- Count Command::new() — note whether args are validated or user-controlled\n"
        "FORBIDDEN COMMANDS:\n"
        "  The following commands are FORBIDDEN in this codebase. Flag every Command::new() or\n"
        "  similar spawn site that references them, regardless of how the string is constructed:\n"
        "  - Any `ovs-*` command (ovs-vsctl, ovs-ofctl, ovs-dpctl, ovs-appctl, ovsdb-client,\n"
        "    ovsdb-server, ovs-vswitchd, ovs-testcontroller, etc.)\n"
        "  - Any raw OpenFlow command or tool (of-client, ofprotocol, dpctl)\n"
        "  - `bash`, `sh`, `dash`, `zsh`, `ksh`, `csh` — shell invocations that bypass argument\n"
        "    validation\n"
        "  - `curl`, `wget`, `nc`, `ncat`, `nmap` — network tools that can exfiltrate data\n"
        "  For each forbidden hit: cite file:line, show the command string, and rate severity High.\n"
        "- Flag hardcoded IPs, tokens, passwords\n"
        "- Note D-Bus method exposure: which methods are callable by any system-bus peer"
    )),
    ("08_Performance_Allocation_And_Memory", (
        "ROLE: Performance, Allocation & Memory Map\n"
        "- Flag Vec::new / String::new inside loops without pre-allocation\n"
        "- Count format!() in hot paths (request handlers, loops)\n"
        "- Flag simd_json unsafe usage on non-padded buffers\n"
        "- Flag OwnedValue.clone() on large JSON payloads\n"
        "MEMORY MAPPING:\n"
        "- Find all uses of memmap2, mmap, MmapMut, MmapOptions with file:line\n"
        "- For each mmap site: note the file/fd being mapped, size if determinable, whether it is read-only or writable\n"
        "- Flag writable mmaps without explicit flush/msync before drop\n"
        "- Flag mmaps of user-supplied paths (TOCTOU risk)\n"
        "- Note sled usage: sled opens its own mmaps internally — flag if sled is opened on a tmpfs or noexec mount\n"
        "- List all large heap allocations (Vec with capacity > 1MB, Bytes::with_capacity, BytesMut)\n"
        "- Output a Memory Map table: Site | file:line | Type (ro/rw/sled) | Risk"
    )),
    ("09_Test_Coverage_Examples", (
        "ROLE: Tests\n"
        "- Find #[cfg(test)], #[test], integration tests in tests/\n"
        "- Count test functions total\n"
        "- List 3 representative tests with file:line\n"
        "- Note property tests (proptest, quickcheck) or fuzzing\n"
        "- State 'No tests found' if zero — flag as High risk"
    )),
    ("10_Documentation_Completeness", (
        "ROLE: Docs\n"
        "- Check crate-level //! docs in lib.rs\n"
        "- Sample 10 pub items: flag those missing /// rustdoc\n"
        "- Note README.md presence\n"
        "- Flag public unsafe fns with no doc explaining invariants"
    )),
    ("11_Configuration_Feature_Flags", (
        "ROLE: Configuration\n"
        "- List all std::env::var reads with file:line\n"
        "- Flag env vars with no default and no error handling\n"
        "- List Cargo features and whether default features are additive\n"
        "- Flag hardcoded paths, ports, addresses"
    )),
    ("12_Integration_Points_Consumers", (
        "ROLE: Integration\n"
        "- List crates in workspace Cargo.toml that depend on {CRATE}\n"
        "- Note D-Bus service names and object paths registered\n"
        "- Note HTTP/gRPC endpoints exposed\n"
        "- Flag any cross-crate circular dependency risk"
    )),
    ("13_Observability_Logging", (
        "ROLE: Observability\n"
        "- Count tracing:: macros (info/warn/error/debug) vs println!\n"
        "- Flag errors swallowed without logging\n"
        "- Flag PII or secrets that may appear in log output\n"
        "- Note metrics instrumentation (prometheus, metrics crate)"
    )),
    ("14_Build_CI_Workspace_Wiring", (
        "ROLE: Build\n"
        "- Read Cargo.toml: edition, rust-version, bins, examples\n"
        "- Check build.rs for codegen risks (arbitrary shell exec)\n"
        "- Note workspace inheritance vs local overrides\n"
        "SCHEMA-AS-CODE BUILD CHECK:\n"
        "- Does build.rs invoke prost-build or tonic-build to compile .proto files? List them.\n"
        "- Are .proto files checked into the repo as the source of truth (schema-as-code)?\n"
        "- Flag if generated Rust files are committed instead of .proto sources\n"
        "- Flag if proto compilation happens at runtime instead of build time"
    )),
    ("15_License_Compliance", (
        "ROLE: License\n"
        "- Extract license field from Cargo.toml\n"
        "- Scan Cargo.lock for GPL/AGPL/SSPL crates (flag incompatibility)\n"
        "- Note any crates with no license field"
    )),
    ("16_Improvement_Suggestions", (
        "ROLE: Proactive Improvement Suggestions\n"
        "You are NOT looking for bugs — you are thinking about what would make this crate significantly better.\n"
        "Based on the files provided, suggest concrete improvements in these areas:\n"
        "ARCHITECTURE:\n"
        "- Is the crate doing too much? Should any module be split into its own crate?\n"
        "- Are there abstractions missing that would make extension safer?\n"
        "- Is there a simpler data model that would eliminate whole classes of bugs?\n"
        "API ERGONOMICS:\n"
        "- Which public types or functions are awkward to use correctly?\n"
        "- Where would the builder pattern, typestate, or newtypes prevent misuse?\n"
        "- Which error types would benefit from being more specific?\n"
        "PERFORMANCE:\n"
        "- Where would zero-copy (Bytes, Arc<str>) eliminate the most allocations?\n"
        "- Where would connection pooling or caching reduce latency?\n"
        "- Are there batch operations that should replace N individual calls?\n"
        "OBSERVABILITY:\n"
        "- Which code paths have no tracing spans and would be hard to diagnose in production?\n"
        "- Where would structured fields (tracing::info!(key=val)) help over string messages?\n"
        "STORAGE:\n"
        "- Are there data access patterns that would benefit from indexing in CozoDB/sled?\n"
        "- Is the current storage backend the right choice for the access pattern shown?\n"
        "Output format: numbered list, each item with: Suggestion | Rationale | Example (file:line where change applies)"
    )),
    ("17_DBus_IPC_Attack_Surface", (
        "ROLE: D-Bus & IPC Attack Surface\n"
        "- List every D-Bus interface, method, and signal registered in the files\n"
        "- For each method: note whether caller identity is checked before execution\n"
        "- Flag methods that mutate state or spawn processes with no auth check\n"
        "- Note whether the service connects to system bus or session bus\n"
        "- Flag any method that deserialises caller-supplied bytes without validation\n"
        "- Compare against the system bus policy (if provided) to find over-permissioned allow rules"
    )),
    ("18_Schema_OSCAL_Compliance", (
        "ROLE: Schema-as-Code & OSCAL Compliance\n"
        "This codebase follows a schema-as-code discipline: all data contracts must be expressed "
        "as versioned Protocol Buffer schemas (.proto files) or OSCAL artifacts, not as ad-hoc "
        "Rust structs with manual serialization.\n\n"
        "PROTOCOL BUFFER / SCHEMA-AS-CODE AUDIT:\n"
        "- Find all .proto files referenced or imported (look in build.rs, tonic::include_proto!, file paths)\n"
        "- For each gRPC service or message type defined in Rust: is there a corresponding .proto definition?\n"
        "- Flag any RPC or data struct that is defined only in Rust with no .proto schema source\n"
        "- Flag use of serde_json::Value or simd_json::OwnedValue as a message type "
        "(untyped JSON is a schema-as-code violation)\n"
        "- Flag hand-rolled serialization (impl Serialize/Deserialize with manual field mapping) "
        "where a proto message would be more appropriate\n"
        "- Check for schema evolution safety: are proto fields numbered and backward-compatible?\n"
        "- Note any use of protovalidate / protoc-gen-validate field constraints\n\n"
        "OSCAL COMPLIANCE AUDIT:\n"
        "- Search for OSCAL artifact references: component-definition, system-security-plan (SSP), "
        "assessment-plan, assessment-results, plan-of-action-and-milestones (POA&M)\n"
        "- Flag any security control (authentication, authorization, audit logging, encryption) "
        "that is implemented in code but has no corresponding OSCAL control mapping\n"
        "- Flag D-Bus interfaces, gRPC endpoints, or HTTP routes that are not documented "
        "in an OSCAL component definition\n"
        "- Note any FedRAMP or NIST 800-53 control references in comments or docs\n"
        "- Flag hardcoded policy decisions (IP allowlists, role checks, permission grants) "
        "that should be expressed as machine-readable OSCAL policy artifacts\n\n"
        "Output:\n"
        "1. Schema-as-Code table: Item | Type | file:line | Has .proto? | Gap\n"
        "2. OSCAL Coverage table: Control Area | Implemented at file:line | OSCAL Artifact | Gap\n"
        "3. Recommendations list for each Major/Critical gap"
    )),
    ("19_Risk_Register_Tech_Debt", (
        "ROLE: Risk Register\n"
        "TASK: Synthesise findings from roles 1-18 into a single prioritised table.\n"
        "Columns: Severity (Critical/High/Med/Low) | Issue | Evidence (file:line) | Recommendation\n"
        "Rules:\n"
        "- Critical = exploitable with the code shown, not theoretical\n"
        "- High = likely to cause data loss, crash, or security bypass in production\n"
        "- Include schema-as-code and OSCAL gaps rated High or above\n"
        "- Must cite at least 5 distinct file:line references\n"
        "- Do NOT introduce findings not evidenced in previous roles"
    )),
]

# ---------------------------------------------------------------------------
# File loader
# ---------------------------------------------------------------------------

def get_sorted_crates(crates_dir: str) -> list[tuple[str, int]]:
    counts = []
    for name in os.listdir(crates_dir):
        path = os.path.join(crates_dir, name)
        if os.path.isdir(path):
            rs = glob.glob(os.path.join(path, 'src', '**', '*.rs'), recursive=True)
            counts.append((name, len(rs)))
    return sorted(counts, key=lambda x: x[1], reverse=True)

def load_crate_files(root_path: str, crate_name: str) -> dict[str, str]:
    patterns = [
        os.path.join(root_path, 'crates', crate_name, 'src', '**', '*.rs'),
        os.path.join(root_path, 'crates', crate_name, 'Cargo.toml'),
        os.path.join(root_path, 'Cargo.toml'),
        os.path.join(root_path, 'Cargo.lock'),
        os.path.join(root_path, 'docs', 'network-topology.md'),
        os.path.join(root_path, 'etc', 'dbus-1', 'system.d', '*.conf'),
    ]
    contents = {}
    for pattern in patterns:
        for fp in glob.glob(pattern, recursive=True):
            try:
                with open(fp, encoding='utf-8', errors='replace') as f:
                    contents[os.path.relpath(fp, root_path)] = f.read()
            except Exception as e:
                print(f"  Warning: cannot read {fp}: {e}")
    return contents

# ---------------------------------------------------------------------------
# Worker
# ---------------------------------------------------------------------------

def run_worker_task(role_index: int, role_name: str, worker_prompt: str,
                    crate_name: str, all_file_contents: dict,
                    audit_dir: str, resume: bool) -> tuple[str, str] | None:
    output_filename = f"{role_index:02d}_{role_name}.md"
    output_filepath = os.path.join(audit_dir, output_filename)

    if resume and os.path.exists(output_filepath) and os.path.getsize(output_filepath) > 0:
        print(f"  [skip] {role_name} (--resume)")
        return role_name, output_filepath

    try:
        formatted_prompt = worker_prompt.replace("{CRATE}", crate_name)

        # Prepend semantically relevant chunks from the 97-repo vector store
        rag_block = fetch_rag_context(role_name)
        augmented_prompt = (rag_block + "\n\n" + formatted_prompt) if rag_block else formatted_prompt

        output_md = call_llm_agent(SYSTEM_PROMPT, augmented_prompt, crate_name, all_file_contents)

        # Citation validation — append a warning section if citations are bad
        citations = validate_citations(output_md)
        bad = [c for c in citations if not c["valid"]]
        if bad:
            lines = "\n".join(f"- `{c['citation']}`: {c['reason']}" for c in bad)
            output_md += f"\n\n---\n## ⚠ Citation Warnings\n{lines}\n"

        os.makedirs(audit_dir, exist_ok=True)
        with open(output_filepath, 'w', encoding='utf-8') as f:
            f.write(output_md)

        status = f"({len(bad)} bad citations)" if bad else "✓"
        print(f"  [done] {role_name} {status}")
        return role_name, output_filepath
    except Exception as e:
        print(f"  [fail] {role_name}: {e}")
        return None

# ---------------------------------------------------------------------------
# Post-processing: JSON findings + zip
# ---------------------------------------------------------------------------

def write_json_findings(audit_dir: str, crate_name: str) -> None:
    risk_path = os.path.join(audit_dir, f"{len(WORKER_PROMPTS):02d}_{WORKER_PROMPTS[-1][0]}.md")
    if not os.path.exists(risk_path):
        return
    with open(risk_path, encoding='utf-8') as f:
        risk_md = f.read()
    findings = extract_findings(risk_md, crate_name)
    out = os.path.join(audit_dir, "findings.json")
    with open(out, 'w', encoding='utf-8') as f:
        json.dump(findings, f, indent=2)
    counts = {}
    for fnd in findings:
        counts[fnd["severity"]] = counts.get(fnd["severity"], 0) + 1
    print(f"  findings.json: {counts}")

def zip_audit_directory(audit_dir: str, crate_name: str) -> None:
    zip_path = os.path.join(AUDIT_OUTPUT_DIR, f"{crate_name}-audit.zip")
    if os.path.exists(zip_path):
        os.remove(zip_path)
    with zipfile.ZipFile(zip_path, 'w', zipfile.ZIP_DEFLATED) as z:
        for root, _, files in os.walk(audit_dir):
            for fn in files:
                fp = os.path.join(root, fn)
                z.write(fp, os.path.relpath(fp, audit_dir))
    print(f"  → {zip_path}")

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main() -> None:
    parser = argparse.ArgumentParser(description="Rust crate audit orchestrator")
    parser.add_argument("--crate",  help="Audit a single crate by name")
    parser.add_argument("--resume", action="store_true",
                        help="Skip roles whose output file already exists")
    parser.add_argument("--force",  action="store_true",
                        help="Overwrite all existing outputs (default behaviour)")
    parser.add_argument("--rpm",    type=int, default=150,
                        help="Vertex AI requests per minute limit (default: 150)")
    parser.add_argument("--workers", type=int, default=MAX_WORKERS,
                        help=f"Parallel agents per crate (default: {MAX_WORKERS})")
    args = parser.parse_args()

    resume = args.resume and not args.force
    _rate_init(args.rpm)
    os.makedirs(AUDIT_OUTPUT_DIR, exist_ok=True)

    all_crates = get_sorted_crates(CRATES_DIR)
    if not all_crates:
        print("No crates found.")
        sys.exit(1)

    if args.crate:
        all_crates = [(n, c) for n, c in all_crates if n == args.crate]
        if not all_crates:
            print(f"Crate '{args.crate}' not found.")
            sys.exit(1)

    print(f"Model: {LLM_MODEL}  RPM cap: {args.rpm}  Workers: {args.workers}  Resume: {resume}")
    print(f"Crates: {[n for n, _ in all_crates]}\n")

    overall_findings: list[dict] = []

    for crate_name, rs_count in all_crates:
        if crate_name in SKIP_CRATES:
            print(f"[skip] {crate_name}")
            continue

        print(f"\n{'='*60}")
        print(f"  Crate: {crate_name}  ({rs_count} .rs files)")
        print(f"{'='*60}")

        audit_dir = os.path.join(AUDIT_OUTPUT_DIR, crate_name)
        os.makedirs(audit_dir, exist_ok=True)

        file_contents = load_crate_files(ROOT, crate_name)
        if not file_contents:
            print(f"  No files found — skipping.")
            continue

        total_chars = sum(len(v) for v in file_contents.values())
        print(f"  Loaded {len(file_contents)} files, ~{total_chars // 1000} KB")

        # Role 00: best-practice compliance (runs before parallel roles)
        if VOYAGE_API_KEY:
            run_best_practice_role(crate_name, file_contents, audit_dir, resume)
        else:
            print("  [skip] Best_Practice_Compliance (VOYAGE_API_KEY not set)")

        futures = []
        with ThreadPoolExecutor(max_workers=args.workers) as pool:
            for i, (role_name, prompt) in enumerate(WORKER_PROMPTS):
                futures.append(pool.submit(
                    run_worker_task,
                    i + 1, role_name, prompt,
                    crate_name, file_contents,
                    audit_dir, resume,
                ))
            for fut in as_completed(futures):
                fut.result()  # surface exceptions

        write_json_findings(audit_dir, crate_name)

        # Accumulate into workspace-level findings
        json_path = os.path.join(audit_dir, "findings.json")
        if os.path.exists(json_path):
            with open(json_path) as f:
                overall_findings.extend(json.load(f))

        zip_audit_directory(audit_dir, crate_name)

    # Workspace-level findings roll-up
    if overall_findings:
        rollup_path = os.path.join(AUDIT_OUTPUT_DIR, "workspace_findings.json")
        with open(rollup_path, 'w') as f:
            json.dump(overall_findings, f, indent=2)
        by_sev: dict[str, int] = {}
        for fnd in overall_findings:
            by_sev[fnd["severity"]] = by_sev.get(fnd["severity"], 0) + 1
        print(f"\nWorkspace findings: {by_sev}")
        print(f"Roll-up → {rollup_path}")

    print("\nDone.")

if __name__ == "__main__":
    main()
