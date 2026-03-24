"""Discover git repos and walk code files respecting .gitignore."""

from dataclasses import dataclass, field
from pathlib import Path

import pathspec
from rich.console import Console

from .config import Config
from .gemini_context import GeminiContext, parse_gemini_md

console = Console()


@dataclass
class RepoInfo:
    """Discovered repository with metadata."""
    name: str
    path: Path
    gemini: GeminiContext | None = None
    files: list[Path] = field(default_factory=list)
    language: str = ""


@dataclass
class FileInfo:
    """A code file to be indexed."""
    repo_name: str
    path: Path
    relative_path: str
    language: str
    content: str = ""


EXTENSION_LANGUAGES = {
    # Code
    ".rs": "rust",
    ".py": "python",
    ".pyi": "python",
    ".ts": "typescript",
    ".tsx": "typescript",
    ".js": "javascript",
    ".jsx": "javascript",
    ".mjs": "javascript",
    ".cjs": "javascript",
    ".go": "go",
    ".c": "c",
    ".h": "c",
    ".cpp": "cpp",
    ".hpp": "cpp",
    # Web
    ".html": "html",
    ".css": "css",
    ".scss": "css",
    ".less": "css",
    ".svg": "xml",
    # Data / config
    ".proto": "protobuf",
    ".toml": "toml",
    ".yaml": "yaml",
    ".yml": "yaml",
    ".json": "json",
    ".xml": "xml",
    ".ini": "config",
    ".cfg": "config",
    ".conf": "config",
    ".env.example": "config",
    # Docs / text
    ".md": "markdown",
    ".mdx": "markdown",
    ".txt": "text",
    ".rst": "text",
    ".log": "text",
    # Scripts / infra
    ".sh": "shell",
    ".sql": "sql",
    ".service": "config",
    ".template": "text",
    ".patch": "text",
    # Binary-with-extraction
    ".pdf": "pdf",
}


def discover_repos(config: Config) -> list[RepoInfo]:
    """Find all directories under git_root (2 levels deep), excluding skip_repos."""
    repos = []
    seen_names: set[str] = set()
    git_root = config.git_root

    if not git_root.exists():
        console.print(f"[red]Git root not found: {git_root}[/red]")
        return repos

    # Scan 2 levels deep: ~/git/foo and ~/git/subdir/foo
    for entry in sorted(git_root.iterdir()):
        if not entry.is_dir() or entry.name.startswith("."):
            continue
        if entry.name in config.skip_repos:
            continue
        # If it looks like a repo (has code files or .git), add it
        if (entry / ".git").exists() or _has_code_files(entry, config):
            _add_repo(entry, repos, seen_names, config)
        else:
            # Check one level deeper (e.g. indexed-repos/zbus)
            for sub in sorted(entry.iterdir()):
                if not sub.is_dir() or sub.name.startswith("."):
                    continue
                if sub.name in config.skip_repos:
                    continue
                if (sub / ".git").exists() or _has_code_files(sub, config):
                    _add_repo(sub, repos, seen_names, config)

    # Include subdirectories from skipped repos (e.g. operation-dbus/crates)
    for repo_name, subdirs in config.include_subdirs.items():
        repo_path = git_root / repo_name
        if not repo_path.is_dir():
            continue
        for subdir in subdirs:
            sub_path = repo_path / subdir
            if sub_path.is_dir():
                synthetic_name = f"{repo_name}/{subdir}"
                if synthetic_name not in seen_names:
                    seen_names.add(synthetic_name)
                    gemini = parse_gemini_md(repo_path / "GEMINI.md")
                    repos.append(RepoInfo(
                        name=synthetic_name,
                        path=sub_path,
                        gemini=gemini,
                        language=gemini.language if gemini else "",
                    ))

    console.print(f"[green]Discovered {len(repos)} repos[/green]")
    return repos


def _has_code_files(path: Path, config: Config) -> bool:
    """Quick check if a directory has any code files."""
    try:
        for f in path.iterdir():
            if f.is_file() and f.suffix in config.code_extensions:
                return True
    except PermissionError:
        pass
    return False


def _add_repo(path: Path, repos: list[RepoInfo], seen: set[str], config: Config):
    """Add a repo if not already seen."""
    name = path.name
    if name in seen:
        name = f"{path.parent.name}/{name}"
    if name in config.skip_repos:
        return
    seen.add(name)
    gemini = parse_gemini_md(path / "GEMINI.md")
    repos.append(RepoInfo(
        name=name,
        path=path,
        gemini=gemini,
        language=gemini.language if gemini else "",
    ))


def load_gitignore(repo_path: Path) -> pathspec.PathSpec | None:
    """Load .gitignore patterns for a repo."""
    gitignore = repo_path / ".gitignore"
    if not gitignore.exists():
        return None
    patterns = gitignore.read_text(errors="replace").splitlines()
    return pathspec.PathSpec.from_lines("gitwildmatch", patterns)


def walk_repo_files(repo: RepoInfo, config: Config) -> list[FileInfo]:
    """Walk a repo, yielding code files respecting .gitignore and skip dirs."""
    files = []
    gitignore_spec = load_gitignore(repo.path)

    for path in repo.path.rglob("*"):
        if not path.is_file():
            continue

        # Check extension
        if path.suffix not in config.code_extensions:
            continue

        # Get path relative to repo root
        try:
            rel = path.relative_to(repo.path)
        except ValueError:
            continue

        rel_str = str(rel)

        # Skip directories
        parts = rel.parts
        if any(part in config.skip_dirs for part in parts):
            continue

        # Check gitignore
        if gitignore_spec and gitignore_spec.match_file(rel_str):
            continue

        # Skip very large files (>500KB)
        try:
            if path.stat().st_size > 500_000:
                continue
        except OSError:
            continue

        language = EXTENSION_LANGUAGES.get(path.suffix, "unknown")
        files.append(FileInfo(
            repo_name=repo.name,
            path=path,
            relative_path=rel_str,
            language=language,
        ))

    return files


def crawl_all(config: Config) -> tuple[list[RepoInfo], list[FileInfo]]:
    """Crawl all repos and collect files."""
    repos = discover_repos(config)
    all_files = []

    for repo in repos:
        files = walk_repo_files(repo, config)
        repo.files = [f.path for f in files]
        all_files.extend(files)
        console.print(f"  {repo.name}: {len(files)} files")

    console.print(f"[green]Total: {len(all_files)} files across {len(repos)} repos[/green]")
    return repos, all_files
