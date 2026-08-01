# Chimera Linux ICUS Container Setup

Complete setup for running Chimera Linux in an ICUS (Isolated Container User Space) with WireGuard VPN, SSH MCP tunnel, and DHCP auto-configuration.

## Quick Start

### Prerequisites

- Linux host with systemd
- Root access
- Existing WireGuard server
- Existing MCP SSH server
- `wget`, `tar`, `wg` (wireguard-tools), `ip` (iproute2)

### One-Line Setup

```bash
sudo ./deploy/quick-setup-chimera.sh \
  --wg-endpoint wg.yourdomain.com:51820 \
  --wg-pubkey YOUR_WG_SERVER_PUBLIC_KEY \
  --mcp-host mcp.yourdomain.com
```

### Full Setup (with all options)

```bash
sudo ./deploy/setup-chimera-icus.sh
```

With environment variables:

```bash
export WIREGUARD_ENDPOINT="wg.yourdomain.com:51820"
export WIREGUARD_PUBLIC_KEY="abcd1234..."
export WIREGUARD_ALLOWED_IPS="0.0.0.0/0, ::/0"
export MCP_SSH_HOST="mcp.yourdomain.com"
export MCP_SSH_PORT="22"
export ICUS_NAME="my-chimera"
export DHCP_INTERFACE="eth0"

sudo ./deploy/setup-chimera-icus.sh
```

## What Gets Installed

### 1. Chimera Linux Rootfs
- Downloads latest Chimera Linux rootfs
- Extracts to `/var/lib/icus/<name>/rootfs`
- Sets up overlay filesystem for writes

### 2. WireGuard VPN
- Generates WireGuard key pair
- Creates client configuration
- Connects to your existing WireGuard server
- Auto-starts on boot

### 3. DHCP Client
- Auto-configures network via DHCP
- Runs on specified interface (default: eth0)
- Auto-starts with container

### 4. MCP SSH Tunnel
- Generates SSH key pair
- Creates reverse SSH tunnel to MCP server
- Enables remote access to container
- Auto-starts after WireGuard

## Container Management

Use the `icus-ctl.sh` script:

```bash
# Start container
sudo ./deploy/icus-ctl.sh chimera-icus start

# Stop container
sudo ./deploy/icus-ctl.sh chimera-icus stop

# Enter shell
sudo ./deploy/icus-ctl.sh chimera-icus shell

# Check WireGuard status
sudo ./deploy/icus-ctl.sh chimera-icus wg-status

# View logs
sudo ./deploy/icus-ctl.sh chimera-icus logs

# Create backup
sudo ./deploy/icus-ctl.sh chimera-icus backup

# Destroy container (DANGER!)
sudo ./deploy/icus-ctl.sh chimera-icus destroy
```

Or use systemd:

```bash
sudo systemctl start icus-chimera-icus
sudo systemctl stop icus-chimera-icus
sudo systemctl status icus-chimera-icus
```

## Post-Setup Steps

### 1. Add WireGuard Client to Server

After setup completes, you'll see the container's WireGuard public key. Add it to your WireGuard server:

```bash
# On your WireGuard server, add this to /etc/wireguard/wg0.conf:
[Peer]
PublicKey = <CONTAINER_PUBLIC_KEY>
AllowedIPs = 10.200.200.2/32

# Then restart WireGuard
sudo wg-quick down wg0 && sudo wg-quick up wg0
```

Or use the helper script on the server:

```bash
sudo ./deploy/wireguard-add-client.sh <CONTAINER_PUBLIC_KEY> chimera-icus
```

### 2. Add SSH Key to MCP Server

The setup generates an SSH key pair at:
- Private: `/var/lib/icus/<name>/config/ssh/mcp_key`
- Public: `/var/lib/icus/<name>/config/ssh/mcp_key.pub`

Add the public key to your MCP server's `~/.ssh/authorized_keys`:

```bash
# On MCP server
cat /var/lib/icus/chimera-icus/config/ssh/mcp_key.pub >> ~/.ssh/authorized_keys
```

### 3. Start the Container

```bash
sudo systemctl start icus-chimera-icus
```

### 4. Verify Connectivity

```bash
# Check WireGuard
sudo ./deploy/icus-ctl.sh chimera-icus wg-status

# Check network inside container
sudo ./deploy/icus-ctl.sh chimera-icus shell
# Then inside container:
ping 1.1.1.1
wg show
```

## Directory Structure

```
/var/lib/icus/<name>/
├── rootfs/          # Chimera Linux root filesystem
├── overlay/         # Writeable overlay layer
├── work/            # Overlay work directory
├── config/
│   ├── wireguard/
│   │   ├── private.key
│   │   ├── public.key
│   │   └── preshared.key
│   └── ssh/
│       ├── mcp_key
│       └── mcp_key.pub
└── logs/            # Container logs
```

## Troubleshooting

### WireGuard Not Connecting

1. Check server has client's public key:
   ```bash
   sudo wg show
   ```

2. Verify endpoint is reachable:
   ```bash
   nc -zv wg.yourdomain.com 51820
   ```

3. Check WireGuard logs:
   ```bash
   sudo ./deploy/icus-ctl.sh chimera-icus shell
   dmesg | grep wireguard
   ```

### DHCP Not Working

1. Check interface name:
   ```bash
   ip link show
   ```

2. Verify DHCP client is running:
   ```bash
   sudo ./deploy/icus-ctl.sh chimera-icus shell
   ps aux | grep dhcpcd
   ```

### SSH Tunnel Not Connecting

1. Verify MCP server is reachable via WireGuard:
   ```bash
   sudo ./deploy/icus-ctl.sh chimera-icus shell
   ping <MCP_SERVER_IP>
   ```

2. Check SSH key is added to MCP server

3. Test SSH manually:
   ```bash
   sudo ./deploy/icus-ctl.sh chimera-icus shell
   ssh -i /root/.ssh/mcp_key -p <PORT> root@<MCP_HOST>
   ```

## Security Notes

- WireGuard uses Curve25519 for key exchange
- Pre-shared keys add extra layer of security
- SSH keys are Ed25519 (modern, secure)
- Container runs isolated with overlay filesystem
- All credentials stored in `/var/lib/icus/<name>/config/`

## Architecture

```
┌─────────────────────────────────────────────┐
│           Host System (Linux)               │
│  ┌─────────────────────────────────────┐   │
│  │     ICUS Container (systemd-nspawn  │   │
│  │     or chroot + dinit)              │   │
│  │                                     │   │
│  │  ┌──────────────┐  ┌─────────────┐ │   │
│  │  │ WireGuard    │  │  DHCP Client│ │   │
│  │  │ Client (wg0) │  │  (dhcpcd)   │ │   │
│  │  └──────┬───────┘  └─────────────┘ │   │
│  │         │                          │   │
│  │  ┌──────┴──────┐  ┌─────────────┐  │   │
│  │  │ SSH Tunnel  │  │  MCP Client │  │   │
│  │  │ (autossh)   │──┤  (reverse)  │  │   │
│  │  └─────────────┘  └─────────────┘  │   │
│  └─────────────────────────────────────┘   │
└─────────────────────────────────────────────┘
         │                    │
         ▼                    ▼
┌─────────────────┐  ┌─────────────────┐
│ WireGuard Server│  │   MCP Server    │
│ (Your existing) │  │ (Your existing) │
└─────────────────┘  └─────────────────┘
```

## Uninstallation

To completely remove the container:

```bash
sudo ./deploy/icus-ctl.sh <name> destroy
```

This will:
- Stop the container
- Remove all files
- Disable systemd service
- Clean up mount points

## License

Part of the op-dbus project.
