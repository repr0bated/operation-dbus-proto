# Deployment Scripts

Modular deployment system for op-dbus-v2.

## Quick Start

```bash
# Full installation
sudo ./deploy/install.sh

# With options
sudo ./deploy/install.sh --domain example.com
sudo ./deploy/install.sh --dry-run
sudo ./deploy/install.sh --skip-tls --skip-nginx
sudo ./deploy/install.sh --yes --domain example.com
```

## Scripts

| Script | Purpose |
|--------|--------|
| `install.sh` | Main installer - orchestrates everything |
| `upgrade.sh` | Rebuild and reinstall binaries |
| `uninstall.sh` | Remove installation |

## Dinit Runtime (Chimera)

Tracked standalone `dinit` runtime files live in `deploy/dinit/`.

```bash
doas ./deploy/dinit/install-op-dbus-dinit.sh
```

## Options

```
--dry-run       Preview without making changes
--skip-tls      Skip TLS setup
--skip-nginx    Skip nginx configuration  
--skip-systemd  Skip systemd service setup
--skip-build    Use existing binaries
--domain DOMAIN Set domain name
--user USER     Set service user
--yes           Non-interactive (skip confirmation)
```

## Directory Structure

```
deploy/
├── install.sh        # Main entry point
├── upgrade.sh        # Quick upgrade
├── uninstall.sh      # Clean removal
├── dinit/            # dinit service + env templates + wrappers
├── lib/              # Shared functions
│   ├── common.sh     # Colors, logging, utilities
│   ├── build.sh      # Build functions
│   ├── install-binaries.sh
│   ├── systemd.sh    # Service setup
│   ├── nginx.sh      # Web server config
│   └── tls.sh        # Certificate setup
└── README.md
```

## What Gets Installed

### Binaries (`/usr/local/sbin/`)
- `op-web-server` - Unified web/API/MCP server
- `op-dbus-service` - D-Bus + HTTP service
- `op-mcp-server` - MCP stdio adapter
- `dbus-agent` - D-Bus agent runner

### Configuration (`/etc/op-dbus/`)
- `op-web.env` - Environment variables

### Services
- `op-web.service`
- `op-dbus-service.service`

### Nginx
- `/etc/nginx/sites-available/op-web`
- `/etc/nginx/ssl/` - Certificates

### Logs (`/var/log/op-dbus/`)
- Service logs
- Nginx access/error logs

## Common Tasks

```bash
# Check status
systemctl status op-web op-dbus-service

# View logs
journalctl -u op-web -f

# Restart
systemctl restart op-web

# Upgrade
sudo ./deploy/upgrade.sh

# Uninstall (keep data)
sudo ./deploy/uninstall.sh --keep-data
```

## Environment Variables

Set before running or in `~/.bashrc`:

```bash
export DOMAIN=example.com
export SERVICE_USER=jeremy
export HF_TOKEN=your_token
export CF_DNS_ZONE_TOKEN=your_token
```

## Troubleshooting

### Build fails
```bash
# Check Rust
cargo --version

# Install dependencies
sudo apt install libdbus-1-dev pkg-config libssl-dev

# Clean build
cargo clean && cargo build --release
```

### Service won't start
```bash
journalctl -u op-web -n 50
ls -la /usr/local/sbin/op-*
cat /etc/op-dbus/op-web.env
```

### Nginx errors
```bash
nginx -t
ls -la /etc/nginx/ssl/
tail -f /var/log/op-dbus/nginx-error.log
```
