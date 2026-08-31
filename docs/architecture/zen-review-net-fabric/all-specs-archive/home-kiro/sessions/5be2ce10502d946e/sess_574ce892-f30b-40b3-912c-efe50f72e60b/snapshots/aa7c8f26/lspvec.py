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

VOYAGE_MODEL = "voyage-code-3.5"
VOYAGE_DIM = 1024  # voyage-code-3.5 output dimension (same as voyage-4 family)
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
        # Insert one at a time using parameters to avoid escaping issues
        for s in symbols:
            self.db.run("""
                ?[id, repo, path, name, kind, lang, start_line, end_line, signature, doc, body_hash, indexed_at] <-
                    [[$id, $repo, $path, $name, $kind, $lang, $start_line, $end_line, $signature, $doc, $body_hash, $indexed_at]]
                :put symbols {
                    id =>
                    repo, path, name, kind, lang, start_line, end_line,
                    signature, doc, body_hash, indexed_at
                }
            """, {
                "id": s.id, "repo": s.repo, "path": s.path, "name": s.name,
                "kind": s.kind, "lang": s.lang, "start_line": s.start_line,
                "end_line": s.end_line, "signature": s.signature, "doc": s.doc,
                "body_hash": s.body_hash, "indexed_at": time.time(),
            })

    def put_edges(self, edges: list[Edge]):
        if not edges:
            return
        for e in edges:
            self.db.run("""
                ?[src, dst, kind, repo, weight] <- [[$src, $dst, $kind, $repo, $weight]]
                :put edges { src, dst, kind => repo, weight }
            """, {"src": e.src, "dst": e.dst, "kind": e.kind, "repo": e.repo, "weight": e.weight})

    def put_chunks(self, chunks: list[Chunk], embedded: bool = False):
        if not chunks:
            return
        for c in chunks:
            self.db.run("""
                ?[id, repo, path, name, text_hash, embedded] <-
                    [[$id, $repo, $path, $name, $text_hash, $embedded]]
                :put chunks { id => repo, path, name, text_hash, embedded }
            """, {
                "id": c.id, "repo": c.repo, "path": c.path, "name": c.name,
                "text_hash": c.text_hash, "embedded": embedded,
            })

    def mark_embedded(self, chunk_ids: list[str]):
        if not chunk_ids:
            return
        for cid in chunk_ids:
            # Update the embedded flag - need to read existing row first
            try:
                res = self.db.run("""
                    ?[id, repo, path, name, text_hash] := *chunks{id, repo, path, name, text_hash}, id = $id
                """, {"id": cid})
                if res["rows"]:
                    row = res["rows"][0]
                    self.db.run("""
                        ?[id, repo, path, name, text_hash, embedded] <-
                            [[$id, $repo, $path, $name, $text_hash, true]]
                        :put chunks { id => repo, path, name, text_hash, embedded }
                    """, {"id": row[0], "repo": row[1], "path": row[2],
                          "name": row[3], "text_hash": row[4]})
            except Exception:
                pass

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

class UnixSocketTransport(httpx.HTTPTransport):
    """httpx transport that connects via a Unix domain socket."""
    def __init__(self, socket_path: str):
        import httpcore
        super().__init__()
        self._pool = httpcore.ConnectionPool(uds=socket_path)


class QdrantUnixClient:
    """Minimal Qdrant REST client over a Unix socket."""
    def __init__(self, socket_path: str):
        self.socket_path = socket_path
        self._http = httpx.Client(
            transport=UnixSocketTransport(socket_path),
            base_url="http://localhost",
            timeout=60.0,
        )

    def _get(self, path: str, params: dict = None):
        r = self._http.get(path, params=params)
        r.raise_for_status()
        return r.json().get("result", r.json())

    def _post(self, path: str, json_data: dict = None):
        r = self._http.post(path, json=json_data)
        r.raise_for_status()
        return r.json().get("result", r.json())

    def _put(self, path: str, json_data: dict = None):
        r = self._http.put(path, json=json_data)
        r.raise_for_status()
        return r.json().get("result", r.json())

    def _patch(self, path: str, json_data: dict = None):
        r = self._http.patch(path, json=json_data)
        r.raise_for_status()
        return r.json().get("result", r.json())


class QdrantStore:
    def __init__(self, url: str = "http://127.0.0.1:6333", api_key: Optional[str] = None,
                 collection: str = QDRANT_COLLECTION):
        self.collection = collection
        self._unix_client: Optional[QdrantUnixClient] = None
        self.client: Optional[QdrantClient] = None

        if url.startswith("unix://"):
            sock_path = url[len("unix://"):]
            self._unix_client = QdrantUnixClient(sock_path)
        elif url.startswith("/run/") or url.startswith("/tmp/") or url.endswith(".sock"):
            self._unix_client = QdrantUnixClient(url)
        elif url.startswith("http://") or url.startswith("https://"):
            self.client = QdrantClient(url=url, api_key=api_key, timeout=60)
        elif url == ":memory:":
            self.client = QdrantClient(location=":memory:")
        else:
            self.client = QdrantClient(path=url)
        self._ensure_collection()

    def _ensure_collection(self):
        if self._unix_client:
            collections = self._unix_client._get("/collections")
            names = [c["name"] for c in collections.get("collections", [])]
            if self.collection not in names:
                self._unix_client._put(f"/collections/{self.collection}", json_data={
                    "vectors": {"size": VOYAGE_DIM, "distance": "Cosine"},
                    "on_disk_payload": True,
                })
                for field_name, field_type in [
                    ("repo", "keyword"), ("path", "keyword"),
                    ("kind", "keyword"), ("lang", "keyword"), ("name", "text"),
                ]:
                    self._unix_client._put(
                        f"/collections/{self.collection}/index",
                        json_data={"field_name": field_name, "field_schema": field_type},
                    )
                print(f"Created Qdrant collection '{self.collection}' (dim={VOYAGE_DIM})")
        else:
            collections = [c.name for c in self.client.get_collections().collections]
            if self.collection not in collections:
                self.client.create_collection(
                    collection_name=self.collection,
                    vectors_config=VectorParams(
                        size=VOYAGE_DIM, distance=Distance.COSINE, on_disk=True,
                    ),
                )
                for field_name, schema_type in [
                    ("repo", PayloadSchemaType.KEYWORD),
                    ("path", PayloadSchemaType.KEYWORD),
                    ("kind", PayloadSchemaType.KEYWORD),
                    ("lang", PayloadSchemaType.KEYWORD),
                    ("name", PayloadSchemaType.TEXT),
                ]:
                    self.client.create_payload_index(
                        collection_name=self.collection,
                        field_name=field_name, field_schema=schema_type,
                    )
                print(f"Created Qdrant collection '{self.collection}' (dim={VOYAGE_DIM})")

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
            point_id = hashlib.md5(chunk.id.encode()).hexdigest()
            points.append({"id": point_id, "vector": vec, "payload": payload})

        batch_size = 100
        if self._unix_client:
            for i in range(0, len(points), batch_size):
                batch = points[i : i + batch_size]
                self._unix_client._put(
                    f"/collections/{self.collection}/points",
                    json_data={"points": batch},
                )
        else:
            for i in range(0, len(points), batch_size):
                batch = points[i : i + batch_size]
                structs = [
                    PointStruct(id=p["id"], vector=p["vector"], payload=p["payload"])
                    for p in batch
                ]
                self.client.upsert(collection_name=self.collection, points=structs)

    def count(self, repo: Optional[str] = None) -> int:
        if self._unix_client:
            body = {}
            if repo:
                body["filter"] = {"must": [{"key": "repo", "match": {"value": repo}}]}
            res = self._unix_client._post(f"/collections/{self.collection}/points/count",
                                          json_data=body)
            return res.get("count", 0)
        else:
            if repo:
                result = self.client.count(
                    collection_name=self.collection,
                    count_filter=Filter(
                        must=[FieldCondition(key="repo", match=MatchValue(value=repo))]
                    ),
                )
            else:
                result = self.client.count(collection_name=self.collection)
            return result.count

    def search(self, vector: list[float], limit: int = 10, repo: Optional[str] = None):
        if self._unix_client:
            body = {"vector": vector, "limit": limit, "with_payload": True}
            if repo:
                body["filter"] = {"must": [{"key": "repo", "match": {"value": repo}}]}
            return self._unix_client._post(
                f"/collections/{self.collection}/points/query",
                json_data=body,
            )
        else:
            filt = None
            if repo:
                filt = Filter(
                    must=[FieldCondition(key="repo", match=MatchValue(value=repo))]
                )
            return self.client.query_points(
                collection_name=self.collection,
                query=vector, limit=limit, query_filter=filt, with_payload=True,
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
        """Embed a batch of texts. Max 128 texts per API call. Retries on rate limit."""
        if not texts:
            return []
        import time as _time
        batch_size = 128
        all_vectors = []
        for i in range(0, len(texts), batch_size):
            batch = texts[i : i + batch_size]
            while True:
                try:
                    result = self.client.embed(
                        batch,
                        model=self.model,
                        input_type=input_type,
                    )
                    all_vectors.extend(result.embeddings)
                    self.total_tokens += result.total_tokens
                    self.total_calls += 1
                    break
                except Exception as e:
                    err_str = str(e).lower()
                    if "rate" in err_str or "429" in err_str or "too many" in err_str:
                        wait = 5
                        print(f"    rate limited, polling in {wait}s...", file=sys.stderr)
                        _time.sleep(wait)
                    else:
                        raise
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
    qdrant = QdrantStore(args.qdrant, api_key=args.qdrant_key, collection=args.collection)

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
    qdrant = QdrantStore(args.qdrant, api_key=args.qdrant_key, collection=args.collection)
    print(f"Cozo: {args.cozo}")
    print(f"Qdrant: {args.qdrant} (collection: {args.collection})")
    print("Stores initialized.")


def main():
    parser = argparse.ArgumentParser(
        description="lspvec - Code symbol indexer with LSP extraction, Voyage embeddings, Qdrant + Cozo storage"
    )
    parser.add_argument("--cozo", default="./lspvec.cozo",
                        help="Cozo path: rocksdb dir, .sqlite file, :memory:, or http(s) URL")
    parser.add_argument("--cozo-auth", default=os.environ.get("COZO_AUTH"),
                        help="Auth token for Cozo HTTP mode")
    parser.add_argument("--qdrant", default=os.environ.get("QDRANT_URL", "/run/ghostbridge/fwd-qdrant.sock"),
                        help="Qdrant URL, unix socket path, or local dir")
    parser.add_argument("--qdrant-key", default=os.environ.get("QDRANT_API_KEY"),
                        help="Qdrant API key")
    parser.add_argument("--collection", default=os.environ.get("QDRANT_COLLECTION", QDRANT_COLLECTION),
                        help="Qdrant collection name")
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
    qdrant = QdrantStore(args.qdrant, api_key=args.qdrant_key, collection=args.collection)
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
