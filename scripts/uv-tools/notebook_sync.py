#!/usr/bin/env python3
"""Create a NotebookLM notebook and ingest Markdown through PluginV1.Call.

The only execution door is the session-bus object owned by ``op-grpc-bridge``::

    org.opdbus.v1.plugins
    /org/opdbus/v1/plugins/cognitive_mcp
    org.opdbus.v1.PluginV1.Call("invoke_tool", JSON)

The D-Bus path deliberately carries no caller-provided identity headers. Its current
policy is local bus access plus the explicit ``*`` capability grants, and the bridge
records the D-Bus actor in the accountability chain. Reading an identity projection
here and replaying it as metadata would be self-assertion, not authentication.

Usage (from scripts/uv-tools):
  uv run python notebook_sync.py create
  uv run python notebook_sync.py ingest
  uv run python notebook_sync.py create-and-ingest

Or from the repository root:
  uv run --project scripts/uv-tools python scripts/uv-tools/notebook_sync.py create-and-ingest
"""

from __future__ import annotations

import argparse
import ast
import json
import os
import re
import subprocess
import sys
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

BRIDGE_BUS_NAME = "org.opdbus.v1.plugins"
PLUGIN_INTERFACE = "org.opdbus.v1.PluginV1"
COGNITIVE_OBJECT_PATH = "/org/opdbus/v1/plugins/cognitive_mcp"
DEFAULT_SESSION_BUS_ADDRESS = "unix:path=/run/opdbus/session-bus.sock"

CREATE_NOTEBOOK_TOOL = "create_notebook"
ADD_SOURCE_TEXT_TOOL = "add_source_text"
MAX_SOURCE_CHARS = 180_000

HOME = Path.home()
SOURCES = Path(os.environ.get("NOTEBOOKLM_SOURCES_DIR", HOME / ".notebooklm-sources"))
DEFAULT_TITLE = os.environ.get(
    "NOTEBOOK_TITLE", "3tched LLM Sessions / Control Plane"
)


class NotebookSyncError(RuntimeError):
    """The canonical D-Bus call or its response contract failed."""


def session_bus_address(environ: Mapping[str, str] | None = None) -> str:
    """Resolve the bridge session bus without consulting legacy network settings."""

    env = os.environ if environ is None else environ
    return (
        env.get("DBUS_SESSION_BUS_ADDRESS")
        or env.get("COGNITIVE_MCP_BUS_ADDRESS")
        or DEFAULT_SESSION_BUS_ADDRESS
    )


def compact_json(value: Any) -> str:
    """Serialize stable, single-line JSON for the D-Bus ``ss`` argument."""

    return json.dumps(value, ensure_ascii=False, separators=(",", ":"), sort_keys=True)


def build_plugin_call_command(
    method: str,
    arguments: Mapping[str, Any],
    *,
    bus_address: str | None = None,
) -> list[str]:
    """Build an argv-only PluginV1.Call command; no shell quoting is involved."""

    address = bus_address or session_bus_address()
    return [
        "busctl",
        f"--address={address}",
        "call",
        BRIDGE_BUS_NAME,
        COGNITIVE_OBJECT_PATH,
        PLUGIN_INTERFACE,
        "Call",
        "ss",
        method,
        compact_json(arguments),
    ]


def parse_busctl_string(stdout: str) -> str:
    """Decode the single D-Bus string emitted by ``busctl call ... Call``."""

    parts = stdout.strip().split(None, 1)
    if len(parts) != 2 or parts[0] != "s":
        raise NotebookSyncError(f"expected one D-Bus string, got: {stdout!r}")
    try:
        value = ast.literal_eval(parts[1])
    except (SyntaxError, ValueError) as error:
        raise NotebookSyncError(f"invalid busctl string encoding: {stdout!r}") from error
    if not isinstance(value, str):
        raise NotebookSyncError(f"busctl returned a non-string value: {value!r}")
    return value


def parse_call_reply(stdout: str) -> Any:
    """Decode and validate the bridge accountability envelope."""

    body = parse_busctl_string(stdout)
    try:
        envelope = json.loads(body)
    except json.JSONDecodeError as error:
        raise NotebookSyncError(f"PluginV1.Call returned invalid JSON: {body!r}") from error
    if not isinstance(envelope, dict):
        raise NotebookSyncError("PluginV1.Call returned a non-object envelope")
    if envelope.get("success") is not True:
        raise NotebookSyncError(f"PluginV1.Call failed: {envelope!r}")
    if "result" not in envelope:
        raise NotebookSyncError("PluginV1.Call envelope has no result")
    return envelope["result"]


def invoke_tool(
    tool_name: str,
    arguments: Mapping[str, Any],
    *,
    timeout: int,
    bus_address: str | None = None,
) -> Any:
    """Invoke one live cognitive tool through the schema method ``invoke_tool``."""

    call_args = {"tool_name": tool_name, "arguments": dict(arguments)}
    command = build_plugin_call_command(
        "invoke_tool", call_args, bus_address=bus_address
    )
    try:
        completed = subprocess.run(
            command,
            capture_output=True,
            check=False,
            text=True,
            timeout=timeout,
        )
    except FileNotFoundError as error:
        raise NotebookSyncError("busctl is required for session-bus dispatch") from error
    except subprocess.TimeoutExpired as error:
        raise NotebookSyncError(
            f"{tool_name} timed out after {timeout}s through PluginV1.Call"
        ) from error
    if completed.returncode != 0:
        detail = (completed.stderr or completed.stdout).strip()
        raise NotebookSyncError(
            f"PluginV1.Call({tool_name}) failed with exit {completed.returncode}: {detail}"
        )
    return parse_call_reply(completed.stdout)


def unwrap_tool_payload(result: Any) -> Any:
    """Unwrap MCP structured/text content while preserving plain JSON results."""

    if not isinstance(result, dict):
        return result
    if result.get("isError") is True:
        raise NotebookSyncError(f"tool returned isError: {result!r}")

    structured = result.get("structuredContent")
    if structured is not None:
        return structured

    content = result.get("content")
    if not isinstance(content, list):
        return result

    text_blocks: list[str] = []
    for block in content:
        if not isinstance(block, dict) or block.get("type") != "text":
            continue
        text = block.get("text")
        if not isinstance(text, str):
            continue
        try:
            return json.loads(text)
        except json.JSONDecodeError:
            text_blocks.append(text)
    return "\n".join(text_blocks) if text_blocks else result


def _find_notebook_id(value: Any) -> str | None:
    if isinstance(value, dict):
        for key in ("notebook_id", "notebookId", "id"):
            candidate = value.get(key)
            if isinstance(candidate, str) and candidate.strip():
                return candidate.strip()
        notebook = value.get("notebook")
        candidate = _find_notebook_id(notebook)
        if candidate:
            return candidate
        for nested in value.values():
            candidate = _find_notebook_id(nested)
            if candidate:
                return candidate
    elif isinstance(value, Sequence) and not isinstance(value, (str, bytes, bytearray)):
        for nested in value:
            candidate = _find_notebook_id(nested)
            if candidate:
                return candidate
    elif isinstance(value, str):
        match = re.search(
            r"(?:notebook[ _-]*id|\bid)\s*[:=]\s*['\"]?"
            r"([A-Za-z0-9][A-Za-z0-9._:-]{5,})",
            value,
            flags=re.IGNORECASE,
        )
        if match:
            return match.group(1)
        uuid_match = re.search(
            r"\b[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-"
            r"[89ab][0-9a-f]{3}-[0-9a-f]{12}\b",
            value,
            flags=re.IGNORECASE,
        )
        if uuid_match:
            return uuid_match.group(0)
    return None


def extract_notebook_id(result: Any) -> str:
    payload = unwrap_tool_payload(result)
    notebook_id = _find_notebook_id(payload)
    if not notebook_id:
        raise NotebookSyncError(
            f"{CREATE_NOTEBOOK_TOOL} returned no notebook identifier: {payload!r}"
        )
    return notebook_id


def ensure_tool_success(tool_name: str, result: Any) -> Any:
    payload = unwrap_tool_payload(result)
    if isinstance(payload, dict):
        for flag in ("success", "ok", "added"):
            if payload.get(flag) is False:
                raise NotebookSyncError(f"{tool_name} reported failure: {payload!r}")
        status = payload.get("status")
        if isinstance(status, str) and status.lower() in {"error", "failed", "failure"}:
            raise NotebookSyncError(f"{tool_name} reported failure: {payload!r}")
    return payload


def shell_single_quote(value: str) -> str:
    """Quote one value for the POSIX-shell-compatible notebook.env file."""

    return "'" + value.replace("'", "'\"'\"'") + "'"


def write_notebook_state(notebook_id: str, title: str, sources: Path = SOURCES) -> None:
    sources.mkdir(parents=True, exist_ok=True)
    (sources / "NOTEBOOK_ID").write_text(notebook_id + "\n", encoding="utf-8")
    quoted_title = shell_single_quote(title)
    (sources / "notebook.env").write_text(
        f"OP_NOTEBOOK_ID={notebook_id}\n"
        f"NOTEBOOKLM_NOTEBOOK_ID={notebook_id}\n"
        f"OP_NOTEBOOK_SOURCE_KEY={quoted_title}\n"
        f"NOTEBOOK_TITLE={quoted_title}\n"
        f"NOTEBOOK_NAME={shell_single_quote(f'project:{title}')}\n",
        encoding="utf-8",
    )


def saved_notebook_id(
    sources: Path = SOURCES,
    environ: Mapping[str, str] | None = None,
) -> str:
    env = os.environ if environ is None else environ
    for key in ("OP_NOTEBOOK_ID", "NOTEBOOKLM_NOTEBOOK_ID"):
        candidate = env.get(key, "").strip()
        if candidate:
            return candidate

    id_file = sources / "NOTEBOOK_ID"
    if id_file.is_file():
        candidate = id_file.read_text(encoding="utf-8").strip()
        if candidate:
            return candidate

    env_file = sources / "notebook.env"
    if env_file.is_file():
        for line in env_file.read_text(encoding="utf-8").splitlines():
            key, separator, raw_value = line.partition("=")
            if separator and key in {"OP_NOTEBOOK_ID", "NOTEBOOKLM_NOTEBOOK_ID"}:
                candidate = raw_value.strip().strip("'\"")
                if candidate:
                    return candidate
    raise NotebookSyncError("no saved notebook id; run notebook-sync create first")


def create(title: str = DEFAULT_TITLE) -> str:
    result = invoke_tool(CREATE_NOTEBOOK_TOOL, {"title": title}, timeout=60)
    notebook_id = extract_notebook_id(result)
    write_notebook_state(notebook_id, title)
    print(f"created id={notebook_id} title={title!r}")
    return notebook_id


def ingest(title: str | None = None, notebook_id: str | None = None) -> None:
    """Add every non-trivial Markdown source through ``add_source_text``."""

    title = title or DEFAULT_TITLE
    target_id = notebook_id or saved_notebook_id()
    markdown_files = sorted(SOURCES.rglob("*.md"))
    added = failed = skipped = 0
    for path in markdown_files:
        try:
            text = path.read_text(encoding="utf-8", errors="replace")
        except OSError as error:
            failed += 1
            if failed <= 5:
                print(f"err {path}: {error}", file=sys.stderr)
            continue
        if len(text.strip()) < 20:
            skipped += 1
            continue
        if len(text) > MAX_SOURCE_CHARS:
            text = text[:MAX_SOURCE_CHARS] + "\n…[truncated]"
        relative_path = str(path.relative_to(SOURCES))
        arguments = {
            "notebook_id": target_id,
            "text": text,
            "title": relative_path,
        }
        try:
            result = invoke_tool(ADD_SOURCE_TEXT_TOOL, arguments, timeout=90)
            ensure_tool_success(ADD_SOURCE_TEXT_TOOL, result)
            added += 1
        except (NotebookSyncError, OSError) as error:
            failed += 1
            if failed <= 5:
                print(f"err {relative_path}: {error}", file=sys.stderr)
    print(
        f"ingest added={added} failed={failed} skipped={skipped} "
        f"notebook_id={target_id!r} title={title!r}"
    )


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Notebook create/ingest through session-bus PluginV1.Call"
    )
    parser.add_argument(
        "cmd",
        choices=["create", "ingest", "create-and-ingest", "show"],
        help="create notebook, ingest Markdown, both, or show saved state",
    )
    parser.add_argument("--title", default=DEFAULT_TITLE)
    args = parser.parse_args()

    if args.cmd == "show":
        state = SOURCES / "notebook.env"
        print(state.read_text() if state.is_file() else "no notebook.env yet")
        return 0
    if args.cmd == "create":
        create(args.title)
        return 0
    if args.cmd == "ingest":
        ingest(args.title)
        return 0
    if args.cmd == "create-and-ingest":
        notebook_id = create(args.title)
        ingest(args.title, notebook_id)
        return 0
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
