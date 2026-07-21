# Deployment Scripts

Modular deployment system for op-dbus-v2.

## Scripts

| Script | Purpose |
|--------|---------|
| `install.sh` | Main installer - orchestrates everything |
| `upgrade.sh` | Rebuild and reinstall binaries |
| `uninstall.sh` | Remove installation |

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

<!-- Extracted from deploy/README.md on 2026-07-20 -->
