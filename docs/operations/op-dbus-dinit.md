# op-dbus Dinit Service

This document tracks the standalone `op-dbus` + `op-mcp-proxy` runtime setup for
Chimera Linux using `dinit` instead of `systemd`.

## Files in Repo

- `deploy/dinit/op-dbus`
- `deploy/dinit/op-session-bus`
- `deploy/dinit/op-ovsdb-bridge`
- `deploy/dinit/op-dbus-dinit.sh`
- `deploy/dinit/op-web-dinit.sh`
- `deploy/dinit/op-session-bus.sh`
- `deploy/dinit/op-networkd-dinit.sh`
- `deploy/dinit/op-ovsdb-bridge-start.sh`
- `deploy/dinit/systemd-networkd`
- `deploy/dinit/op-mcp-proxy-select3`
- `deploy/dinit/environment.op-dbus.template`
- `deploy/dinit/install-op-dbus-dinit.sh`
- `deploy/systemd/networkd/10-ens3.network`
- `deploy/systemd/networkd/20-ovsbr0.network`

## Install

```bash
cd /path/to/operation-dbus
doas ./deploy/dinit/install-op-dbus-dinit.sh
```

The installer writes:

- `/etc/dinit.d/op-dbus`
- `/etc/dinit.d/op-session-bus`
- `/etc/dinit.d/op-ovsdb-bridge`
- `/etc/dinit.d/systemd-networkd`
- `/etc/dinit.d/boot.d/op-dbus` symlink
- `/etc/dinit.d/boot.d/op-session-bus` symlink
- `/etc/dinit.d/boot.d/op-ovsdb-bridge` symlink
- `/usr/local/bin/op-dbus-dinit.sh`
- `/usr/local/sbin/op-dbus-dinit.sh`
- `/usr/local/sbin/op-web-dinit.sh`
- `/usr/local/sbin/op-session-bus`
- `/etc/dinit.d/scripts/op-ovsdb-bridge-start.sh`
- `/usr/local/bin/op-mcp-proxy-select3`
- `/usr/local/sbin/op-networkd-dinit.sh`
- `/etc/op-dbus/environment` (only if missing)
- `/etc/systemd/network/10-ens3.network`
- `/etc/systemd/network/20-ovsbr0.network`

## OVS Boot Protocol

`op-ovsdb-bridge` is idempotent at boot and uses `busctl` -> `org.opdbus` only:

- Creates `PRIVACY_BRIDGE_NAME` (default `ovsbr0`) if missing via `org.opdbus.OvsdbV1.CreateBridge`.
- Ensures `PRIVACY_UPLINK_PORT` is attached via `org.opdbus.OvsdbV1.AddPort`.
- Runs mirror reconcile via `org.opdbus.v1` at `/org/opdbus/v1` (with legacy fallback).

`systemd-networkd` is responsible for L3 on the restored OVS internal interface:

- `10-ens3.network` keeps the physical uplink unmanaged so OVSDB owns membership.
- `20-ovsbr0.network` assigns MAC, the current public `/32`, DNS, and default route on `ovsbr0`.
- `op-networkd-dinit.sh` renders `/run/resolvconf/resolv.conf` from the static `DNS=` lines before starting standalone `systemd-networkd`.
- The shipped template is aligned to the host state observed during debugging: `148.113.204.83/32` via `148.113.204.1`.
- Any extra public IPv4 aliases should be added deliberately after cutover rather than carried as defaults.

Important cutover rule:

- Do not deploy the networkd config onto a host where a non-OVS kernel link already exists with the same name as `PRIVACY_BRIDGE_NAME`.
- In that state, `op-ovsdb-bridge` now fails closed with a clear error instead of attempting an unsafe automatic conversion that could drop connectivity.
- The installer now enables `systemd-networkd` in dinit boot once the host is on the OVS bridge model. `op-ovsdb-bridge` restores the bridge and uplink first; `systemd-networkd` then applies only L3 to `ovsbr0`.

## Binary Paths

Install or update runtime binaries:

```bash
doas install -m 0755 target/release/op-dbus /usr/local/bin/op-dbus
doas install -m 0755 target/release/op-mcp-proxy /usr/local/bin/op-mcp-proxy
```

## Model Selection

`LLM_MODEL=auto` is constrained to Gemini 3 family:

- `gemini-3-flash`
- `gemini-3-pro`
- With preview mode enabled:
  - `gemini-3-flash-preview`
  - `gemini-3-pro-preview`

Selector thresholds are configured in `/etc/op-dbus/environment` with:

- `MCP_PROXY_AUTO_FLASH_MODEL`
- `MCP_PROXY_AUTO_PRO_MODEL`
- `MCP_PROXY_AUTO_PRO_THRESHOLD_CHARS`
- `MCP_PROXY_EXPERIMENTAL`

If `MCP_PROXY_EXPERIMENTAL` is not set, selector follows
`~/.gemini/settings.json` -> `general.previewFeatures`.

## Health Check

```bash
dinitctl status op-dbus
curl -fsS http://127.0.0.1:7010/api/health
```

## Reverse Proxy and TLS

Enable nginx at boot (dinit system instance):

```bash
doas ln -sfn ../nginx /etc/dinit.d/boot.d/nginx
doas dinitctl restart nginx || doas dinitctl start nginx
```

Install nginx config from repo:

```bash
doas install -m 0644 deploy/nginx/op-web-3etched.com.conf /etc/nginx/http.d/op-web-3etched.conf
doas nginx -t && doas nginx -s reload
```

Issue/expand cert to include dashboard:

```bash
doas certbot certonly --webroot -w /var/www/certbot \
  --cert-name 3tched.com \
  -d 3tched.com -d www.3tched.com -d dashboard.3tched.com --expand
doas dinitctl restart nginx
```
