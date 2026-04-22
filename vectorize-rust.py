#!/usr/bin/env python3
import asyncio
import hashlib
import json
import logging
import os
import re
import subprocess
import sys
import time
from dataclasses import dataclass, field
from pathlib import Path

import aiohttp
import pycozo
from qdrant_client import AsyncQdrantClient
from qdrant_client.models import Distance, PointStruct, VectorParams

# =============================================================================
# CONFIGURATION
# =============================================================================

GIT_DIR          = Path("/home/jeremy/git/rust")
QDRANT_URL       = os.environ.get("QDRANT_URL", "http://localhost:6333")
COLLECTION       = "3tched_code"
COZO_DB_PATH     = "3tched_metadata.db"
VECTOR_DIM       = 1024  # voyage-code-3
VOYAGE_MODEL     = "voyage-code-3"
CONCURRENT_REPOS = 2

VOYAGE_API_KEY = os.environ.get("VOYAGE_API_KEY")
if not VOYAGE_API_KEY:
    sys.exit("ERROR: VOYAGE_API_KEY not set")

# Your target Rust repositories
REPOS = [
    ("repr0bated/operation-dbus", "https://github.com/repr0bated/operation-dbus.git"),
    ("repr0bated/zbus-1", "https://github.com/repr0bated/zbus-1.git"),
]

logging.basicConfig(level=logging.INFO, format="%(asctime)s [%(levelname)s] %(message)s")
log = logging.getLogger(__name__)

# =============================================================================
# DB & EMBEDDING CLIENTS
# =============================================================================

class HybridStore:
    def __init__(self):
        self.qdrant = AsyncQdrantClient(url=QDRANT_URL)
        # Using RocksDB engine for persistence
        self.cozo = pycozo.Client("rocksdb", COZO_DB_PATH)
        self._init_schemas()

    def _init_schemas(self):
        # Create Cozo table for metadata
        # We use 'id' as a string (hex hash) to link with Qdrant
        try:
            self.cozo.run("::relations") 
            if "rust_metadata" not in self.cozo.run("::relations")['name'].values:
                self.cozo.run("""
                :create rust_metadata {
                    chunk_id: String
                    =>
                    repo: String,
                    file_path: String,
                    content: String,
                    start_line: Int,
                    end_line: Int
                }
                """)
                log.info("✔ CozoDB schema initialized.")
        except Exception as e:
            log.warning(f"Cozo check failed (might already exist): {e}")

    async def init_qdrant(self):
        try:
            await self.qdrant.get_collection(COLLECTION)
        except:
            await self.qdrant.create_collection(
                COLLECTION,
                vectors_config=VectorParams(size=VECTOR_DIM, distance=Distance.COSINE)
            )
            log.info(f"✔ Qdrant collection '{COLLECTION}' created.")

async def embed_voyage(texts: list[str]) -> list[list[float]]:
    url = "https://api.voyageai.com/v1/embeddings"
    headers = {"Authorization": f"Bearer {VOYAGE_API_KEY}"}
    async with aiohttp.ClientSession() as session:
        async with session.post(url, headers=headers, json={"input": texts, "model": VOYAGE_MODEL}) as r:
            r.raise_for_status()
            data = await r.json()
            return [item["embedding"] for item in data["data"]]

# =============================================================================
# CHUNKING & EXTRACTION
# =============================================================================

def kiro_lsp_chunk(repo_dir: Path, repo_name: str) -> list[dict]:
    """Uses Agentic Kiro to extract semantic Rust blocks."""
    prompt = (
        "Analyze this Rust project using LSP. Chunk into semantic blocks (functions, structs, impls). "
        "Return ONLY a JSON array. Each object must have 'file', 'text', and 'range' (with 'start' line)."
    )
    
    # We use -vvv for internal logging as requested
    res = subprocess.run(
        ["kiro-cli", "-vvv", "chat", "--no-interactive", "--trust-all-tools", prompt],
        cwd=repo_dir, capture_output=True, text=True
    )
    
    if res.returncode != 0:
        log.error(f"Kiro failed for {repo_name}. Stderr: {res.stderr[:200]}")
        return []

    # Robust JSON extraction from agent prose
    match = re.search(r'\[.*\]', res.stdout, re.DOTALL)
    if not match:
        return []
        
    try:
        return json.loads(match.group(0))
    except:
        return []

# =============================================================================
# PIPELINE
# =============================================================================

async def process_rust_repo(repo_name: str, store: HybridStore, sem: asyncio.Semaphore):
    async with sem:
        short_name = repo_name.split("/")[-1]
        path = GIT_DIR / short_name

        if not path.exists() or not (path / "Cargo.toml").exists():
            log.info(f"[skip] {short_name} is not a Rust repository.")
            return

        log.info(f"▶ Processing {short_name}...")
        
        # 1. Chunk via Kiro LSP (Offload blocking call)
        raw_chunks = await asyncio.get_event_loop().run_in_executor(
            None, kiro_lsp_chunk, path, repo_name
        )
        if not raw_chunks: return

        # 2. Embed via Voyage
        texts = [c['text'] for c in raw_chunks]
        embeddings = await embed_voyage(texts)

        # 3. Double Upsert: Qdrant & Cozo
        q_points = []
        c_rows = []
        
        for i, (chunk, vector) in enumerate(zip(raw_chunks, embeddings)):
            # Deterministic ID for idempotency
            cid = hashlib.sha256(f"{repo_name}{chunk['file']}{i}".encode()).hexdigest()
            
            # Qdrant Point (Vector + ID)
            q_points.append(PointStruct(
                id=int(cid[:16], 16), # Qdrant likes 64-bit ints or UUIDs
                vector=vector,
                payload={"cid": cid} # Link back to Cozo
            ))
            
            # Cozo Row (Full Metadata)
            c_rows.append({
                "chunk_id": cid,
                "repo": repo_name,
                "file_path": chunk['file'],
                "content": chunk['text'],
                "start_line": chunk.get('range', {}).get('start', 0),
                "end_line": chunk.get('range', {}).get('end', 0)
            })

        # Atomic-style push
        await store.qdrant.upsert(COLLECTION, q_points)
        store.cozo.insert("rust_metadata", c_rows)
        
        log.info(f"✔ Completed {short_name}: {len(q_points)} chunks.")

async def main():
    store = HybridStore()
    await store.init_qdrant()
    
    sem = asyncio.Semaphore(CONCURRENT_REPOS)
    tasks = [process_rust_repo(name, store, sem) for name, _ in REPOS]
    
    log.info("--- Starting Rust-Only Hybrid Pipeline (Voyage + Qdrant + Cozo) ---")
    await asyncio.gather(*tasks)
    log.info("--- Pipeline Finished ---")

if __name__ == "__main__":
    asyncio.run(main())
