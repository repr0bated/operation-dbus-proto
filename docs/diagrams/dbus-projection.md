# D-Bus Projection — op-dbus-mirror

```mermaid
flowchart TD
  Main["op-dbus main()"]

  Plugins["DefaultPluginRegistry<br/>plugin objects"]
  NonNet["NonNetDb<br/>OpNonNet tables"]
  OVSDB["OVSDB<br/>(Open_vSwitch socket)"]
  Mirror["DbusMirror<br/>owner of org.opdbus.v1"]

  Main --> Plugins
  Main --> Mirror

  Plugins -->|"schema metadata + rows"| NonNet
  NonNet -->|"schema + row data"| Mirror

  OVSDB -->|"monitor events + table data<br/>(read via OvsdbClient tool)"| Mirror

  Mirror -->|"publishes"| Root["/org/opdbus/v1"]
  Mirror -->|"publishes"| OvsTree["/org/opdbus/v1/ovsdb/..."]
  Mirror -->|"publishes"| NonNetTree["/org/opdbus/v1/nonnet/..."]
  Mirror -->|"publishes"| PluginTree["schema-derived<br/>/org/opdbus/&lt;plugin&gt;/..."]
  Mirror -->|"lazy-load only"| LazyTree["/org/opdbus/v1/dynamic/..."]
```

**Key relationships:**

- `OvsdbClient` is an internal tool inside `DbusMirror` — not a separate actor in the data path
- `NonNetDb` is seeded from plugin schema at boot; mirror reads it directly
- `OVSDB` monitor events drive refresh; mirror never writes to OVSDB
- `/org/opdbus/v1/dynamic/...` is lazy-load only for large schema-derived tables that passed the same `schema_derived=true` filter — not a fallback
