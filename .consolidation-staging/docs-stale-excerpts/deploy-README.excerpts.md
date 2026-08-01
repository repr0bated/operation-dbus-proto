# Dropped Excerpts from deploy/README.md

These sections were systemd-specific and not migrated to the Artix s6 deployment documentation.

## Common Tasks (systemd-specific)

**Dropped because:** These commands are systemd-specific and replaced by `service6` wrapper and s6 tooling in Artix.

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

**Artix s6 equivalents:**
- `systemctl status` → `service6 status <service>`
- `journalctl -u` → read s6 log directories directly
- `systemctl restart` → `service6 restart <service>`

## Service Troubleshooting (systemd-specific)

**Dropped section:** Service won't start troubleshooting using systemd tooling

```bash
journalctl -u op-web -n 50
ls -la /usr/local/sbin/op-*
cat /etc/op-dbus/op-web.env
```

**Note:** The file listing and env inspection remain valid; only the journalctl command is systemd-specific.

## Services Installed (systemd-specific)

**Dropped items from "What Gets Installed" section:**

```
### Services
- `op-web.service`
- `op-dbus-service.service`
```

**Artix s6 equivalent:** Service bundles in `/etc/s6/sv/op-web/`, `/etc/s6/sv/op-dbus-service/`, each containing `run` and `log/run` scripts instead of systemd unit files.

---

<!-- Extracted from /mnt/opt-inspect/home/git/operation-dbus-proto/deploy/README.md on 2026-07-20 -->
