#!/usr/bin/env python3
"""CLI entry point for openclaw-indexer."""

import sys
from pathlib import Path

# Add parent dir to path so indexer package is importable
sys.path.insert(0, str(Path(__file__).parent.parent))

from indexer.job import cli

if __name__ == "__main__":
    cli()
