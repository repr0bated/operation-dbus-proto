# Socket-Based Container Networking - Implementation Spec

Verified: 2026-04-13

## Scope

This document is the implementation-level spec for the current socket-based
OpenClaw gateway deployment and the container socket model it establishes.

It is deliberately split into:

- implemented state: what is live and verified on the host now
- target fabric: the related WARP/OVS/OpenFlow design that future container
  traffic should use
- gaps: items that are known not to be finished

## Source Inputs

This spec is based on:

- `/home/jeremy/socket.txt`
- live Incus state on the host
- live nginx config on the host
- live systemd units inside `services`
- OpenClaw gateway CLI help and installed config type definitions
- existing privacy network docs under `/home/jeremy/docs` and
  `/home/jeremy/git/operation-dbus-proto/docs/operations`

## Normative Rules

The words MUST, MUST NOT, SHOULD, and MAY are used with their normal RFC 2119
meaning.

## Implemented Gateway Contract

### Client-Facing Endpoint

Clients access OpenClaw through:

```text
https://dashboard.3tched.com/gateway/
```

For this route to work as intended, clients MUST be on the WireGuard path where
`dashboard.3tched.com` resolves or routes to:

```text
10.0.0.1
```

The route MUST NOT depend on:

- `openclaw.3tched.com`
- container IP addresses
- public nginx dashboard listeners

WireGuard clients SHOULD get the WG address through NextDNS:

```text
dashboard.3tched.com -> 10.0.0.1
```

The public IPv4 listener is a certificate-correct sink and returns `404`; it is
not the gateway route.

### HTTP/WebSocket Proxy Contract

nginx MUST proxy `/gateway/` to:

```text
http://unix:/run/services0/gateway.sock:/
```

nginx MUST set:

```nginx
proxy_http_version 1.1;
proxy_set_header Upgrade $http_upgrade;
proxy_set_header Connection $connection_upgrade;
proxy_set_header Authorization "Bearer <redacted>";
proxy_read_timeout 7d;
```

The upstream OpenClaw gateway speaks HTTP/WebSocket over TCP loopback inside the
container. The Unix socket is an adapter layer, not an OpenClaw-native bind.

### Authentication

Access control is layered:

1. WireGuard `wg0` authenticates the device/session.
2. nginx only exposes the dashboard gateway route on `10.0.0.1`.
3. nginx injects an OpenClaw bearer token.
4. OpenClaw validates token auth.

The OpenClaw token is secret material and MUST NOT appear in this spec.

## Implemented Container Contract

### System Services Container

Container:

```text
services
```

Required interface state:

```text
/proc/net/dev contains lo only
```

Required Incus metadata:

```yaml
user.function: system-services
```

Required Incus socket mount:

```yaml
services0:
  type: disk
  source: /run/services0
  path: /run/services0
```

### User Container Template

Mock container:

```text
user-be7efab0cc2a
```

Required interface state:

```text
/proc/net/dev contains lo only
```

Required socket mount:

```yaml
sockets:
  type: disk
  source: /run/user-sockets/be7efab0cc2a
  path: /run/sockets
```

Required metadata:

```yaml
user.wg-pubkey: <wireguard-public-key>
```

Name derivation:

```text
container_name = "user-" + sha256(wg_pubkey)[0:12]
```

Magic-link workflow implementations MUST create user containers with this
contract unless a later spec explicitly replaces it.

## Process Contract

### OpenClaw Gateway Unit

Authoritative unit:

```text
/etc/systemd/system/openclaw-gateway.service
```

Required state:

```text
enabled
active/running
```

Required process owner:

```text
root
```

Required environment:

```text
HOME=/root
OPENCLAW_GATEWAY_PORT=18789
OPENCLAW_SYSTEMD_UNIT=openclaw-gateway.service
```

Expected listener:

```text
127.0.0.1:18789
::1:18789
```

### User-Level OpenClaw Unit

Non-authoritative unit:

```text
/home/jeremy/.config/systemd/user/openclaw-gateway.service
```

Required state:

```text
disabled
inactive
```

Reason: it uses `HOME=/home/jeremy` and can start a competing gateway process
with different config. That caused a real port conflict during verification.

### Socket Bridge Unit

Unit:

```text
/etc/systemd/system/openclaw-socket.service
```

Required state:

```text
enabled
active/running
```

Required bridge command:

```text
/usr/bin/socat UNIX-LISTEN:/run/services0/gateway.sock,fork,mode=660,gid=980 TCP:127.0.0.1:18789
```

The unit MUST remove `/run/services0/gateway.sock` before start.

## Filesystem Contract

Required host directories:

```text
/run/services0
/run/user-sockets
/run/user-sockets/<container-id>
```

Current verified modes:

```text
/run/services0                         drwxrwx--- root:_nginx
/run/user-sockets                      drwxr-xr-x
/run/user-sockets/be7efab0cc2a         drwxrwxrwx
/run/services0/gateway.sock            srw-rw---- root:_nginx
```

The `services0-sockets` dinit service recreates `/run/services0` on boot before
Incus and nginx use it.

## nginx Contract

Authoritative file:

```text
/etc/nginx/http.d/dashboard-3tched.conf
```

Required dashboard listeners for the gateway route:

```nginx
listen 10.0.0.1:80;
listen 10.0.0.1:443 ssl;
server_name dashboard.3tched.com;
```

Required public sink listeners:

```nginx
listen 148.113.204.83:80;
listen 148.113.204.83:443 ssl;
server_name dashboard.3tched.com;
return 404;
```

The public sink MUST use the `3tched.com` certificate that includes
`dashboard.3tched.com`. Its purpose is to avoid an unrelated default certificate
when public DNS is used by mistake. It MUST NOT proxy `/gateway/`.

Required gateway location:

```nginx
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
```

Disabled legacy file:

```text
/etc/nginx/http.d/openclaw-3tched.conf.disabled
```

No enabled nginx config SHOULD expose `openclaw.3tched.com`.

## WireGuard Contract

Host identity/session interface:

```text
wg0
```

Required state:

```text
10.0.0.1/24
listen port 51820
```

Known peer address model:

```text
10.0.0.2/32  tablet or user device
10.0.0.3/32  laptop or user device
10.0.0.4/32  legacy services peer
```

The services container currently does not require its own `wg0` for OpenClaw.
The gateway path terminates at host nginx and crosses into the container through
`/run/services0/gateway.sock`.

## NextDNS Contract

NextDNS profile:

```text
689ec7
```

Required DNS Rewrite:

```text
dashboard.3tched.com -> 10.0.0.1
```

The API helper is:

```sh
NEXTDNS_API_KEY=... NEXTDNS_PROFILE=689ec7 \
  deploy/nextdns/upsert-rewrite.py dashboard.3tched.com 10.0.0.1
```

At verification time `NEXTDNS_PROFILE` was present locally, but
`NEXTDNS_API_KEY` was not present.

## WARP / OVS / OpenFlow Contract

The privacy fabric is still part of the system design but is not the current
container service access mechanism for OpenClaw.

Current verified host interfaces:

```text
wgcf           172.16.0.2/32, WARP IPv6
ovsbr0         10.88.88.1/24
ovsbr0-mgmt    10.200.0.1/24
grpc-bridge    10.200.0.2/24
ovsbr0-sock    no IP
priv_wg        no IP
priv_warp      no IP
priv_xray      15.235.37.41/32
```

Target OpenFlow model:

```text
wgcf / WARP ingress
  -> ovsbr0
  -> OpenFlow policy
  -> priv_wg / priv_warp / priv_xray
  -> future dynamic sock_* container ports
```

Current OpenClaw model:

```text
wg0 client
  -> host nginx
  -> Unix socket file
  -> container loopback
```

Specs and code MUST NOT confuse these two paths.

## Network Installer Contract

The network installer is `deploy/deploy-network.sh`.

Default behavior:

- require existing `/etc/wireguard/wgcf.conf`;
- install `services0-sockets`, `wg-quick-all`, `op-ovs-services`,
  `systemd-networkd`, `netplan-apply`, and `ovs-attach-ports`;
- install `/etc/netplan/01-ovsbr0.yaml`;
- start `wgcf` through `wg-quick-all`;
- apply netplan so `ovsbr0` exists with OpenFlow settings;
- attach `wgcf`, `priv_wg`, `priv_warp`, `priv_xray`, `ovsbr0-sock`,
  `ovsbr0-mgmt`, and `grpc-bridge`;
- keep the `services` container loopback-only;
- install `openclaw-socket.service`;
- apply the NextDNS rewrite only when `NEXTDNS_API_KEY` is present.

One-off update flags:

```sh
WG_SERVER_PRIVATE_KEY_FILE=/etc/wireguard/wg0.netplan.key \
WG_SERVER_PEERS='pubkey1|10.0.0.2/32;pubkey2|10.0.0.3/32' \
  deploy/deploy-network.sh --update-wgconf

deploy/deploy-network.sh --update-wgcf
```

`--update-wgconf` writes the host `wg0` netplan tunnel and removes legacy
`wg-quick-wg0` dinit artifacts. `--update-wgcf` runs the `wgcf` CLI using the
existing account file and writes `/etc/wireguard/wgcf.conf`.

Normal runs MUST NOT regenerate `wg0`, rotate WGCF state, or rewrite lifecycle
WireGuard values.

## Verification Matrix

### Container No-Network Checks

```sh
incus list --format csv
incus exec services -- cat /proc/net/dev
incus exec user-be7efab0cc2a -- cat /proc/net/dev
```

Expected:

- `services` has no IPv4/IPv6 in `incus list`.
- `user-be7efab0cc2a` has no IPv4/IPv6 in `incus list`.
- both `/proc/net/dev` outputs show only `lo`.

### Gateway Process Checks

```sh
incus exec services -- systemctl is-enabled openclaw-gateway.service
incus exec services -- systemctl is-active openclaw-gateway.service
incus exec services -- ss -tlnp | grep 18789
```

Expected:

- enabled
- active
- listener on `127.0.0.1:18789` and optionally `::1:18789`
- process owner root

### Duplicate User Gateway Checks

```sh
incus exec services -- systemctl --user --machine=jeremy@ is-enabled openclaw-gateway.service
incus exec services -- systemctl --user --machine=jeremy@ is-active openclaw-gateway.service
incus exec services -- ps -ef | grep openclaw-gateway
```

Expected:

- disabled
- inactive
- no `jeremy` owned OpenClaw gateway process

### Socket Checks

```sh
doas ls -l /run/services0
doas curl -s -o /dev/null -w '%{http_code}' --unix-socket /run/services0/gateway.sock http://localhost/
```

Expected:

- `gateway.sock` exists
- HTTP `200`

### nginx Route Checks

```sh
doas nginx -t
curl -sk -o /dev/null -w '%{http_code}' https://10.0.0.1/gateway/ -H 'Host: dashboard.3tched.com'
curl -sk -o /dev/null -w '%{http_code}' https://148.113.204.83/gateway/ -H 'Host: dashboard.3tched.com'
```

Expected:

- nginx syntax OK
- WG address returns `200`
- public IP does not return the gateway route; current observed result is `404`

## Known Gaps

1. `privacy-xray-ingress` and `xray-server` still have `eth0` and are not yet
   socket-only containers.
2. The NextDNS rewrite is pending until `NEXTDNS_API_KEY` is available locally.
3. `wgcf` existed but its latest handshake was stale during verification. That
   affects the privacy fabric, not the verified WG-to-nginx-to-socket OpenClaw
   route.
4. The target OpenFlow `sock_*` dynamic container publication model is not the
   same as the current shared filesystem socket mount. Future docs should keep
   the distinction explicit.
5. Public nginx has other listeners for unrelated host sites. The security
   invariant is not "nginx has no public listeners"; it is "the dashboard
   gateway location is only in the `10.0.0.1` dashboard server block."

## Change Control

Any future change that touches the gateway path MUST update these files
together:

- `/home/jeremy/.kiro/specs/requirements.md`
- `/home/jeremy/.kiro/specs/design.md`
- `/home/jeremy/.kiro/specs/spec.md`

Any future change that introduces or removes an exposed route MUST also update:

- `/etc/nginx/http.d/dashboard-3tched.conf`
- Incus device metadata for affected containers
- systemd unit ownership notes
- verification matrix expected results
