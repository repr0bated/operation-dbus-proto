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
