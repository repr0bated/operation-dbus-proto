"""Multi-backend embedding with automatic failover.

Cascade order: HuggingFace API → Lightning.ai → Paperspace → local CPU
When one backend hits rate limits or errors, automatically falls through to the next.
"""

import os
import time
from abc import ABC, abstractmethod

import httpx
from rich.console import Console

from .config import Config

console = Console()


class EmbedderBackend(ABC):
    """Base class for embedding backends."""

    @abstractmethod
    def embed_batch(self, texts: list[str]) -> list[list[float]]:
        ...

    @abstractmethod
    def embed_single(self, text: str) -> list[float]:
        ...

    @property
    @abstractmethod
    def dimensions(self) -> int:
        ...

    @property
    def name(self) -> str:
        return self.__class__.__name__


class HuggingFaceAPIEmbedder(EmbedderBackend):
    """HuggingFace free Inference API."""

    def __init__(self, config: Config):
        self.model = config.embedding_model
        self.token = config.hf_token
        self.batch_size = config.embed_batch_size
        self._dim = config.embedding_dim
        self.api_url = f"https://router.huggingface.co/hf-inference/models/{self.model}"
        self.client = httpx.Client(timeout=60.0)

    @property
    def dimensions(self) -> int:
        return self._dim

    def _call_api(self, texts: list[str]) -> list[list[float]]:
        headers = {
            "Authorization": f"Bearer {self.token}",
            "Content-Type": "application/json",
        }
        payload = {"inputs": texts, "options": {"wait_for_model": True}}

        for attempt in range(3):
            try:
                resp = self.client.post(self.api_url, json=payload, headers=headers)
                if resp.status_code == 503:
                    wait = resp.json().get("estimated_time", 20)
                    console.print(f"[yellow]HF: model loading, waiting {wait:.0f}s...[/yellow]")
                    time.sleep(min(wait, 60))
                    continue
                if resp.status_code == 429:
                    # Rate limited - signal caller to failover
                    raise RateLimitError("HuggingFace rate limited")
                resp.raise_for_status()
                return resp.json()
            except httpx.HTTPStatusError as e:
                if e.response.status_code == 429:
                    raise RateLimitError("HuggingFace rate limited")
                if attempt == 2:
                    raise
                console.print(f"[yellow]HF: error {e.response.status_code}, retrying...[/yellow]")
                time.sleep(2 ** attempt)

        raise RuntimeError("HF API failed after 3 attempts")

    def embed_batch(self, texts: list[str]) -> list[list[float]]:
        all_embeddings = []
        for i in range(0, len(texts), self.batch_size):
            batch = texts[i:i + self.batch_size]
            embeddings = self._call_api(batch)
            all_embeddings.extend(embeddings)
        return all_embeddings

    def embed_single(self, text: str) -> list[float]:
        return self._call_api([text])[0]


class LightningEmbedder(EmbedderBackend):
    """Lightning.ai Inference API for embeddings."""

    def __init__(self, config: Config):
        self.api_key = config.lightning_api_key
        self._dim = config.embedding_dim
        self.model = config.embedding_model
        self.batch_size = config.embed_batch_size
        self.client = httpx.Client(timeout=120.0)
        # Lightning.ai uses HF-compatible inference endpoints
        self.api_url = "https://api.lightning.ai/v1/embeddings"

    @property
    def dimensions(self) -> int:
        return self._dim

    def _call_api(self, texts: list[str]) -> list[list[float]]:
        """Call Lightning.ai embedding API."""
        headers = {
            "Authorization": f"Bearer {self.api_key}",
            "Content-Type": "application/json",
        }

        # Lightning.ai supports OpenAI-compatible embedding endpoint
        payload = {
            "model": self.model,
            "input": texts,
        }

        for attempt in range(3):
            try:
                resp = self.client.post(self.api_url, json=payload, headers=headers)
                if resp.status_code == 429:
                    raise RateLimitError("Lightning.ai rate limited")
                if resp.status_code == 404 or resp.status_code == 422:
                    # Model not available on Lightning, try HF-compatible route
                    return self._call_hf_compat(texts)
                resp.raise_for_status()
                data = resp.json()
                # OpenAI embedding response format
                if "data" in data:
                    return [item["embedding"] for item in data["data"]]
                # Direct array format
                return data
            except httpx.HTTPStatusError as e:
                if e.response.status_code == 429:
                    raise RateLimitError("Lightning.ai rate limited")
                if attempt == 2:
                    raise
                console.print(f"[yellow]Lightning: error {e.response.status_code}, retrying...[/yellow]")
                time.sleep(2 ** attempt)

        raise RuntimeError("Lightning API failed after 3 attempts")

    def _call_hf_compat(self, texts: list[str]) -> list[list[float]]:
        """Fallback: use Lightning.ai's HF-compatible inference."""
        url = f"https://api.lightning.ai/v1/hf/models/{self.model}"
        headers = {
            "Authorization": f"Bearer {self.api_key}",
            "Content-Type": "application/json",
        }
        payload = {"inputs": texts}
        resp = self.client.post(url, json=payload, headers=headers)
        if resp.status_code == 429:
            raise RateLimitError("Lightning.ai rate limited")
        resp.raise_for_status()
        return resp.json()

    def embed_batch(self, texts: list[str]) -> list[list[float]]:
        all_embeddings = []
        for i in range(0, len(texts), self.batch_size):
            batch = texts[i:i + self.batch_size]
            embeddings = self._call_api(batch)
            all_embeddings.extend(embeddings)
        return all_embeddings

    def embed_single(self, text: str) -> list[float]:
        return self._call_api([text])[0]


class PaperspaceEmbedder(EmbedderBackend):
    """Paperspace GPU via their Gradient API."""

    def __init__(self, config: Config):
        self.api_key = config.paperspace_api_key
        self.notebook_id = config.paperspace_notebook_id
        self._dim = config.embedding_dim
        self.model = config.embedding_model
        self.batch_size = config.embed_batch_size
        self.client = httpx.Client(timeout=300.0)

    @property
    def dimensions(self) -> int:
        return self._dim

    def _call_api(self, texts: list[str]) -> list[list[float]]:
        """Try Paperspace Gradient deployments API."""
        if self.notebook_id:
            url = f"https://api.paperspace.com/v1/deployments/{self.notebook_id}/predict"
        else:
            url = "https://api.paperspace.com/v1/deployments/inference"
        headers = {
            "Authorization": f"Bearer {self.api_key}",
            "Content-Type": "application/json",
        }
        payload = {
            "model": self.model,
            "input": texts,
        }

        for attempt in range(2):
            try:
                resp = self.client.post(url, json=payload, headers=headers)
                if resp.status_code == 429:
                    raise RateLimitError("Paperspace rate limited")
                resp.raise_for_status()
                data = resp.json()
                if "data" in data:
                    return [item["embedding"] for item in data["data"]]
                return data
            except httpx.HTTPStatusError as e:
                if e.response.status_code == 429:
                    raise RateLimitError("Paperspace rate limited")
                if attempt == 1:
                    raise
                time.sleep(2)

        raise RuntimeError("Paperspace API failed")

    def embed_batch(self, texts: list[str]) -> list[list[float]]:
        all_embeddings = []
        for i in range(0, len(texts), self.batch_size):
            batch = texts[i:i + self.batch_size]
            embeddings = self._call_api(batch)
            all_embeddings.extend(embeddings)
        return all_embeddings

    def embed_single(self, text: str) -> list[float]:
        return self._call_api([text])[0]


class LocalEmbedder(EmbedderBackend):
    """Local CPU embedding via sentence-transformers (OpenVINO if available)."""

    def __init__(self, config: Config):
        self.model_name = config.embedding_model
        self._dim = config.embedding_dim
        self.batch_size = config.embed_batch_size
        self._model = None

    def _load_model(self):
        if self._model is not None:
            return
        from sentence_transformers import SentenceTransformer

        try:
            self._model = SentenceTransformer(self.model_name, backend="openvino")
            console.print(f"[green]Loaded {self.model_name} with OpenVINO[/green]")
        except Exception:
            self._model = SentenceTransformer(self.model_name)
            console.print(f"[green]Loaded {self.model_name} on CPU[/green]")

    @property
    def dimensions(self) -> int:
        return self._dim

    def embed_batch(self, texts: list[str]) -> list[list[float]]:
        self._load_model()
        return self._model.encode(
            texts, batch_size=self.batch_size,
            show_progress_bar=False, normalize_embeddings=True,
        ).tolist()

    def embed_single(self, text: str) -> list[float]:
        self._load_model()
        return self._model.encode([text], normalize_embeddings=True)[0].tolist()


class GeminiEmbedder(EmbedderBackend):
    """Gemini embedding via Developer API (gemini-embedding-001, 3072d)."""

    MODEL = "gemini-embedding-001"
    # API limit: 100 texts per batchEmbedContents call
    _API_BATCH = 100

    def __init__(self, config: Config):
        self._dim = 3072
        self.batch_size = min(config.embed_batch_size, self._API_BATCH)
        self.api_key = config.gemini_api_key
        self.client = httpx.Client(timeout=60.0)

    @property
    def dimensions(self) -> int:
        return self._dim

    def _call_api(self, texts: list[str]) -> list[list[float]]:
        """Call batchEmbedContents — up to 100 texts per request."""
        url = (
            f"https://generativelanguage.googleapis.com/v1beta/models/"
            f"{self.MODEL}:batchEmbedContents?key={self.api_key}"
        )
        requests = [
            {"model": f"models/{self.MODEL}", "content": {"parts": [{"text": t}]}}
            for t in texts
        ]
        for attempt in range(3):
            resp = self.client.post(url, json={"requests": requests})
            if resp.status_code == 429:
                time.sleep(2 ** attempt)
                continue
            resp.raise_for_status()
            return [e["values"] for e in resp.json()["embeddings"]]
        raise RateLimitError("Gemini embedding rate limited")

    def embed_batch(self, texts: list[str]) -> list[list[float]]:
        all_embeddings = []
        for i in range(0, len(texts), self.batch_size):
            all_embeddings.extend(self._call_api(texts[i:i + self.batch_size]))
        return all_embeddings

    def embed_single(self, text: str) -> list[float]:
        return self._call_api([text])[0]


class RateLimitError(Exception):
    """Raised when a backend hits rate limits, triggering failover."""
    pass


class FallbackEmbedder(EmbedderBackend):
    """Automatic failover across multiple embedding backends.

    When the current backend hits a rate limit or error, seamlessly
    falls through to the next backend in the chain.

    Default chain: HuggingFace API → Lightning.ai → Paperspace → local CPU
    """

    def __init__(self, backends: list[EmbedderBackend]):
        self._backends = backends
        self._current = 0
        self._dim = backends[0].dimensions if backends else 768
        self._failed = set()  # Track permanently failed backends

    @property
    def dimensions(self) -> int:
        return self._dim

    @property
    def active_backend(self) -> str:
        if self._current < len(self._backends):
            return self._backends[self._current].name
        return "none"

    def _try_with_failover(self, func_name: str, *args, **kwargs):
        """Try current backend, failover on rate limit or error."""
        last_error = None

        for i in range(self._current, len(self._backends)):
            if i in self._failed:
                continue

            backend = self._backends[i]
            try:
                result = getattr(backend, func_name)(*args, **kwargs)
                if i != self._current:
                    console.print(f"[green]Switched to {backend.name}[/green]")
                    self._current = i
                return result
            except RateLimitError as e:
                console.print(f"[yellow]{backend.name}: rate limited, trying next backend...[/yellow]")
                last_error = e
                continue
            except NotImplementedError:
                # Backend doesn't support this operation (e.g., batch-only)
                continue
            except Exception as e:
                console.print(f"[yellow]{backend.name}: error ({e}), trying next backend...[/yellow]")
                last_error = e
                # Mark as failed for this session if it's not a transient error
                if "401" in str(e) or "403" in str(e) or "not found" in str(e).lower():
                    self._failed.add(i)
                continue

        raise RuntimeError(
            f"All embedding backends exhausted. Last error: {last_error}"
        )

    def embed_batch(self, texts: list[str]) -> list[list[float]]:
        """Embed batch with automatic failover per-batch.

        If rate limited mid-way through a large job, seamlessly continues
        with the next backend from where it left off.
        """
        all_embeddings = []
        batch_size = 32
        remaining_texts = texts

        while remaining_texts:
            batch = remaining_texts[:batch_size]
            try:
                embeddings = self._try_with_failover("embed_batch", batch)
                all_embeddings.extend(embeddings)
                remaining_texts = remaining_texts[batch_size:]
            except RuntimeError:
                if all_embeddings:
                    console.print(f"[red]Partial result: {len(all_embeddings)}/{len(texts)} embedded before all backends exhausted[/red]")
                raise

        return all_embeddings

    def embed_single(self, text: str) -> list[float]:
        return self._try_with_failover("embed_single", text)


def create_embedder(config: Config) -> EmbedderBackend:
    """Create embedder with automatic failover chain."""
    backend = config.embedding_backend

    # Explicit backend selection (no failover)
    if backend == "gemini" or (backend == "huggingface_api" and config.gemini_api_key):
        console.print("[green]Using Gemini embedding (gemini-embedding-001, 3072d)[/green]")
        return GeminiEmbedder(config)
    elif backend == "local":
        return LocalEmbedder(config)

    # Default: build failover chain
    # Paperspace GPU → HuggingFace API → Lightning.ai → (local CPU last resort)
    backends: list[EmbedderBackend] = []

    if config.paperspace_api_key:
        backends.append(PaperspaceEmbedder(config))
        console.print("[green]+ Paperspace GPU (primary)[/green]")

    if config.hf_token:
        backends.append(HuggingFaceAPIEmbedder(config))
        console.print("[green]+ HuggingFace API (failover)[/green]")

    if config.lightning_api_key:
        backends.append(LightningEmbedder(config))
        console.print("[green]+ Lightning.ai (failover)[/green]")

    # Local CPU as last resort (if sentence-transformers is available)
    try:
        import sentence_transformers
        backends.append(LocalEmbedder(config))
        console.print("[green]+ Local CPU (last resort)[/green]")
    except ImportError:
        pass

    if not backends:
        raise RuntimeError(
            "No embedding backends available. Set HF_TOKEN, LIGHTNINGAI_API_KEY, "
            "or PAPERSPACE_API_KEY in ~/.bash_secrets"
        )

    if len(backends) == 1:
        return backends[0]

    console.print(f"[bold]Failover chain: {' → '.join(b.name for b in backends)}[/bold]")
    return FallbackEmbedder(backends)
