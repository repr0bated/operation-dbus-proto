# Op-Web Status

This file is operational status, not deployment guidance.

For the authoritative UI deployment workflow, use:

- [`update-ui.sh`](/home/jeremy/git/operation-dbus-proto/update-ui.sh)
- [`docs/operations/op-web-ui-build.md`](/home/jeremy/git/operation-dbus-proto/docs/operations/op-web-ui-build.md)

Current live UI verification should be done with:

```bash
curl -sS https://dashboard.3tched.com/ | rg 'assets/index-[A-Za-z0-9_-]+\.(js|css)' -o
```

The public dashboard is served by the embedded UI in `op-web-server`, not by a separate static web root.

