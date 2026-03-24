"""Language-aware code chunking using tree-sitter with keyword enrichment."""

import re
from dataclasses import dataclass, field
from pathlib import Path

from rich.console import Console

from .config import Config
from .crawler import FileInfo

console = Console()

# Tree-sitter lazy imports
_PARSERS: dict[str, object] = {}


@dataclass
class CodeChunk:
    """A chunk of code with metadata and extracted keywords."""
    repo_name: str
    file_path: str
    language: str
    chunk_type: str  # function, struct, class, module, block, gemini_context
    name: str  # function/struct/class name, or section header
    content: str
    line_start: int
    line_end: int
    keywords: list[str] = field(default_factory=list)
    # Set by job.py from GEMINI.md context
    gemini_focus: str = ""
    gemini_concepts: list[str] = field(default_factory=list)

    @property
    def embed_text(self) -> str:
        """Build enriched text for embedding: metadata prefix + code.

        Prepending structured context dramatically improves semantic search
        because the embedding model sees domain keywords alongside the code.
        """
        parts = []

        # Structured metadata header
        parts.append(f"repo:{self.repo_name} language:{self.language} type:{self.chunk_type} name:{self.name}")
        parts.append(f"file:{self.file_path}")

        # GEMINI.md context (repo-level domain knowledge)
        if self.gemini_focus:
            parts.append(f"domain:{self.gemini_focus}")
        if self.gemini_concepts:
            parts.append(f"concepts:{', '.join(self.gemini_concepts)}")

        # Extracted keywords from the code itself
        if self.keywords:
            parts.append(f"keywords:{', '.join(self.keywords)}")

        parts.append("")  # blank line separator
        parts.append(self.content)

        return "\n".join(parts)


def _extract_keywords(content: str, language: str, name: str) -> list[str]:
    """Extract meaningful keywords from code content.

    Pulls out identifiers, type names, trait/interface names, imports,
    and splits snake_case/camelCase into individual words.
    """
    keywords = set()

    # Add the chunk name, split on _ and camelCase
    if name and name != "<anonymous>":
        keywords.update(_split_identifier(name))

    if language == "rust":
        # Imports: use foo::bar::Baz
        for m in re.finditer(r'use\s+([\w:]+)', content):
            path = m.group(1)
            # Last segment is most meaningful
            parts = path.split("::")
            for p in parts[-2:]:
                keywords.update(_split_identifier(p))

        # Type annotations: -> ReturnType, : SomeType
        for m in re.finditer(r'(?:->|:\s*&?\s*(?:mut\s+)?)\s*([A-Z]\w+)', content):
            keywords.update(_split_identifier(m.group(1)))

        # Trait bounds: impl Trait, where T: Trait
        for m in re.finditer(r'(?:impl|dyn)\s+([A-Z]\w+)', content):
            keywords.update(_split_identifier(m.group(1)))

        # Struct/enum field names
        for m in re.finditer(r'(\w+)\s*:', content):
            if not m.group(1)[0].isupper():
                keywords.update(_split_identifier(m.group(1)))

        # Macro calls: foo!()
        for m in re.finditer(r'(\w+)!\s*[(\[]', content):
            keywords.add(m.group(1))

    elif language == "python":
        # Imports
        for m in re.finditer(r'(?:from|import)\s+([\w.]+)', content):
            parts = m.group(1).split(".")
            for p in parts[-2:]:
                keywords.update(_split_identifier(p))

        # Type hints
        for m in re.finditer(r':\s*([A-Z]\w+)', content):
            keywords.update(_split_identifier(m.group(1)))

        # Decorators
        for m in re.finditer(r'@(\w+)', content):
            keywords.update(_split_identifier(m.group(1)))

    elif language in ("typescript", "javascript"):
        # Imports
        for m in re.finditer(r'(?:from|import)\s+[\'"]([^"\']+)[\'"]', content):
            parts = m.group(1).split("/")
            keywords.update(_split_identifier(parts[-1]))

        # Type annotations
        for m in re.finditer(r':\s*([A-Z]\w+)', content):
            keywords.update(_split_identifier(m.group(1)))

        # Interface/type names
        for m in re.finditer(r'(?:interface|type)\s+(\w+)', content):
            keywords.update(_split_identifier(m.group(1)))

    elif language == "go":
        # Package imports
        for m in re.finditer(r'"([\w/.-]+)"', content):
            parts = m.group(1).split("/")
            keywords.update(_split_identifier(parts[-1]))

        # Type names
        for m in re.finditer(r'(?:type|func.*?)\s+([A-Z]\w+)', content):
            keywords.update(_split_identifier(m.group(1)))

    elif language == "protobuf":
        # Service, message, rpc names
        for m in re.finditer(r'(?:service|message|rpc)\s+(\w+)', content):
            keywords.update(_split_identifier(m.group(1)))

    # Universal: any CamelCase or SCREAMING_CASE identifiers
    for m in re.finditer(r'\b([A-Z][a-z]+(?:[A-Z][a-z]+)+)\b', content):
        keywords.update(_split_identifier(m.group(1)))

    # Filter noise
    noise = {"self", "str", "string", "int", "bool", "true", "false", "none",
             "pub", "fn", "let", "mut", "const", "async", "await", "return",
             "def", "class", "import", "from", "if", "else", "for", "in",
             "var", "func", "type", "struct", "enum", "impl", "trait",
             "the", "a", "an", "is", "it", "to", "of", "and", "or"}
    keywords = {k.lower() for k in keywords if len(k) > 2} - noise

    return sorted(keywords)


def _split_identifier(name: str) -> list[str]:
    """Split snake_case and camelCase identifiers into words."""
    words = []
    # Handle snake_case
    if "_" in name:
        words.extend(name.split("_"))
    # Handle camelCase / PascalCase
    parts = re.sub(r'([A-Z])', r' \1', name).split()
    words.extend(parts)
    # Also add the full name
    words.append(name)
    return [w.strip() for w in words if w.strip()]


def _get_parser(language: str):
    """Lazy-load tree-sitter parser for a language."""
    if language in _PARSERS:
        return _PARSERS[language]

    try:
        import tree_sitter_rust
        import tree_sitter_python
        import tree_sitter_typescript
        import tree_sitter_go
        from tree_sitter import Language, Parser

        lang_map = {
            "rust": tree_sitter_rust.language(),
            "python": tree_sitter_python.language(),
            "typescript": tree_sitter_typescript.language_typescript(),
            "javascript": tree_sitter_typescript.language_typescript(),
            "go": tree_sitter_go.language(),
        }

        if language not in lang_map:
            _PARSERS[language] = None
            return None

        parser = Parser(Language(lang_map[language]))
        _PARSERS[language] = parser
        return parser
    except ImportError:
        _PARSERS[language] = None
        return None


# Node types to extract per language
EXTRACT_NODES = {
    "rust": {
        "function_item": "function",
        "struct_item": "struct",
        "enum_item": "enum",
        "impl_item": "impl",
        "trait_item": "trait",
        "mod_item": "module",
        "type_item": "type",
    },
    "python": {
        "function_definition": "function",
        "class_definition": "class",
        "decorated_definition": "function",
    },
    "typescript": {
        "function_declaration": "function",
        "class_declaration": "class",
        "interface_declaration": "interface",
        "type_alias_declaration": "type",
        "arrow_function": "function",
        "method_definition": "function",
    },
    "javascript": {
        "function_declaration": "function",
        "class_declaration": "class",
        "arrow_function": "function",
        "method_definition": "function",
    },
    "go": {
        "function_declaration": "function",
        "method_declaration": "function",
        "type_declaration": "type",
        "interface_type": "interface",
    },
}


def _extract_name(node, source_bytes: bytes, language: str) -> str:
    """Extract the name identifier from an AST node."""
    # Look for name/identifier child
    for child in node.children:
        if child.type in ("identifier", "type_identifier", "field_identifier"):
            return source_bytes[child.start_byte:child.end_byte].decode("utf-8", errors="replace")
        if child.type == "name" and child.children:
            return source_bytes[child.start_byte:child.end_byte].decode("utf-8", errors="replace")
    # For impl blocks, get the type name
    if node.type == "impl_item":
        for child in node.children:
            if child.type == "type_identifier":
                return f"impl {source_bytes[child.start_byte:child.end_byte].decode('utf-8', errors='replace')}"
    return "<anonymous>"


def chunk_with_treesitter(file_info: FileInfo, content: str) -> list[CodeChunk]:
    """Chunk a file using tree-sitter AST parsing."""
    parser = _get_parser(file_info.language)
    if parser is None:
        return chunk_sliding_window(file_info, content)

    source_bytes = content.encode("utf-8")
    tree = parser.parse(source_bytes)

    node_types = EXTRACT_NODES.get(file_info.language, {})
    chunks = []

    def visit(node):
        if node.type in node_types:
            chunk_type = node_types[node.type]
            name = _extract_name(node, source_bytes, file_info.language)
            text = source_bytes[node.start_byte:node.end_byte].decode("utf-8", errors="replace")

            # Skip tiny chunks (<3 lines)
            if text.count("\n") >= 2:
                chunks.append(CodeChunk(
                    repo_name=file_info.repo_name,
                    file_path=file_info.relative_path,
                    language=file_info.language,
                    chunk_type=chunk_type,
                    name=name,
                    content=text,
                    line_start=node.start_point[0] + 1,
                    line_end=node.end_point[0] + 1,
                    keywords=_extract_keywords(text, file_info.language, name),
                ))

        for child in node.children:
            visit(child)

    visit(tree.root_node)

    # If tree-sitter found nothing meaningful, fall back to sliding window
    if not chunks:
        return chunk_sliding_window(file_info, content)

    return chunks


def chunk_sliding_window(file_info: FileInfo, content: str, max_lines: int = 60, overlap: int = 10) -> list[CodeChunk]:
    """Fallback: fixed-size sliding window chunks."""
    lines = content.split("\n")
    chunks = []
    i = 0

    while i < len(lines):
        end = min(i + max_lines, len(lines))
        chunk_lines = lines[i:end]
        text = "\n".join(chunk_lines)

        if text.strip():
            chunks.append(CodeChunk(
                repo_name=file_info.repo_name,
                file_path=file_info.relative_path,
                language=file_info.language,
                chunk_type="block",
                name=f"block@L{i+1}",
                content=text,
                line_start=i + 1,
                line_end=end,
                keywords=_extract_keywords(text, file_info.language, f"block@L{i+1}"),
            ))

        i += max_lines - overlap

    return chunks


def chunk_markdown(file_info: FileInfo, content: str) -> list[CodeChunk]:
    """Chunk markdown by headers."""
    import re
    chunks = []
    sections = re.split(r"(^#{1,3}\s+.+$)", content, flags=re.MULTILINE)

    current_header = "top"
    current_content = []
    line_num = 1

    for section in sections:
        if re.match(r"^#{1,3}\s+", section):
            # Save previous section
            if current_content:
                text = "\n".join(current_content)
                if text.strip():
                    chunks.append(CodeChunk(
                        repo_name=file_info.repo_name,
                        file_path=file_info.relative_path,
                        language="markdown",
                        chunk_type="section",
                        name=current_header,
                        content=text,
                        line_start=line_num,
                        line_end=line_num + len(current_content),
                        keywords=_extract_keywords(text, "markdown", current_header),
                    ))
            current_header = section.strip().lstrip("#").strip()
            line_num += len(current_content) + 1
            current_content = [section]
        else:
            current_content.extend(section.split("\n"))

    # Last section
    if current_content:
        text = "\n".join(current_content)
        if text.strip():
            chunks.append(CodeChunk(
                repo_name=file_info.repo_name,
                file_path=file_info.relative_path,
                language="markdown",
                chunk_type="section",
                name=current_header,
                content=text,
                line_start=line_num,
                line_end=line_num + len(current_content),
                keywords=_extract_keywords(text, "markdown", current_header),
            ))

    return chunks


def chunk_proto(file_info: FileInfo, content: str) -> list[CodeChunk]:
    """Chunk protobuf files by service/message/rpc blocks."""
    import re
    chunks = []

    # Extract service blocks
    for m in re.finditer(r"(service\s+(\w+)\s*\{[^}]*\})", content, re.DOTALL):
        start = content[:m.start()].count("\n") + 1
        end = content[:m.end()].count("\n") + 1
        chunks.append(CodeChunk(
            repo_name=file_info.repo_name,
            file_path=file_info.relative_path,
            language="protobuf",
            chunk_type="service",
            name=m.group(2),
            content=m.group(1),
            line_start=start,
            line_end=end,
            keywords=_extract_keywords(m.group(1), "protobuf", m.group(2)),
        ))

    # Extract message blocks
    for m in re.finditer(r"(message\s+(\w+)\s*\{[^}]*\})", content, re.DOTALL):
        start = content[:m.start()].count("\n") + 1
        end = content[:m.end()].count("\n") + 1
        chunks.append(CodeChunk(
            repo_name=file_info.repo_name,
            file_path=file_info.relative_path,
            language="protobuf",
            chunk_type="message",
            name=m.group(2),
            content=m.group(1),
            line_start=start,
            line_end=end,
            keywords=_extract_keywords(m.group(1), "protobuf", m.group(2)),
        ))

    if not chunks:
        return chunk_sliding_window(file_info, content)

    return chunks


def chunk_pdf(file_info: FileInfo) -> list[CodeChunk]:
    """Chunk a PDF by extracting text per page."""
    try:
        import pdfplumber
    except ImportError:
        console.print(f"[yellow]pdfplumber not installed, skipping PDF: {file_info.relative_path}[/yellow]")
        return []

    try:
        pdf = pdfplumber.open(str(file_info.path))
    except Exception as e:
        console.print(f"[yellow]Failed to open PDF {file_info.relative_path}: {e}[/yellow]")
        return []

    chunks = []
    for page_num, page in enumerate(pdf.pages):
        text = (page.extract_text() or "").strip()
        if not text:
            continue

        chunks.append(CodeChunk(
            repo_name=file_info.repo_name,
            file_path=file_info.relative_path,
            language="pdf",
            chunk_type="page",
            name=f"page-{page_num + 1}",
            content=text,
            line_start=page_num + 1,
            line_end=page_num + 1,
            keywords=_extract_keywords(text, "pdf", f"page-{page_num + 1}"),
        ))

    pdf.close()
    return chunks


def chunk_file(file_info: FileInfo, config: Config) -> list[CodeChunk]:
    """Chunk a file using the appropriate strategy."""
    # PDF is binary — handle before read_text
    if file_info.language == "pdf":
        return chunk_pdf(file_info)

    try:
        content = file_info.path.read_text(errors="replace")
    except (OSError, UnicodeDecodeError):
        return []

    file_info.content = content

    if not content.strip():
        return []

    if file_info.language == "markdown":
        return chunk_markdown(file_info, content)
    elif file_info.language == "protobuf":
        return chunk_proto(file_info, content)
    elif file_info.language in EXTRACT_NODES:
        return chunk_with_treesitter(file_info, content)
    else:
        return chunk_sliding_window(file_info, content)
