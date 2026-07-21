# Dropped Excerpts from OVS-NATIVE-SETUP.md

**Reason for extraction:** These sections contain shell-script setup patterns and systemctl commands that are not aligned with the native Rust-based control plane approach.

---

## Section: Quick Start - Using the Shell Script

**Dropped because:** References shell script CLI wrappers instead of the native Rust API.

```bash
# Create a bridge named 'br0'
sudo ./scripts/setup-ovs-bridge-native.sh br0

# Create a bridge and add a port
sudo ./scripts/setup-ovs-bridge-native.sh br0 eth1

# See all available examples
./scripts/ovs-native-examples.sh
```

---

## Section: Quick Start - Using Raw JSON-RPC (with socat)

**Dropped because:** Shows raw socat examples which are CLI-based rather than programmatic API usage.

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

---

## Section: Integration with MCP Tools

**Dropped because:** References MCP compact mode shell commands rather than native D-Bus/Rust integration.

```bash
# Via MCP compact mode
execute_tool ovs_create_bridge '{"name": "br0"}'
execute_tool ovs_add_port '{"bridge": "br0", "port": "eth1"}'
execute_tool ovs_list_bridges '{}'
execute_tool ovs_list_ports '{"bridge": "br0"}'
```

---

## Section: Troubleshooting

**Dropped because:** Contains systemctl commands which violate the host service policy (agents must use `sudo service6` exclusively).

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

---

## Section: Examples

**Dropped because:** References shell script examples rather than Rust crate examples.

See the following files for complete examples:

- `scripts/setup-ovs-bridge-native.sh` - Shell script example
- `scripts/ovs-native-examples.sh` - Collection of JSON-RPC examples
- `examples/ovs_native_rust.rs` - Rust example with full workflow
- `crates/op-tools/src/builtin/ovs_tools.rs` - MCP tool implementations

---

**Note:** No port 18789 → 8090 replacement was necessary as port 18789 does not appear in this source document.

<!-- Extracted from OVS-NATIVE-SETUP.md on 2026-07-20 -->
