# OVS Bridge Setup - Native OVSDB JSON-RPC

This guide shows how to create and manage OVS bridges using **native OVSDB JSON-RPC** without any CLI commands like `ovs-vsctl`.

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

## Quick Start

### 1. Using the Shell Script

```bash
# Create a bridge named 'br0'
sudo ./scripts/setup-ovs-bridge-native.sh br0

# Create a bridge and add a port
sudo ./scripts/setup-ovs-bridge-native.sh br0 eth1

# See all available examples
./scripts/ovs-native-examples.sh
```

### 2. Using Rust Code

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

### 3. Using Raw JSON-RPC (with socat)

```bash
# List all bridges
cat << 'EOF' | socat - UNIX-CONNECT:/var/run/openvswitch/db.sock
{"method":"transact","params":["Open_vSwitch",[{
  "op":"select",
  "table":"Bridge",
  "where":[],
  "columns":["name"]
}]],"id":0}
EOF

# Create a bridge
cat << 'EOF' | socat - UNIX-CONNECT:/var/run/openvswitch/db.sock
{"method":"transact","params":["Open_vSwitch",[
  {
    "op":"insert",
    "table":"Bridge",
    "row":{"name":"br0","ports":["set",[["named-uuid","port-br0"]]]},
    "uuid-name":"bridge-br0"
  },
  {
    "op":"insert",
    "table":"Port",
    "row":{"name":"br0","interfaces":["set",[["named-uuid","iface-br0"]]]},
    "uuid-name":"port-br0"
  },
  {
    "op":"insert",
    "table":"Interface",
    "row":{"name":"br0","type":"internal"},
    "uuid-name":"iface-br0"
  },
  {
    "op":"mutate",
    "table":"Open_vSwitch",
    "where":[],
    "mutations":[["bridges","insert",["set",[["named-uuid","bridge-br0"]]]]]
  }
]],"id":0}
EOF
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

## Integration with MCP Tools

The OVS tools are available through the MCP interface:

```bash
# Via MCP compact mode
execute_tool ovs_create_bridge '{"name": "br0"}'
execute_tool ovs_add_port '{"bridge": "br0", "port": "eth1"}'
execute_tool ovs_list_bridges '{}'
execute_tool ovs_list_ports '{"bridge": "br0"}'
```

## Troubleshooting

### OVSDB Socket Not Found

```bash
# Check if OVS is running
systemctl status openvswitch-switch

# Check socket permissions
ls -la /var/run/openvswitch/db.sock

# Start OVS if needed
sudo systemctl start openvswitch-switch
```

### Permission Denied

OVSDB socket requires root or membership in the `openvswitch` group:

```bash
# Add user to openvswitch group
sudo usermod -a -G openvswitch $USER

# Or run with sudo
sudo ./scripts/setup-ovs-bridge-native.sh br0
```

### Bridge Not Appearing in Kernel

After creating a bridge in OVSDB, it may take a moment for ovs-vswitchd to create the kernel interface:

```bash
# Wait a moment, then check
sleep 1
ip link show br0
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

## Examples

See the following files for complete examples:

- `scripts/setup-ovs-bridge-native.sh` - Shell script example
- `scripts/ovs-native-examples.sh` - Collection of JSON-RPC examples
- `examples/ovs_native_rust.rs` - Rust example with full workflow
- `crates/op-tools/src/builtin/ovs_tools.rs` - MCP tool implementations
