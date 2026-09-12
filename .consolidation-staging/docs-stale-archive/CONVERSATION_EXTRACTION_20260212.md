# Conversation Extraction & Architectural Decisions
## Date: 2026-02-12 (Extracted from .codex)

### Core Architecture: The "Hotwired" Hub

The system has moved away from traditional reconciliation loops toward a reactive, event-driven model centered around the **SyncEngine** and **MCP Hub**.

#### 1. Mutation Trigger Pipeline
The update trigger follows a strict, auditable path:
- **Ingress**: Mutations arrive via gRPC, JSON-RPC, or Internal calls.
- **Enforcement**: The `SyncEngine` (`op-grpc-bridge`) validates the contract and pushes it to the canonical D-Bus ingress: `org.opdbus.StateManager.ApplyContractMutation`.
- **Materialization**: The `StateManager` merges the mutation with schema defaults from the `SchemaRegistry`, ensuring the resulting state is always "complete" according to the plugin contract.
- **Footprinting**: Every change is hashed and appended to the immutable `EventChain`, providing a snowball-ready audit trail.
- **Reactive Update**: A `StateChange` event is broadcasted immediately, allowing the Hub (Chatbot/MCP) to react without polling.

#### 2. D-Bus Surface Optimization
To keep the D-Bus surface "small" and fast:
- **Write Path**: Limited to the `StateManager` ingress.
- **Read Path**: The `DbusMirror` crate provides a 1:1 projection of OVSDB and NonNet databases into `org.opdbus.mirror`.
- **Object Mapping**: Database IDs (UUIDs) are sanitized (hyphens to underscores) for D-Bus compliance.

#### 3. Recent Focus Areas (from Conversation Logs)
- **Enterprise Benefits**: Debugging issues where the session was picking up consumer quotas instead of enterprise entitlements.
- **MCP Proxy**: Reproducing IDE extension logic within `mcp-proxy` to allow the bridge to function without a full IDE environment.
- **Strict Write Path**: Configuration knobs added to `SyncEngine` to force all mutations through the auditable D-Bus path (`OP_DBUS_STRICT_WRITE_PATH`).

### Key Knowledge Assets
- **OVSDB Source**: `/var/run/openvswitch/db.sock` (Open_vSwitch DB).
- **NonNet Source**: Managed via `op_jsonrpc` dispatcher.
- **Contract Envelope**: Uniform sections: `stub`, `immutable`, `tunable`, `observed`, `meta`, `semantic_index`, `privacy_index`.
