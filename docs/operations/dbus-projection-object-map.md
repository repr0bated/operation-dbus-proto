# D-Bus Projection Object Map

This map treats the projected D-Bus tree as the primary object in the system.

The main runtime object that owns the tree is `op_dbus_mirror::DbusMirror`, published as
`org.opdbus.v1`. Everything else in this map either feeds that tree, bridges out of it, or
records it.

`DbusProjection` is a separate helper in `op-introspection`; despite the name, it is not the
owner of the live `/org/opdbus/v1/...` tree.

## D-Bus As System Ontology

D-Bus is not just a transport in this architecture.

It is intended to be the system's:

- index
- namespace
- object directory
- switchboard
- operator
- retriever
- housekeeping layer

In practical terms, that means:

- important runtime things should have an address in the tree
- discoverability should come from the tree, not from side knowledge
- actions should have a stable object/interface home
- retrieval should be path/interface based, not guesswork
- the running system should organize itself through the bus

Short version:

`org.opdbus.v1` is supposed to be the live rolodex, phone book, operator desk, and
"where did I put that?" layer for the running system.

This is why projection-first consistency matters so much: if core state and control require
side channels that bypass the tree, then the system is no longer organized around its own
index.

## Primary Object Graph

```mermaid
flowchart LR
  subgraph Sources["Authoritative source objects feeding the projection"]
    Plugins["State plugins\nquery_current_state()"]
    LiveState["initial_plugin_states\nHashMap<String, Value>"]
    SchemaDefs["plugin_schema_defs()\nobject_types + base_path + rcp_db + rcp_table"]
    NonNet["NonNetDb\nOpNonNet schema + tables"]
    OVSDB["Open_vSwitch DB\nunix:/var/run/openvswitch/db.sock"]
    OvsClient["OvsdbClient"]

    Plugins --> LiveState
    LiveState --> NonNet
    SchemaDefs --> NonNet
    OVSDB <--> OvsClient
  end

  subgraph Projection["Primary object: the projected D-Bus tree"]
    Mirror["DbusMirror\nwell-known name: org.opdbus.v1"]
    ObjectServer["zbus ObjectServer"]
    Root["/org/opdbus/v1"]
    OvsRoot["/org/opdbus/v1/ovsdb"]
    NonNetRoot["/org/opdbus/v1/nonnet"]
    BasePaths["Schema-derived base paths\nexamples:\n/org/opdbus/network/interfaces/*\n/org/opdbus/incus/containers/*"]
    Dynamic["/org/opdbus/v1/dynamic/<plugin>/<object_type>\nLazyTable summaries"]
    Compat["/org/freedesktop/network1/link/<id>\ncompat mirror for Interface/Port"]

    Mirror --> ObjectServer
    ObjectServer --> Root
    ObjectServer --> OvsRoot
    ObjectServer --> NonNetRoot
    ObjectServer --> BasePaths
    ObjectServer --> Dynamic
    ObjectServer --> Compat
  end

  NonNet -->|"get_schema + select/count rows"| Mirror
  SchemaDefs -->|"projection specs"| Mirror
  OvsClient -->|"get_schema + select_table + monitor_db"| Mirror
```

## What Each Object Actually Owns

```mermaid
flowchart TD
  Main["op-dbus main()"]

  Plugins["DefaultPluginRegistry\nplugin objects"]
  NonNet["NonNetDb\nOpNonNet tables"]
  OvsClient["OvsdbClient"]
  Mirror["DbusMirror"]

  Main --> Plugins
  Plugins -->|"query_current_state()"| NonNet
  Main --> OvsClient
  Main --> Mirror

  NonNet -->|"schema metadata + rows"| Mirror
  OvsClient -->|"OVS tables + updates"| Mirror

  Mirror -->|"publishes"| Root["/org/opdbus/v1"]
  Mirror -->|"publishes"| OvsTree["/org/opdbus/v1/ovsdb/..."]
  Mirror -->|"publishes"| NonNetTree["/org/opdbus/v1/nonnet/..."]
  Mirror -->|"publishes"| PluginTree["schema-derived /org/opdbus/<plugin>/..."]
  Mirror -->|"publishes"| LazyTree["/org/opdbus/v1/dynamic/..."]
```

## Bridge And Audit Objects Around The Tree

```mermaid
flowchart LR
  subgraph Projection["Primary projected tree"]
    Tree["org.opdbus.v1\n/org/opdbus/v1/... object hierarchy"]
  end

  subgraph Bridge["Bridge objects attached to the same state"]
    SchemaEngine["SchemaEngine"]
    Grpc["OperationGrpcServer\nStateSync / PluginService / EventChainService / MCP"]
  end

  subgraph Audit["Audit / persistence side objects"]
    EventChain["EventChain\nop-state-store"]
    DbusProjection["DbusProjection helper\nop-introspection"]
    Blockchain["StreamingBlockchain\nBTRFS state + block events"]
  end

  NonNet["NonNetDb"] --> SchemaEngine
  OvsClient["OvsdbClient"] --> SchemaEngine
  SchemaEngine --> EventChain
  SchemaEngine --> Grpc

  Tree -. introspect_and_persist() .-> DbusProjection
  DbusProjection --> Blockchain
```

## Timeline Layers

These timelines show when the projected tree exists, when it is refreshed, and where adjacent DB
or orchestration activity touches it.

### Boot Timeline

```mermaid
sequenceDiagram
  participant Dinit as dinit
  participant SessionBus as op-session-bus
  participant OpDbus as op-dbus main
  participant Plugins as state plugins
  participant NonNet as NonNetDb
  participant OVS as OvsdbClient/Open_vSwitch DB
  participant Mirror as DbusMirror
  participant Tree as org.opdbus.v1 tree
  participant Engine as SchemaEngine
  participant Chain as EventChain

  Dinit->>SessionBus: start private session bus
  Dinit->>OpDbus: start main process
  OpDbus->>OVS: create OvsdbClient
  OpDbus->>NonNet: create NonNetDb
  OpDbus->>Engine: create SchemaEngine(EventChain, OVS, NonNet)
  OpDbus->>Plugins: load default plugins
  Plugins-->>OpDbus: query_current_state() per plugin
  OpDbus->>NonNet: load_from_plugins(initial_plugin_states, schema_defs)
  NonNet-->>Engine: broadcast initial table updates
  Engine->>Chain: record initial authoritative changes
  OpDbus->>Mirror: create DbusMirror(bus, OVS, NonNet, SchemaEngine)
  Mirror->>Tree: claim org.opdbus.v1
  Mirror->>Tree: mount /org/opdbus/v1
  Mirror->>Tree: mount /org/opdbus/v1/ovsdb
  Mirror->>Tree: mount /org/opdbus/v1/nonnet
  Mirror->>Tree: publish schema-derived base paths
  Mirror->>OVS: get_schema/select_table/monitor_db
  Mirror->>NonNet: get_schema/select/count/subscribe
  Mirror->>Tree: refresh_full_tree()
```

### Steady-State Runtime Timeline

```mermaid
sequenceDiagram
  participant Plugin as state plugin
  participant NonNet as NonNetDb
  participant OVS as Open_vSwitch DB
  participant Mirror as DbusMirror
  participant Tree as org.opdbus.v1 tree
  participant Engine as SchemaEngine
  participant Chain as EventChain
  participant Grpc as gRPC bridge

  alt non-network plugin state changes
    Plugin-->>NonNet: update table / load rows
    NonNet-->>Mirror: broadcast::Receiver event
    Mirror->>Tree: refresh_full_tree()
    NonNet-->>Engine: broadcast::Receiver event
    Engine->>Chain: record authoritative change
    Engine-->>Grpc: publish StateChange
  else OVSDB changes
    OVS-->>Mirror: monitor_db() event
    Mirror->>Tree: refresh OVSDB-backed branches
    OVS-->>Engine: monitor_db() event
    Engine->>Chain: record authoritative change
    Engine-->>Grpc: publish StateChange
  end
```

### Orchestration / Spawned Activity Timeline

```mermaid
sequenceDiagram
  participant Web as op-web / websocket
  participant Orch as orchestrator
  participant Tool as tool execution
  participant OVS as OVSDB or plugin backend
  participant NonNet as NonNetDb
  participant Mirror as DbusMirror
  participant Tree as org.opdbus.v1 tree
  participant Engine as SchemaEngine
  participant Chain as EventChain

  Web->>Orch: session request / tool invocation
  Orch->>Tool: execute operation
  alt operation mutates OVS-backed reality
    Tool->>OVS: transact / mutate
    OVS-->>Mirror: monitor_db update
    Mirror->>Tree: refresh affected objects
    OVS-->>Engine: monitor_db update
  else operation mutates NonNet-backed state
    Tool->>NonNet: update_table / load state
    NonNet-->>Mirror: update event
    Mirror->>Tree: refresh affected objects
    NonNet-->>Engine: update event
  end
  Engine->>Chain: append audit event
```

### Privacy Login / Verification Timeline

This is the login-adjacent path that currently overlaps the D-Bus side but does not go through
the `org.opdbus.v1` projection tree directly.

```mermaid
sequenceDiagram
  participant User as browser/user
  participant Web as op-web privacy handlers
  participant UserStore as user_store
  participant Mail as email_sender
  participant SmClient as state_manager_client
  participant StateMgr as org.opdbus.StateManager
  participant DBusTree as org.opdbus.v1 tree

  User->>Web: POST /api/privacy/signup
  Web->>UserStore: create_user / create_magic_link
  Web->>Mail: send_magic_link()
  Mail-->>User: email with token

  User->>Web: GET /api/privacy/verify?token=...
  Web->>UserStore: verify_magic_link()
  Web->>SmClient: query_plugin_state(\"incus\")
  SmClient->>StateMgr: QueryState
  Web->>SmClient: apply_plugin_state(\"incus\", desired_state)
  SmClient->>StateMgr: ApplyContractMutation
  Note over DBusTree: The live projection tree may later reflect the resulting\nIncus/plugin state if that state reaches NonNet/OVS-backed projection inputs.
```

## Layer Relationship Summary

- Boot layer:
  creates the projection owner and mounts the tree.
- Runtime layer:
  keeps the tree fresh from `NonNetDb` and `Open_vSwitch`.
- Orchestration layer:
  causes mutations in backends; the tree updates after those backends change.
- Login/privacy layer:
  currently uses the older `StateManager` path and only intersects the projection tree
  indirectly.

## Runtime Reality In This Repo

- The live `/org/opdbus/v1/...` tree comes from `DbusMirror`, not from `DbusProjection`.
- `NonNetDb` is the in-process object table for non-network plugin state. It is seeded from live
  plugin state plus schema definitions.
- `OvsdbClient` talks directly to the native Open vSwitch database socket and projects rows into
  the same tree.
- `SchemaEngine` is the bridge/audit coordinator beside the tree. It records changes into
  `EventChain` and exposes them over gRPC.
- `StreamingBlockchain` is attached through `DbusProjection` for introspection persistence and
  BTRFS-backed state snapshots, not as the owner of the live projection tree.
- `op-web` privacy signup/verify currently still uses `state_manager_client` against
  `org.opdbus.StateManager`, which is adjacent to the main projection story rather than the owner
  of it.

## Intentionally Excluded From The Primary Map

- `org.opdbus.StateManager`:
  The crate code still exists, but it is not the primary owner of the current
  `/org/opdbus/v1/...` projection path in `op-dbus` startup.
- `MemoryStore` / `SqliteStore`:
  These are application storage objects, but they are not the direct source of the mirrored
  `org.opdbus.v1` object hierarchy.
