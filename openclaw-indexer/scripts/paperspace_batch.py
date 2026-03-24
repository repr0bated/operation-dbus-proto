#!/usr/bin/env python3
"""
Batch embedding job for Paperspace GPU notebook.

Usage:
  python paperspace_batch.py <input.jsonl> [output.npy] [--batch-size 256]

  1. Upload chunks JSONL to your Paperspace notebook
  2. Run this script on the GPU notebook
  3. Download the output .npy file
  4. Run: openclaw-index import-embeddings --chunks chunks.jsonl --embeddings embeddings.npy
"""

import argparse
import sys
import time

import numpy as np
import orjson


def main():
    parser = argparse.ArgumentParser(description="Batch embed chunks on GPU")
    parser.add_argument("input", nargs="?", default="all_chunks.jsonl", help="Input JSONL file")
    parser.add_argument("output", nargs="?", default="embeddings.npy", help="Output .npy file")
    parser.add_argument("--model", default="Qwen/Qwen3-Embedding-0.6B", help="Model name")
    parser.add_argument("--batch-size", type=int, default=256, help="Batch size")
    parser.add_argument("--dims", type=int, default=768, help="Output embedding dimensions (MRL)")
    args = parser.parse_args()

    print(f"Loading model: {args.model}")
    from sentence_transformers import SentenceTransformer
    model = SentenceTransformer(args.model)

    # Check GPU
    import torch
    device = "cuda" if torch.cuda.is_available() else "cpu"
    print(f"Device: {device}")
    if device == "cuda":
        print(f"GPU: {torch.cuda.get_device_name(0)}")
        props = torch.cuda.get_device_properties(0)
        vram = getattr(props, 'total_memory', getattr(props, 'total_mem', 0)) / 1024**3
        print(f"VRAM: {vram:.1f} GB")

    # Load chunks
    texts = []
    with open(args.input, "rb") as f:
        for line in f:
            record = orjson.loads(line)
            texts.append(record["text"])

    print(f"Loaded {len(texts)} chunks")
    print(f"Batch size: {args.batch_size}, output dims: {args.dims}")

    # Embed
    t0 = time.time()
    embeddings = model.encode(
        texts,
        batch_size=args.batch_size,
        show_progress_bar=True,
        normalize_embeddings=True,
        device=device,
        output_value="sentence_embedding",
    )

    # Truncate to desired dimensions (MRL)
    if embeddings.shape[1] > args.dims:
        embeddings = embeddings[:, :args.dims]
        # Re-normalize after truncation
        norms = np.linalg.norm(embeddings, axis=1, keepdims=True)
        embeddings = embeddings / norms

    elapsed = time.time() - t0
    print(f"Embedded {len(texts)} chunks in {elapsed:.1f}s ({len(texts)/elapsed:.0f} chunks/s)")

    # Save
    np.save(args.output, embeddings)
    print(f"Saved embeddings to {args.output} (shape: {embeddings.shape})")


if __name__ == "__main__":
    main()
