# Factory mission 1 — cognitive and memory bridge implementation

Implement the intended outcome of these specifications:

- `.kiro/specs/cognitive-mcp-bridge-only-door/`
- `.kiro/specs/cognitive-mcp-only-door-phase2/`
- Relevant memory/blob/embedding behavior in
  `.kiro/specs/unified-blob-catalog-mcp/` and
  `.kiro/specs/voyage-plugin-cognitive-mcp-boundaries/`

## Goal

Make `op-grpc-bridge` the only application door to cognitive MCP and memory
tools. Every invocation must pass through schema validation, capability and
footprint enforcement, event-chain recording, and the existing D-Bus
`PluginService`/`MutationEngine` path.

## Rules

- Inspect the current code and existing phase-1 implementation before editing.
- Preserve tool reachability; remove direct listeners only after the phase-1
  equivalence gate is demonstrably passing.
- Keep memory, embedding, blob-catalog, and cognitive tools schema-driven.
- New plugin operations belong in `PluginSchema` and generated gRPC routes.
- Do not add a parallel per-plugin proto or direct backend path.
- Do not authorize from container names, aliases, IPs, mesh membership, or
  system-container identity.
- Do not add WireGuard interfaces, handshake watchers, polling identity, s6,
  systemd, direct model writes, or live-host mutations.
- Do not use `sudo`, restart services, deploy, or edit `/etc`.

## Deliverables

- Production code implementing the unblocked requirements.
- Focused unit/integration/equivalence tests.
- Documentation of any prerequisite that prevents Phase 2 completion.
- A reviewed commit on the receiving checkout’s working branch.

## Verification

Run focused `op-grpc-bridge`, cognitive MCP, schema-generation, and memory
tests. Confirm that direct cognitive listeners cannot bypass the bridge and
that all successful mutations retain event-chain attribution.

