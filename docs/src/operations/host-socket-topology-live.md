# Host socket & network topology (live snapshot)

**Host:** Artix Linux, runit (not s6), post 2026-07-20/21 provider move  
**Verified:** 2026-08-14 against live `ip`, `ovs-vsctl`, `ovs-ofctl`, `incus config device show`, `ss`, `wg`, `/etc/runit/sv`
**Status:** Read-only inventory of **live reality**. Older Jul-22 wording is superseded.

> **2026-08-14 correction.** The 08-10 revision asserted "never enslave the WireGuard `netmaker` iface into `ovsbr0`" as a hard invariant in §2, §10 and §13.2. That is **wrong and was acted on in error**: the enslavement is deliberate and load-bearing (mesh↔tonic/gRPC), performed at boot by the enabled `netmaker-ovs-attach`. The 08-10 snapshot also predates the retirement of all Incus `proxy` devices. Treat any pre-08-14 copy of §2/§5/§10/§13 as unsafe.

Re-verify with:

```bash
ip -br addr; ip route
sudo ovs-vsctl show
sudo ovs-vsctl get-controller ovsbr0; sudo ovs-vsctl get Bridge ovsbr0 fail_mode
sudo incus list -c nst4
for c in assistant cozo mail-3tched NetMaker qdrant xray; do
  echo "=== $c ==="; sudo incus config device show "$c"
done
ss -ltnp | grep -E '10\.(0|200)\.|8090|8081|6653'
ls /run/ghostbridge/
sudo sv status ovsbr0-uplink ovsbr0-addr ovsbr0-svc-addr ovsbr0-eth0 \
  op-of-controller netclient op-web op-grpc-bridge op-cognitive-mcp \
  fwd-nm-tonic-8081 fwd-nm-broker-8083 xsock-netmaker xsock-netmaker-broker
```

---

## 1. Mental model (two layers)

| Layer | Role |
| --- | --- |
| **L2/L3 fabric** | Single OVS bridge `ovsbr0`; host L3 on internal ports `pub0` / `3tched` / `svc0` (+ bridge LOCAL `ovsbr0`); physical `eth0` enslaved, unaddressed |
| **Service attachment** | NIC-less CTs: ghostbridge UDS mounts + host `socket-relay` / `fwd-*` / `uds-*` / `xsock-*`. WireGuard mesh (`netmaker` iface) **is** an `ovsbr0` port — it is the mesh↔fabric junction |

Role split (from live `/etc/op-dbus/network.conf`):

| Port | Address | Role |
| --- | --- | --- |
| **pub0** | `188.68.58.237/22` | Public uplink L3 + default route |
| **3tched** | `10.0.0.2/24`, `10.200.0.2/24` | Single incoming interface carrying both identities |
| **svc0** | `10.0.0.3/24` | Tonic service (`:8090`) |
| **ovsbr0** (LOCAL) | `10.200.0.1/24` | Fabric / OpenFlow controller address |
| **netmaker** (WG) | `100.69.0.1/24` | Mesh — **is** an `ovsbr0` port (L3, needs `encap` flows) |

**Port `:50051` is not used.** Live gRPC is **`:8090`** (+ UDS). Drafts that mention `10.200.0.1:50051` are stale (`SIGNALS.md` 2026-07-22).

---

## 2. Host L3 map (live 2026-08-10)

```text
Internet
   |
   v
eth0  (physical, enslaved, NO IPv4)
   |
ovsbr0  (OVS bridge)
   |
   +-- pub0    188.68.58.237/22   default via 188.68.56.1
   +-- 3tched  10.0.0.2/24        incoming identity
   |           10.200.0.2/24      incoming identity
   +-- svc0    10.0.0.3/24        Tonic :8090 service
   +-- ovsbr0  10.200.0.1/24      fabric / OF listen (LOCAL)

   +-- netmaker  100.69.0.1/24  listen :51821  (WireGuard, L3 port;
                                   mesh <-> fabric junction via encap flows)
```

### Routes (host)

| Destination | Device | Notes |
| --- | --- | --- |
| default | `pub0` via `188.68.56.1` | Public egress |
| `10.0.0.0/24` | `3tched` and `svc0` | Incoming identity + Tonic service |
| `10.200.0.0/24` | `3tched` and `ovsbr0` | Incoming identity + fabric (same /24, two host addrs) |
| `100.69.0.0/24` | `netmaker` | Mesh — reaches `3tched`/`svc0` **through** `ovsbr0` |

### Hard invariants

- The WireGuard `netmaker` iface **is deliberately enslaved into `ovsbr0`** — that membership connects the mesh to `3tched` and `svc0`. Do **not** `del-port` it.
  - It is a layer-3, `NOARP`, MAC-less port, so plain `NORMAL` L2 forwarding cannot carry it. The companion **netmaker L3→`encap(ethernet)` OpenFlow flows** supply the Ethernet header (see §9). Without those flows the port still shows link-up and healthy WG handshakes while L2-dependent paths fail.
  - Elevated **TX errors** on `netmaker` are the expected cost of L2 flood attempts against an L3 port, not evidence of misconfiguration. Do not diagnose from that counter alone.
- Keep both incoming identities on `3tched`; Tonic serves on `svc0` at `10.0.0.3:8090`.
- Mesh egress for clients: Netmaker API egress `host-svc0-net` → range `10.0.0.0/24`, `mode=direct_nat`.
- Do not treat `:50051` as a live bind target.

Canonical config: `/etc/op-dbus/network.conf`  
Helpers: `/usr/local/libexec/3tched/{ovsbr0-uplink-up,ovsbr0-svc-addr-up,ovsbr0-eth0-up,…}`

---

## 3. Runit boot graph (network + control plane)

```text
opdbus-rundirs
  -> ovsdb-server -> ovs-vswitchd     # seed bridge + internal ports (no eth0)
  -> ovsbr0-uplink -> ovsbr0-addr -> ovsbr0-svc-addr
  -> ovsbr0-eth0                      # enslave physical uplink last
  -> incus-ct-netmaker -> op-grpc-bridge -> netclient
  -> netmaker-ovs-attach              # typed rovs_commands add_port
  -> op-of-controller                 # OF1.5, after symbolic netmaker port exists
  -> op-web / op-cognitive-mcp
  -> fwd-nm-tonic-8081 / fwd-nm-broker-8083   # svc0 publishes NM
  -> xsock-netmaker / xsock-netmaker-broker   # UDS -> loopback
```

`nm-ovs-nic` (bridged NetMaker CT nic) is **retired**. `incus-ct-netmaker` keeps the CT nic-less.

### Netmaker on svc0 (tonic)

| Service | Listen | Connect |
| --- | --- | --- |
| `fwd-nm-tonic-8081` | `10.0.0.2:8081` | `127.0.0.1:8081` (Incus proxy from `NetMaker`) |
| `fwd-nm-broker-8083` | `10.0.0.2:8083` | `127.0.0.1:8083` |
| `xsock-netmaker` | `/run/ghostbridge/netmaker.sock` | `127.0.0.1:8081` |
| `xsock-netmaker-broker` | `/run/ghostbridge/netmaker-broker.sock` | `127.0.0.1:8083` |

---

## 4. Host TCP / UDS listeners (live)

### Control plane / fabric

| Bind | Process | Role |
| --- | --- | --- |
| `0.0.0.0:8080` | `op-web-server` | HTTP GUI + API |
| `127.0.0.1:8090` | `op-grpc-bridge` | Local gRPC / gRPC-Web |
| `10.0.0.3:8090` | `fwd-8090` | Tonic publication to `127.0.0.1:8090` |
| `10.200.0.1:6653` | `op-of-controller` | OpenFlow 1.5 listen (`fail_mode=standalone`) |
| `10.200.0.1:10809` | Incus proxy → xray | HTTP egress door |
| `10.0.0.2:443` | `fwd-443` / SNI demux | Entrance TLS (mail vs Reality) |
| `10.0.0.2:8081` | `fwd-nm-tonic-8081` | Netmaker API (tonic) |
| `10.0.0.2:8083` | `fwd-nm-broker-8083` | Netmaker MQTT WS |
| `/run/opdbus/grpc.sock` | `op-grpc-bridge` | Host-local gRPC UDS |
| `/run/ghostbridge/container.sock` | `op-grpc-bridge` | Shared CT gRPC UDS |

### Ghostbridge socks (live)

`container.sock`, `decoy.sock`, `fwd-qdrant.sock`, `fwd-web.sock`, `mail-web.sock`, `netmaker.sock`, `netmaker-broker.sock`, `xray-reality.sock`

---

## 5. Containers: attachment mode (live 2026-08-10)

**No running CT has a bridged NIC on `ovsbr0`.** All infra CTs are nic-less; attachment is UDS + Incus proxy.

| Container | State | NIC | Shared UDS | Incus TCP proxy | Notes |
| --- | --- | --- | --- | --- | --- |
| **xray** | RUNNING | no | `/run/ghostbridge`, `/run/opdbus` | `0.0.0.0:8444`, `10.200.0.1:10809` | SNI / Reality; conf via `xrayconf` device |
| **NetMaker** | RUNNING | no | ghostbridge + opdbus mounts | loopback `8081`/`8083`, license proxy `3128→13128` | CT name is **`NetMaker`** (case-sensitive) |
| **mail-3tched** | RUNNING | no | ghostbridge | public mail ports on `pub0`; web `127.0.0.1:80`; TLS web `127.0.0.1:8440` | Entrance via svc0 `:443` / mail-web.sock |
| **assistant** | STOPPED | no | ghostbridge | none | Shared-socket shape |
| **qdrant** | STOPPED | no | ghostbridge | `10.200.0.2:6333/6334`, `10.0.0.2:6333` | Hybrid when up |
| **cozo** | STOPPED | no | ghostbridge | `50053→50052` | |

### NetMaker device detail

- Disk: `ghostbridge-socket`, `netmaker-runtime`, `opdbus-rt`, `opdbus-sock`
- **No Incus `proxy` devices remain on any container** (verified 2026-08-14). The `proxy8081` / `proxy8083` / `proxy3128` devices this section previously listed were retired in favour of UDS relays; `uds-netmaker-api` records the swap in its own header.
- Inbound: `uds-netmaker-api` / `uds-netmaker-broker` (host TCP → ghostbridge UDS → CT loopback)
- Outbound (license): `xsock-netmaker-egress` → `/run/ghostbridge/NetMaker/egress.sock` → `nm-warp-egress-proxy` `:13128` → mark `0x51821` → WARP. CT side is a `--tcp-to-unix 127.0.0.1:3128` leg on `op-uds-relay.service`. This is the **license path only** — **not** mesh egress NAT.
  - Added 2026-08-14: the outbound half was lost when the proxy devices were retired (only the inbound legs were converted), leaving `HTTP(S)_PROXY=127.0.0.1:3128` pointing at nothing and the Pro license stuck at `record not found`.
- Mesh egress NAT is Netmaker API `host-svc0-net` (`10.0.0.0/24`, `direct_nat`) on host node `100.69.0.1`

---

## 6. Shared socket

| Path | Role |
| --- | --- |
| `/run/opdbus/grpc.sock` | Host-local gRPC |
| `/run/ghostbridge/container.sock` | Shared fabric for CTs |
| `/run/ghostbridge/netmaker.sock` | Netmaker API relay |
| `/run/ghostbridge/netmaker-broker.sock` | EMQX MQTT WS relay |

```text
ZEROCLAW_UNIX_SOCKET=/run/opdbus/grpc.sock
GHOSTBRIDGE_SOCKET_PATH=/run/ghostbridge/container.sock
GRPC_BIND / ZEROCLAW_BIND_ADDR = 127.0.0.1:8090   # live op-grpc-bridge run
COGNITIVE_MCP_BIND = 10.200.0.2:8090              # live op-cognitive-mcp run
```

---

## 7. Public / identity path (sketch)

```text
Client → DNS → Oracle decoy → WireGuard mesh → host
  |
  +-- mesh peers reach svc0 10.0.0.0/24 via Netmaker egress (direct_nat)
  |     Netmaker API/broker: 10.0.0.2:8081 / :8083
  |
  +-- Tonic/gRPC / cognitive: 10.0.0.3:8090 (svc0) — not :50051
  |
  +-- Reality / SNI: xray (host proxies + ghostbridge socks)
        mail.* → mail-web.sock ; default → xray-reality.sock
```

Xray live config policy: materialize into the container at **`/etc/xray/xray_config.json`** (see `AGENTS.md`). Do not treat `/dev/shm/xray_config.json` as the long-term sole live path if policy has moved.

---

## 8. Common exit (xray) + WARP (`wgcf-egress`)

| Host | Interface | Role |
| --- | --- | --- |
| Hypervisor | `wgcf-egress` | CT privacy egress (mark `0x51821` → table 51820) |
| Oracle decoy | `wgcf-ingress` | Edge ingress (not host exit) |

NetMaker license traffic: CT `HTTP(S)_PROXY=127.0.0.1:3128` → Incus → `nm-warp-egress-proxy` → marked WARP path. Do not “fix” licensing with naive host NAT/`nmctl`.

---

## 9. OpenFlow

- `op-of-controller` listens on **`10.200.0.1:6653`**; bridge `fail_mode=standalone`, controller `is_connected: true`.
- Live flows (2026-08-14): 4 × `priority=100 … actions=NORMAL` (cookie `0x…02`, NM `8081` + QUIC `443`, both directions, all `n_packets=0`) plus the `priority=0 actions=NORMAL` fallback (cookie `0x…01`) carrying essentially all traffic.

### netmaker L3→`encap(ethernet)` flow — required and controller-managed

The `netmaker` WG port is layer-3 (`NOARP`, MAC-less). Moving its traffic across an L2 bridge requires OpenFlow to push an Ethernet header — `NORMAL` cannot. Spec (from `SIGNALS.md` 2026-07-14 Fable 5 / 2026-07-15 GLM-5.2):

- **match**: `OFPXMT_OFB_PACKET_TYPE` + `OFPXMT_OFB_IN_PORT` (the `netmaker` port)
- **actions**: `OFPAT_ENCAP` + `OFPAT_SET_FIELD` + `OFPAT_OUTPUT LOCAL`

The earlier "not currently needed" conclusion was disproved by a simultaneous capture on 2026-08-14. Decoy ICMP requests and origin-side echo replies both emerged on `netmaker`, while the host ping socket received neither. `ofproto/trace` then showed `NORMAL -> learned port is input port, dropping`. WireGuard encryption, handshakes, AllowedIPs, and the NetMaker API route were healthy; OVS was consuming the decrypted L3 packet before the Linux host stack.

The durable repair is schema-driven in `deploy/config/openflow-static-flows.json`: match the dynamically resolved `netmaker` ofport plus `packet_type=(1,0x800)`, then `encap(ethernet)`, set a synthetic source MAC, resolve the current `ovsbr0` MAC from PortDesc, and output `LOCAL`. The controller negotiates OF1.5, validates programming with a barrier, and reconnects/re-discovers ports after PortStatus so netclient ofport changes are not hard-coded. The priority-0 NORMAL fallback and static upstream FDB pin remain separate and must be preserved.

---

## 10. WireGuard mesh (`netmaker`)

- Host: `100.69.0.1/24` on iface **`netmaker`**, listen `:51821` (netclient).
- Peers (example): `100.69.0.2` (decoy), `100.69.0.3` (wrt). WireGuard **terminates at the decoy**.
- **Belongs on `ovsbr0`.** `netmaker-ovs-attach` adds it at boot through typed `rovs_commands add_port` and that is correct — it is the mesh↔fabric junction for Tonic.
- Because it is an L3/`NOARP` port, preserve the verified OpenFlow/FDB handling in §9. **Do not** "fix" a mesh reachability problem with `del-port` — that severs mesh→`3tched`/`svc0` entirely.
- If mesh peers cannot reach `10.0.0.0/24`, check in this order: (1) the `encap` flows are present in `dump-flows`; (2) the Netmaker egress gateway (`host-svc0-net`, `direct_nat`) exists — it is a **Pro** feature and needs a valid license; (3) the interim SNAT covering that path is still in place.

---

## 11. Intent vs live (checklist)

| Topic | Intent | Live 2026-08-10 |
| --- | --- | --- |
| Supervisor | runit | ✓ |
| Single OVS bridge | `ovsbr0` | ✓ |
| Host L3 | pub0 / 3tched / svc0 (+ bridge LOCAL) | ✓ |
| 3tched = incoming `10.0.0.2` + `10.200.0.2` | yes | ✓ |
| svc0 = Tonic `10.0.0.3:8090` | yes | ✓ |
| ovsbr0 LOCAL = fabric `10.200.0.1` | yes | ✓ addr; OF not attached |
| gRPC port | `:8090` | ✓ — **not** `:50051` |
| Netmaker tonic on svc0 | yes | ✓ `fwd-nm-*` |
| WG L3 port on OVS + encap flow | yes | ✓ |
| CT NICs | xray-only historically | **none** today (all nic-less) |
| Shared socks | ghostbridge | ✓ populated |
| OpenFlow policy | OF1.5 + safety fallback + L3 delivery | ✓ |

---

## 12. Related tree / stale docs

| Path | Note |
| --- | --- |
| `docs/network-address-table.md` | Historical pre-runit host — do not use |
| `deploy/runit/ZCALL-INIT-HANDOFF.md` | Mentions `:50051` fabric bind — **stale vs live run** |
| `deploy/config/op-grpc-bridge-run.sh` | May still list `:50051` — live `/etc/runit/sv/op-grpc-bridge/run` hardcodes `127.0.0.1:8090` |
| `SIGNALS.md` (2026-07-22) | Documents `:50051` → `:8090` fix |
| CLAUDE.md “Transport & identity” | Hybrid note; re-check devices |

---

## 13. Safe change policy

1. Re-read live devices before adding proxy / `fwd-*` / SNI routes.
2. Keep the `netmaker` WG iface **on** `ovsbr0` with its `encap` flows; never `del-port` it.
3. Keep both incoming identities on **3tched** (`10.0.0.2`, `10.200.0.2`); serve Tonic on **svc0** at `10.0.0.3:8090`.
4. Do not revive `10.200.0.1:50051` or put NM API on `10.200.*`.
5. Prefer Netmaker REST egress API for mesh→svc0 NAT; do not fight OF with `nmctl` for that path.
6. License egress stays WARP/mark path — separate from mesh `direct_nat`.
