# Netclient Container Netns — Design

## 1 · Architecture

```text
netmaker container
  netclient
      │ peer UDP
      ▼
  netmk 10.200.1.1/30
      │ default via 10.200.1.2
      ▼
──────────────────────────── ovsbr0 ─────────────────────────────
      │
      ▼
xray container, current OVS-facing interface
  existing 10.200.0.1
  secondary 10.200.1.2/30
  ip_forward=1, send_redirects=0
      │ L3 forward to existing next hop 10.200.0.2
      ▼
──────────────────────────── ovsbr0 ─────────────────────────────
      │
      ▼
host svc0 10.200.0.2
  mangle: all 10.200.1.1 traffic → mark 0x51821
  policy: mark 0x51821 → existing table 51820
  filter: marked, approved WireGuard UDP only
  NAT: approved flow only on wgcf-egress
      │
      ▼
wgcf-egress (host, existing and self-contained)
  config / FwMark 0x51820 / table 51820 owned elsewhere
      │
      ▼
Internet peer
```

Two independent controls enforce the path:

1. Netmaker's only default gateway is xray, so candidate traffic cannot go
   directly to the host.
2. After xray returns the packet to host `svc0`, mandatory mark `0x51821`
   selects the existing wgcf table. A source-specific blackhole prevents
   unmarked or failed-lookup fallback.

This is the operator-required reality/egress-signature path. The mark does not
create the signature by itself; it makes the selected xray-plus-wgcf path
non-bypassable.

---

## 2 · Ownership Boundaries

### This feature owns

- OVS internal port `netmk` and its OpenFlow cookie namespace.
- Netmaker namespace address and default route.
- Secondary xray gateway address `10.200.1.2/30`.
- Xray forwarding/redirect sysctls required for the new subnet.
- Dedicated xray chain `OP_NETMK_XRAY_FWD`.
- Host return route for `10.200.1.0/30`.
- Host chains `OP_NETMK_MARK`, `OP_NETMK_FWD`, `OP_NETMK_NAT`.
- Feature-created policy rules at priorities `10518` and `10519`.
- Adapter socket, loopback bridge, dedicated client endpoint, and runit gates.

### This feature observes but does not own

- Host interface `wgcf-egress`.
- `/etc/wireguard/wgcf-egress.conf`.
- Underlay FwMark `0x51820`.
- Existing table `51820` routes and the wgcf owning service.
- Existing xray addresses, routes, application configuration, and proxy paths.
- Existing `pub0`, `svc0`, and host `3tched` state.

Reconciliation never “repairs” observed-only state. It fails readiness and
identifies the external owner.

---

## 3 · Phase 1 Detailed Design

### 3.1 OVS internal port

Call native `rovs_commands.add_port` with bridge `ovsbr0`, port `netmk`, and
`interface_type="internal"`. The current runtime dispatcher must be fixed to
honor `interface_type`; reflected schema metadata is not enough.

After creation:

1. Query OVSDB to prove type and bridge membership.
2. Resolve the current netmaker init PID.
3. Move `netmk` with `IFLA_NET_NS_PID`.
4. Configure `10.200.1.1/30`, link UP, and default via `10.200.1.2`.
5. Verify through an independent namespace-local read.

### 3.2 Safe cross-netns rtnetlink

Linux network namespace membership is thread-scoped. Calling `setns()` around
async awaits can migrate unrelated Tokio work into the wrong namespace.

Use a dedicated OS thread per transaction:

1. Open original and target netns descriptors.
2. Enter target netns.
3. Create the rtnetlink socket after entry.
4. Perform all namespace-local operations on that thread.
5. Verify the resulting state.
6. Restore the original namespace or terminate the thread.
7. Send typed results back to async callers.

The rtnetlink plugin needs real dispatch for move, address, link, default route,
and host return route methods. Each new mutation receives an OSCAL subid and
disposable-netns test coverage.

### 3.3 Xray gateway

Discover xray's current OVS-facing interface and current init PID. Add
`10.200.1.2/30` without replacing any existing state. Persist:

```text
net.ipv4.ip_forward=1
net.ipv4.conf.<inside>.send_redirects=0
```

The same interface receives and transmits the forwarded packet. Disabling
redirects is mandatory: otherwise Linux may teach netmaker a next hop that
bypasses xray.

`OP_NETMK_XRAY_FWD` permits one outbound UDP rule per live peer endpoint port,
permits established/related return packets, and drops other traffic from
`10.200.1.1/32`. Xray performs no NAT for this subnet.

### 3.4 Host mark, route, and fail-closed policy

Add the return route:

```text
10.200.1.0/30 via 10.200.0.1 dev svc0
```

Packets returned by xray arrive through `svc0`. A PREROUTING jump marks every
packet from `10.200.1.1/32` as `0x51821/0xffffffff` before route lookup.
Protocol/port filtering happens later.

Policy priorities are feature-owned and reserved:

```text
10518: fwmark 0x51821/0xffffffff lookup 51820
10519: from 10.200.1.1/32 blackhole
```

Priority 10518 consumes the existing table without changing it. Priority 10519
is immediately after it, so an unusable lookup or missed mark cannot continue
to the main table. Table `51820` is verified but never mutated.

Host filter and NAT rules require the mark and approved UDP endpoint port.
Every other packet from the source is dropped. NAT is applied only with
`-o wgcf-egress`.

Dedicated chain ownership allows deterministic reconciliation:

- create if absent;
- flush only the feature chain;
- ensure exactly the expected jump set for each table and direction;
- repopulate from current peer ports;
- on teardown remove every owned jump before deleting its chain.

No broad built-in-chain flush is allowed.

### 3.5 Self-contained wgcf prerequisite

The host's existing `wgcf-egress` service owns its interface, underlay mark
`0x51820`, table routes, and configuration. This feature performs read-only
checks:

- interface exists and is UP;
- a recent handshake is present;
- table `51820` has a usable default path through `wgcf-egress`;
- underlay mark `0x51820` remains unchanged;
- no route operation in this feature targets table `51820`.

The feature separately owns priorities 10518/10519; those rules consume the
existing table but are not part of wgcf configuration or lifecycle.

A failed check blocks readiness. The feature does not invoke `sv` against the
wgcf owner and does not edit its files.

### 3.6 OpenFlow policy

Feature flows use cookie prefix `0x4e4d4b0000000000` and match `in_port=netmk`:
ARP allow, one UDP destination-endpoint-port allow per live peer, then catch-all
drop. A source-port-only allow is prohibited because it would permit arbitrary
destinations. Existing priority-0 `NORMAL` remains for other ports.

The existing controller's `dump_flows()` is process memory, not switch state.
Complete native verification by adding OF1.3 `OFPMP_FLOW` multipart handling to
the active controller connection and exposing parsed results over D-Bus as
`DumpSwitchFlows`. `netmk-of-restrict` filters the result by cookie prefix and
compares normalized desired/actual entries.

`OvsNetlinkClient::dump_flows()` provides a second layer after traffic probes.
It proves datapath behavior and counters but cannot replace logical table
verification because kernel megaflows are demand-created caches.

### 3.7 Adapter transport and endpoint

`op-grpc-adapters` already binds only `ADAPTERS_SOCKET`. Run it inside
netmaker with:

```text
ADAPTERS_SOCKET=/var/lib/opdbus-runtime/netmaker/op-grpc-adapters.sock
```

The runtime directory is bind-mounted to the host. Use the proven
`qdrant-grpc-loopback` runit pattern for service
`netmaker-adapter-loopback`:

```text
wait for socket
exec socat TCP-LISTEN:50061,bind=127.0.0.1,reuseaddr,fork \
           UNIX-CONNECT:/var/lib/opdbus-runtime/netmaker/op-grpc-adapters.sock
```

Port `50054` is already assigned to the memory agent; repository search found
`50061` unallocated, so `50061` is fixed by this spec.

Add `NETMAKER_ADAPTER_ADDR=http://127.0.0.1:50061`. Extend
`RemoteOperationClient` with a separate `netmaker_address`; do not repoint
`default_address`. All Netmaker requests attach Ghostbridge metadata before
calling the adapter.

### 3.8 OCI reconciliation

Normalize on `port_name`, then implement real calculate/apply/verify behavior.
The current schema-only `port_attach` example does not reconcile OVS,
namespace, address, or route state and therefore does not satisfy Phase 1.

### 3.9 Runit graph

```text
existing xray + existing wgcf-egress
            │
            ▼
netmk-egress-policy
            │
            ▼
netmk-port-attach
            │
            ▼
netmk-of-restrict ───────────────────────────┐
                                             ├──► netmk-netclient-start
op-grpc-adapters socket ─► adapter-loopback ─┘
```

`netmk-egress-policy` owns only integration state. Its check of wgcf is
read-only. Every runit stage removes stale readiness before work, has bounded
waits, exits non-zero for retry, and invalidates downstream readiness after
container PID or OVS ofport changes.

---

## 4 · Failure Modes

| Failure | Required behavior |
| --- | --- |
| External wgcf prerequisite absent | Fail `netmk-egress-policy`; report missing interface/rule/table/handshake; make no wgcf mutation. |
| `netmk` exists with wrong type | Reconcile through native OVSDB or fail before namespace move. |
| Container PID changes | Abort stale transaction, resolve fresh PID, retry boundedly. |
| Namespace worker fails | Restore/terminate worker; no async runtime thread remains in target netns. |
| Xray sends redirect | Readiness fails until `send_redirects=0` is verified. |
| Mark rule absent | Reconcile feature-owned priority 10518; never modify table 51820. |
| Table lookup unusable | Priority 10519 blackholes the source; netclient is not started. |
| Unapproved flow | OpenFlow, xray filter, and host filter independently deny it. |
| Controller memory disagrees with switch | Switch multipart state wins; reconcile feature-cookie flows. |
| Adapter socket/bridge unavailable | Join controls remain unavailable; no restart storm. |
| Phase 2 probe fails | Restore veth NIC and interface-scoped rules immediately. |

---

## 5 · Security and Validation Model

The design uses defense in depth:

1. `/30` default gateway forces xray traversal.
2. Xray filter restricts source/protocol/ports.
3. Host marks every returned source packet.
4. Mark selects the existing privacy-egress table.
5. Terminal source rule prevents main-table fallback.
6. Host filter and wgcf-only NAT enforce the approved flow.
7. OpenFlow blocks other traffic at `netmk` ingress.

Mutation responses are never proof by themselves. Each stage uses a separate
read path: OVSDB for interface type/membership, namespace rtnetlink reads for
addresses/routes, netfilter/rule inventory for policy, OF multipart for switch
flows, kernel OVS netlink after probes, adapter health plus process state, and
`wg show` for the final handshake.

---

## 6 · Phase 2 Design

After 48 stable hours and approval:

1. Capture xray's live interface, Incus NIC definition, MAC, MTU, addresses,
   routes, and `OP_NETMK_XRAY_FWD` jump interface.
2. Create idle OVS internal port `xray0` through native OVSDB.
3. Prove move/configure/rollback on a non-production container.
4. In the maintenance window, move `xray0`, restore captured state including
   `10.200.0.1` and `10.200.1.2/30`, retarget only interface-scoped rules, and
   remove the veth-backed NIC.
5. Probe every xray domain and the netclient handshake.
6. Roll back immediately if either traffic class fails.

Host `svc0`, host marks/chains, policy priorities, table `51820`, and
`wgcf-egress` do not change during Phase 2.
