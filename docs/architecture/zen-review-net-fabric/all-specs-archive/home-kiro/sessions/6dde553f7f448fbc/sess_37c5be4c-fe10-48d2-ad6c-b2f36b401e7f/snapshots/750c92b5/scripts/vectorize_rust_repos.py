#!/usr/bin/env python3
"""
Vectorize Rust git repos into Qdrant using voyage-code-4.

Pipeline: git tree walk → tree-sitter parse → semantic chunk → Voyage embed → Qdrant upsert
Also populates Cozo graph with code_symbol / code_edge relations via HTTP.

Usage:
    source .venv-vectorize/bin/activate
    python scripts/vectorize_rust_repos.py [--repo REPO_PATH] [--all] [--dry-run]

Environment:
    VOYAGE_API_KEY      - Voyage AI API key (default: hardcoded for this run)
    QDRANT_URL          - Qdrant HTTP endpoint (default: http://10.200.0.2:6333)
    COZO_URL            - Cozo HTTP endpoint (default: http://10.200.0.2:9070)
"""

import argparse
import hashlib
import json
import os
import re
import sys
import time
import uuid
from pathlib import Path
from typing import Optional

import httpx
import tree_sitter_rust as ts_rust
from tree_sitter import Language, Parser
from pycozo import Client as CozoClient
from qdrant_client import QdrantClient
from qdrant_client.models import (
    Distance,
    PointStruct,
    VectorParams,
)

# ─── Configuration ────────────────────────────────────────────────────────────

VOYAGE_API_KEY = os.environ.get(
    "VOYAGE_API_KEY", "pa-xOCRtSrTFsM4Ft3yeEtV07zgmjCu7sbXeDfV9NJ6kD8"
)
VOYAGE_MODEL = "voyage-code-4"
VOYAGE_FALLBACK_MODEL = "voyage-4-large"  # fallback if code-4 is overloaded
VOYAGE_ENDPOINT = "https://api.voyageai.com/v1/embeddings"
VOYAGE_DIMS = 1024

QDRANT_URL = os.environ.get("QDRANT_URL", "http://10.200.0.2:6333")
COLLECTION = "repos_lsp_rust_voyage_code_4"

COZO_DB_PATH = os.environ.get("COZO_DB_PATH", "/var/lib/op-dbus/code-intelligence-cozo")

REPOS_DIR = Path("/srv/git/repos-bulk/rust")

# Chunking params
CHUNK_LINES = 80
OVERLAP_LINES = 12
MAX_BATCH_TOKENS = 100_000  # voyage-code-4 batch token limit (conservative)
MAX_BATCH_TEXTS = 16  # max texts per embed call (smaller = less likely to timeout)

# Retry config for Voyage (handles "heavy load" responses)
MAX_RETRIES = 5
INITIAL_BACKOFF = 2.0  # seconds
MAX_BACKOFF = 10.0
FALLBACK_AFTER_RETRIES = 2  # switch to fallback model after this many retries

# ─── Tree-sitter setup ────────────────────────────────────────────────────────

RUST_LANGUAGE = Language(ts_rust.language())


def make_parser() -> Parser:
    parser = Parser(RUST_LANGUAGE)
    return parser


# ─── Symbol extraction via tree-sitter ────────────────────────────────────────

# Node types that represent top-level definitions
SYMBOL_NODE_TYPES = {
    "function_item": "function",
    "struct_item": "struct",
    "enum_item": "enum",
    "trait_item": "trait",
    "impl_item": "impl",
    "type_item": "type_alias",
    "const_item": "const",
    "static_item": "static",
    "macro_definition": "macro",
    "mod_item": "module",
}


def extract_symbols(tree, source_bytes: bytes) -> list[dict]:
    """Extract top-level symbols from a tree-sitter parse tree."""
    symbols = []
    root = tree.root_node

    for child in root.children:
        node_type = child.type
        if node_type not in SYMBOL_NODE_TYPES:
            continue

        kind = SYMBOL_NODE_TYPES[node_type]
        name = _extract_name(child, source_bytes)
        if not name:
            continue

        # Extract visibility
        vis = "private"
        for sub in child.children:
            if sub.type == "visibility_modifier":
                vis_text = source_bytes[sub.start_byte:sub.end_byte].decode("utf-8", errors="replace")
                vis = vis_text.strip()
                break

        # Extract doc comments (preceding the node)
        doc = _extract_doc_comment(child, source_bytes)

        # For impl blocks, get the type and trait
        impl_target = None
        impl_trait = None
        if kind == "impl":
            impl_target, impl_trait = _extract_impl_info(child, source_bytes)
            name = impl_target or "impl"

        # Signature: first line of the node
        start_line = child.start_point[0]
        end_line = child.end_point[0]
        first_line = source_bytes[child.start_byte:].split(b"\n")[0].decode("utf-8", errors="replace").strip()

        symbols.append({
            "name": name,
            "kind": kind,
            "visibility": vis,
            "signature": first_line[:200],
            "doc_summary": doc[:300] if doc else "",
            "line_start": start_line + 1,
            "line_end": end_line + 1,
            "impl_trait": impl_trait,
            "impl_target": impl_target,
        })

    return symbols


def _extract_name(node, source_bytes: bytes) -> Optional[str]:
    """Get the name identifier from a definition node."""
    for child in node.children:
        if child.type in ("identifier", "type_identifier"):
            return source_bytes[child.start_byte:child.end_byte].decode("utf-8", errors="replace")
    # For macro_definition, look for the name in the first identifier
    if node.type == "macro_definition":
        for child in node.children:
            if child.type == "identifier":
                return source_bytes[child.start_byte:child.end_byte].decode("utf-8", errors="replace")
    return None


def _extract_doc_comment(node, source_bytes: bytes) -> str:
    """Extract /// or //! doc comments immediately preceding a node."""
    lines = source_bytes[:node.start_byte].decode("utf-8", errors="replace").split("\n")
    doc_lines = []
    for line in reversed(lines[:-1]):  # exclude the line the node starts on
        stripped = line.strip()
        if stripped.startswith("///") or stripped.startswith("//!"):
            doc_lines.insert(0, stripped.lstrip("/!").strip())
        elif stripped.startswith("#[") or stripped == "":
            continue  # skip attributes and blank lines
        else:
            break
    return " ".join(doc_lines)


def _extract_impl_info(node, source_bytes: bytes) -> tuple[Optional[str], Optional[str]]:
    """Extract type and trait from an impl block."""
    target = None
    trait = None
    for child in node.children:
        if child.type == "type_identifier":
            if target is None:
                target = source_bytes[child.start_byte:child.end_byte].decode("utf-8", errors="replace")
        elif child.type == "generic_type":
            # e.g., impl<T> Foo<T>
            for sub in child.children:
                if sub.type == "type_identifier":
                    target = source_bytes[sub.start_byte:sub.end_byte].decode("utf-8", errors="replace")
                    break
    # Check for trait impl: `impl Trait for Type`
    text = source_bytes[node.start_byte:node.end_byte].decode("utf-8", errors="replace")
    m = re.match(r"impl\s+(?:<[^>]*>\s+)?(\w+)\s+for\s+(\w+)", text)
    if m:
        trait = m.group(1)
        target = m.group(2)
    return target, trait


# ─── Chunking ─────────────────────────────────────────────────────────────────

def chunk_file(
    file_path: str,
    content: str,
    repo_name: str,
    symbols: list[dict],
) -> list[dict]:
    """Chunk a file into overlapping segments with metadata."""
    lines = content.split("\n")
    if not lines or (len(lines) == 1 and not lines[0].strip()):
        return []

    is_test = (
        "/tests/" in file_path
        or "/test/" in file_path
        or file_path.endswith("_test.rs")
        or "#[cfg(test)]" in content
        or "#[test]" in content
    )

    # Detect imports
    imports = [l.strip() for l in lines if l.strip().startswith("use ")][:20]

    # Symbol names for this file
    sym_names = [s["name"] for s in symbols]

    chunks = []
    total_chunks = max(1, (len(lines) - OVERLAP_LINES) // (CHUNK_LINES - OVERLAP_LINES) + 1)

    i = 0
    chunk_idx = 0
    while i < len(lines):
        end = min(i + CHUNK_LINES, len(lines))
        chunk_content = "\n".join(lines[i:end])

        if not chunk_content.strip():
            i += CHUNK_LINES - OVERLAP_LINES
            continue

        # Find symbols in this chunk's line range
        chunk_symbols = [
            s for s in symbols
            if s["line_start"] >= (i + 1) and s["line_start"] <= end
        ]
        chunk_sym_names = [s["name"] for s in chunk_symbols]
        chunk_sym_kinds = list(set(s["kind"] for s in chunk_symbols))

        # Build embed text with metadata header for better retrieval
        header = f"// {repo_name}:{file_path} L{i+1}-L{end}\n"
        header += f"// lang: rust | symbols: {', '.join(chunk_sym_names[:5])}\n"
        if chunk_symbols and chunk_symbols[0].get("doc_summary"):
            header += f"// doc: {chunk_symbols[0]['doc_summary'][:100]}\n"

        embed_text = header + chunk_content

        # Deterministic point ID from content
        content_hash = hashlib.sha256(
            f"{repo_name}:{file_path}:{i}:{chunk_content}".encode()
        ).hexdigest()
        point_id = str(uuid.uuid5(uuid.NAMESPACE_URL, f"urn:code:{repo_name}:{file_path}:{content_hash[:16]}"))

        chunks.append({
            "point_id": point_id,
            "embed_text": embed_text,
            "payload": {
                "repo": repo_name,
                "file_path": file_path,
                "language": "rust",
                "symbols": chunk_sym_names,
                "symbol_kind": chunk_sym_kinds,
                "doc_comments": [s["doc_summary"] for s in chunk_symbols if s.get("doc_summary")],
                "imports": imports[:10],
                "is_test": is_test,
                "line_start": i + 1,
                "line_end": end,
                "chunk_index": chunk_idx,
                "total_chunks": total_chunks,
                "content": chunk_content,
                "content_hash": content_hash[:32],
                "visibility": next((s["visibility"] for s in chunk_symbols if s.get("visibility")), "private"),
            },
        })

        chunk_idx += 1
        i += CHUNK_LINES - OVERLAP_LINES

    # Update total_chunks now that we know the real count
    for c in chunks:
        c["payload"]["total_chunks"] = chunk_idx

    return chunks


# ─── Voyage embedding with retry ─────────────────────────────────────────────

def embed_batch(texts: list[str], client: httpx.Client) -> list[list[float]]:
    """Embed a batch of texts via Voyage API with exponential backoff retry.
    Falls back to voyage-4-large if voyage-code-4 is persistently overloaded."""
    model = VOYAGE_MODEL

    backoff = INITIAL_BACKOFF
    for attempt in range(MAX_RETRIES):
        # After FALLBACK_AFTER_RETRIES attempts, switch to fallback model
        if attempt >= FALLBACK_AFTER_RETRIES and model == VOYAGE_MODEL:
            model = VOYAGE_FALLBACK_MODEL
            print(f"  [FALLBACK] Switching to {model} after {attempt} retries")

        payload = {
            "input": texts,
            "model": model,
            "input_type": "document",
            "truncation": True,
            "output_dimension": VOYAGE_DIMS,
        }

        try:
            resp = client.post(
                VOYAGE_ENDPOINT,
                json=payload,
                headers={"Authorization": f"Bearer {VOYAGE_API_KEY}"},
                timeout=30.0,
            )

            if resp.status_code == 200:
                data = resp.json()
                return [d["embedding"] for d in data["data"]]

            body = resp.json() if resp.headers.get("content-type", "").startswith("application/json") else {"detail": resp.text}
            detail = body.get("detail", "")

            if "heavy load" in detail or resp.status_code in (429, 503):
                print(f"  [retry {attempt+1}/{MAX_RETRIES}] {model} overloaded, waiting {backoff:.0f}s...")
                time.sleep(backoff)
                backoff = min(backoff * 1.5, MAX_BACKOFF)
                continue
            elif resp.status_code == 422:
                # Possibly too many tokens — split batch
                if len(texts) > 1:
                    mid = len(texts) // 2
                    left = embed_batch(texts[:mid], client)
                    right = embed_batch(texts[mid:], client)
                    return left + right
                else:
                    print(f"  [ERROR] Single text too long, skipping: {texts[0][:80]}...")
                    return [[0.0] * VOYAGE_DIMS]
            else:
                print(f"  [ERROR] Voyage returned {resp.status_code}: {detail[:200]}")
                return [[0.0] * VOYAGE_DIMS] * len(texts)

        except (httpx.TimeoutException, httpx.ConnectError) as e:
            print(f"  [retry {attempt+1}/{MAX_RETRIES}] Network error: {e}, waiting {backoff:.0f}s...")
            time.sleep(backoff)
            backoff = min(backoff * 1.5, MAX_BACKOFF)

    print(f"  [FATAL] Exhausted {MAX_RETRIES} retries for batch of {len(texts)} texts")
    return [[0.0] * VOYAGE_DIMS] * len(texts)


# ─── Cozo graph population ────────────────────────────────────────────────────

def ensure_cozo_schema(cozo: CozoClient) -> bool:
    """Create Cozo relations for code intelligence if they don't exist."""
    queries = [
        """:create code_symbol {
            repo: String,
            fqn: String,
            kind: String,
            file_path: String,
            line_start: Int,
            line_end: Int,
            visibility: String,
            signature: String,
            doc_summary: String default '',
            content_hash: String default '',
            => indexed_at: Float default 0.0
        }""",
        """:create code_edge {
            from_fqn: String,
            to_fqn: String,
            edge_type: String,
            repo: String,
            file_path: String,
            line: Int,
            => weight: Float default 1.0
        }""",
    ]

    for q in queries:
        try:
            result = cozo.run(q)
            # pycozo returns dict with 'headers'/'rows' on success
        except Exception as e:
            msg = str(e)
            if "already exists" not in msg.lower():
                print(f"  [COZO] Schema issue: {msg[:100]}")
                # Non-fatal — relation might already exist
    return True


def insert_symbols_to_cozo(
    cozo: CozoClient,
    repo: str,
    file_path: str,
    symbols: list[dict],
) -> None:
    """Insert extracted symbols into Cozo code_symbol relation."""
    if not symbols:
        return

    rows = []
    for s in symbols:
        fqn = f"{repo}::{file_path}::{s['name']}"
        rows.append([
            repo, fqn, s["kind"], file_path,
            s["line_start"], s["line_end"],
            s["visibility"], s["signature"],
            s.get("doc_summary", ""), "", time.time()
        ])

    rows_str = json.dumps(rows)
    script = f"""?[repo, fqn, kind, file_path, line_start, line_end, visibility, signature, doc_summary, content_hash, indexed_at] <- {rows_str}
:put code_symbol {{
    repo, fqn, kind, file_path, line_start, line_end, visibility, signature, doc_summary, content_hash, => indexed_at
}}"""

    try:
        cozo.run(script)
    except Exception:
        pass  # Non-fatal


def insert_edges_to_cozo(
    cozo: CozoClient,
    repo: str,
    file_path: str,
    symbols: list[dict],
) -> None:
    """Insert impl/trait edges into Cozo."""
    rows = []
    for s in symbols:
        if s["kind"] == "impl" and s.get("impl_trait") and s.get("impl_target"):
            from_fqn = f"{repo}::{file_path}::{s['impl_target']}"
            to_fqn = s["impl_trait"]
            rows.append([from_fqn, to_fqn, "implements", repo, file_path, s["line_start"], 1.0])

    if not rows:
        return

    rows_str = json.dumps(rows)
    script = f"""?[from_fqn, to_fqn, edge_type, repo, file_path, line, weight] <- {rows_str}
:put code_edge {{
    from_fqn, to_fqn, edge_type, repo, file_path, line, => weight
}}"""

    try:
        cozo.run(script)
    except Exception:
        pass


# ─── Repo walker ──────────────────────────────────────────────────────────────

SKIP_DIRS = {
    ".git", "target", "node_modules", ".cargo", "vendor",
    "benches", "examples", "fuzz", ".github",
}


def walk_rust_files(repo_path: Path) -> list[Path]:
    """Walk a repo and yield .rs files, skipping noise directories."""
    rs_files = []
    for root, dirs, files in os.walk(repo_path):
        # Prune skip dirs
        dirs[:] = [d for d in dirs if d not in SKIP_DIRS]
        for f in files:
            if f.endswith(".rs"):
                rs_files.append(Path(root) / f)
    return rs_files


# ─── Main pipeline ────────────────────────────────────────────────────────────

def index_repo(
    repo_path: Path,
    qdrant: QdrantClient,
    voyage_client: httpx.Client,
    cozo: Optional[CozoClient],
    parser: Parser,
    dry_run: bool = False,
) -> dict:
    """Index a single Rust repo into Qdrant + Cozo."""
    repo_name = repo_path.name
    rs_files = walk_rust_files(repo_path)

    stats = {
        "repo": repo_name,
        "files": len(rs_files),
        "chunks": 0,
        "points_upserted": 0,
        "symbols_extracted": 0,
        "errors": 0,
    }

    print(f"\n{'='*60}")
    print(f"  Indexing: {repo_name} ({len(rs_files)} .rs files)")
    print(f"{'='*60}")

    all_chunks = []

    for rs_file in rs_files:
        try:
            content = rs_file.read_text(encoding="utf-8", errors="replace")
        except Exception as e:
            stats["errors"] += 1
            continue

        # Skip very small files (less than 3 lines)
        if content.count("\n") < 3:
            continue

        # Tree-sitter parse
        source_bytes = content.encode("utf-8")
        tree = parser.parse(source_bytes)

        # Extract symbols
        symbols = extract_symbols(tree, source_bytes)
        stats["symbols_extracted"] += len(symbols)

        # Relative path within repo
        rel_path = str(rs_file.relative_to(repo_path))

        # Insert symbols into Cozo
        if cozo and symbols:
            insert_symbols_to_cozo(cozo, repo_name, rel_path, symbols)
            insert_edges_to_cozo(cozo, repo_name, rel_path, symbols)

        # Chunk the file
        chunks = chunk_file(rel_path, content, repo_name, symbols)
        all_chunks.extend(chunks)

    stats["chunks"] = len(all_chunks)
    print(f"  Parsed {stats['files']} files → {stats['chunks']} chunks, {stats['symbols_extracted']} symbols")

    if dry_run:
        print("  [DRY RUN] Skipping embedding and upsert")
        return stats

    # Embed and upsert in batches
    batch_size = MAX_BATCH_TEXTS
    for batch_start in range(0, len(all_chunks), batch_size):
        batch = all_chunks[batch_start:batch_start + batch_size]
        texts = [c["embed_text"] for c in batch]

        print(f"  Embedding batch {batch_start//batch_size + 1}/{(len(all_chunks) + batch_size - 1)//batch_size} ({len(texts)} texts)...")

        embeddings = embed_batch(texts, voyage_client)

        # Build Qdrant points
        points = []
        for chunk, embedding in zip(batch, embeddings):
            # Skip zero vectors (error sentinels)
            if all(v == 0.0 for v in embedding[:10]):
                stats["errors"] += 1
                continue

            points.append(PointStruct(
                id=chunk["point_id"],
                vector=embedding,
                payload=chunk["payload"],
            ))

        if points:
            try:
                qdrant.upsert(collection_name=COLLECTION, points=points, wait=True)
                stats["points_upserted"] += len(points)
                print(f"    Upserted {len(points)} points")
            except Exception as e:
                print(f"    [ERROR] Qdrant upsert failed: {e}")
                stats["errors"] += 1

        # Rate limit between batches
        time.sleep(0.2)

    print(f"  Done: {stats['points_upserted']} points in Qdrant, {stats['symbols_extracted']} symbols in Cozo")
    return stats


def main():
    parser_arg = argparse.ArgumentParser(description="Vectorize Rust repos into Qdrant with voyage-code-4")
    parser_arg.add_argument("--repo", type=str, help="Index a single repo (directory name under repos-bulk/rust/)")
    parser_arg.add_argument("--all", action="store_true", help="Index all repos in repos-bulk/rust/")
    parser_arg.add_argument("--dry-run", action="store_true", help="Parse and chunk only, no embedding/upsert")
    parser_arg.add_argument("--skip-cozo", action="store_true", help="Skip Cozo graph population")
    args = parser_arg.parse_args()

    if not args.repo and not args.all:
        print("Usage: specify --repo <name> or --all")
        sys.exit(1)

    # Clients
    qdrant = QdrantClient(url=QDRANT_URL, timeout=60)
    voyage_client = httpx.Client(timeout=60.0)

    cozo = None
    if not args.skip_cozo:
        try:
            cozo = CozoClient('rocksdb', path=COZO_DB_PATH)
            ensure_cozo_schema(cozo)
            print(f"[COZO] RocksDB opened at {COZO_DB_PATH}")
        except Exception as e:
            print(f"[COZO] Failed to open: {e} — continuing without graph population")
            cozo = None

    # Tree-sitter parser
    ts_parser = make_parser()

    # Determine repos to index
    if args.repo:
        repos = [REPOS_DIR / args.repo]
        if not repos[0].exists():
            print(f"ERROR: Repo not found: {repos[0]}")
            sys.exit(1)
    else:
        repos = sorted([p for p in REPOS_DIR.iterdir() if p.is_dir()])

    print(f"Model: {VOYAGE_MODEL}")
    print(f"Collection: {COLLECTION}")
    print(f"Qdrant: {QDRANT_URL}")
    print(f"Repos to index: {len(repos)}")
    print(f"Cozo: {'enabled at ' + COZO_DB_PATH if cozo else 'disabled'}")

    # Run
    all_stats = []
    for repo_path in repos:
        stats = index_repo(repo_path, qdrant, voyage_client, cozo, ts_parser, args.dry_run)
        all_stats.append(stats)

    # Summary
    print(f"\n{'='*60}")
    print(f"  SUMMARY")
    print(f"{'='*60}")
    total_files = sum(s["files"] for s in all_stats)
    total_chunks = sum(s["chunks"] for s in all_stats)
    total_points = sum(s["points_upserted"] for s in all_stats)
    total_symbols = sum(s["symbols_extracted"] for s in all_stats)
    total_errors = sum(s["errors"] for s in all_stats)
    print(f"  Repos: {len(all_stats)}")
    print(f"  Files: {total_files}")
    print(f"  Chunks: {total_chunks}")
    print(f"  Points upserted: {total_points}")
    print(f"  Symbols (Cozo): {total_symbols}")
    print(f"  Errors: {total_errors}")


if __name__ == "__main__":
    # Force unbuffered output for real-time progress
    sys.stdout.reconfigure(line_buffering=True)
    sys.stderr.reconfigure(line_buffering=True)
    main()
