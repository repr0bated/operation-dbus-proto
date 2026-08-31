#!/usr/bin/env python3
"""
lspvec - Bulk code indexer for repos-bulk.

Architecture: This script is meant to be driven by a Kiro agent session that
acts as the LSP/chunker. The agent uses its code intelligence tools to extract
symbols, types, hover docs, and references, then this script handles:
  - Embedding via Voyage AI (voyage-4-code)
  - Writing symbols + edges to Cozo (embedded RocksDB or HTTP client)
  - Writing vectors to Qdrant (HTTP)
  - Incremental mode: hash-gated to skip unchanged chunks

The agent calls index_chunk() / index_batch() for each symbol it extracts.
"""

import argparse
import hashlib
import json
import os
import sys
import time
from dataclasses import dataclass, field, asdict
from pathlib import Path
from typing import Optional

import httpx
import voyageai
from pycozo import Client as CozoClient
from qdrant_client import QdrantClient
from qdrant_client.models import (
    Distance,
    VectorParams,
    PointStruct,
    PayloadSchemaType,
    Filter,
    FieldCondition,
    MatchValue,
)


# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------

VOYAGE_MODEL = "voyage-4-code"
VOYAGE_DIM = 1024  # voyage-4-code output dimension
QDRANT_COLLECTION = "lspvec"
COZO_RELATIONS = {
    "symbols": """:create symbols {
        id: String =>
        repo: String,
        path: String,
        name: String,
        kind: String,
        lang: String,
        start_line: Int,
        end_line: Int,
        signature: String default "",
        doc: String default "",
        body_hash: String default "",
        indexed_at: Float default 0.0
    }""",
    "edges": """:create edges {
        src: String, dst: String, kind: String =>
        repo: String,
        weight: Float default 1.0
    }""",
    "chunks": """:create chunks {
        id: String =>
        repo: String,
        path: String,
        name: String,
        text_hash: String,
        embedded: Bool default false
    }""",
}


# ---------------------------------------------------------------------------
# Data types
# ---------------------------------------------------------------------------

@dataclass
class Symbol:
    id: str
    repo: str
    path: str
    name: str
    kind: str  # function, class, method, interface, type, enum, module, constant
    lang: str
    start_line: int
    end_line: int
    signature: str = ""
    doc: str = ""
    body: str = ""  # source text for embedding, not stored in cozo
    body_hash: str = ""

    def __post_init__(self):
        if not self.body_hash and self.body:
            self.body_hash = hashlib.sha256(self.body.encode()).hexdigest()[:16]
        if not self.id:
            self.id = f"{self.repo}::{self.path}::{self.name}::{self.start_line}"


@dataclass
class Edge:
    src: str
    dst: str
    kind: str  # calls, references, implements, imports, contains
    repo: str
    weight: float = 1.0


@dataclass
class Chunk:
    """A text chunk ready for embedding."""
    id: str
    repo: str
    path: str
    name: str
    text: str
    text_hash: str = ""
    symbol_kind: str = ""
    lang: str = ""
    start_line: int = 0
    end_line: int = 0
    signature: str = ""
    doc: str = ""

    def __post_init__(self):
        if not self.text_hash:
            self.text_hash = hashlib.sha256(self.text.encode()).hexdigest()[:16]


# ---------------------------------------------------------------------------
# Cozo store
# ---------------------------------------------------------------------------

class CozoStore:
    def __init__(self, path: str, auth: Optional[str] = None):
        self.path = path
        self.db = self._open(path, auth)
        self._ensure_relations()

    def _open(self, path: str, auth: Optional[str] = None) -> CozoClient:
        if path == ":memory:":
            db = CozoClient("mem", dataframe=False)
        elif path.startswith("http://") or path.startswith("https://"):
            opts = {"host": path}
            if auth:
                opts["auth"] = auth
            db = CozoClient("http", options=opts, dataframe=False)
        elif path.endswith(".sqlite") or path.endswith(".db"):
            db = CozoClient("sqlite", path=path, dataframe=False)
        else:
            # RocksDB directory
            try:
                db = CozoClient("rocksdb", path=path, dataframe=False)
            except Exception as e:
                msg = str(e)
                if "lock" in msg.lower() or "no locks" in msg.lower():
                    print(
                        f"ERROR: RocksDB at '{path}' is locked by another process.\n"
                        f"  If you want concurrent access, run it as a server:\n"
                        f"  cozo server --engine rocksdb --path {path} --bind 127.0.0.1 --port 9070\n"
                        f"  Then use: --cozo http://127.0.0.1:9070",
                        file=sys.stderr,
                    )
                raise
        # Probe
        try:
            db.run("?[x] <- [[1]]")
        except Exception as e:
            raise RuntimeError(f"Cozo probe failed on '{path}': {e}") from e
        return db

    def _ensure_relations(self):
        for name, ddl in COZO_RELATIONS.items():
            try:
                self.db.run(f"?[x] := *{name}{{id: x}}, x = '__probe__'")
            except Exception:
                # Relation doesn't exist, create it
                self.db.run(ddl)

    def known_hashes(self, repo: str) -> dict[str, str]:
        """Return {chunk_id: text_hash} for all chunks in a repo."""
        try:
            res = self.db.run(
                "?[id, text_hash] := *chunks{id, repo, text_hash}, repo = $repo",
                {"repo": repo},
            )
            return {row[0]: row[1] for row in res["rows"]}
        except Exception:
            return {}

    def put_symbols(self, symbols: list[Symbol]):
        if not symbols:
            return
        rows = []
        for s in symbols:
            rows.append([
                s.id, s.repo, s.path, s.name, s.kind, s.lang,
                s.start_line, s.end_line, s.signature, s.doc,
                s.body_hash, time.time(),
            ])
        data_literal = ", ".join(
            json.dumps(r) for r in rows
        )
        self.db.run(f"""
            ?[id, repo, path, name, kind, lang, start_line, end_line, signature, doc, body_hash, indexed_at] <-
                [{data_literal}]
            :put symbols {{
                id =>
                repo, path, name, kind, lang, start_line, end_line,
                signature, doc, body_hash, indexed_at
            }}
        """)

    def put_edges(self, edges: list[Edge]):
        if not edges:
            return
        rows = []
        for e in edges:
            rows.append([e.src, e.dst, e.kind, e.repo, e.weight])
        data_literal = ", ".join(json.dumps(r) for r in rows)
        self.db.run(f"""
            ?[src, dst, kind, repo, weight] <- [{data_literal}]
            :put edges {{ src, dst, kind => repo, weight }}
        """)

    def put_chunks(self, chunks: list[Chunk], embedded: bool = False):
        if not chunks:
            return
        rows = []
        for c in chunks:
            rows.append([c.id, c.repo, c.path, c.name, c.text_hash, embedded])
        data_literal = ", ".join(json.dumps(r) for r in rows)
        self.db.run(f"""
            ?[id, repo, path, name, text_hash, embedded] <- [{data_literal}]
            :put chunks {{ id => repo, path, name, text_hash, embedded }}
        """)

    def mark_embedded(self, chunk_ids: list[str]):
        if not chunk_ids:
            return
        ids_literal = ", ".join(json.dumps([cid]) for cid in chunk_ids)
        self.db.run(f"""
            ?[id] <- [{ids_literal}]
            :update chunks {{ id => embedded = true }}
        """)

    def stats(self, repo: Optional[str] = None) -> dict:
        if repo:
            syms = self.db.run(
                "?[count(id)] := *symbols{id, repo}, repo = $repo",
                {"repo": repo},
            )
            edgs = self.db.run(
                "?[count(src)] := *edges{src, repo}, repo = $repo",
                {"repo": repo},
            )
            chnks = self.db.run(
                "?[count(id)] := *chunks{id, repo}, repo = $repo",
                {"repo": repo},
            )
        else:
            syms = self.db.run("?[count(id)] := *symbols{id}")
            edgs = self.db.run("?[count(src)] := *edges{src}")
            chnks = self.db.run("?[count(id)] := *chunks{id}")
        return {
            "symbols": syms["rows"][0][0] if syms["rows"] else 0,
            "edges": edgs["rows"][0][0] if edgs["rows"] else 0,
            "chunks": chnks["rows"][0][0] if chnks["rows"] else 0,
        }

    def fan_in(self, repo: Optional[str] = None, limit: int = 20) -> list:
        """Top symbols by incoming edge count."""
        if repo:
            res = self.db.run(f"""
                ?[dst, count(src)] := *edges{{src, dst, repo}}, repo = $repo
                :order -count(src)
                :limit {limit}
            """, {"repo": repo})
        else:
            res = self.db.run(f"""
                ?[dst, count(src)] := *edges{{src, dst}}
                :order -count(src)
                :limit {limit}
            """)
        return res["rows"]


# ---------------------------------------------------------------------------
# Qdrant store
# ---------------------------------------------------------------------------

class QdrantStore:
    def __init__(self, url: str = "http://127.0.0.1:6333", api_key: Optional[str] = None):
        self.client = QdrantClient(url=url, api_key=api_key, timeout=60)
        self._ensure_collection()

    def _ensure_collection(self):
        collections = [c.name for c in self.client.get_collections().collections]
        if QDRANT_COLLECTION not in collections:
            self.client.create_collection(
                collection_name=QDRANT_COLLECTION,
                vectors_config=VectorParams(
                    size=VOYAGE_DIM,
                    distance=Distance.COSINE,
                    on_disk=True,
                ),
            )
            # Payload indexes for filtering
            for field_name, schema_type in [
                ("repo", PayloadSchemaType.KEYWORD),
                ("path", PayloadSchemaType.KEYWORD),
                ("kind", PayloadSchemaType.KEYWORD),
                ("lang", PayloadSchemaType.KEYWORD),
                ("name", PayloadSchemaType.TEXT),
            ]:
                self.client.create_payload_index(
                    collection_name=QDRANT_COLLECTION,
                    field_name=field_name,
                    field_schema=schema_type,
                )
            print(f"Created Qdrant collection '{QDRANT_COLLECTION}' (dim={VOYAGE_DIM})")

    def upsert(self, chunks: list[Chunk], vectors: list[list[float]]):
        if not chunks or not vectors:
            return
        points = []
        for chunk, vec in zip(chunks, vectors):
            payload = {
                "repo": chunk.repo,
                "path": chunk.path,
                "name": chunk.name,
                "kind": chunk.symbol_kind,
                "lang": chunk.lang,
                "start_line": chunk.start_line,
                "end_line": chunk.end_line,
                "signature": chunk.signature,
                "doc": chunk.doc,
                "text": chunk.text,
                "text_hash": chunk.text_hash,
            }
            points.append(PointStruct(
                id=hashlib.md5(chunk.id.encode()).hexdigest(),
                vector=vec,
                payload=payload,
            ))
        # Batch in groups of 100
        batch_size = 100
        for i in range(0, len(points), batch_size):
            self.client.upsert(
                collection_name=QDRANT_COLLECTION,
                points=points[i : i + batch_size],
            )

    def count(self, repo: Optional[str] = None) -> int:
        if repo:
            result = self.client.count(
                collection_name=QDRANT_COLLECTION,
                count_filter=Filter(
                    must=[FieldCondition(key="repo", match=MatchValue(value=repo))]
                ),
            )
        else:
            result = self.client.count(collection_name=QDRANT_COLLECTION)
        return result.count

    def search(self, vector: list[float], limit: int = 10, repo: Optional[str] = None):
        filt = None
        if repo:
            filt = Filter(
                must=[FieldCondition(key="repo", match=MatchValue(value=repo))]
            )
        return self.client.query_points(
            collection_name=QDRANT_COLLECTION,
            query=vector,
            limit=limit,
            query_filter=filt,
            with_payload=True,
        )


# ---------------------------------------------------------------------------
# Voyage embedder
# ---------------------------------------------------------------------------

class Embedder:
    def __init__(self, api_key: str, model: str = VOYAGE_MODEL):
        self.client = voyageai.Client(api_key=api_key)
        self.model = model
        self.total_tokens = 0
        self.total_calls = 0

    def embed(self, texts: list[str], input_type: str = "document") -> list[list[float]]:
        """Embed a batch of texts. Max 128 texts or ~120k tokens per call."""
        if not texts:
            return []
        # Voyage API limit: 128 texts per batch
        batch_size = 128
        all_vectors = []
        for i in range(0, len(texts), batch_size):
            batch = texts[i : i + batch_size]
            result = self.client.embed(
                batch,
                model=self.model,
                input_type=input_type,
            )
            all_vectors.extend(result.embeddings)
            self.total_tokens += result.total_tokens
            self.total_calls += 1
        return all_vectors

    def embed_query(self, text: str) -> list[float]:
        result = self.client.embed([text], model=self.model, input_type="query")
        self.total_tokens += result.total_tokens
        self.total_calls += 1
        return result.embeddings[0]


# ---------------------------------------------------------------------------
# Chunk builder - formats symbol data into embeddable text
# ---------------------------------------------------------------------------

def build_chunk_text(symbol: Symbol) -> str:
    """Build the text representation of a symbol for embedding."""
    parts = []
    # Header: kind, qualified name, language
    parts.append(f"[{symbol.kind}] {symbol.name} ({symbol.lang})")
    parts.append(f"// {symbol.repo} :: {symbol.path}:{symbol.start_line}-{symbol.end_line}")

    if symbol.signature:
        parts.append(f"signature: {symbol.signature}")

    if symbol.doc:
        parts.append(f"doc: {symbol.doc}")

    if symbol.body:
        parts.append(symbol.body)

    return "\n".join(parts)


def symbol_to_chunk(symbol: Symbol) -> Chunk:
    """Convert a Symbol into an embeddable Chunk."""
    text = build_chunk_text(symbol)
    return Chunk(
        id=symbol.id,
        repo=symbol.repo,
        path=symbol.path,
        name=symbol.name,
        text=text,
        symbol_kind=symbol.kind,
        lang=symbol.lang,
        start_line=symbol.start_line,
        end_line=symbol.end_line,
        signature=symbol.signature,
        doc=symbol.doc,
    )


# ---------------------------------------------------------------------------
# Indexer - orchestrates the pipeline
# ---------------------------------------------------------------------------

class Indexer:
    def __init__(
        self,
        cozo: CozoStore,
        qdrant: QdrantStore,
        embedder: Embedder,
        incremental: bool = True,
    ):
        self.cozo = cozo
        self.qdrant = qdrant
        self.embedder = embedder
        self.incremental = incremental
        self._known_hashes: dict[str, dict[str, str]] = {}

    def _get_known_hashes(self, repo: str) -> dict[str, str]:
        if repo not in self._known_hashes:
            self._known_hashes[repo] = self.cozo.known_hashes(repo)
        return self._known_hashes[repo]

    def index_symbols(self, symbols: list[Symbol], edges: list[Edge] | None = None):
        """Index a batch of symbols: store in Cozo, embed new ones, upsert to Qdrant."""
        if not symbols:
            return

        repo = symbols[0].repo

        # 1. Store symbols in Cozo (always - graph is rewritten each run)
        self.cozo.put_symbols(symbols)

        # 2. Store edges
        if edges:
            self.cozo.put_edges(edges)

        # 3. Build chunks and filter by hash for incremental
        chunks = [symbol_to_chunk(s) for s in symbols]
        known = self._get_known_hashes(repo) if self.incremental else {}

        new_chunks = []
        for chunk in chunks:
            if self.incremental and chunk.id in known:
                if known[chunk.id] == chunk.text_hash:
                    continue  # unchanged, skip embedding
            new_chunks.append(chunk)

        # 4. Record all chunks in Cozo (with current hash)
        self.cozo.put_chunks(chunks)

        if not new_chunks:
            return

        # 5. Embed new/changed chunks
        texts = [c.text for c in new_chunks]
        vectors = self.embedder.embed(texts)

        # 6. Upsert to Qdrant
        self.qdrant.upsert(new_chunks, vectors)

        # 7. Mark as embedded
        self.cozo.mark_embedded([c.id for c in new_chunks])

        print(
            f"  [{repo}] indexed {len(new_chunks)} new chunks "
            f"(skipped {len(chunks) - len(new_chunks)} unchanged, "
            f"{self.embedder.total_tokens} total tokens)"
        )

    def stats(self, repo: Optional[str] = None):
        cozo_stats = self.cozo.stats(repo)
        qdrant_count = self.qdrant.count(repo)
        return {**cozo_stats, "qdrant_vectors": qdrant_count}


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------

def cmd_stats(args):
    cozo = CozoStore(args.cozo, auth=args.cozo_auth)
    print(json.dumps(cozo.stats(args.repo), indent=2))


def cmd_fan_in(args):
    cozo = CozoStore(args.cozo, auth=args.cozo_auth)
    rows = cozo.fan_in(args.repo, limit=args.limit)
    for sym_id, count in rows:
        print(f"  {count:4d}  {sym_id}")


def cmd_query(args):
    api_key = args.voyage_key or os.environ.get("VOYAGE_API_KEY", "")
    if not api_key:
        print("ERROR: --voyage-key or VOYAGE_API_KEY required", file=sys.stderr)
        sys.exit(1)

    embedder = Embedder(api_key)
    qdrant = QdrantStore(args.qdrant, api_key=args.qdrant_key)

    vec = embedder.embed_query(args.query_text)
    results = qdrant.search(vec, limit=args.limit, repo=args.repo)

    for point in results.points:
        p = point.payload
        score = point.score
        print(f"  {score:.4f}  [{p.get('kind','')}] {p.get('name','')}  "
              f"({p.get('repo','')} :: {p.get('path','')}:{p.get('start_line','')})")
        if args.expand:
            sig = p.get("signature", "")
            if sig:
                print(f"           sig: {sig}")
            doc = p.get("doc", "")
            if doc:
                print(f"           doc: {doc[:200]}")
            print()


def cmd_init(args):
    """Just initialize stores (create collections/relations) without indexing."""
    api_key = args.voyage_key or os.environ.get("VOYAGE_API_KEY", "")
    cozo = CozoStore(args.cozo, auth=args.cozo_auth)
    qdrant = QdrantStore(args.qdrant, api_key=args.qdrant_key)
    print(f"Cozo: {args.cozo}")
    print(f"Qdrant: {args.qdrant} (collection: {QDRANT_COLLECTION})")
    print("Stores initialized.")


def main():
    parser = argparse.ArgumentParser(
        description="lspvec - Code symbol indexer with LSP extraction, Voyage embeddings, Qdrant + Cozo storage"
    )
    parser.add_argument("--cozo", default="./lspvec.cozo",
                        help="Cozo path: rocksdb dir, .sqlite file, :memory:, or http(s) URL")
    parser.add_argument("--cozo-auth", default=os.environ.get("COZO_AUTH"),
                        help="Auth token for Cozo HTTP mode")
    parser.add_argument("--qdrant", default=os.environ.get("QDRANT_URL", "http://127.0.0.1:6333"),
                        help="Qdrant URL")
    parser.add_argument("--qdrant-key", default=os.environ.get("QDRANT_API_KEY"),
                        help="Qdrant API key")
    parser.add_argument("--voyage-key", default=os.environ.get("VOYAGE_API_KEY"),
                        help="Voyage AI API key")

    sub = parser.add_subparsers(dest="command")

    # init
    p_init = sub.add_parser("init", help="Initialize stores without indexing")

    # stats
    p_stats = sub.add_parser("stats", help="Show index statistics")
    p_stats.add_argument("--repo", help="Filter by repo name")

    # fan-in
    p_fan = sub.add_parser("fan-in", help="Show top symbols by incoming references")
    p_fan.add_argument("--repo", help="Filter by repo name")
    p_fan.add_argument("--limit", type=int, default=20)

    # query
    p_query = sub.add_parser("query", help="Semantic search across indexed symbols")
    p_query.add_argument("query_text", help="Search query")
    p_query.add_argument("--repo", help="Filter by repo name")
    p_query.add_argument("--limit", type=int, default=10)
    p_query.add_argument("--expand", action="store_true", help="Show signatures and docs")

    # index-batch (reads JSON lines from stdin - for agent piping)
    p_batch = sub.add_parser("index-batch",
                             help="Index symbols from JSON lines on stdin (agent mode)")
    p_batch.add_argument("--repo", required=True, help="Repository name")
    p_batch.add_argument("--no-incremental", action="store_true",
                         help="Re-embed everything regardless of hash")

    parser.set_defaults(command=None)
    args = parser.parse_args()

    if args.command is None:
        parser.print_help()
        sys.exit(0)
    elif args.command == "init":
        cmd_init(args)
    elif args.command == "stats":
        cmd_stats(args)
    elif args.command == "fan-in":
        cmd_fan_in(args)
    elif args.command == "query":
        cmd_query(args)
    elif args.command == "index-batch":
        cmd_index_batch(args)


def cmd_index_batch(args):
    """Read JSON lines from stdin, each being a symbol dict. Embed and store."""
    api_key = args.voyage_key or os.environ.get("VOYAGE_API_KEY", "")
    if not api_key:
        print("ERROR: --voyage-key or VOYAGE_API_KEY required", file=sys.stderr)
        sys.exit(1)

    cozo = CozoStore(args.cozo, auth=args.cozo_auth)
    qdrant = QdrantStore(args.qdrant, api_key=args.qdrant_key)
    embedder = Embedder(api_key)
    indexer = Indexer(cozo, qdrant, embedder, incremental=not args.no_incremental)

    symbols = []
    edges = []
    batch_size = 64

    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            obj = json.loads(line)
        except json.JSONDecodeError:
            continue

        if obj.get("type") == "edge":
            edges.append(Edge(
                src=obj["src"],
                dst=obj["dst"],
                kind=obj["kind"],
                repo=args.repo,
                weight=obj.get("weight", 1.0),
            ))
        else:
            # Symbol
            symbols.append(Symbol(
                id=obj.get("id", ""),
                repo=args.repo,
                path=obj.get("path", ""),
                name=obj.get("name", ""),
                kind=obj.get("kind", "function"),
                lang=obj.get("lang", ""),
                start_line=obj.get("start_line", 0),
                end_line=obj.get("end_line", 0),
                signature=obj.get("signature", ""),
                doc=obj.get("doc", ""),
                body=obj.get("body", ""),
            ))

        if len(symbols) >= batch_size:
            indexer.index_symbols(symbols, edges)
            symbols = []
            edges = []

    # Flush remaining
    if symbols or edges:
        indexer.index_symbols(symbols, edges)

    stats = indexer.stats(args.repo)
    print(f"\nDone. {args.repo}: {stats}")
    print(f"Voyage: {embedder.total_calls} API calls, {embedder.total_tokens} tokens")


if __name__ == "__main__":
    main()
