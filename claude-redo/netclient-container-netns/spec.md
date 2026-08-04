# Netclient Container Netns — Interface Contract

> Activation status: blocked after the 2026-08-03 network outage. This file is
> the target contract, not a statement that the topology is currently live.
> The operator deliberately deleted the OVS bridge to regain connectivity;
> direct `eth0` is the protective fallback and must not be undone remotely.
> Base OVS recovery and the FR-0/R-* gates in `requirements.md`/`tasks.md` must
> pass before any feature service is enabled.

---

## 1 · Fixed Names and Values

| Item | Value |
| --- | --- |
| OVS bridge | `ovsbr0` |
| Netmaker port | `netmk` (`type=internal`) |
| Netmaker address | `10.200.1.1/30` |
| Netmaker effective MTU | `1280` for the current `wgcf-egress` path |
| Xray gateway address | `10.200.1.2/30` |
| Existing xray fabric address | `10.200.0.1` |
| Xray Phase 1 interface | `grpc0` (discover by preserved fabric address) |
| Host fabric address/interface | `10.200.0.2`, `svc0` |
| Netclient payload mark | `0x51821/0xffffffff` |
| Existing wgcf underlay mark | `0x51820` — observe only |
| Existing policy table | `51820` — observe only |
| Netclient policy priorities | `10518` lookup, `10519` blackhole |
| Adapter UDS | `/var/lib/opdbus-runtime/netmaker/op-grpc-adapters.sock` |
| Adapter loopback | `http://127.0.0.1:50061` |
| Adapter environment | `NETMAKER_ADAPTER_ADDR` |
| OpenFlow cookie prefix | `0x4e4d4b0000000000/0xffffff0000000000` |
| Ready directory | `/run/opdbus/runit-ready/` |

No placeholder values remain in this contract. Live peer endpoint ports and
container init PIDs are intentionally discovered values.

---

## 2 · Namespace and OVS Contract

### `netmk`

```text
Bridge:          ovsbr0
Port name:       netmk
OVS type:        internal
Namespace:       current netmaker init PID
Address:         10.200.1.1/30
State:           UP
Default route:   via 10.200.1.2 dev netmk
MTU:             1280 for current selected upstream
```

### Xray gateway, Phase 1

```text
Interface:       discover current OVS-facing interface (current target grpc0)
Namespace:       current xray init PID
Existing IP:     10.200.0.1
Added IP:        10.200.1.2/30
Forwarding:      net.ipv4.ip_forward=1
Redirects:       net.ipv4.conf.<inside>.send_redirects=0
Host next hop:   10.200.0.2
```

### Host return route

```text
10.200.1.0/30 via 10.200.0.1 dev svc0
```

### Mutation invariants

- Create `netmk` through `rovs_commands.add_port` with
  `interface_type="internal"`; runtime dispatch must honor the field.
- Move it with `RTM_NEWLINK` + `IFLA_NET_NS_PID`.
- Configure addresses, state, and routes through rtnetlink.
- Do not address the `ovsbr0` bridge device.
- Preserve all pre-existing xray interface properties.
- Resolve fresh Incus init PIDs on every reconciliation.
- Execute target-netns operations on a dedicated OS thread. Enter netns, create
  the netlink socket there, perform all operations, restore/terminate, and only
  then return to async code.

---

## 3 · Existing wgcf Dependency Contract

```text
Namespace:          host
Interface:          wgcf-egress
Configuration:      /etc/wireguard/wgcf-egress.conf
Underlay FwMark:    0x51820 — external
Policy table:       51820 — external routes
Integration mark:   0x51821 — feature-owned priority-10518 rule
Ownership boundary: interface/config/table/lifecycle external; integration rule local
```

This feature may read interface state, latest handshake, existing rules, and
table routes. It must not:

- write the configuration file;
- invoke interface up/down;
- create/delete `wgcf-egress`;
- modify underlay mark `0x51820`;
- add, delete, flush, or replace routes in table `51820`;
- restart or reconfigure the owning service.

Missing prerequisite state is a readiness failure with an actionable message;
it is not repaired by this feature.

---

## 4 · Forced Egress Contract

### Packet path

```text
netclient peer UDP
  → netmk 10.200.1.1
  → default gateway 10.200.1.2 (xray)
  → xray L3 forward, same OVS-facing interface
  → xray existing next hop 10.200.0.2
  → host svc0 PREROUTING
  → mark 0x51821
  → existing table 51820
  → wgcf-egress
  → Internet
```

Return traffic is reverse-translated on the host, routed through
`10.200.1.0/30 via 10.200.0.1 dev svc0`, forwarded by xray, and delivered by
OVS to `netmk`.

### Xray-owned integration chain

Dedicated chain: `OP_NETMK_XRAY_FWD` in filter table.

Logical rules are generated from the single approved `PolicyClass` manifest in
`requirements.md` FR-4. UDP peer endpoints are only one class; DNS and
HTTPS/licensing classes are mandatory, and local listen ports remain distinct
ingress classes.

```text
FORWARD jumps:
  -i <inside> -s 10.200.1.1/32 -j OP_NETMK_XRAY_FWD
  -i <inside> -d 10.200.1.1/32 -j OP_NETMK_XRAY_FWD

OP_NETMK_XRAY_FWD:
  one source-scoped ACCEPT projection per applicable EgressFromNetmaker class
  -i <inside> -o <inside> -d 10.200.1.1/32 -m conntrack \
      --ctstate ESTABLISHED,RELATED -j ACCEPT
  explicitly approved IngressToNetmaker listener projections, when required
  -s 10.200.1.1/32 -j DROP
```

No NAT occurs in xray. The chain is idempotently created/flushed by the
`netmk-egress-policy` runit bootstrap. Reconciliation ensures exactly the two
expected built-in jumps; teardown removes both jumps before deleting the
chain.

### Host-owned integration chains

Dedicated chains:

- mangle: `OP_NETMK_MARK`
- filter: `OP_NETMK_FWD`
- nat: `OP_NETMK_NAT`

Logical rules:

```text
mangle PREROUTING:
  -i svc0 -s 10.200.1.1/32 -j OP_NETMK_MARK
OP_NETMK_MARK:
  -j MARK --set-xmark 0x51821/0xffffffff

filter FORWARD:
  -i svc0 -s 10.200.1.1/32 -j OP_NETMK_FWD
  -i wgcf-egress -d 10.200.1.1/32 -j OP_NETMK_FWD
OP_NETMK_FWD:
  one source/mark/egress-scoped ACCEPT projection per applicable manifest class
  -i wgcf-egress -o svc0 -d 10.200.1.1/32 -m conntrack \
      --ctstate ESTABLISHED,RELATED -j ACCEPT
  explicitly approved IngressToNetmaker listener projections, when required
  -s 10.200.1.1/32 -j DROP

nat POSTROUTING:
  -o wgcf-egress -s 10.200.1.1/32 -m mark --mark 0x51821/0xffffffff \
      <applicable manifest class> -j OP_NETMK_NAT
OP_NETMK_NAT:
  -j MASQUERADE
```

### Host policy rules

- Ensure feature-owned priority `10518` is exactly
  `fwmark 0x51821/0xffffffff lookup 51820`. This consumes the existing table
  without adding, deleting, flushing, or replacing any table-51820 route.
- Ensure feature-owned priority `10519` is
  `from 10.200.1.1/32 blackhole`. This catches packets that missed marking or
  for which table `51820` has no usable route.
- Teardown removes only feature-owned priorities `10518`/`10519`; it never
  changes the wgcf interface, configuration, underlay mark, or table contents.

### Security invariants

- Marking is source/interface scoped and occurs before route lookup for every
  candidate IP packet returned by xray.
- Protocol/port restriction occurs after marking.
- No feature-created route or NAT rule sends the source through `pub0`.
- Xray redirect suppression prevents route-learning bypass.
- Existing xray traffic is unaffected because all new rules match
  `10.200.1.1/32`.

---

## 5 · OpenFlow Contract

Required logical flows on `ovsbr0`:

| Priority | Cookie suffix | Match | Action |
| --- | ---: | --- | --- |
| 100 | `0x01` | `in_port=netmk,arp` | `NORMAL` |
| 100 | per manifest class | normalized applicable `EgressFromNetmaker` match | `NORMAL` |
| 50 | `0xff` | `in_port=netmk` | drop |
| 0 | existing | other traffic | existing `NORMAL` baseline |

All feature flows use cookie prefix
`0x4e4d4b0000000000/0xffffff0000000000`. Reconciliation deletes/replaces only
that cookie namespace.

### Authoritative verification

The existing `openflow` plugin query returns controller memory and is not
sufficient. Implement:

1. OF1.3 `OFPMP_FLOW` multipart request/reply support in
   `op-network::controller` for the active switch connection.
2. A D-Bus method `DumpSwitchFlows` returning parsed table, priority, cookie,
   matches, actions, packet count, and byte count.
3. `netmk-of-restrict` verification against `DumpSwitchFlows`, filtering by
   the feature cookie prefix.
4. After generated probes, secondary datapath confirmation through
   `OvsNetlinkClient::dump_flows("ovsbr0")`.

The kernel dump is corroboration only: cached megaflows do not replace a live
logical OpenFlow table query.

---

## 6 · Rtnetlink Contract

Required typed operations:

```rust
MoveLinkInput { iface_name: String, netns_pid: u32 }
SetLinkStateInput { name: String, state: LinkState, netns_pid: Option<u32> }
SetLinkMtuInput { name: String, mtu: u32, netns_pid: Option<u32> }
AddIpv4AddressInput { name: String, address: String, netns_pid: Option<u32> }
SetDefaultRouteInput { name: String, gateway: String, netns_pid: Option<u32> }
AddRouteInput {
    destination: String,
    gateway: Option<String>,
    device: String,
    table: Option<u32>,
    netns_pid: Option<u32>,
}
```

Every operation has schema, runtime dispatch, native implementation, typed
result, OSCAL subid, unit tests, and disposable-netns integration coverage.
For the Netmaker feature call, `set_link_mtu` requires a nonzero target PID and
exact read-back; a host-namespace `None` path is not accepted by this contract.

---

## 7 · Adapter Contract

### Container service

```text
Binary:       op-grpc-adapters
Supervisor:   container systemd, started/verified over D-Bus by host runit
Environment:  ADAPTERS_SOCKET=/var/lib/opdbus-runtime/netmaker/op-grpc-adapters.sock
Service:      op.adapters.v1.NetmakerService
```

### Host bridge service

```text
Name:         netmaker-adapter-loopback
Definition:   deploy/runit/netmaker-adapter-loopback/run
Listen:       127.0.0.1:50061
Connect:      /var/lib/opdbus-runtime/netmaker/op-grpc-adapters.sock
Pattern:      qdrant-grpc-loopback (wait for socket, then exec socat)
```

Canonical run command:

```text
socat TCP-LISTEN:50061,bind=127.0.0.1,reuseaddr,fork \
      UNIX-CONNECT:/var/lib/opdbus-runtime/netmaker/op-grpc-adapters.sock
```

### Client separation

`RemoteOperationClient` gains `netmaker_address` in addition to
`default_address`:

- existing constructor preserves backward compatibility by initializing both
  addresses to the same value;
- a dedicated constructor/builder accepts `NETMAKER_ADAPTER_ADDR`, defaulting
  to `http://127.0.0.1:50061`;
- every `netmaker_*`/`netclient_*` method uses `netmaker_address`;
- all unrelated methods continue using `default_address`;
- every Netmaker request calls `attach_ghostbridge_metadata` before dispatch.

---

## 8 · OCI Reconciliation Contract

```json
{
  "name": "netmaker",
  "loopback_required": true,
  "port_attach": {
    "bridge": "ovsbr0",
    "port_name": "netmk",
    "ip_addrs": ["10.200.1.1/30"],
    "gateway": "10.200.1.2",
    "mtu": 1280,
    "routes": []
  },
  "grpc_adapter": {
    "binary": "op-grpc-adapters",
    "socket": "/var/lib/opdbus-runtime/netmaker/op-grpc-adapters.sock",
    "host_endpoint": "http://127.0.0.1:50061"
  },
  "supervision": "systemd"
}
```

Use `port_name` everywhere. `calculate_diff`, `apply_state`, and
`verify_state` must implement attachment behavior; schema-only
`FieldType::Any` state is not completion.

---

## 9 · Host Runit / Container Systemd Readiness Contract

| Service | Ready stamp | Responsibility |
| --- | --- | --- |
| `xray-attachment` | `xray-attachment` | Restore `grpc0`/addresses/default, start/verify existing `xray.service`, publish PID/ofport readiness |
| `netmk-egress-policy` | `netmk-egress-policy` | Verify immutable wgcf prerequisite; render policy, reconcile host route/rules/chains, start/verify xray policy unit |
| `netmk-port-attach` | `netmk-port-attach` | Create/move/configure `netmk` using current PID |
| `netmk-of-restrict` | `netmk-of-restrict` | Install and live-query feature-cookie flows |
| `netmaker-adapter-loopback` | `netmaker-adapter-loopback` | Expose adapter UDS on `127.0.0.1:50061` |
| `netmk-netclient-start` | `netmk-netclient-start` | Start/restart container `netclient.service` over D-Bus and verify process plus named-peer traffic |

Dependencies:

```text
base OVS + existing xray container + existing wgcf-egress
            │
            ▼
xray-attachment → netmk-egress-policy → netmk-port-attach → netmk-of-restrict ─┐
                                                                                ├→ netmk-netclient-start
container systemd op-grpc-adapters → netmaker-adapter-loopback ──────────────────┘
```

Each service removes its own stale stamp before work, uses bounded waits, exits
non-zero on failure, and invalidates downstream stamps when its owned state
changes. Operators use `sudo sv ...` for host services.

---

## 10 · Phase 2 Contract

`xray0` is created as OVS `internal`, moved into the fresh xray namespace, and
configured with captured MAC, MTU, addresses, and routes. The captured set must
include `10.200.0.1` and `10.200.1.2/30`.

Cutover atomically changes only interface-scoped xray/netclient rules from the
old interface to `xray0`. Host `svc0`, marks, policy priorities, table `51820`,
and `wgcf-egress` are unchanged.

Rollback restores the captured Incus NIC definition and interface-scoped
rules, verifies proxy traffic and netclient handshake, then removes `xray0`.
