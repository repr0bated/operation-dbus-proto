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
