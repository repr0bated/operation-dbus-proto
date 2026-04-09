"""Configuration loaded from environment / ~/.bash_secrets."""

import os
from dataclasses import dataclass, field
from pathlib import Path


@dataclass
class Config:
    # Paths
    git_root: Path = field(default_factory=lambda: Path(os.environ.get("GIT_ROOT", str(Path.home() / "git"))))

    # Qdrant
    qdrant_url: str = field(default_factory=lambda: os.environ.get("QDRANT_URL", "http://10.149.181.190:6333"))
    collection_name: str = "code_chunks_gemini"

    # Embedding
    embedding_model: str = "gemini-embedding-001"
    embedding_dim: int = 3072
    embedding_backend: str = field(default_factory=lambda: os.environ.get("EMBEDDING_BACKEND", "gemini"))

    # Gemini
    gemini_api_key: str = field(default_factory=lambda: os.environ.get("GEMINI_API_KEY", ""))  # huggingface_api | local | paperspace | lightning

    # HuggingFace
    hf_token: str = field(default_factory=lambda: os.environ.get("HF_TOKEN", ""))

    # Paperspace
    paperspace_api_key: str = field(default_factory=lambda: os.environ.get("PAPERSPACE_API_KEY", ""))
    paperspace_notebook_id: str = field(default_factory=lambda: os.environ.get("PAPERSPACE_NOTEBOOK_ID", ""))

    # Lightning.ai
    lightning_api_key: str = field(default_factory=lambda: os.environ.get("LIGHTNINGAI_API_KEY", os.environ.get("LIGHTNING_API_KEY", "")))

    # Chunking
    max_chunk_tokens: int = 512
    chunk_overlap_tokens: int = 64

    # File extensions to index
    code_extensions: set = field(default_factory=lambda: {
        # Code
        ".rs", ".py", ".pyi", ".ts", ".tsx", ".js", ".jsx",
        ".mjs", ".cjs", ".go", ".c", ".h", ".cpp", ".hpp",
        # Web
        ".html", ".css", ".scss", ".less", ".svg",
        # Data / config
        ".proto", ".toml", ".yaml", ".yml", ".json", ".xml",
        ".ini", ".cfg", ".conf", ".env.example",
        # Docs / text
        ".md", ".mdx", ".txt", ".rst", ".log",
        # Scripts / infra
        ".sh", ".sql", ".service", ".template", ".patch",
        # Binary-with-extraction
        ".pdf",
    })

    # Directories to skip
    skip_dirs: set = field(default_factory=lambda: {
        ".git", "target", "node_modules", "__pycache__",
        ".mypy_cache", ".pytest_cache", "vendor", "dist",
        "build", ".next", ".nuxt", "venv", ".venv",
        ".tox", "eggs", "*.egg-info",
    })

    # Repos to exclude from indexing
    skip_repos: set = field(default_factory=lambda: set())

    # Subdirectories from skipped repos to include anyway
    # Format: {repo_name: [subdir_path, ...]}
    include_subdirs: dict = field(default_factory=lambda: {})

    # Batch sizes
    embed_batch_size: int = 32
    qdrant_batch_size: int = 100


def load_config() -> Config:
    """Load config, sourcing bash_secrets if env vars are missing."""
    secrets_path = Path.home() / ".bash_secrets"
    if secrets_path.exists():
        # Always parse bash_secrets - file is source of truth
        with open(secrets_path) as f:
            for line in f:
                line = line.strip()
                if line.startswith("#") or not line.startswith("export "):
                    continue
                if "=" not in line:
                    continue
                key_val = line[7:]  # strip "export "
                key, _, val = key_val.partition("=")
                val = val.strip("'\"")
                if key and val:
                    os.environ[key] = val

    return Config()
