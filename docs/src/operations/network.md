# Network Operations

This page documents the release-owned OVS/OpenFlow path in the current tree.
The [host topology snapshot](host-socket-topology-live.md) is useful incident
history, but it was captured on 2026-07-22 and now disagrees with the release
configuration. Re-read the host before making topology changes:

```bash
ip -br address
ip route
sudo ovs-vsctl show
sudo incus config device show <container>
sudo sv status ovsdb-server ovs-vswitchd ovsbr0-uplink ovsbr0-eth0 \
  ovsbr0-svc-addr op-of-controller
```

## Release model

`deploy/config/network.conf` is installed as `/etc/op-dbus/network.conf` by
`deploy/runit/build-golden.sh`. Its current target is:

| Item | Release value |
| --- | --- |
| Bridge and datapath | `ovsbr0`, kernel (`system`) datapath, `standalone` fail mode |
| Physical uplink | `eth0` |
| Public internal port | `pub0`, `188.68.58.237/22`, gateway `188.68.56.1` |
| Internal ports | `3tched`, `svc0` |
| Service addresses | `10.0.0.2/24` and `10.200.0.2/24` on `3tched`; `10.0.0.3/24` on `svc0` |
| Bridge address | `10.200.0.1/24` |
| OpenFlow controller | OVS connects to `tcp:10.200.0.1:6653` |
| Accepted protocols | OpenFlow 1.0, 1.3, and 1.5 |

These are desired release values, not proof of live state. In particular, do
not carry the older snapshot's address ownership or `127.0.0.1:6653` listener
forward without checking the current host.

Container attachment remains mixed. Check each Incus device list before
assuming a container has a NIC, shared UDS mount, or TCP proxy. The shared
control-plane transports are `/run/opdbus/grpc.sock` for host clients and
`/run/ghostbridge/container.sock` for mounted containers.

Xray's live configuration inside its container must be
`/etc/xray/xray_config.json`. Do not point Xray at
`/dev/shm/xray_config.json`, `/usr/local/etc/xray/config.json`, or
`/etc/xray/config.json`.

## Boot and ownership

The current runit dependency chain is:

```text
opdbus-rundirs
  -> uplink-dhcp
  -> ovsdb-server
  -> ovs-vswitchd
  -> ovsbr0-uplink
  -> ovsbr0-eth0
  -> ovsbr0-svc-addr
  -> op-of-controller
```

`ovsdb-server` owns durable topology in `/etc/openvswitch/conf.db`.
`ovs-vswitchd` materializes that topology; its run script deliberately does
not recreate the bridge or ports. Topology changes must therefore update
OVSDB deliberately instead of adding repair commands to the service script.

The release config declares `OPENFLOW_CONTROLLER`, but the current
`op-of-controller` run script does not consume it and no runit helper calls
`set-controller`. A fresh OVSDB must be attached separately through the safe
attach path; an existing host normally retains its Controller row in
`conf.db`. Always verify:

```bash
sudo ovs-vsctl get-controller ovsbr0
sudo ovs-vsctl get Bridge ovsbr0 fail_mode
sudo ovs-vsctl get Bridge ovsbr0 protocols
```

An empty controller result means `op-of-controller` may be listening while
OVS never connects to it.

## What `op-of-controller` does

The controller is passive: OVS opens the TCP connection. On each connection
the controller:

1. negotiates OpenFlow 1.5 and enables asynchronous delivery with
   `miss_send_len=128`;
2. requests all port descriptions with `OFPP_ANY`, building name-to-ofport and
   name-to-MAC maps;
3. installs the cookied `priority=0,actions=NORMAL` fallback before removing
   only controller-managed flows;
4. reasserts static FDB pins from `OF_STATIC_FDB`;
5. translates and installs
   `/etc/op-dbus/openflow-static-flows.json`, then waits on a barrier; and
6. keeps the session alive with five-second OpenFlow echo requests.

The two reserved cookies are:

| Cookie | Purpose |
| --- | --- |
| `0x3344434800000001` | Host-safety `NORMAL` fallback |
| `0x3344434800000002` | Controller-managed port-pair and current release static flows |

The controller never bulk-deletes the table during reconnect. It deletes only
the managed cookie and preserves the fallback. This prevents the empty-table
window that can black-hole host management traffic.

Static flows may use symbolic port names. `port_mac:<name>` resolves a MAC
from the same port-description reply, so host-specific bridge MACs do not need
to be copied into release JSON. Unsupported match fields or actions are hard
errors rather than silently becoming broader flows. One invalid static flow
is logged and skipped; it does not abort the connection or remove the
fallback and FDB pins.

The vendored `rovs-openflow` supports the OpenFlow 1.5 `packet_type` match and
`encap(ethernet)` action. The old Netmaker PTAP rule and its watchdog are not
active in the current release: the rule was removed from
`openflow-static-flows.json` when that mesh path was retired, and no current
runit service calls `ensure-ptap-flow`.

## D-Bus surfaces

The controller backend owns:

```text
service:   org.opdbus.v1.plugins.openflow
path:      /org/opdbus/v1/plugins/openflow
interface: org.opdbus.v1.plugins.openflow
```

It exposes dynamic flow add/delete, in-memory flow tracking, datapath health,
and guarded controller attachment. Inspect the deployed contract before use:

```bash
busctl --system introspect org.opdbus.v1.plugins.openflow \
  /org/opdbus/v1/plugins/openflow
busctl --system call org.opdbus.v1.plugins.openflow \
  /org/opdbus/v1/plugins/openflow \
  org.opdbus.v1.plugins.openflow GetDatapathHealth s ovsbr0
```

`DumpFlows` reports only flows pushed through the controller handle since
process start. It is not a switch query. Use `ovs-ofctl` for read-only
operator verification.

The schema-driven `openflow` plugin is a separate front door. Its mutation
methods ultimately call this backend over D-Bus. The plugin's
`add_flow`/`delete_flow`/`modify_flow` declarations are validated contracts,
but direct dispatch currently rejects those three names; state application or
the backend `SendFlow`/`DeleteFlow` path performs the actual mutation.

## Verification after restart or release

```bash
sudo sv status ovsdb-server ovs-vswitchd op-of-controller
sudo ovs-vsctl get-controller ovsbr0
sudo ovs-vsctl --columns=target,is_connected,status list Controller
sudo ovs-ofctl -O OpenFlow15 dump-flows ovsbr0
sudo ovs-appctl fdb/show ovsbr0
sudo tail -n 100 /var/log/op-dbus/op-of-controller/current
```

Confirm all of the following:

- the controller endpoint is present and OVS reports a connected controller;
- the fallback cookie has `priority=0,actions=NORMAL`;
- expected static flows are present and counters move when exercised;
- the configured VRRP MAC is marked static in the FDB; and
- logs show discovered ports, installed/skipped static-flow counts, and a
  barrier reply rather than repeated reconnect errors.

## Troubleshooting

| Symptom | Likely cause | Check or action |
| --- | --- | --- |
| Controller listens but logs no OVS connection | No Controller row in persistent OVSDB | Check `get-controller`; use the guarded `AttachControllerSafe` D-Bus method during a maintenance window, not a bare attach |
| OVS immediately drops the controller session | Bridge protocol set excludes OpenFlow 1.5 | Ensure `protocols` contains `OpenFlow15` |
| Every symbolic flow is skipped | Port discovery returned no matching names | Compare controller's discovered-port log with `ovs-vsctl list-ports ovsbr0` |
| A static flow is skipped as unsupported | JSON requests a field/action the translator cannot encode | Fix the release JSON; the controller intentionally refuses a looser flow |
| Gateway traffic floods multiple ports | Static FDB pin is absent | Check `OF_STATIC_FDB`, controller reconnect logs, and `fdb/show` |
| Host traffic fails after controller changes | Fallback missing, fail mode changed, or in-band disabled | Remove the controller if necessary, restore standalone mode and the fallback, then use guarded attach |
| `DumpFlows` omits visible switch flows | It is process-local tracking, not a live dump | Use `ovs-ofctl -O OpenFlow15 dump-flows ovsbr0` |

Do not run an unfiltered `del-flows` on the live bridge. Do not set
`disable-in-band=true`, and do not make host L3 depend on `PACKET_IN`.

## Related and historical material

- [Host socket & network topology](host-socket-topology-live.md) — dated live
  inventory; re-verify before use.
- `docs/runbook-vps-checklist.md` — incidents and provider constraints.
- `docs/network-address-table.md` and
  `docs/operations/ghostbridge-incus-ovs-architecture.md` — older hosts and
  design history, not current operational authority.
