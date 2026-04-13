# Socket-Based Container Networking - Design

Verified: 2026-04-13

## Design Summary

The implemented OpenClaw gateway path is:

```text
WireGuard client
  -> wg0 on host (10.0.0.1)
  -> nginx server block for dashboard.3tched.com
  -> /gateway/
  -> Unix socket /run/services0/gateway.sock
  -> socat inside services container
  -> 127.0.0.1:18789 inside services
  -> OpenClaw WebSocket gateway
```

The container has no veth and no IP address. The host reaches it through a
shared filesystem socket, not by routing packets to a container interface.

The broader privacy network remains:

```text
wgcf / WARP
  -> ovsbr0
  -> OpenFlow policy
  -> privacy ports such as priv_wg, priv_warp, priv_xray
  -> future dynamic sock_* ports
```

That fabric is related, but current OpenClaw service access is a host nginx to
Unix socket path.

## Current Component Map

| Component | Current state | Role |
| --- | --- | --- |
| `services` container | Running, loopback only | System services namespace |
| `user-be7efab0cc2a` | Running, loopback only | Mock magic-link user container |
| Incus `default` profile | Root disk only, no `eth0` | Prevents default networking on new containers |
| `/run/services0` | Host tmpfs directory, shared into `services` | System service socket directory |
| `/run/services0/gateway.sock` | Unix socket, mode `srw-rw-rw-` | OpenClaw gateway socket |
| `openclaw-gateway.service` | Enabled/running as root in `services` | Owns OpenClaw process |
| `openclaw-socket.service` | Enabled/running in `services` | Bridges socket to loopback TCP |
| user OpenClaw unit | Disabled/inactive | Must not compete with root unit |
| nginx dashboard config | `/etc/nginx/http.d/dashboard-3tched.conf` | Host proxy and TLS layer |
| old OpenClaw config | `.disabled` file | Standalone subdomain removed |
| `services0-sockets` | Started dinit service | Creates `/run/services0` before Incus/nginx |
| `wg0` | `10.0.0.1/24`, listen `51820` | Device/session identity |
| NextDNS rewrite | pending API key | Should map dashboard name to `10.0.0.1` |
| `wgcf` | WARP tunnel, stale at last check | Privacy/obfuscation transport |
| `ovsbr0` | `10.88.88.1/24` | Privacy bridge |
| `ovsbr0-sock` | no IP | Shared socket-port anchor in target fabric |

## OpenClaw Gateway Process

OpenClaw is not a Unix-socket-native service. Its gateway command runs an
HTTP/WebSocket server and supports bind modes:

- `loopback`
- `lan`
- `tailnet`
- `auto`
- `custom`

The current root config uses token auth and custom loopback bind:

```json
{
  "gateway": {
    "mode": "local",
    "port": 18789,
    "bind": "custom",
    "customBindHost": "127.0.0.1",
    "trustedProxies": ["127.0.0.1"]
  },
  "auth": {
    "mode": "token",
    "token": "<redacted>"
  }
}
```

The running listener is expected to be:

```text
127.0.0.1:18789
::1:18789
```

The `services` container has only `lo`, so even if an application accidentally
binds wider than loopback there is no non-loopback container interface to accept
traffic. The required design still treats explicit loopback binding as the
correct state.

## Systemd Ownership

The root service is the authoritative process owner:

```ini
[Unit]
Description=OpenClaw Gateway (v2026.4.9)
After=network-online.target
Wants=network-online.target

[Service]
ExecStart=/usr/bin/node /usr/lib/node_modules/openclaw/dist/index.js gateway --port 18789
Restart=always
RestartSec=5
Environment=HOME=/root
Environment=OPENCLAW_GATEWAY_PORT=18789
Environment=OPENCLAW_SYSTEMD_UNIT=openclaw-gateway.service

[Install]
WantedBy=default.target
```

The user service at
`/home/jeremy/.config/systemd/user/openclaw-gateway.service` must remain
disabled. If it starts, it can claim `18789` first and force the root unit into
restart loops.

## Socket Bridge

The socket bridge runs inside the `services` container:

```ini
[Unit]
Description=OpenClaw Unix Socket Bridge
After=openclaw-gateway.service
Requires=openclaw-gateway.service

[Service]
ExecStartPre=/bin/rm -f /run/services0/gateway.sock
ExecStart=/usr/bin/socat UNIX-LISTEN:/run/services0/gateway.sock,fork,mode=660,gid=980 TCP:127.0.0.1:18789
Restart=always
RestartSec=2

[Install]
WantedBy=multi-user.target
```

The bridge is deliberately generic at the socket directory level:

- `services0` names the system-services socket plane.
- `gateway.sock` names the specific OpenClaw gateway socket.
- Future services should use names such as `mail.sock` and `dns.sock`.

## nginx Dashboard Route

The dashboard config is the network-facing layer for OpenClaw:

```nginx
server {
    listen 148.113.204.83:80;
    server_name dashboard.3tched.com;
    return 404;
}

server {
    listen 148.113.204.83:443 ssl;
    http2 on;
    server_name dashboard.3tched.com;

    ssl_certificate /etc/letsencrypt/live/3tched.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/3tched.com/privkey.pem;
    ssl_protocols TLSv1.2 TLSv1.3;

    return 404;
}

server {
    listen 10.0.0.1:80;
    server_name dashboard.3tched.com;

    location / {
        return 301 https://$host$request_uri;
    }
}

server {
    listen 10.0.0.1:443 ssl;
    http2 on;
    server_name dashboard.3tched.com;

    location /gateway/ {
        proxy_pass http://unix:/run/services0/gateway.sock:/;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection $connection_upgrade;
        proxy_set_header Authorization "Bearer <redacted>";
        proxy_read_timeout 7d;
    }
}
```

The active host nginx process also has other public listeners for unrelated
sites. That does not mean the dashboard gateway route is public; the dashboard
server block for this route is bound to `10.0.0.1`.

The public dashboard listener is intentionally a sink. It exists only to present
a certificate valid for `dashboard.3tched.com` when DNS or browser state hits
the public VPS address. It does not proxy `/gateway/`.

Verified behavior:

- `https://10.0.0.1/gateway/` with `Host: dashboard.3tched.com` returns `200`.
- `https://148.113.204.83/gateway/` with `Host: dashboard.3tched.com` returns
  `404` through the public sink, not the gateway route.
- `https://dashboard.3tched.com/gateway/` from the host currently returns `404`
  because host DNS/public routing hits the public sink. A real WG client should
  resolve or route `dashboard.3tched.com` to `10.0.0.1`.

## NextDNS Rewrite

WireGuard clients should use NextDNS profile `689ec7` and that profile should
carry this DNS Rewrite:

```text
dashboard.3tched.com -> 10.0.0.1
```

That rewrite is the clean route for browser access because the browser still
uses the normal host name while TLS terminates on the WG-bound dashboard server
block.

The repo contains an API helper:

```sh
NEXTDNS_API_KEY=... NEXTDNS_PROFILE=689ec7 \
  deploy/nextdns/upsert-rewrite.py dashboard.3tched.com 10.0.0.1
```

At verification time `NEXTDNS_PROFILE` was present locally, but
`NEXTDNS_API_KEY` was not.

## Incus Layout

### System Container

`services` has no NIC device and uses disk mounts:

```yaml
services0:
  type: disk
  source: /run/services0
  path: /run/services0
openclaw-home:
  type: disk
  source: /home/jeremy/.openclaw
  path: /home/jeremy/.openclaw
openclaw-src:
  type: disk
  source: /home/jeremy/openclaw
  path: /home/jeremy/openclaw
```

The container metadata includes:

```yaml
user.function: system-services
```

### Mock User Container

`user-be7efab0cc2a` has no NIC device and uses:

```yaml
sockets:
  type: disk
  source: /run/user-sockets/be7efab0cc2a
  path: /run/sockets
```

The container metadata includes:

```yaml
user.wg-pubkey: <wireguard-public-key>
```

The name is derived from:

```text
user-<sha256(wg_pubkey)[:12]>
```

## Authentication Layers

### Layer 1: WireGuard Session Identity

The host `wg0` interface authenticates devices:

- host: `10.0.0.1/24`
- listen port: `51820`
- peers: known device public keys with allowed IPs

nginx binds the dashboard gateway route to `10.0.0.1`, so clients must have a
valid WG session to reach the route.

### Layer 2: OpenClaw Token Auth

nginx injects the gateway bearer token before proxying to the Unix socket.

This keeps the real token off the client-facing URL and out of docs. The token
still exists in nginx config and OpenClaw config, so those files remain secret
material.

### Optional Future Layer: Device Metadata

The design allows additional device filtering by MAC, WG public key, or
OpenFlow-observed metadata. That is not currently required for the OpenClaw
socket path.

## OVS and OpenFlow Role

The OpenFlow/OVS layer should be documented as a separate dataplane:

```text
wgcf
  -> ovsbr0
  -> OpenFlow
  -> priv_wg / priv_warp / priv_xray
  -> future sock_* ports
```

Current addresses:

| Interface | Address | Purpose |
| --- | --- | --- |
| `wgcf` | `172.16.0.2/32` and WARP IPv6 | Cloudflare WARP tunnel |
| `ovsbr0` | `10.88.88.1/24` | Host OVS bridge |
| `ovsbr0-mgmt` | `10.200.0.1/24` | Management internal port |
| `grpc-bridge` | `10.200.0.2/24` | gRPC control plane |
| `ovsbr0-sock` | none | Socket-network anchor |
| `priv_wg` | none | Privacy chain port |
| `priv_warp` | none | Privacy chain port |
| `priv_xray` | `15.235.37.41/32` | Xray identity/egress |

Do not describe the current OpenClaw gateway request path as crossing
`ovsbr0-sock`. It does not. nginx opens a Unix socket file on the host, and the
same file is mounted into the container.

## Startup Sequence

1. Host creates `/run/services0`.
2. Host `services0-sockets` dinit service sets `/run/services0` to
   `root:_nginx` mode `0770`.
3. Incus starts `services` with `/run/services0` mounted.
4. `openclaw-gateway.service` starts as root with `HOME=/root`.
5. OpenClaw listens on `127.0.0.1:18789` and `::1:18789` inside `services`.
6. `openclaw-socket.service` removes stale `gateway.sock`.
7. `socat` listens on `/run/services0/gateway.sock` and forwards to
   `127.0.0.1:18789`.
8. Host nginx routes `/gateway/` to
   `http://unix:/run/services0/gateway.sock:/`.

## Deployment Script Behavior

`deploy/deploy-network.sh` installs and verifies the current socket/OpenFlow
network without treating every run as a WireGuard rewrite.

Default runs:

- install dinit artifacts for `services0-sockets`, `wg-quick-all`,
  `op-ovs-services`, `systemd-networkd`, `netplan-apply`, and
  `ovs-attach-ports`;
- apply the netplan `ovsbr0` definition;
- start `wgcf` through `wg-quick-all`;
- attach `wgcf` and internal ports through `ovs-attach-ports`;
- keep `services` socket-only and mount `/run/services0`;
- install the OpenClaw socket bridge.

One-off deployment flags:

- `--update-wgconf` writes the host `wg0` netplan tunnel from supplied
  `WG_SERVER_*` values and removes any legacy `wg-quick-wg0` dinit artifact.
- `--update-wgcf` runs `wgcf update`/`wgcf generate` against the existing
  account file and refreshes `/etc/wireguard/wgcf.conf`.

Without those flags, the script uses the already-deployed WireGuard values.
netplan is still required because it creates the `wg0` and `ovsbr0`
kernel/networkd objects; `wg-quick` is retained for `wgcf`, which is then
attached to `ovsbr0` as an OVS port.

## Failure Modes

### Duplicate OpenClaw Process

Symptom:

```text
Port 18789 is already in use.
Gateway already running locally.
```

Likely cause:

- user-level `jeremy` OpenClaw unit started and claimed port `18789`.

Required state:

```text
systemctl --user --machine=jeremy@ is-enabled openclaw-gateway.service
# disabled
systemctl --user --machine=jeremy@ is-active openclaw-gateway.service
# inactive
```

### Missing Socket

Symptom:

```text
curl --unix-socket /run/services0/gateway.sock http://localhost/
# fails
```

Likely causes:

- `/run/services0` missing after boot.
- Incus mount not attached to `services`.
- `openclaw-socket.service` not running.
- OpenClaw loopback port not listening.

### Gateway Publicly Routed

Symptom:

- Public IP with `Host: dashboard.3tched.com` returns the OpenClaw gateway.

Required correction:

- Ensure the dashboard server block that contains `/gateway/` listens only on
  `10.0.0.1:443`.
- Ensure no standalone `openclaw.3tched.com` config is active.

## Security Notes

- Current socket permissions are tightened to `_nginx` group access:
  `/run/services0` mode `0770`, `gateway.sock` mode `0660`.
- Do not store real gateway bearer tokens in docs.
- Do not re-enable the user-level OpenClaw unit.
- The services container is privileged and has host bind mounts. That is
  acceptable for the current host-owned system container but should not be used
  as the template for untrusted user containers.
