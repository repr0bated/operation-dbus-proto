#!/usr/bin/env python3
"""Create cognitive notebook + ingest ~/.notebooklm-sources via gRPC (uv-managed).

Usage (from scripts/uv-tools):
  uv run python notebook_sync.py create
  uv run python notebook_sync.py ingest
  uv run python notebook_sync.py create-and-ingest

Or from repo root:
  uv run --project scripts/uv-tools python scripts/uv-tools/notebook_sync.py create-and-ingest
"""
from __future__ import annotations

import argparse
import os
import sys
from pathlib import Path

# generated stubs next to this file
sys.path.insert(0, str(Path(__file__).resolve().parent / "gen"))

import cognitive_pb2 as pb  # type: ignore
import cognitive_pb2_grpc as pbg  # type: ignore
import grpc

HOME = Path.home()
SOURCES = Path(os.environ.get("NOTEBOOKLM_SOURCES_DIR", HOME / ".notebooklm-sources"))
DEFAULT_TITLE = os.environ.get(
    "NOTEBOOK_TITLE", "3tched LLM Sessions / Control Plane"
)
GRPC_ADDR = os.environ.get("COGNITIVE_MCP_GRPC", "10.200.0.2:50052")
SLED = Path(os.environ.get("OP_IDENTITY_SLED", "/dev/shm/plugin_schema.dat"))


def read_sled(path: Path = SLED) -> tuple[str, str]:
    data = path.read_bytes()
    if len(data) < 88:
        raise SystemExit(f"identity sled too short: {path} ({len(data)} bytes)")
    footprint, trace = data[40:72], data[72:88]
    if all(b == 0 for b in footprint):
        raise SystemExit("identity sled footprint empty — seed op-identity-sled")
    return footprint.hex(), trace.hex()


def meta() -> tuple[tuple[str, str], ...]:
    fp, tr = read_sled()
    return (
        ("x-ghostbridge-footprint", fp),
        ("x-ghostbridge-trace-id", tr),
    )


def stub() -> pbg.CognitiveToolServiceStub:
    return pbg.CognitiveToolServiceStub(grpc.insecure_channel(GRPC_ADDR))


def create(title: str = DEFAULT_TITLE) -> str:
    s = stub()
    desc = (
        "Agent CLI conversation backfill (factory, codex, claude, cursor, opencode, "
        "agy, grok, kilo) + or-fusion from ~/.notebooklm-sources"
    )
    resp = s.CreateNotebook(
        pb.CreateNotebookRequest(title=title, description=desc, kind="project"),
        metadata=meta(),
        timeout=30,
    )
    nb = resp.notebook
    uuid = nb.id
    SOURCES.mkdir(parents=True, exist_ok=True)
    (SOURCES / "NOTEBOOK_ID").write_text(uuid + "\n")
    # Quote title fields so runit can `. notebook.env` safely
    (SOURCES / "notebook.env").write_text(
        f"OP_NOTEBOOK_ID={uuid}\n"
        f"NOTEBOOKLM_NOTEBOOK_ID={uuid}\n"
        f"OP_NOTEBOOK_SOURCE_KEY='{title.replace(chr(39), chr(39)+chr(39))}'\n"
        f"NOTEBOOK_TITLE='{title.replace(chr(39), chr(39)+chr(39))}'\n"
        f"NOTEBOOK_NAME='project:{title.replace(chr(39), chr(39)+chr(39))}'\n"
    )
    print(f"created id={uuid} name={nb.name}")
    return uuid


def ingest(title: str | None = None) -> None:
    """AddSource every *.md under SOURCES.

    Cognitive AddSource namespaces as project:{notebook_id}, so pass the *title*
    (not the UUID) as notebook_id.
    """
    s = stub()
    title = title or DEFAULT_TITLE
    key = title
    mds = sorted(SOURCES.rglob("*.md"))

    # Read the identity sled once, up front. Every source sends the same
    # metadata, so re-reading it per item was pure waste — and worse, it turned
    # a single unreadable file into one failure per source. That printed
    # "added=0 failed=335", which reads like 335 bad documents rather than one
    # broken precondition, and it stayed invisible for eight days because the
    # per-item errors were throttled to five and the summary logged at INFO.
    try:
        md = meta()
    except OSError as e:
        raise SystemExit(
            f"identity sled unreadable: {SLED}: {e}\n"
            f"ingest aborted before contacting {GRPC_ADDR}; "
            f"{len(mds)} sources left untouched"
        ) from e

    added = failed = skipped = 0
    for p in mds:
        try:
            text = p.read_text(encoding="utf-8", errors="replace")
        except Exception:
            failed += 1
            continue
        if len(text.strip()) < 20:
            skipped += 1
            continue
        if len(text) > 180_000:
            text = text[:180_000] + "\n…[truncated]"
        rel = str(p.relative_to(SOURCES))
        try:
            r = s.AddSource(
                pb.AddSourceRequest(
                    notebook_id=key,
                    source_type="text",
                    content=text,
                    title=rel,
                    tags=["llm-session", "uv-sync", p.parent.name],
                ),
                metadata=md,
                timeout=60,
            )
            if r.success:
                added += 1
            else:
                failed += 1
        except Exception as e:
            failed += 1
            if failed <= 5:
                print(f"err {rel}: {e}", file=sys.stderr)
    if failed > 5:
        print(f"… {failed - 5} further errors not shown", file=sys.stderr)
    print(f"ingest added={added} failed={failed} skipped={skipped} key={key!r}")

    # A cycle that adds nothing while sources were available is a failure, not
    # a quiet INFO line. Exiting non-zero lets the runit wrapper's
    # `|| log "ingest failed"` actually fire.
    if added == 0 and failed > 0:
        raise SystemExit(
            f"ingest added nothing: {failed} of {len(mds)} sources failed"
        )


def main() -> int:
    ap = argparse.ArgumentParser(description="Notebook create/ingest via uv + cognitive gRPC")
    ap.add_argument(
        "cmd",
        choices=["create", "ingest", "create-and-ingest", "show"],
        help="create notebook, ingest md sources, both, or show saved id",
    )
    ap.add_argument("--title", default=DEFAULT_TITLE)
    args = ap.parse_args()

    if args.cmd == "show":
        p = SOURCES / "notebook.env"
        print(p.read_text() if p.is_file() else "no notebook.env yet")
        return 0
    if args.cmd == "create":
        create(args.title)
        return 0
    if args.cmd == "ingest":
        ingest(args.title)
        return 0
    if args.cmd == "create-and-ingest":
        create(args.title)
        ingest(args.title)
        return 0
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
