"""Parse GEMINI.md metadata files from repos."""

import re
from dataclasses import dataclass, field
from pathlib import Path


@dataclass
class GeminiContext:
    """Parsed metadata from a repo's GEMINI.md file."""
    focus_area: str = ""
    why_index: str = ""
    key_concepts: list[str] = field(default_factory=list)
    language: str = ""
    architecture: str = ""
    raw_text: str = ""


def parse_gemini_md(path: Path) -> GeminiContext | None:
    """Parse a GEMINI.md file into structured context."""
    if not path.exists():
        return None

    text = path.read_text(errors="replace")
    ctx = GeminiContext(raw_text=text)

    # Helper: extract section content between ## headers
    def get_section(header: str) -> str:
        pattern = rf"(?i)##\s*{header}\s*\n(.*?)(?=\n##\s|\Z)"
        m = re.search(pattern, text, re.DOTALL)
        return m.group(1).strip() if m else ""

    # Extract Focus Area
    focus = get_section("Focus Area")
    if focus:
        ctx.focus_area = focus.strip("*").strip()

    # Extract Why Index
    why = get_section("Why Index\\??")
    if why:
        ctx.why_index = why.strip("*").strip()

    # Extract Key Concepts (bullet list)
    concepts = get_section("Key Concepts?")
    if concepts:
        bullets = re.findall(r"[-*]\s*\*?\*?(.+?)(?:\*?\*?\s*$)", concepts, re.MULTILINE)
        if bullets:
            ctx.key_concepts = [b.strip().rstrip(".") for b in bullets]

    # Extract Language from Conventions section
    conventions = get_section("Conventions")
    if conventions:
        m = re.search(r"(?i)\*?\*?Language\*?\*?[:\s]*(.+?)(?:\n|$)", conventions)
        if m:
            ctx.language = m.group(1).strip().strip("*").strip()
    # Fallback: standalone Language line
    if not ctx.language:
        m = re.search(r"(?i)^-?\s*\*?\*?Language\*?\*?[:\s]*(.+?)$", text, re.MULTILINE)
        if m:
            ctx.language = m.group(1).strip().strip("*").strip()

    # Extract Architecture
    arch = get_section("Architecture")
    if arch:
        ctx.architecture = arch

    return ctx
