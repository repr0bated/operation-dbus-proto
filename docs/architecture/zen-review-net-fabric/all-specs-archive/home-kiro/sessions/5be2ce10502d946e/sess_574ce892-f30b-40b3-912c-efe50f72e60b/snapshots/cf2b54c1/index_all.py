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
