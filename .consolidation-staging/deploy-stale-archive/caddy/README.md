# Caddy Reverse Proxy Configuration for op-web

## Overview

This directory contains Caddy configuration for serving `op-web.ghostbridge.tech` behind a reverse proxy. The op-web server runs on localhost:8080 and is exposed through Caddy for SSL termination and static asset serving.

## Files

- `op-web-ghostbridge.conf` - Caddy site configuration
- `README.md` - This file

## Installation

### 1. Copy Configuration

```bash
sudo cp deploy/caddy/op-web-ghostbridge.conf /etc/caddy/conf.d/
```

Or add to main Caddyfile:
```bash
sudo cat deploy/caddy/op-web-ghostbridge.conf >> /etc/caddy/Caddyfile
```

### 2. Reload Caddy

```bash
sudo systemctl reload caddy
# or
sudo caddy reload --config /etc/caddy/Caddyfile
```

### 3. Verify DNS

Ensure `op-web.ghostbridge.tech` resolves to your Proxmox host IP:
```bash
nslookup op-web.ghostbridge.tech
```

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Proxmox Host                              │
│  ┌───────────────────────────────────────────────────────┐  │
│  │  Caddy (Shared Web Server)                            │  │
│  │  - SSL termination                                    │  │
│  │  - Reverse proxy to localhost:8080                    │  │
│  │  - Static asset caching                               │  │
│  └────────────────────┬──────────────────────────────────┘  │
│                       │                                      │
│                       │ localhost:8080                       │
│                       ▼                                      │
│  ┌───────────────────────────────────────────────────────┐  │
│  │  op-web (Rust/Axum)                                   │  │
│  │  - Serves MCP Control Center UI                       │  │
│  │  - MCP endpoints (/mcp/compact, /mcp/agents)          │  │
│  │  - API endpoints (/api/*)                             │  │
│  │  - WebSocket (/ws)                                    │  │
│  └───────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

## Features

- **Automatic HTTPS**: Caddy handles Let's Encrypt certificate provisioning
- **WebSocket Support**: Real-time chat via `/ws` endpoint
- **Compression**: gzip/zstd for better performance
- **Security Headers**: HSTS, XSS protection, clickjacking prevention
- **Static Asset Caching**: 24-hour cache for CSS/JS/WASM files
- **Client IP Forwarding**: Preserves original client IP for logging

## Troubleshooting

### Check Caddy Status
```bash
sudo systemctl status caddy
sudo journalctl -u caddy -f
```

### Test op-web Directly
```bash
curl http://localhost:8080/api/health
```

### Test Through Caddy
```bash
curl https://op-web.ghostbridge.tech/api/health
```

### View Logs
```bash
sudo tail -f /var/log/caddy/op-web-ghostbridge-access.log
```

## Port Configuration

| Service | Port | Description |
|---------|------|-------------|
| Caddy (HTTP) | 80 | Redirects to HTTPS |
| Caddy (HTTPS) | 443 | Main entry point |
| op-web | 8080 | Internal, localhost only |

## Security Notes

- op-web binds to `0.0.0.0:8080` but should be firewalled externally
- Only Caddy should have external access (ports 80/443)
- Consider binding op-web to `127.0.0.1:8080` for additional security