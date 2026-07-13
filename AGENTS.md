MindStudio is not a direct Codex/Cursor tool surface for this repo.

Treat MindStudio as a ZeroClaw-consumed backend only:
- Do not add MindStudio MCP to Codex/Cursor global MCP settings.
- Do not invoke MindStudio tools directly during normal repo work.
- If work mentions MindStudio, route it through ZeroClaw configuration, prompts,
  or agent wiring unless the user explicitly asks to inspect MindStudio itself.
- Keep general coding and control-plane work on the repo, op-dbus, cognitive-mcp,
  routing, and ZeroClaw surfaces.
