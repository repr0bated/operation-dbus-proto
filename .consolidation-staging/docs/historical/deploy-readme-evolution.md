# Historical Deploy Structure Evolution

This document preserves the historical deployment structure from the systemd-based operation-dbus-proto repository before migration to Artix s6.

## Original Modular Deployment System

The deployment system evolved from a monolithic install script to a modular structure with shared libraries and separate concerns.

### Scripts Organization

| Script | Purpose |
|--------|--------|
| `install.sh` | Main installer - orchestrates everything |
| `upgrade.sh` | Rebuild and reinstall binaries |
| `uninstall.sh` | Remove installation |

### Directory Structure

The deploy directory used a modular structure separating concerns:

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

### Installation Options

The installer supported multiple deployment scenarios through flags:

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

### Quick Start Patterns

```bash
# Full installation
sudo ./deploy/install.sh

# With options
sudo ./deploy/install.sh --domain example.com
sudo ./deploy/install.sh --dry-run
sudo ./deploy/install.sh --skip-tls --skip-nginx
sudo ./deploy/install.sh --yes --domain example.com
```

### Dinit Runtime Support (Chimera Linux)

The deployment system included standalone dinit runtime support for Chimera Linux:

```bash
doas ./deploy/dinit/install-op-dbus-dinit.sh
```

Tracked dinit runtime files lived in `deploy/dinit/`.

### What Was Installed

**Binaries** (`/usr/local/sbin/`):
- `op-web-server` - Unified web/API/MCP server
- `op-dbus-service` - D-Bus + HTTP service
- `op-mcp-server` - MCP stdio adapter
- `dbus-agent` - D-Bus agent runner

**Configuration** (`/etc/op-dbus/`):
- `op-web.env` - Environment variables

**Logs** (`/var/log/op-dbus/`):
- Service logs
- Nginx access/error logs

**Nginx Configuration**:
- `/etc/nginx/sites-available/op-web`
- `/etc/nginx/ssl/` - Certificates

### Environment Variables Pattern

Configuration was externalized via environment variables:

```bash
export DOMAIN=example.com
export SERVICE_USER=jeremy
export HF_TOKEN=your_token
export CF_DNS_ZONE_TOKEN=your_token
```

### Troubleshooting Structure

**Build failures** were addressed through dependency verification:
```bash
# Check Rust
cargo --version

# Install dependencies
sudo apt install libdbus-1-dev pkg-config libssl-dev

# Clean build
cargo clean && cargo build --release
```

**Nginx errors** were debugged through:
```bash
nginx -t
ls -la /etc/nginx/ssl/
tail -f /var/log/op-dbus/nginx-error.log
```

## Migration Notes

The current Artix s6 deployment replaces:
- systemd services → s6 service directories
- `systemctl` commands → `service6` wrapper
- `/etc/systemd/system/` → `/etc/s6/sv/`
- journalctl → s6 log directories

The modular structure (lib/*.sh, separate concerns) was preserved, but the service setup layer was completely rewritten for s6.

<!-- Extracted from /mnt/opt-inspect/home/git/operation-dbus-proto/deploy/README.md on 2026-07-20 -->
