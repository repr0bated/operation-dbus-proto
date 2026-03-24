"""Main orchestrator: crawl → chunk → embed → store in Qdrant."""

import time

import click
import orjson
from rich.console import Console
from rich.progress import Progress, SpinnerColumn, TextColumn, BarColumn, MofNCompleteColumn
from rich.table import Table

from .chunker import CodeChunk, chunk_file
from .config import Config, load_config
from .crawler import crawl_all, discover_repos, walk_repo_files
from .embedder import create_embedder
from .lsp_chunker import LspChunk, chunk_file_lsp, shutdown_all
from .qdrant_store import QdrantStore

console = Console()


def run_full_index(config: Config, repo_filter: str | None = None):
    """Full index: crawl all repos, chunk, embed, store."""
    console.print("[bold]Starting full index...[/bold]")
    t0 = time.time()

    # Crawl
    repos, files = crawl_all(config)

    if repo_filter:
        files = [f for f in files if f.repo_name == repo_filter]
        repos = [r for r in repos if r.name == repo_filter]
        console.print(f"[yellow]Filtered to repo: {repo_filter} ({len(files)} files)[/yellow]")

    # Chunk
    all_chunks: list[LspChunk] = []
    with Progress(
        SpinnerColumn(),
        TextColumn("[progress.description]{task.description}"),
        BarColumn(),
        MofNCompleteColumn(),
        TextColumn("[cyan]{task.fields[chunks]} chunks"),
        console=console,
    ) as progress:
        task = progress.add_task(f"Chunking {len(files)} files", total=len(files), chunks=0)
        for file_info in files:
            chunks = chunk_file_lsp(file_info, config)
            all_chunks.extend(chunks)
            progress.advance(task)
            progress.update(task, chunks=len(all_chunks),
                            description=f"[dim]{file_info.repo_name}[/dim] {file_info.path.name}")

    console.print(f"[green]Generated {len(all_chunks)} chunks from {len(files)} files[/green]")

    if not all_chunks:
        console.print("[yellow]No chunks to index[/yellow]")
        return

    # Enrich chunks with GEMINI.md context before embedding
    repo_gemini = {}
    for repo in repos:
        if repo.gemini:
            repo_gemini[repo.name] = repo.gemini

    for chunk in all_chunks:
        gemini = repo_gemini.get(chunk.repo_name)
        if gemini:
            chunk.gemini_focus = gemini.focus_area or ""
            chunk.gemini_concepts = gemini.key_concepts or []

    # Embed + upsert incrementally (progress saved per batch)
    embedder = create_embedder(config)
    store = QdrantStore(config)
    store.ensure_collection()

    batch_size = config.embed_batch_size
    total_batches = (len(all_chunks) + batch_size - 1) // batch_size
    console.print(f"Embedding + upserting {len(all_chunks)} chunks in {total_batches} batches (backend: [cyan]{embedder.__class__.__name__}[/cyan])...")

    with Progress(
        SpinnerColumn(),
        TextColumn("[progress.description]{task.description}"),
        BarColumn(),
        MofNCompleteColumn(),
        TextColumn("[cyan]{task.fields[stored]} stored"),
        console=console,
    ) as progress:
        task = progress.add_task(f"Embedding batches", total=total_batches, stored=0)
        total_embedded = 0
        for i in range(0, len(all_chunks), batch_size):
            batch_chunks = all_chunks[i:i + batch_size]
            embeddings = embedder.embed_batch([c.embed_text for c in batch_chunks])
            store.upsert_chunks(batch_chunks, embeddings)
            total_embedded += len(batch_chunks)
            batch_num = i // batch_size + 1
            progress.advance(task)
            progress.update(task, stored=total_embedded,
                            description=f"Batch [bold]{batch_num}[/bold]/{total_batches}")

    elapsed = time.time() - t0
    shutdown_all()
    stats = store.get_stats()
    console.print(f"\n[bold green]Index complete in {elapsed:.1f}s[/bold green]")
    console.print(f"  Points: {stats.get('points_count', 'unknown')}")
    console.print(f"  Status: {stats.get('status', 'unknown')}")


def run_incremental(config: Config):
    """Incremental index: only new/changed files since last run."""
    # TODO: track file hashes, use git diff to find changes
    console.print("[yellow]Incremental mode not yet implemented, running full index[/yellow]")
    run_full_index(config)


def export_chunks(config: Config, output_path: str, repo_filter: str | None = None):
    """Export chunks to JSONL for batch embedding on Paperspace/Lightning."""
    repos, files = crawl_all(config)

    if repo_filter:
        files = [f for f in files if f.repo_name == repo_filter]

    all_chunks = []
    for file_info in files:
        chunks = chunk_file(file_info, config)
        all_chunks.extend(chunks)

    with open(output_path, "wb") as f:
        for i, chunk in enumerate(all_chunks):
            record = {
                "id": i,
                "text": chunk.embed_text,
                "content": chunk.content,
                "repo": chunk.repo_name,
                "file": chunk.file_path,
                "language": chunk.language,
                "chunk_type": chunk.chunk_type,
                "name": chunk.name,
                "line_start": chunk.line_start,
                "line_end": chunk.line_end,
                "keywords": chunk.keywords,
            }
            f.write(orjson.dumps(record) + b"\n")

    console.print(f"[green]Exported {len(all_chunks)} chunks to {output_path}[/green]")


def import_embeddings(config: Config, chunks_path: str, embeddings_path: str):
    """Import pre-computed embeddings from batch GPU job and store in Qdrant."""
    store = QdrantStore(config)
    store.ensure_collection()

    # Load chunks
    chunks = []
    with open(chunks_path, "rb") as f:
        for line in f:
            record = orjson.loads(line)
            chunks.append(CodeChunk(
                repo_name=record["repo"],
                file_path=record["file"],
                language=record["language"],
                chunk_type=record["chunk_type"],
                name=record["name"],
                content=record["text"],
                line_start=record["line_start"],
                line_end=record["line_end"],
            ))

    # Load embeddings
    import numpy as np
    embeddings_data = np.load(embeddings_path)
    embeddings = embeddings_data.tolist()

    if len(chunks) != len(embeddings):
        console.print(f"[red]Mismatch: {len(chunks)} chunks vs {len(embeddings)} embeddings[/red]")
        return

    batch_size = config.qdrant_batch_size
    for i in range(0, len(chunks), batch_size):
        store.upsert_chunks(
            chunks[i:i + batch_size],
            embeddings[i:i + batch_size],
        )

    console.print(f"[green]Imported {len(chunks)} vectors into Qdrant[/green]")


@click.group()
def cli():
    """OpenClaw Code Indexer - RAG pipeline for op-dbus chatbot."""
    pass


@cli.command()
@click.option("--repo", default=None, help="Index only this repo")
def full(repo):
    """Run full index of all repos."""
    config = load_config()
    run_full_index(config, repo_filter=repo)


@cli.command()
def incremental():
    """Run incremental index (changed files only)."""
    config = load_config()
    run_incremental(config)


@cli.command("crawl-only")
def crawl_only():
    """Just crawl and list repos + file counts."""
    config = load_config()
    repos, files = crawl_all(config)

    table = Table(title="Discovered Repos")
    table.add_column("Repo")
    table.add_column("Files", justify="right")
    table.add_column("Language")
    table.add_column("GEMINI.md")

    for repo in repos:
        repo_files = [f for f in files if f.repo_name == repo.name]
        table.add_row(
            repo.name,
            str(len(repo_files)),
            repo.language or "-",
            "yes" if repo.gemini else "no",
        )
    console.print(table)


@cli.command("chunk-only")
@click.option("--repo", required=True, help="Repo to chunk")
def chunk_only(repo):
    """Chunk a single repo and display results."""
    config = load_config()
    repos = discover_repos(config)
    target = next((r for r in repos if r.name == repo), None)
    if not target:
        console.print(f"[red]Repo not found: {repo}[/red]")
        return

    files = walk_repo_files(target, config)
    total_chunks = 0
    for file_info in files:
        chunks = chunk_file(file_info, config)
        total_chunks += len(chunks)
        for c in chunks[:3]:  # Show first 3 per file
            console.print(f"  [{c.chunk_type}] {c.name} @ {c.file_path}:{c.line_start}-{c.line_end} ({len(c.content)} chars)")

    console.print(f"\n[green]Total: {total_chunks} chunks from {len(files)} files[/green]")


@cli.command("embed-test")
@click.argument("text")
def embed_test(text):
    """Test embedding a single text."""
    config = load_config()
    embedder = create_embedder(config)
    vec = embedder.embed_single(text)
    console.print(f"[green]Embedding ({len(vec)}d): [{vec[0]:.4f}, {vec[1]:.4f}, ... {vec[-1]:.4f}][/green]")


@cli.command("export-chunks")
@click.option("--output", default="/tmp/openclaw_chunks.jsonl", help="Output JSONL path")
@click.option("--repo", default=None, help="Export only this repo")
def export_cmd(output, repo):
    """Export chunks to JSONL for batch embedding on GPU."""
    config = load_config()
    export_chunks(config, output, repo_filter=repo)


@cli.command("import-embeddings")
@click.option("--chunks", required=True, help="Path to chunks JSONL")
@click.option("--embeddings", required=True, help="Path to embeddings .npy file")
def import_cmd(chunks, embeddings):
    """Import pre-computed embeddings from GPU batch job."""
    config = load_config()
    import_embeddings(config, chunks, embeddings)


@cli.command()
def status():
    """Show index status."""
    config = load_config()
    store = QdrantStore(config)
    stats = store.get_stats()

    console.print(f"\n[bold]Collection: {stats.get('collection', '?')}[/bold]")
    console.print(f"  Points: {stats.get('points_count', 'unknown')}")
    console.print(f"  Vectors: {stats.get('vectors_count', 'unknown')}")
    console.print(f"  Status: {stats.get('status', 'unknown')}")

    repo_stats = store.get_repo_stats()
    if repo_stats:
        table = Table(title="Per-Repo Stats")
        table.add_column("Repo")
        table.add_column("Chunks", justify="right")
        for rs in repo_stats:
            table.add_row(rs["repo"], str(rs["chunks"]))
        console.print(table)


@cli.command()
@click.argument("query")
@click.option("--repo", default=None, help="Filter by repo")
@click.option("--language", default=None, help="Filter by language")
@click.option("--limit", default=5, help="Max results")
def search(query, repo, language, limit):
    """Search indexed code semantically."""
    config = load_config()
    embedder = create_embedder(config)
    store = QdrantStore(config)

    query_vec = embedder.embed_single(query)
    results = store.search(query_vec, repo=repo, language=language, limit=limit)

    if not results:
        console.print("[yellow]No results found[/yellow]")
        return

    for i, r in enumerate(results, 1):
        score = r.get("score", 0)
        console.print(f"\n[bold]#{i}[/bold] (score: {score:.4f}) [{r.get('chunk_type')}] {r.get('name')}")
        console.print(f"  {r.get('repo')}:{r.get('file_path')}:{r.get('line_start')}-{r.get('line_end')}")
        content = r.get("content", "")
        preview = content[:200] + "..." if len(content) > 200 else content
        console.print(f"  {preview}")


if __name__ == "__main__":
    cli()
