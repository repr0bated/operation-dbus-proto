# install/ — Artix Linux s6 installer for 3tched / OP-DBUS

`3tched-artix-s6-install.sh` is the current, self-contained installer. It
targets a fresh Artix Linux **s6** system installed from
`artix-base-s6-20260402-x86_64.iso` and derives everything from the
`crates/` workspace — the `deploy/` tree is deprecated and is not consulted.

```bash
sudo ./install/3tched-artix-s6-install.sh                 # full install
sudo ./install/3tched-artix-s6-install.sh --skip-desktop  # headless host
sudo ./install/3tched-artix-s6-install.sh --help          # all options
```

## What it installs

- **Toolchain**: rustup (stable), clang/llvm/lld, protobuf (`protoc`), cmake,
  nodejs/npm, base-devel — everything needed to build the workspace.
- **The zbus pin**: clones a zbus checkout and guarantees the
  `[patch.crates-io]` path `/home/jeremy/git/zbus` exists (symlinked when the
  operator user differs). Override the fork with `ZBUS_GIT_URL=...`.
- **Runtime**: Open vSwitch, wireguard-tools, Incus, xray, dbus (+`dbus-s6`),
  `busd` (SESSION bus broker, via cargo), xdp-tools, btrfs-progs.
- **Desktop**: Hyprland + xdg-desktop-portal-hyprland, greetd/tuigreet on VT2,
  pipewire/wireplumber, waybar/foot/wofi/mako/grim/slurp, elogind (seatd
  fallback). `--with-headless-gui` adds weston + wayvnc services.
- **Workspace binaries** (built `--release`, installed to `/usr/local/bin`):
  `op-web-server`, `opdbus`, `projection_server`, `op-grpc-bridge`,
  `op-cognitive-mcp`, `op-mcp-compact`, `s6d`/`op-s6-systemctl`,
  `op-xray-daemon`, `op-of-controller`, `op-ovsbr0-setup`, `op-ovsbr0-afxdp`,
  `opblob`, `op-dbus-mirror`, and friends.

## s6 layout it generates

Service sources go to `/etc/s6/sv/<name>`, collected in a **`3tched` bundle**
added to the boot `default` bundle. Every longrun is paired with a dedicated
`<name>-log` s6-log consumer (`producer-for`/`consumer-for`/`pipeline-name`,
`notification-fd 3`) writing to `/var/log/op-dbus/<name>/` as the `s6log`
user with `n10 s4000000 T` rotation. Read logs through the control plane:
`s6d journalctl <svc>`.

Network is brought up entirely by s6 (matching the crates/op-network
contract):

```
opdbus-rundirs (oneshot: /run/opdbus, /dev/shm/opdbus/plugin-blobs)
└─ ovsdb-server ─ ovs-vswitchd (op-ovsbr0-setup --seed-only, netdev datapath)
   └─ ovsbr0-addr (oneshot: op-ovsbr0-setup, 10.200.0.1/24, NAT)
      ├─ op-of-controller (OpenFlow 1.3 @ 10.200.0.1:6653)
      ├─ incusd
      └─ opdbus (gRPC state manager @ ovsbr0:50051)
op-session-bus (busd @ unix:/run/opdbus/session-bus.sock)
├─ op-projection ─ op-web (:8080)
├─ op-grpc-bridge (127.0.0.1:18789)
├─ op-cognitive-mcp · op-mcp-compact · op-dbus-mirror · op-xray-daemon
wg-opdbus (wg0 identity tunnel 10.0.0.2 <-> 10.0.0.1; idles until peer set)
ovsbr0-uplink (AF_XDP cutover — installed, enabled only via --with-uplink)
```

Configuration lands in `/etc/op-dbus/environment` (canonical, read by
op-core) and `/etc/op-dbus/network.conf` (consumed by the network services
and op-network binaries). A `wg0.conf` is generated with fresh keys; the
tunnel idles until the `[Peer]` section is filled in.
