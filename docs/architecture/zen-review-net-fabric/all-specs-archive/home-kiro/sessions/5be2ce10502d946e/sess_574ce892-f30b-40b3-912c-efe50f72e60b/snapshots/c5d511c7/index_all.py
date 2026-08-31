#!/usr/bin/env python3
"""
Bulk symbol indexer - extracts symbols from all repos via tree-sitter,
then embeds and stores via lspvec pipeline.

Usage:
    python3 index_all.py [--lang rust] [--repo rust/tokio-rs__tokio]
    python3 index_all.py --all
"""
import argparse
import os
import sys
import time
from pathlib import Path
from typing import Iterator

import tree_sitter as ts

sys.path.insert(0, str(Path(__file__).parent))
from lspvec import (
    CozoStore, QdrantStore, Embedder, Indexer, Symbol, Edge,
)


# ---------------------------------------------------------------------------
# Tree-sitter language setup
# ---------------------------------------------------------------------------

LANGS = {}

def _load_langs():
    global LANGS
    import tree_sitter_rust
    import tree_sitter_python
    import tree_sitter_go
    import tree_sitter_c
    import tree_sitter_typescript
    import tree_sitter_bash

    LANGS = {
        "rust": ts.Language(tree_sitter_rust.language()),
        "python": ts.Language(tree_sitter_python.language()),
        "go": ts.Language(tree_sitter_go.language()),
        "c": ts.Language(tree_sitter_c.language()),
        "cpp": ts.Language(tree_sitter_c.language()),  # close enough for symbols
        "typescript": ts.Language(tree_sitter_typescript.language_typescript()),
        "bash": ts.Language(tree_sitter_bash.language()),
    }

_load_langs()


# ---------------------------------------------------------------------------
# Symbol node types per language
# ---------------------------------------------------------------------------

SYMBOL_NODES = {
    "rust": {
        "function_item": "function",
        "struct_item": "struct",
        "enum_item": "enum",
        "impl_item": "impl",
        "trait_item": "interface",
        "mod_item": "module",
        "type_item": "type",
        "const_item": "constant",
        "static_item": "constant",
        "macro_definition": "function",
    },
    "python": {
        "function_definition": "function",
        "class_definition": "class",
        "decorated_definition": "function",
    },
    "go": {
        "function_declaration": "function",
        "method_declaration": "method",
        "type_declaration": "type",
        "type_spec": "type",
    },
    "c": {
        "function_definition": "function",
        "struct_specifier": "struct",
        "enum_specifier": "enum",
        "type_definition": "type",
    },
    "cpp": {
        "function_definition": "function",
        "struct_specifier": "struct",
        "enum_specifier": "enum",
        "class_specifier": "class",
        "type_definition": "type",
    },
    "typescript": {
        "function_declaration": "function",
        "class_declaration": "class",
        "interface_declaration": "interface",
        "type_alias_declaration": "type",
        "enum_declaration": "enum",
        "method_definition": "method",
    },
    "bash": {
        "function_definition": "function",
    },
}


# ---------------------------------------------------------------------------
# File extensions per language
# ---------------------------------------------------------------------------

EXTENSIONS = {
    "rust": {".rs"},
    "python": {".py"},
    "go": {".go"},
    "c": {".c", ".h"},
    "cpp": {".c", ".h", ".cc", ".cpp", ".hpp", ".cxx"},
    "typescript": {".ts", ".tsx", ".js", ".jsx"},
    "bash": {".sh", ".bash"},
}

# Files to skip
SKIP_DIRS = {
    ".git", "target", "node_modules", "__pycache__", ".tox",
    "vendor", "third_party", "third-party", "build", "dist",
    ".eggs", "venv", ".venv", "pkg", "testdata",
}

MAX_FILE_SIZE = 256 * 1024  # 256KB


# ---------------------------------------------------------------------------
# File walker
# ---------------------------------------------------------------------------

def walk_files(repo_path: Path, lang: str) -> Iterator[Path]:
    """Yield source files for the given language in the repo."""
    exts = EXTENSIONS.get(lang, set())
    for root, dirs, files in os.walk(repo_path):
        # Prune skip dirs in-place
        dirs[:] = [d for d in dirs if d not in SKIP_DIRS]
        for f in files:
            fp = Path(root) / f
            if fp.suffix in exts:
                try:
                    if fp.stat().st_size <= MAX_FILE_SIZE:
                        yield fp
                except OSError:
                    continue


# ---------------------------------------------------------------------------
# Symbol extractor
# ---------------------------------------------------------------------------

def extract_name(node, lang: str) -> str:
    """Extract the name from a symbol node."""
    # Try common field names
    for field in ("name", "declarator"):
        n = node.child_by_field_name(field)
        if n:
            # For C function_definition, declarator may be a function_declarator
            if n.type == "function_declarator":
                inner = n.child_by_field_name("declarator")
                if inner:
                    return inner.text.decode("utf-8", errors="replace")
            return n.text.decode("utf-8", errors="replace")

    # Go type_spec has the name as first named child
    if lang == "go" and node.type == "type_spec":
        for child in node.named_children:
            if child.type == "type_identifier":
                return child.text.decode("utf-8", errors="replace")

    # For impl blocks in Rust, grab the type
    if lang == "rust" and node.type == "impl_item":
        type_node = node.child_by_field_name("type")
        trait_node = node.child_by_field_name("trait")
        if trait_node and type_node:
            return f"{trait_node.text.decode()} for {type_node.text.decode()}"
        elif type_node:
            return type_node.text.decode("utf-8", errors="replace")

    # Python decorated_definition - dig into the inner definition
    if lang == "python" and node.type == "decorated_definition":
        for child in node.named_children:
            if child.type in ("function_definition", "class_definition"):
                n = child.child_by_field_name("name")
                if n:
                    return n.text.decode("utf-8", errors="replace")

    return "<anonymous>"


def extract_signature(node, source: bytes, lang: str) -> str:
    """Extract a short signature (first line or up to opening brace)."""
    text = node.text.decode("utf-8", errors="replace")
    # Take up to first '{' or first 200 chars
    brace = text.find("{")
    if brace > 0:
        sig = text[:brace].strip()
    else:
        sig = text.split("\n")[0]
    # Cap length
    if len(sig) > 300:
        sig = sig[:300] + "..."
    return sig


def extract_doc_comment(node, source: bytes, lang: str) -> str:
    """Extract doc comment above a symbol node."""
    # Look at preceding siblings for comments
    comments = []
    prev = node.prev_named_sibling
    while prev and prev.type in ("line_comment", "comment", "block_comment",
                                   "doc_comment", "string", "expression_statement"):
        text = prev.text.decode("utf-8", errors="replace").strip()
        # Rust doc comments: /// or //!
        if text.startswith("///") or text.startswith("//!"):
            comments.insert(0, text.lstrip("/!").strip())
        # Python docstrings - check if it's inside the function
        elif lang == "python" and prev.type == "expression_statement":
            inner = prev.named_children[0] if prev.named_children else None
            if inner and inner.type == "string":
                comments.insert(0, inner.text.decode().strip("\"'").strip())
        # C/Go block comments
        elif text.startswith("/*"):
            cleaned = text.strip("/*").strip()
            comments.insert(0, cleaned)
        elif text.startswith("//"):
            comments.insert(0, text.lstrip("/").strip())
        else:
            break
        prev = prev.prev_named_sibling

    # For Python, also check first child (docstring)
    if lang == "python" and not comments:
        body = node.child_by_field_name("body")
        if body and body.named_children:
            first = body.named_children[0]
            if first.type == "expression_statement":
                inner = first.named_children[0] if first.named_children else None
                if inner and inner.type == "string":
                    doc = inner.text.decode().strip("\"'").strip()
                    if len(doc) < 500:
                        return doc

    doc = "\n".join(comments)
    return doc[:500] if doc else ""


def extract_symbols_from_file(
    file_path: Path, repo_path: Path, repo_name: str, lang: str
) -> list[Symbol]:
    """Parse a file and extract all top-level symbols."""
    ts_lang = LANGS.get(lang)
    if not ts_lang:
        return []

    node_types = SYMBOL_NODES.get(lang, {})
    if not node_types:
        return []

    try:
        source = file_path.read_bytes()
    except (OSError, PermissionError):
        return []

    parser = ts.Parser(ts_lang)
    tree = parser.parse(source)
    root = tree.root_node

    rel_path = str(file_path.relative_to(repo_path))
    symbols = []

    def walk(node, depth=0):
        if depth > 5:  # don't go too deep
            return
        if node.type in node_types:
            kind = node_types[node.type]
            name = extract_name(node, lang)
            if name == "<anonymous>" and kind not in ("impl",):
                # Skip unnamed nodes unless they're impl blocks
                pass
            else:
                sig = extract_signature(node, source, lang)
                doc = extract_doc_comment(node, source, lang)
                body_text = node.text.decode("utf-8", errors="replace")
                # Cap body at 2000 chars for embedding
                if len(body_text) > 2000:
                    body_text = body_text[:2000] + "\n// ... truncated"

                sym = Symbol(
                    id=f"{repo_name}::{rel_path}::{name}::{node.start_point[0]+1}",
                    repo=repo_name,
                    path=rel_path,
                    name=name,
                    kind=kind,
                    lang=lang,
                    start_line=node.start_point[0] + 1,
                    end_line=node.end_point[0] + 1,
                    signature=sig,
                    doc=doc,
                    body=body_text,
                )
                symbols.append(sym)

            # For impl blocks, also extract methods inside
            if lang == "rust" and node.type == "impl_item":
                body = None
                for child in node.named_children:
                    if child.type == "declaration_list":
                        body = child
                        break
                if body:
                    for child in body.named_children:
                        if child.type == "function_item":
                            walk(child, depth + 1)
                return  # don't recurse further into impl

        for child in node.named_children:
            walk(child, depth + 1)

    walk(root)
    return symbols


# ---------------------------------------------------------------------------
# Manifest parser
# ---------------------------------------------------------------------------

REPOS_ROOT = Path("/srv/git/repos-bulk")
MANIFEST = REPOS_ROOT / ".venv" / "repos.manifest"


def load_manifest(filter_lang=None, filter_repo=None) -> list[tuple[str, str]]:
    """Load repos.manifest, return [(path, lang), ...]"""
    entries = []
    with open(MANIFEST) as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            parts = line.split("|")
            if len(parts) < 2:
                continue
            path, lang = parts[0], parts[1]
            if filter_lang and lang != filter_lang:
                continue
            if filter_repo and path != filter_repo:
                continue
            entries.append((path, lang))
    return entries


# ---------------------------------------------------------------------------
# Main indexing loop
# ---------------------------------------------------------------------------

VOYAGE_KEY = "pa-Q59qlNy_jn_-8b8ABlB4u02VEwxL97h3_zZ5HT-rVxI"
QDRANT_SOCK = "/run/ghostbridge/fwd-qdrant.sock"
COZO_PATH = str(REPOS_ROOT / ".venv" / "lspvec.cozo")

# Collection naming: repos_lsp_{lang}_voyage_code_3_5
def collection_for_lang(lang: str) -> str:
    return f"repos_lsp_{lang}_voyage_code_3_5"


def index_repo(repo_path_str: str, lang: str, indexer: Indexer, batch_size: int = 64):
    """Index all symbols in a single repo."""
    repo_path = REPOS_ROOT / repo_path_str
    if not repo_path.is_dir():
        print(f"  SKIP {repo_path_str}: not a directory")
        return 0

    repo_name = repo_path_str.replace("/", "__")
    files = list(walk_files(repo_path, lang))
    total_syms = 0
    batch: list[Symbol] = []

    for fp in files:
        syms = extract_symbols_from_file(fp, repo_path, repo_name, lang)
        batch.extend(syms)

        if len(batch) >= batch_size:
            indexer.index_symbols(batch)
            total_syms += len(batch)
            batch = []

    # Flush
    if batch:
        indexer.index_symbols(batch)
        total_syms += len(batch)

    return total_syms


def extract_repo_chunks(repo_path_str: str, lang: str) -> list:
    """Extract all symbols from a repo and return as Chunk objects (no embedding)."""
    from lspvec import symbol_to_chunk, Chunk
    repo_path = REPOS_ROOT / repo_path_str
    if not repo_path.is_dir():
        return []

    repo_name = repo_path_str.replace("/", "__")
    files = list(walk_files(repo_path, lang))
    all_chunks = []

    for fp in files:
        syms = extract_symbols_from_file(fp, repo_path, repo_name, lang)
        for sym in syms:
            chunk = symbol_to_chunk(sym)
            all_chunks.append(chunk)

    return all_chunks


def main():
    parser = argparse.ArgumentParser(description="Bulk index repos into lspvec")
    parser.add_argument("--lang", help="Only index repos of this language")
    parser.add_argument("--repo", help="Only index this specific repo path")
    parser.add_argument("--all", action="store_true", help="Index all repos")
    parser.add_argument("--batch-size", type=int, default=64,
                        help="Symbols per embedding batch")
    parser.add_argument("--cozo", default=COZO_PATH, help="Cozo store path")
    parser.add_argument("--collection", help="Override qdrant collection name")
    parser.add_argument("--no-incremental", action="store_true")
    args = parser.parse_args()

    if not args.all and not args.lang and not args.repo:
        parser.print_help()
        sys.exit(1)

    entries = load_manifest(filter_lang=args.lang, filter_repo=args.repo)
    if not entries:
        print("No repos matched filters.")
        sys.exit(1)

    print(f"Repos to index: {len(entries)}")

    # Group by language for per-language collections
    by_lang: dict[str, list[str]] = {}
    for path, lang in entries:
        by_lang.setdefault(lang, []).append(path)

    cozo = CozoStore(args.cozo)
    embedder = Embedder(VOYAGE_KEY)

    total_symbols = 0
    total_repos = 0
    t0 = time.time()

    for lang, repos in sorted(by_lang.items()):
        if lang in ("docs", "json-spec", "protobuf"):
            # Skip non-code langs for now (no tree-sitter grammar or special handling needed)
            print(f"\n=== SKIP {lang} ({len(repos)} repos) - no tree-sitter support ===")
            continue

        coll = args.collection or collection_for_lang(lang)
        qdrant = QdrantStore(QDRANT_SOCK, collection=coll)
        indexer = Indexer(cozo, qdrant, embedder, incremental=not args.no_incremental)

        print(f"\n=== {lang.upper()} ({len(repos)} repos) -> {coll} ===")

        for repo_path in repos:
            print(f"  {repo_path}...", end=" ", flush=True)
            n = index_repo(repo_path, lang, indexer, batch_size=args.batch_size)
            print(f"{n} symbols")
            total_symbols += n
            total_repos += 1

    elapsed = time.time() - t0
    print(f"\n{'='*60}")
    print(f"Done: {total_repos} repos, {total_symbols} symbols")
    print(f"Voyage: {embedder.total_calls} API calls, {embedder.total_tokens} tokens")
    print(f"Time: {elapsed:.1f}s")


if __name__ == "__main__":
    main()
