"""LSP-aware uniform chunker.

Uses textDocument/documentSymbol to get symbol boundaries from the language
server, then emits fixed-size token windows aligned to those boundaries.
Falls back to sliding window for unsupported languages.
"""

import json
import subprocess
import threading
import time
from dataclasses import dataclass, field
from pathlib import Path

from rich.console import Console

from .config import Config
from .crawler import FileInfo
from .chunker import _extract_keywords

console = Console()

CHARS_PER_TOKEN = 4  # conservative estimate for code

# Map language → LSP server command
LSP_SERVERS = {
    "rust":       ["rust-analyzer"],
    "python":     ["pyright-langserver", "--stdio"],
    "typescript": ["typescript-language-server", "--stdio"],
    "javascript": ["typescript-language-server", "--stdio"],
}


@dataclass
class LspChunk:
    repo_name: str
    file_path: str
    language: str
    chunk_type: str
    name: str
    content: str
    line_start: int
    line_end: int
    keywords: list[str] = field(default_factory=list)
    gemini_focus: str = ""
    gemini_concepts: list[str] = field(default_factory=list)
    # semantic enrichment
    docstring: str = ""
    signature: str = ""
    breadcrumb: str = ""          # e.g. "module::Struct::method"
    called_by: list[str] = field(default_factory=list)
    calls: list[str] = field(default_factory=list)
    related_symbols: list[str] = field(default_factory=list)  # e.g. impl ↔ struct

    @property
    def embed_text(self) -> str:
        parts = [
            f"repo:{self.repo_name} lang:{self.language} type:{self.chunk_type} symbol:{self.name}",
            f"file:{self.file_path} lines:{self.line_start}-{self.line_end}",
        ]
        if self.breadcrumb:
            parts.append(f"path:{self.breadcrumb}")
        if self.signature:
            parts.append(f"signature:{self.signature}")
        if self.docstring:
            parts.append(f"doc:{self.docstring}")
        if self.called_by:
            parts.append(f"called_by:{', '.join(self.called_by)}")
        if self.calls:
            parts.append(f"calls:{', '.join(self.calls)}")
        if self.related_symbols:
            parts.append(f"related:{', '.join(self.related_symbols)}")
        if self.gemini_focus:
            parts.append(f"domain:{self.gemini_focus}")
        if self.gemini_concepts:
            parts.append(f"concepts:{', '.join(self.gemini_concepts)}")
        if self.keywords:
            parts.append(f"keywords:{', '.join(self.keywords)}")
        parts.append("")
        parts.append(self.content)
        return "\n".join(parts)


# ── LSP client (single-file, stdio) ──────────────────────────────────────────

class LspClient:
    """Minimal LSP client: initialize → documentSymbol → shutdown."""

    def __init__(self, cmd: list[str], root_uri: str):
        self._proc = subprocess.Popen(
            cmd,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
        )
        self._seq = 0
        self._root_uri = root_uri
        self._lock = threading.Lock()
        self._initialize()

    def _send(self, msg: dict):
        body = json.dumps(msg)
        header = f"Content-Length: {len(body)}\r\n\r\n"
        self._proc.stdin.write((header + body).encode())
        self._proc.stdin.flush()

    def _recv(self, timeout: float = 10.0) -> dict | None:
        deadline = time.monotonic() + timeout
        buf = b""
        content_length = None

        while time.monotonic() < deadline:
            # Read headers
            if content_length is None:
                chunk = self._proc.stdout.read(1)
                if not chunk:
                    return None
                buf += chunk
                if buf.endswith(b"\r\n\r\n"):
                    for line in buf.split(b"\r\n"):
                        if line.lower().startswith(b"content-length:"):
                            content_length = int(line.split(b":")[1].strip())
                    buf = b""
            else:
                remaining = content_length - len(buf)
                chunk = self._proc.stdout.read(remaining)
                buf += chunk
                if len(buf) >= content_length:
                    return json.loads(buf[:content_length])
        return None

    def _request(self, method: str, params: dict, timeout: float = 10.0) -> dict | None:
        self._seq += 1
        req_id = self._seq
        self._send({"jsonrpc": "2.0", "id": req_id, "method": method, "params": params})
        # Drain until we get our response
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            msg = self._recv(timeout=deadline - time.monotonic())
            if msg is None:
                break
            if msg.get("id") == req_id:
                return msg.get("result")
        return None

    def _notify(self, method: str, params: dict):
        self._send({"jsonrpc": "2.0", "method": method, "params": params})

    def _initialize(self):
        self._request("initialize", {
            "processId": None,
            "rootUri": self._root_uri,
            "capabilities": {
                "textDocument": {
                    "documentSymbol": {
                        "hierarchicalDocumentSymbolSupport": False,
                    }
                }
            },
            "initializationOptions": {},
        }, timeout=15.0)
        self._notify("initialized", {})

    def document_symbols(self, file_uri: str, content: str) -> list[dict]:
        """Open a document and get its symbols."""
        self._notify("textDocument/didOpen", {
            "textDocument": {
                "uri": file_uri,
                "languageId": "rust",
                "version": 1,
                "text": content,
            }
        })
        result = self._request("textDocument/documentSymbol", {
            "textDocument": {"uri": file_uri}
        }, timeout=15.0)
        self._notify("textDocument/didClose", {
            "textDocument": {"uri": file_uri}
        })
        return result or []

    def hover(self, file_uri: str, line: int, character: int) -> dict | None:
        """Get hover info (docstring + signature) at a position."""
        return self._request("textDocument/hover", {
            "textDocument": {"uri": file_uri},
            "position": {"line": line, "character": character},
        }, timeout=5.0)

    def references(self, file_uri: str, line: int, character: int) -> list[dict]:
        """Get all references to the symbol at a position."""
        result = self._request("textDocument/references", {
            "textDocument": {"uri": file_uri},
            "position": {"line": line, "character": character},
            "context": {"includeDeclaration": False},
        }, timeout=5.0)
        return result or []

    def shutdown(self):
        try:
            self._request("shutdown", {}, timeout=5.0)
            self._notify("exit", {})
            self._proc.wait(timeout=3)
        except Exception:
            self._proc.kill()


# ── Per-repo LSP server pool ──────────────────────────────────────────────────

_clients: dict[str, LspClient] = {}  # key: "language:root_uri"


def _get_client(language: str, repo_path: Path) -> LspClient | None:
    cmd = LSP_SERVERS.get(language)
    if not cmd:
        return None
    root_uri = repo_path.as_uri()
    key = f"{language}:{root_uri}"
    if key not in _clients:
        try:
            _clients[key] = LspClient(cmd, root_uri)
        except Exception as e:
            console.print(f"[yellow]LSP start failed ({language}): {e}[/yellow]")
            _clients[key] = None  # type: ignore
    return _clients[key]


def shutdown_all():
    """Call at end of indexing run to clean up LSP servers."""
    for client in _clients.values():
        if client:
            try:
                client.shutdown()
            except Exception:
                pass
    _clients.clear()


# ── Symbol extraction ─────────────────────────────────────────────────────────

# LSP SymbolKind → human name
_SYMBOL_KIND = {
    1: "file", 2: "module", 3: "namespace", 4: "package", 5: "class",
    6: "function", 7: "method", 8: "property", 9: "field", 10: "constructor",
    11: "enum", 12: "interface", 13: "function", 14: "variable", 15: "constant",
    16: "string", 17: "number", 18: "boolean", 19: "array", 20: "object",
    21: "key", 22: "null", 23: "enum_member", 24: "struct", 25: "event",
    26: "operator", 27: "type_parameter",
}

_INTERESTING_KINDS = {5, 6, 7, 10, 11, 12, 13, 24}  # class/fn/method/ctor/enum/iface/struct


@dataclass
class _Symbol:
    name: str
    kind: str
    line_start: int  # 1-based
    line_end: int    # 1-based inclusive
    docstring: str = ""
    signature: str = ""
    parent_name: str = ""  # for breadcrumb


def _parse_hover(hover: dict | None) -> tuple[str, str]:
    """Return (signature, docstring) from a hover result."""
    if not hover:
        return "", ""
    contents = hover.get("contents", {})
    # MarkedString array or MarkupContent
    if isinstance(contents, dict):
        text = contents.get("value", "")
    elif isinstance(contents, list):
        text = "\n".join(c.get("value", c) if isinstance(c, dict) else c for c in contents)
    else:
        text = str(contents)

    # Split code fence (signature) from prose (docstring)
    sig, doc = "", []
    in_fence = False
    for line in text.splitlines():
        if line.startswith("```"):
            in_fence = not in_fence
            continue
        if in_fence:
            if not sig:
                sig = line.strip()
        else:
            stripped = line.strip()
            if stripped:
                doc.append(stripped)
    return sig, " ".join(doc[:3])  # cap docstring at 3 sentences


def _lsp_symbols(file_info: FileInfo, content: str) -> list[_Symbol]:
    """Get symbols via LSP documentSymbol, enriched with hover data."""
    client = _get_client(file_info.language, file_info.path.parent.parent)
    if client is None:
        return []

    file_uri = file_info.path.as_uri()
    try:
        raw = client.document_symbols(file_uri, content)
    except Exception:
        return []

    symbols = []
    lines = content.split("\n")

    # Build a flat list; for hierarchical responses track parent context
    def _collect(items: list[dict], parent_name: str = ""):
        for sym in (items or []):
            kind_id = sym.get("kind", 0)
            loc = sym.get("location", {}).get("range") or sym.get("range", {})
            if not loc:
                continue
            name = sym.get("name", "<anon>")
            ls = loc["start"]["line"] + 1
            le = loc["end"]["line"] + 1

            sig, doc = "", ""
            if kind_id in _INTERESTING_KINDS and client:
                # Hover at the start of the symbol definition
                hover_line = loc["start"]["line"]
                # Find the column of the symbol name in that line
                src_line = lines[hover_line] if hover_line < len(lines) else ""
                col = src_line.find(name)
                col = max(col, 0)
                hover_result = client.hover(file_uri, hover_line, col)
                sig, doc = _parse_hover(hover_result)

            if kind_id in _INTERESTING_KINDS:
                symbols.append(_Symbol(
                    name=name,
                    kind=_SYMBOL_KIND.get(kind_id, "symbol"),
                    line_start=ls,
                    line_end=le,
                    docstring=doc,
                    signature=sig,
                    parent_name=parent_name,
                ))
            # Recurse into children (hierarchical documentSymbol)
            _collect(sym.get("children", []), parent_name=name)

    _collect(raw)
    return symbols


# ── Uniform windowed chunking ─────────────────────────────────────────────────

def _chunk_windows(
    file_info: FileInfo,
    lines: list[str],
    symbols: list[_Symbol],
    config: Config,
) -> list[LspChunk]:
    """Emit fixed-size token windows, labelled with dominant LSP symbol."""
    max_chars = config.max_chunk_tokens * CHARS_PER_TOKEN
    overlap_chars = config.chunk_overlap_tokens * CHARS_PER_TOKEN

    # line (1-based) → symbol index
    line_sym: dict[int, int] = {}
    for idx, sym in enumerate(symbols):
        for ln in range(sym.line_start, sym.line_end + 1):
            line_sym[ln] = idx

    # Cross-reference: map struct/class names → their impl/method chunks
    # Group symbols by name to find struct ↔ impl pairs
    name_kinds: dict[str, list[str]] = {}
    for sym in symbols:
        name_kinds.setdefault(sym.name, []).append(sym.kind)

    def _related(sym: _Symbol) -> list[str]:
        """Symbols with the same name but different kind (e.g. struct + impl)."""
        return [f"{k}:{sym.name}" for k in name_kinds.get(sym.name, []) if k != sym.kind]

    chunks: list[LspChunk] = []
    i = 0  # 0-based line index

    while i < len(lines):
        j = i
        chars = 0
        while j < len(lines) and chars + len(lines[j]) + 1 <= max_chars:
            chars += len(lines[j]) + 1
            j += 1
        if j == i:  # single oversized line
            j = i + 1

        text = "\n".join(lines[i:j])
        if not text.strip():
            i = j
            continue

        line_start = i + 1
        line_end = j  # last line number (1-based)

        # Dominant symbol by line coverage
        votes: dict[int, int] = {}
        for ln in range(line_start, line_end + 1):
            if ln in line_sym:
                s = line_sym[ln]
                votes[s] = votes.get(s, 0) + 1

        if votes:
            best = max(votes, key=votes.__getitem__)
            sym = symbols[best]
            name, chunk_type = sym.name, sym.kind
            # Breadcrumb: parent::name or just name
            breadcrumb = f"{sym.parent_name}::{name}" if sym.parent_name else name
            # Prepend repo + file module path
            module = file_info.relative_path.replace("/", "::").removesuffix(".rs").removesuffix(".py")
            breadcrumb = f"{file_info.repo_name}::{module}::{breadcrumb}"
            docstring = sym.docstring
            signature = sym.signature
            related = _related(sym)
        else:
            name, chunk_type = f"block@L{line_start}", "block"
            breadcrumb = docstring = signature = ""
            related = []

        chunks.append(LspChunk(
            repo_name=file_info.repo_name,
            file_path=file_info.relative_path,
            language=file_info.language,
            chunk_type=chunk_type,
            name=name,
            content=text,
            line_start=line_start,
            line_end=line_end,
            keywords=_extract_keywords(text, file_info.language, name),
            breadcrumb=breadcrumb,
            docstring=docstring,
            signature=signature,
            related_symbols=related,
        ))

        # Overlap: step back by overlap_chars worth of lines
        step = max(1, j - i - max(1, overlap_chars // max(1, chars // max(1, j - i))))
        i += step

    return chunks


# ── Public entry point ────────────────────────────────────────────────────────

def chunk_file_lsp(file_info: FileInfo, config: Config) -> list[LspChunk]:
    """Chunk a file using LSP symbols + uniform token windows."""
    if file_info.language == "pdf":
        from .chunker import chunk_pdf
        old = chunk_pdf(file_info)
        return [LspChunk(
            repo_name=c.repo_name, file_path=c.file_path, language=c.language,
            chunk_type=c.chunk_type, name=c.name, content=c.content,
            line_start=c.line_start, line_end=c.line_end, keywords=c.keywords,
        ) for c in old]

    try:
        content = file_info.path.read_text(errors="replace")
    except OSError:
        return []

    if not content.strip():
        return []

    lines = content.split("\n")
    symbols = _lsp_symbols(file_info, content)
    chunks = _chunk_windows(file_info, lines, symbols, config)

    # Enrich call graph: for each symbol, find references within this file
    client = _get_client(file_info.language, file_info.path.parent.parent)
    if client and symbols:
        file_uri = file_info.path.as_uri()
        # name → set of names that reference it (called_by)
        callers: dict[str, set[str]] = {s.name: set() for s in symbols}
        callees: dict[str, set[str]] = {s.name: set() for s in symbols}
        # line → symbol name (for resolving which symbol a reference lands in)
        line_to_sym = {ln: s.name for s in symbols for ln in range(s.line_start, s.line_end + 1)}

        for sym in symbols:
            if sym.kind not in ("function", "method", "constructor"):
                continue
            hover_line = sym.line_start - 1
            src_line = lines[hover_line] if hover_line < len(lines) else ""
            col = max(src_line.find(sym.name), 0)
            refs = client.references(file_uri, hover_line, col)
            for ref in refs:
                ref_line = ref.get("range", {}).get("start", {}).get("line", -1) + 1
                caller_name = line_to_sym.get(ref_line)
                if caller_name and caller_name != sym.name:
                    callers[sym.name].add(caller_name)
                    callees[caller_name].add(sym.name)

        # Attach to chunks
        for chunk in chunks:
            if chunk.name in callers:
                chunk.called_by = sorted(callers[chunk.name])
            if chunk.name in callees:
                chunk.calls = sorted(callees[chunk.name])

    return chunks
