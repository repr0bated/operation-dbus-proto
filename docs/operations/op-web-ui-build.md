# Op-Web UI Authoritative Workflow

This is the only authoritative workflow for updating the live dashboard UI.

## Source of Truth

- The only source of UI updates is:

```text
https://github.com/3tched-com/operation-dashboard-ui.git
```

- Do not copy from a sibling checkout.
- Do not edit `crates/op-web/ui/dist` by hand.
- Do not treat any top-level static directory as authoritative.

## Embedded UI Path

The live dashboard is served from the `op-web-server` binary using embedded assets from:

```text
crates/op-web/ui/dist
```

Relevant code:

- [`crates/op-web/build.rs`](/home/jeremy/git/operation-dbus-proto/crates/op-web/build.rs)
- [`crates/op-web/src/embedded_ui.rs`](/home/jeremy/git/operation-dbus-proto/crates/op-web/src/embedded_ui.rs)

## Update Workflow

From repo root:

```bash
./update-ui.sh
cargo build --release -p op-web --bin op-web-server
sudo install -m 755 target/release/op-web-server /usr/local/sbin/op-web-server
sudo dinitctl stop op-chat
sudo dinitctl stop op-services
sudo dinitctl stop op-web
sudo dinitctl start op-web
sudo dinitctl start op-services
sudo dinitctl start op-chat
```

What `./update-ui.sh` does:

1. Clones the UI repo directly from GitHub into a temp directory
2. Syncs the clone into `crates/op-web/ui`
3. Builds `crates/op-web/ui/dist`
4. Deletes the temp clone on exit

## Full Deploy

If you want the full stack deploy path after syncing the UI:

```bash
sudo ./deploy/deploy.sh all
```

That path should also rebuild and install the embedded `op-web-server`, but the command sequence above is the shortest authoritative path for UI-only rollout and verification.

## Verification

Check the embedded bundle hash locally:

```bash
curl -sS http://127.0.0.1:8080/ | rg 'assets/index-[A-Za-z0-9_-]+\.(js|css)' -o
```

Check the public host:

```bash
curl -sS https://dashboard.3tched.com/ | rg 'assets/index-[A-Za-z0-9_-]+\.(js|css)' -o
```

The local and public hashes should match.

## Non-Authoritative Paths

These are not source-of-truth deployment paths:

- local sibling `operation-dashboard-ui` checkout
- manually editing `crates/op-web/ui/dist`
- any old top-level static UI directory
- assuming `cargo build` alone updates the live host without reinstalling and restarting `op-web`
