# Socket-Based Container Networking — Product Requirements Document

## Overview

The `services` Incus container (running OpenClaw gateway, Maddy mail, NextDNS)
must be reachable from WireGuard-authenticated clients without exposing any
container IP address or TCP port. The access path is:

```
WireGuard client → host wg0 → nginx (10.0.0.1) → Unix socket → container loopback
```

## Problem Statement

Previously the `services` container had an `eth0` NIC on `incusbr0` with a
static IP, and a WireGuard peer (`wg0` at `10.0.0.4`). This exposed container
IPs to the network, created scannable TCP ports, and required a separate
WireGuard peer per container. The goal is to eliminate all of that.

## User Stories

**US-1** — As a WireGuard client (tablet, laptop), I can reach the OpenClaw
gateway at `https://dashboard.3tched.com/gateway/` without knowing or routing
to any container IP address.

**US-2** — As an operator, I can add a new service to the `services` container
by adding a socat bridge unit and an nginx location block, without changing
container networking.

**US-3** — As a security reviewer, I can verify that no container has a
routable IP or open TCP port by inspecting `/proc/net/dev` inside each
container.

**US-4** — As an operator, I can run `deploy/deploy-network.sh` on a fresh
host and have the full socket gateway path come up without manual steps.

**US-5** — As a WireGuard client, `dashboard.3tched.com` resolves to
`10.0.0.1` through NextDNS split DNS so my browser reaches the WG-bound nginx
listener automatically.

## Functional Requirements

### FR-1: No container network interfaces
Socket-mode containers MUST have only `lo`. No veth, no `eth0`, no `wg0`.

### FR-2: Default Incus profile has no NIC
The Incus `default` profile MUST NOT define an `eth0` device.

### FR-3: Services bind to loopback
All services inside `services` MUST bind to `127.0.0.1` only.

### FR-4: Unix socket bridge per service
Each service needing host access MUST have a socat bridge from
`/run/services0/<name>.sock` to `127.0.0.1:<port>`.

Current required bridge: `gateway.sock → 127.0.0.1:18789`

### FR-5: Shared socket directory `/run/services0`
The `services` container MUST have an Incus disk device `services0` mounting
`/run/services0` from host into container at the same path.

### FR-6: Per-user socket directories
User containers created by the magic-link flow MUST receive
`/run/user-sockets/<container-id>` on the host, mounted as `/run/sockets`
inside the container.

### FR-7: Container identity metadata
- System containers: `user.function = system-services`
- User containers: `user.wg-pubkey = <key>`, name = `user-<sha256(key)[:12]>`

### FR-8: nginx is the only network-facing proxy
nginx on the host MUST proxy `/gateway/` to
`http://unix:/run/services0/gateway.sock:/` on the WG-bound listener only.

### FR-9: No stack-revealing service names
There MUST NOT be an active `openclaw.3tched.com` nginx config. The public
path is `/gateway/` under `dashboard.3tched.com`.

### FR-10: WireGuard gates dashboard access
The dashboard nginx server block MUST listen on `10.0.0.1:80` and
`10.0.0.1:443` only. Public IP listeners for `dashboard.3tched.com` return
`404` (certificate sink only).

### FR-11: Token auth injected by nginx
nginx MUST inject the OpenClaw bearer token. The token MUST NOT appear in
documentation or specs — use `<redacted>`.

### FR-12: One gateway process owner
Only the root systemd unit inside `services` may own port `18789`. The
user-level `jeremy` unit MUST remain disabled.

### FR-13: NextDNS split DNS
NextDNS profile `689ec7` MUST carry a rewrite: `dashboard.3tched.com → 10.0.0.1`.

## Non-Functional Requirements

### NFR-1: Reduced attack surface
No container IPs, no container TCP ports, no per-service subdomains.

### NFR-2: Low-latency local transport
Host-to-container hop uses Unix sockets (no TCP/IP routing between namespaces).

### NFR-3: Boot resilience
`/run/services0` is tmpfs and MUST be recreated before Incus and nginx start.
The `services0-sockets` dinit service owns this. Incus and nginx MUST depend
on it.

### NFR-4: Deployment script is idempotent
`deploy/deploy-network.sh` default runs MUST NOT rotate WireGuard keys or
rewrite WireGuard config. Those are opt-in via `--update-wgconf` /
`--update-wgcf`.

### NFR-5: Verification-first
Changes MUST be verifiable with read-only checks before and after:
- `incus exec services -- cat /proc/net/dev`
- `doas curl --unix-socket /run/services0/gateway.sock http://localhost/`
- `curl -sk https://10.0.0.1/gateway/ -H 'Host: dashboard.3tched.com'`

## Out of Scope

- `privacy-xray-ingress` and `xray-server` migration (separate task)
- NextDNS rewrite automation when `NEXTDNS_API_KEY` is unavailable
- OpenFlow `sock_*` dynamic container port model (future)
- gRPC-over-Unix-socket refactor for `op-dbus` ↔ OpenClaw (separate task)
