# OVS Native OVSDB JSON-RPC Guide

## Why Native JSON-RPC?

- **No CLI dependencies**: Works without ovs-vsctl, ovs-ofctl, or any shell commands
- **Direct protocol access**: Communicates directly with OVSDB over Unix socket
- **Atomic transactions**: All operations are transactional and consistent
- **High performance**: SIMD JSON parsing for maximum throughput
- **Programmatic**: Easy to integrate into automation and services

## Architecture

```
Your Application
       ↓
  OvsdbClient (Rust)
       ↓
  SIMD JSON-RPC
       ↓
Unix Socket: /var/run/openvswitch/db.sock
       ↓
  OVSDB Server
       ↓
  ovs-vswitchd
       ↓
  Kernel Datapath
```

## Using Rust Code

```rust
use op_network::OvsdbClient;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = OvsdbClient::new();
    
    // Create bridge
    client.create_bridge("br0").await?;
    
    // Add port
    client.add_port("br0", "eth1").await?;
    
    // List bridges
    let bridges = client.list_bridges().await?;
    println!("Bridges: {:?}", bridges);
    
    Ok(())
}
```

## Common Operations

### Check OVS Availability

```rust
let client = OvsdbClient::new();
match client.list_dbs().await {
    Ok(dbs) => println!("OVS is available: {:?}", dbs),
    Err(e) => eprintln!("OVS not available: {}", e),
}
```

### List All Bridges

```rust
let bridges = client.list_bridges().await?;
for bridge in bridges {
    println!("Bridge: {}", bridge);
}
```

### Get Bridge Details

```rust
let info = client.get_bridge_info("br0").await?;
println!("Bridge info: {:?}", info);
```

### List Ports on a Bridge

```rust
let ports = client.list_ports("br0").await?;
println!("Ports on br0: {:?}", ports);
```

### Delete a Bridge

```rust
client.delete_bridge("br0").await?;
```

### Monitor Database Changes

```rust
let mut monitor_rx = client.monitor_db("Open_vSwitch").await?;
while let Some(update) = monitor_rx.recv().await {
    println!("Database update: {:?}", update);
}
```

## OVSDB Transaction Format

All write operations use OVSDB transactions with these operations:

- **insert**: Create a new row in a table
- **delete**: Remove rows from a table
- **update**: Modify existing rows
- **mutate**: Modify set/map columns (add/remove elements)
- **select**: Query rows (read-only)

### Example Transaction: Create Bridge

```json
{
  "method": "transact",
  "params": [
    "Open_vSwitch",
    [
      {
        "op": "insert",
        "table": "Bridge",
        "row": {
          "name": "br0",
          "ports": ["set", [["named-uuid", "port-br0"]]]
        },
        "uuid-name": "bridge-br0"
      },
      {
        "op": "insert",
        "table": "Port",
        "row": {
          "name": "br0",
          "interfaces": ["set", [["named-uuid", "iface-br0"]]]
        },
        "uuid-name": "port-br0"
      },
      {
        "op": "insert",
        "table": "Interface",
        "row": {
          "name": "br0",
          "type": "internal"
        },
        "uuid-name": "iface-br0"
      },
      {
        "op": "mutate",
        "table": "Open_vSwitch",
        "where": [],
        "mutations": [
          ["bridges", "insert", ["set", [["named-uuid", "bridge-br0"]]]]
        ]
      }
    ]
  ],
  "id": 0
}
```

## Performance Notes

- **SIMD JSON**: Uses simd-json for 2-3x faster parsing than serde_json
- **Connection pooling**: Reuses Unix socket connections
- **Batch operations**: Multiple operations in single transaction
- **Zero-copy**: Minimizes allocations with borrowed values

## Security

- Unix socket permissions control access
- No shell command injection possible
- Atomic transactions prevent partial updates
- Audit logging via snowball integration

## References

- [OVSDB RFC 7047](https://datatracker.ietf.org/doc/html/rfc7047)
- [OVS Documentation](https://docs.openvswitch.org/)
- [op-network crate](../crates/op-network/)
- [op-jsonrpc crate](../crates/op-jsonrpc/)

<!-- Extracted from /mnt/opt-inspect/home/git/operation-dbus-proto/docs/OVS-NATIVE-SETUP.md on 2026-07-20 -->
