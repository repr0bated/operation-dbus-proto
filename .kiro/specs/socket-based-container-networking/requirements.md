# Socket-Based Container Networking - Requirements

Verified: 2026-04-13

## Overview

This spec defines the socket-based networking model for Incus containers on the
`3tched.com` VPS host. The immediate implemented service is the OpenClaw gateway
inside the `services` container, exposed through host nginx at
`dashboard.3tched.com/gateway/` for WireGuard-authenticated clients.

The core rule is that containers that participate in this model have no veth,
no `eth0`, no routable IP address, and no network listener exposed outside their
own loopback namespace. Host processes reach container services through Unix
socket files on shared `/run` mounts.

The broader privacy fabric still uses `wgcf`, `ovsbr0`, and OpenFlow for WARP
and privacy routing. The implemented OpenClaw access path is a host nginx to
Unix socket path and does not require L2 traffic into a container.

## Definitions

- `services`: Incus system container for shared services such as OpenClaw,
  mail, and DNS.
- `services0`: Shared system-services socket directory, mounted at
  `/run/services0` on both host and `services`.
- `gateway.sock`: Unix socket for OpenClaw gateway traffic.
- `user-<id>`: Per-user Incus container created by the magic link flow.
- `wg0`: Host WireGuard interface for device session identity.
- `wgcf`: Cloudflare WARP WireGuard-compatible tunnel for privacy transport and
  obfuscation.
- `ovsbr0`: Host Open vSwitch bridge used by the privacy fabric.
- `OpenFlow`: Policy/routing layer for OVS traffic, especially privacy ports and
  future dynamic socket ports.

## Functional Requirements

### FR-1: Socket containers have no network interfaces

Socket-mode containers MUST NOT have veth devices, Incus NIC devices, `eth0`,
or any non-loopback interface.

The only network interface visible inside a strict socket-mode container MUST be
`lo`.

Current verified containers:

- `services`: loopback only.
- `user-be7efab0cc2a`: loopback only.

Current known exceptions outside this spec:

- `privacy-xray-ingress` still has `eth0`.
- `xray-server` still has `eth0`.

Those containers require separate migration before they comply with this spec.

### FR-2: Default Incus profile must not attach networking

The Incus `default` profile MUST NOT define an `eth0` device. It may provide the
root disk device.

New containers created from the default profile SHOULD start with no network
device unless a specific non-socket workload explicitly opts in.

### FR-3: Services bind to container loopback

Application services inside socket-mode containers MUST bind to loopback only.

For OpenClaw, the intended bind is:

- host inside container: `127.0.0.1`
- port: `18789`
- protocol: HTTP/WebSocket, as provided by OpenClaw

OpenClaw does not provide native Unix socket listening. The Unix socket is
provided by a bridge process.

### FR-4: Unix socket bridge exposes loopback services

Each service that needs host access MUST have a local bridge from container
loopback to a Unix socket in the shared socket directory.

For OpenClaw:

- loopback backend: `127.0.0.1:18789`
- socket path: `/run/services0/gateway.sock`
- bridge service: `openclaw-socket.service`
- bridge command: `socat UNIX-LISTEN:/run/services0/gateway.sock,fork,mode=660,gid=980 TCP:127.0.0.1:18789`

The socket bridge MUST remove stale socket files before start.

### FR-5: System services use `/run/services0`

System-service sockets MUST live under `/run/services0`.

The `services` container MUST have an Incus disk device:

- device name: `services0`
- host source: `/run/services0`
- container path: `/run/services0`
- type: `disk`

The directory name `services0` is intentional: it reads like a network interface
while being a filesystem socket mount.

### FR-6: User containers use per-user socket directories

User containers created by the magic link workflow MUST receive a dedicated
socket directory:

- host path: `/run/user-sockets/<container-id>`
- container path: `/run/sockets`
- Incus device name: `sockets`

User containers MUST NOT share a socket directory with other users.

### FR-7: Container identity is metadata-driven

System containers MUST be identified by function name:

- Incus config key: `user.function`
- current value for `services`: `system-services`

User containers MUST be identified by WireGuard public key:

- name format: `user-<sha256(wg_pubkey)[:12]>`
- Incus config key: `user.wg-pubkey`

The mock user container currently present is:

- name: `user-be7efab0cc2a`
- purpose: mock magic-link-created user container

### FR-8: nginx is the host gateway for service access

nginx on the host MUST be the only network-facing process that proxies to
container service sockets.

For OpenClaw, nginx MUST expose the service under:

- host name: `dashboard.3tched.com`
- path: `/gateway/`
- backend: `http://unix:/run/services0/gateway.sock:/`

nginx MUST preserve WebSocket upgrade headers.

### FR-9: OpenClaw must not be advertised by DNS or path names

There MUST NOT be an active `openclaw.3tched.com` nginx service or public DNS
dependency for OpenClaw.

The public service name MUST be generic. Current path:

- `/gateway/`

The old standalone nginx config is disabled:

- `/etc/nginx/http.d/openclaw-3tched.conf.disabled`

### FR-10: WireGuard gates dashboard gateway access

The dashboard nginx server block for this gateway MUST listen on the host
WireGuard address:

- `10.0.0.1:80`
- `10.0.0.1:443`

WireGuard peers authenticate the device/session before reaching nginx. Current
verified `wg0` host address:

- `10.0.0.1/24`
- listen port: `51820`

Current peers include addresses for tablet/laptop and a legacy services peer.
The services container no longer needs its own `wg0` interface for the socket
gateway path.

### FR-10a: NextDNS split DNS resolves dashboard to WireGuard

WireGuard clients SHOULD resolve `dashboard.3tched.com` to `10.0.0.1` through a
NextDNS DNS Rewrite in profile `689ec7`.

The required rewrite is:

- name: `dashboard.3tched.com`
- content: `10.0.0.1`

This keeps normal browser navigation on the WireGuard-only dashboard listener
instead of the public VPS address. The NextDNS API key is not present on this
host at verification time, so the rewrite must be applied from a session that
has `NEXTDNS_API_KEY`.

### FR-10b: Public dashboard SNI must not show unrelated certificates

The public IPv4 listener for `dashboard.3tched.com` MUST present a certificate
valid for `dashboard.3tched.com` and MUST NOT proxy `/gateway/`.

The public listener is a sink only:

- `148.113.204.83:80`
- `148.113.204.83:443`
- response: `404`
- certificate: `/etc/letsencrypt/live/3tched.com/fullchain.pem`

This removes the browser `ERR_CERT_COMMON_NAME_INVALID` caused by SNI falling
through to `assistant.3tched.com`, while preserving the WG-only gateway route.

### FR-11: Service token is injected by nginx and not documented

OpenClaw gateway auth MUST use token mode.

nginx injects the bearer token into the upstream request. The actual token MUST
NOT be copied into documentation, shell history, commits, or specs.

Documentation MUST use `<redacted>` or `<token>` placeholders.

### FR-12: One gateway owner

Only one OpenClaw gateway process may own port `18789`.

The authoritative current owner MUST be the root systemd unit inside
`services`:

- `/etc/systemd/system/openclaw-gateway.service`
- `HOME=/root`
- enabled at boot

The user-level `jeremy` unit MUST remain disabled to avoid duplicate gateway
processes:

- `/home/jeremy/.config/systemd/user/openclaw-gateway.service`
- current required state: disabled/inactive

### FR-13: Broader privacy fabric remains OpenFlow-based

`ovsbr0`, `wgcf`, and OpenFlow remain the target privacy dataplane for traffic
that needs WARP/privacy routing. The OpenClaw socket gateway does not require
container L2 attachment.

Current host fabric ports and addresses:

- `wgcf`: `172.16.0.2/32`, plus IPv6 WARP address.
- `ovsbr0`: `10.88.88.1/24`.
- `ovsbr0-mgmt`: `10.200.0.1/24`.
- `grpc-bridge`: `10.200.0.2/24`.
- `ovsbr0-sock`: no IP address.
- `priv_wg`: no IP address.
- `priv_warp`: no IP address.
- `priv_xray`: `15.235.37.41/32`.

OpenFlow policy for privacy routing is separate from the current
host-nginx-to-Unix-socket OpenClaw access path.

## Non-Functional Requirements

### NFR-1: Reduced attack surface

The design SHOULD minimize network-visible attack surface:

- no container IPs for socket-mode containers
- no container TCP ports
- no per-service subdomains that reveal stack names
- no Incus NAT path for socket-mode services
- host nginx is the only exposed proxy layer for dashboard service routes

### NFR-2: Low latency local transport

The host-to-container hop SHOULD use Unix sockets because they avoid normal
TCP/IP routing, address resolution, and packet forwarding between namespaces.

OpenClaw still speaks HTTP/WebSocket internally; the optimization is the host
nginx to socket bridge path, not a native OpenClaw Unix socket listener.

### NFR-3: Boot resilience

The following units MUST be enabled and running for the current gateway:

- host `services0-sockets` dinit service
- `openclaw-gateway.service`
- `openclaw-socket.service`

The socket bridge MUST be restarted if OpenClaw restarts. Both units SHOULD use
systemd restart policy.

Because `/run` is tmpfs, `/run/services0` and all socket files MUST be recreated
on boot before Incus and nginx are expected to use them. The host dinit
`incus` and `nginx` services MUST depend on `services0-sockets`.

`/run/services0` MUST be owned by `root:_nginx` with mode `0770`.
`/run/services0/gateway.sock` MUST be owned by `root:_nginx` with mode `0660`.

### NFR-4: Clear separation of current state and target fabric

Specs MUST distinguish:

- current implemented gateway path: WireGuard client -> host nginx ->
  `/run/services0/gateway.sock` -> OpenClaw loopback
- broader target privacy fabric: `wgcf`/`ovsbr0`/OpenFlow privacy routing and
  future dynamic `sock_*` container ports

This distinction is required so implementation docs do not imply that current
OpenClaw traffic traverses `ovsbr0` when it is actually proxied through a shared
Unix socket file.

### NFR-5: Verification-first operations

Changes to this topology SHOULD be verified with read-only checks before and
after modification:

- `incus list --format csv`
- `incus exec <container> -- cat /proc/net/dev`
- `incus exec services -- ss -tlnp | grep 18789`
- `doas curl --unix-socket /run/services0/gateway.sock http://localhost/`
- `curl -k https://10.0.0.1/gateway/ -H 'Host: dashboard.3tched.com'`

Secrets MUST be redacted from any captured output.
