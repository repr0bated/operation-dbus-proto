# Control-plane network startup

Production boot reconciliation for the Artix/s6 control-plane network. Linux
links, addresses, routes, and WireGuard are configured with `ip`/`wg`; every
OVSDB and OpenFlow read or mutation goes through the projected D-Bus plugins.
There is no `ovs-vsctl` or `ovs-ofctl` fallback.

## Persistent policy versus boot state

`/etc/control-plane-network/network.conf` is persistent policy. The installer
creates it only when it is missing and never overwrites it on upgrades. Every
boot reads that file and reconciles the live state; it does not regenerate the
file. `/etc/wireguard/netmaker.conf` is also persistent and is the only source
of WireGuard key material.

The following are refreshed runtime artifacts:

- `/var/lib/control-plane-network/`: lock, heartbeat, DNS fragment, and
  readiness/failure markers.
- `/var/log/control-plane-network/`: mode-specific startup reports.
- `/etc/dhcpcd.conf`: one persistent, idempotent `denyinterfaces <uplink>` edit
  when `TAME_DHCPCD=yes`.

The privileged config must be a root-owned regular file and must not be
group/world writable. The current packaged example is always installed at
`/usr/share/control-plane-network/network.conf.example`.

## Boot lifecycle

The lifecycle is deliberately split to avoid a service dependency cycle:

```text
control-plane-network
  -> op-of-controller-srv
       -> control-plane-network-flows
```

The bootstrap oneshot creates/configures `ovsbr0` and attaches every
`BRIDGE_SYSTEM_PORTS` member (normally `eth0 netmaker`) with one
`rovs_commands.ensure_bridge_ports` call and one OVSDB transaction. It also
reconciles addresses/routes, WireGuard, the bridge protocols/controller, DNS,
and validates the resulting topology.

The flow oneshot waits for `org.opdbus.of_controller`, then installs the
netmaker packet-type flows and governed-socket identity flows. Controller
startup therefore never depends on calls to its own not-yet-running D-Bus
service.

Uplink attachment is guarded by a gateway preflight and bounded post-attach
probes. If a run newly attached the uplink and any critical post-attach step
fails or the process receives a signal, it removes only that new port and
reapplies the plain-interface address/route batch. An uplink that was already
attached before the run is never detached by rollback.

## Install and validate

```sh
sudo ./install.sh
sudo /usr/local/sbin/control-plane-network --check-config

sudo s6 set disable ovsbr0-controller
sudo s6 set enable control-plane-network op-of-controller-srv control-plane-network-flows
sudo s6 set commit -D default && sudo s6 live install
```

`op-of-controller-srv` is supplied by the main repo deployment and must exist
before the flow service is enabled. `ovsbr0-controller` remains as a
compatibility wrapper but now delegates to the same D-Bus implementation; new
deployments should leave it disabled.

The s6 `commit` and live install commands must remain back-to-back. Do not
live-test a first-time `eth0` attachment without console/noVNC access.

Manual modes:

```sh
sudo control-plane-network                 # bootstrap only
sudo control-plane-network --flows-only    # after controller readiness
sudo control-plane-network --controller-only
sudo control-plane-network --check-config  # read-only
```

Readiness markers are published atomically only after the relevant mode fully
succeeds:

- `<role>.ready`
- `<role>.flows-ready`
- `<role>.controller-ready`

A failed run removes the corresponding ready marker and writes
`<role>.<mode>.failed`; it never leaves a stale success marker.

## gRPC and Unix socket ownership

This package does not bind application sockets. It supplies the host network,
service addresses, controller attachment, and governed OpenFlow policy that
the socket services require. Socket files must be created by their supervised
longruns so a pathname can never report ready without a process listening.

The intended Ghostbridge chain is:

```text
op-grpc-bridge-zeroclaw
  owns /run/ghostbridge/grpc-bridge.sock
       -> op-ghostbridge-mux-srv
            owns /run/ghostbridge/container.sock
```

`op-grpc-bridge-zeroclaw` depends on the network bootstrap; the mux depends on
both. A missing `grpc-bridge.sock` therefore means the gRPC service definition
has not been deployed/restarted with the internal socket path, not that the
network bootstrap should create a placeholder. Verify ownership with
`ss -xlpn` and service state with `s6-svstat /run/service/<service>`.
