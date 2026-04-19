# OpenClaw Security Configuration

## Overview
OpenClaw is now configured to run securely without Tailscale by using network isolation on the Incus bridge.

## Security Model

### Network Isolation
- **Bridge Network:** 10.149.181.0/24 (Incus managed)
- **Container IP:** 10.149.181.114
- **Gateway Port:** 18789
- **Access:** Only from containers/VMs on the same Incus bridge

### What's Protected
✓ Not exposed to host localhost (127.0.0.1)
✓ Not exposed to internet
✓ Not exposed to other network interfaces
✓ Only accessible within Incus bridge network
✓ Internal bridge network isolation is the primary access control

### Removed Configuration
- Removed Incus proxy device `port18789` that was forwarding to host
- No longer accessible via `127.0.0.1:18789` on host
- Tailscale completely uninstalled from container
- Container now only has eth0 interface (10.149.181.114)

## Access Configuration

### From Host (operation-dbus)
```bash
OPENCLAW_BASE_URL=http://10.149.181.114:18789/v1
```

### From Other Containers
Any container on the Incus bridge can access:
```
http://10.149.181.114:18789/v1/chat/completions
```

### MCP Servers
The op-dbus MCP servers on the host (10.149.181.1:8080) can access OpenClaw via the bridge.

## Testing

```bash
# Should work (from host via bridge)
curl http://10.149.181.114:18789/v1/models

# Should fail (not exposed to host localhost)
curl http://127.0.0.1:18789/v1/models

# Should fail (not exposed to internet)
curl http://<public-ip>:18789/v1/models
```

## Verification Commands

```bash
# Check container is running
doas incus list openclaw

# Check service status
doas incus exec openclaw -- systemctl status openclaw.service

# Check listening ports
doas incus exec openclaw -- ss -tlnp | grep 18789

# Verify no host exposure
ss -tlnp | grep 18789  # Should return nothing

# Check proxy devices (should not have port18789)
doas incus config device show openclaw
```

## Rollback (if needed)

To re-expose to host:
```bash
doas incus config device add openclaw port18789 proxy \
  listen=tcp:127.0.0.1:18789 \
  connect=tcp:127.0.0.1:18789
```

## Benefits

1. **Network Isolation:** Container network is isolated from host and internet
2. **No Tailscale:** Completely removed - simpler setup, no VPN overhead
3. **Internal-Only Reachability:** Access depends on staying on the trusted Incus bridge
4. **Bridge Security:** Incus bridge provides network-level isolation
5. **Minimal Attack Surface:** Only accessible from trusted containers
6. **Reduced Dependencies:** Fewer packages, smaller attack surface
7. **No External Connections:** Container doesn't connect to Tailscale coordination servers

## Architecture

```
┌─────────────────────────────────────────────────┐
│ Host (10.149.181.1)                             │
│                                                 │
│  ┌──────────────────────────────────────────┐  │
│  │ Incus Bridge (incusbr0)                  │  │
│  │ Network: 10.149.181.0/24                 │  │
│  │                                          │  │
│  │  ┌────────────────────────────────────┐ │  │
│  │  │ openclaw container                 │ │  │
│  │  │ IP: 10.149.181.114                 │ │  │
│  │  │                                    │ │  │
│  │  │ OpenClaw Gateway :18789            │ │  │
│  │  │ - Internal bridge only             │ │  │
│  │  │ - Bridge network only              │ │  │
│  │  └────────────────────────────────────┘ │  │
│  │                                          │  │
│  │  ┌────────────────────────────────────┐ │  │
│  │  │ Other containers can access        │ │  │
│  │  │ via 10.149.181.114:18789           │ │  │
│  │  └────────────────────────────────────┘ │  │
│  └──────────────────────────────────────────┘  │
│                                                 │
│  op-dbus MCP servers: 10.149.181.1:8080        │
│  (accessible from containers via gateway IP)   │
└─────────────────────────────────────────────────┘

Internet ✗ (no access)
Host localhost ✗ (no access)
Bridge network ✓ (access with token)
```

## Tailscale Removal

Tailscale has been completely uninstalled from the container:
```bash
# Stopped and disabled service
systemctl stop tailscaled
systemctl disable tailscaled

# Removed package and dependencies
apt-get remove -y tailscale
apt-get autoremove -y

# Cleaned up state
rm -rf /var/lib/tailscale
```

Container now only has:
- `lo` (127.0.0.1) - loopback
- `eth0` (10.149.181.114) - Incus bridge

## Date Configured
2026-02-20
