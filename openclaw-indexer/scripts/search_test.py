#!/usr/bin/env python3
"""Quick search test against the indexed Qdrant collection."""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent))

from indexer.config import load_config
from indexer.embedder import create_embedder
from indexer.qdrant_store import QdrantStore


def main():
    query = " ".join(sys.argv[1:]) if len(sys.argv) > 1 else "D-Bus indexing FTS5"

    config = load_config()
    embedder = create_embedder(config)
    store = QdrantStore(config)

    print(f"Query: {query}")
    print(f"Embedding with {config.embedding_backend}...")

    vec = embedder.embed_single(query)
    print(f"Vector: {len(vec)}d")

    results = store.search(vec, limit=5)

    if not results:
        print("No results found. Is the index populated?")
        return

    for i, r in enumerate(results, 1):
        print(f"\n--- #{i} (score: {r.get('score', 0):.4f}) ---")
        print(f"[{r.get('chunk_type')}] {r.get('name')}")
        print(f"{r.get('repo')}:{r.get('file_path')}:{r.get('line_start')}-{r.get('line_end')}")
        content = r.get("content", "")
        print(content[:300])


if __name__ == "__main__":
    main()
