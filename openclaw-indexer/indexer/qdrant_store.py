"""Qdrant vector store client."""

import hashlib
import time
from dataclasses import asdict

from qdrant_client import QdrantClient
from qdrant_client.models import (
    Distance,
    FieldCondition,
    Filter,
    MatchValue,
    PointStruct,
    VectorParams,
)
from rich.console import Console

from .chunker import CodeChunk
from .config import Config

console = Console()


class QdrantStore:
    """Manages the code_chunks collection in Qdrant."""

    def __init__(self, config: Config):
        self.config = config
        self.client = QdrantClient(url=config.qdrant_url, timeout=30)
        self.collection = config.collection_name

    def ensure_collection(self):
        """Create collection if it doesn't exist."""
        collections = self.client.get_collections().collections
        names = [c.name for c in collections]

        if self.collection not in names:
            self.client.create_collection(
                collection_name=self.collection,
                vectors_config=VectorParams(
                    size=self.config.embedding_dim,
                    distance=Distance.COSINE,
                ),
            )
            # Create payload indexes for filtering
            self.client.create_payload_index(
                collection_name=self.collection,
                field_name="repo",
                field_schema="keyword",
            )
            self.client.create_payload_index(
                collection_name=self.collection,
                field_name="language",
                field_schema="keyword",
            )
            self.client.create_payload_index(
                collection_name=self.collection,
                field_name="chunk_type",
                field_schema="keyword",
            )
            console.print(f"[green]Created collection '{self.collection}' ({self.config.embedding_dim}d cosine)[/green]")
        else:
            console.print(f"[green]Collection '{self.collection}' exists[/green]")

    def _chunk_id(self, chunk: CodeChunk) -> str:
        """Generate a deterministic ID for a chunk."""
        key = f"{chunk.repo_name}:{chunk.file_path}:{chunk.line_start}:{chunk.line_end}"
        return hashlib.sha256(key.encode()).hexdigest()[:32]

    def upsert_chunks(
        self,
        chunks: list[CodeChunk],
        embeddings: list[list[float]],
    ):
        """Batch upsert chunks with their embeddings."""
        if len(chunks) != len(embeddings):
            raise ValueError(f"Chunk/embedding count mismatch: {len(chunks)} vs {len(embeddings)}")

        points = []
        for chunk, embedding in zip(chunks, embeddings):
            # Use integer hash for Qdrant point ID
            chunk_hash = self._chunk_id(chunk)
            point_id = int(chunk_hash, 16) % (2**63)

            points.append(PointStruct(
                id=point_id,
                vector=embedding,
                payload={
                    "repo": chunk.repo_name,
                    "file_path": chunk.file_path,
                    "language": chunk.language,
                    "chunk_type": chunk.chunk_type,
                    "name": chunk.name,
                    "content": chunk.content[:4000],  # Limit payload size
                    "line_start": chunk.line_start,
                    "line_end": chunk.line_end,
                    "keywords": chunk.keywords,
                    "gemini_focus": chunk.gemini_focus,
                    "gemini_concepts": chunk.gemini_concepts,
                    "indexed_at": int(time.time()),
                },
            ))

        # Batch upsert
        batch_size = self.config.qdrant_batch_size
        for i in range(0, len(points), batch_size):
            batch = points[i:i + batch_size]
            self.client.upsert(
                collection_name=self.collection,
                points=batch,
            )

    def search(
        self,
        query_vector: list[float],
        repo: str | None = None,
        language: str | None = None,
        chunk_type: str | None = None,
        limit: int = 10,
    ) -> list[dict]:
        """Semantic search with optional filters."""
        conditions = []
        if repo:
            conditions.append(FieldCondition(key="repo", match=MatchValue(value=repo)))
        if language:
            conditions.append(FieldCondition(key="language", match=MatchValue(value=language)))
        if chunk_type:
            conditions.append(FieldCondition(key="chunk_type", match=MatchValue(value=chunk_type)))

        query_filter = Filter(must=conditions) if conditions else None

        results = self.client.query_points(
            collection_name=self.collection,
            query=query_vector,
            query_filter=query_filter,
            limit=limit,
            with_payload=True,
        )

        return [
            {
                "score": hit.score,
                **hit.payload,
            }
            for hit in results.points
        ]

    def delete_repo(self, repo_name: str):
        """Delete all chunks for a repo."""
        self.client.delete(
            collection_name=self.collection,
            points_selector=Filter(
                must=[FieldCondition(key="repo", match=MatchValue(value=repo_name))]
            ),
        )
        console.print(f"[yellow]Deleted all chunks for repo: {repo_name}[/yellow]")

    def get_stats(self) -> dict:
        """Get collection stats."""
        try:
            info = self.client.get_collection(self.collection)
            return {
                "collection": self.collection,
                "points_count": info.points_count,
                "vectors_count": info.vectors_count,
                "status": info.status.value,
                "segments": len(info.segments) if hasattr(info, 'segments') else 0,
            }
        except Exception as e:
            return {"error": str(e)}

    def get_repo_stats(self) -> list[dict]:
        """Get per-repo chunk counts."""
        # Use scroll to count unique repos.
        stats = {}
        offset = None
        try:
            while True:
                result = self.client.scroll(
                    collection_name=self.collection,
                    limit=100,
                    offset=offset,
                    with_payload=["repo"],
                )
                points, offset = result
                for point in points:
                    repo = point.payload.get("repo", "unknown")
                    stats[repo] = stats.get(repo, 0) + 1
                if offset is None:
                    break
        except Exception:
            return []

        return [{"repo": k, "chunks": v} for k, v in sorted(stats.items())]
