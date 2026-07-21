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
- **Runtime**: Open vSwitch, Incus, xray, dbus (+`dbus-s6`), `busd` (SESSION
  bus broker, via cargo), btrfs-progs. No wireguard-tools and no AF_XDP
  tooling: the host runs no WireGuard (the netmaker mesh is self-contained in
  the netmaker container). The uplink NIC is enslaved into `ovsbr0` as a
  plain OVS port — **in the same OVSDB transact as bridge creation**
  (`op-ovsbr0-setup` `UPLINK` env; done as two steps the capture does not
  start correctly) — and `ovsbr0-addr` migrates its IPv4 + default route
  onto the bridge.
- **Desktop**: Hyprland + xdg-desktop-portal-hyprland, greetd/tuigreet on VT2,
  pipewire/wireplumber, waybar/foot/wofi/mako/grim/slurp, elogind (seatd
  fallback). `--with-headless-gui` adds weston + wayvnc services.
- **Workspace binaries** (built `--release`, installed to `/usr/local/bin`):
  `op-web-server`, `opdbus`, `projection_server`, `op-grpc-bridge`,
  `op-cognitive-mcp`, `op-mcp-compact`, `s6d`/`op-s6-systemctl`,
  `op-xray-daemon`, `op-of-controller`, `op-ovsbr0-setup`,
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
└─ ovsdb-server ─ ovs-vswitchd (op-ovsbr0-setup --seed-only: system bridge +
   │              UPLINK enslaved in ONE atomic OVSDB transact)
   └─ ovsbr0-addr (oneshot: op-ovsbr0-setup, 10.200.0.1/24, uplink IP
      │            migration, NAT, route 10.0.0.0/24 dev ovsbr0)
      ├─ op-of-controller (OpenFlow 1.3 @ 10.200.0.1:6653)
      ├─ incusd
      │  └─ incus-ct-<name> (per-container s6 longruns, see below)
      └─ opdbus (gRPC state manager @ ovsbr0:50051)
op-session-bus (busd @ unix:/run/opdbus/session-bus.sock)
├─ op-projection ─ op-web (:8080)
├─ op-grpc-bridge (127.0.0.1:8090; Xray publishes it on the uplink)
├─ op-cognitive-mcp · op-mcp-compact · op-dbus-mirror · op-xray-daemon
```

There is **no WireGuard and no AF_XDP on the host**. The netmaker mesh is
self-contained in the netmaker container: its bridge interface carries both
`10.0.0.2` and `10.200.0.2`, the WG protocol terminates at the decoy server,
and `10.0.0.2` egress forwards to host xray for identity header injection —
traffic leaves xray as gRPC with the header.

## Incus containers under s6

Containers come up with **s6, not Incus autostart** (`boot.autostart` is
forced off). The installer ships `3tched-incus-svcgen`, which generates an
`incus-ct-<name>` longrun + s6-log consumer per container — the container
console streams into `/var/log/op-dbus/incus-ct-<name>/`, and stopping the
s6 service stops the container. The installer runs it for every existing
container; run it yourself after creating new ones:

```bash
3tched-incus-svcgen <name>                          # any container
3tched-incus-svcgen netmaker --attach-bridge ovsbr0 # netmaker
```

With `--attach-bridge`, the container joins the OVS bridge **last**: the run
script starts the container, waits for the systemd inside it to settle
(`systemctl is-system-running --wait`, 180 s cap), and only then adds its
`eth0` to the bridge.

Configuration lands in `/etc/op-dbus/environment` (canonical, read by
op-core) and `/etc/op-dbus/network.conf` (consumed by the network services
and op-network binaries).
