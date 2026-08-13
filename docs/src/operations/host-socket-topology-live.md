# Host socket & network topology (live snapshot)

**Host:** Artix Linux, runit (not s6), post 2026-07-20/21 provider move  
**Verified:** 2026-08-10 against live `ip`, `ovs-vsctl`, `incus config device show`, `ss`, `/etc/op-dbus/network.conf`  
**Status:** Read-only inventory of **live reality**. Older Jul-22 wording in this file is superseded.

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
| **L2/L3 fabric** | Single OVS bridge `ovsbr0`; host L3 on internal ports `pub0` / `svc0` / `grpc0` (+ bridge LOCAL `ovsbr0`); physical `eth0` enslaved, unaddressed |
| **Service attachment** | NIC-less CTs: ghostbridge UDS mounts + Incus `proxy` + host `socket-relay` / `fwd-*`. WireGuard mesh is **separate L3** (`netmaker` iface) — never an OVS port |

Role split (from live `/etc/op-dbus/network.conf`):

| Port | Address | Role |
| --- | --- | --- |
| **pub0** | `188.68.58.237/22` | Public uplink L3 + default route |
| **svc0** | `10.0.0.2/24` | Entrance / **tonic** helpers (mail `:443`, Netmaker API/broker) |
| **grpc0** | `10.200.0.2/24` | **gRPC** plane (`:8090` — bridge + cognitive MCP) |
| **ovsbr0** (LOCAL) | `10.200.0.1/24` | Fabric / OpenFlow controller address |
| **netmaker** (WG) | `100.69.0.1/24` | Mesh — **not** on `ovsbr0` |

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
   +-- svc0    10.0.0.2/24        tonic entrance
   +-- grpc0   10.200.0.2/24      gRPC :8090
   +-- ovsbr0  10.200.0.1/24      fabric / OF listen (LOCAL)

WireGuard mesh (NOT an OVS port):
   netmaker  100.69.0.1/24   listen :51821
```

### Routes (host)

| Destination | Device | Notes |
| --- | --- | --- |
| default | `pub0` via `188.68.56.1` | Public egress |
| `10.0.0.0/24` | `svc0` | Tonic entrance / NM egress range |
| `10.200.0.0/24` | `grpc0` and `ovsbr0` | gRPC + fabric (same /24, two host addrs) |
| `100.69.0.0/24` | `netmaker` | Mesh — **must not** land on `ovsbr0` |

### Hard invariants

- **Never** enslave the WireGuard `netmaker` iface into `ovsbr0` (breaks mesh: L3/`NOARP` as OVS slave → TX errors, ICMP dead).
- **Never** publish Netmaker tonic (API/broker) on `10.200.*` — that is gRPC/fabric. Use **svc0** `10.0.0.2`.
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
  -> op-of-controller                 # listen 10.200.0.1:6653 (bridge may still be standalone)
  -> netclient                        # WG mesh (after host L3)
  -> op-grpc-bridge / op-web / op-cognitive-mcp
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
| `10.200.0.2:8090` | `op-cognitive-mcp` | Cognitive MCP HTTP (gRPC plane) |
| `10.200.0.1:6653` | `op-of-controller` | OpenFlow listen (bridge `controller: []`, `fail_mode=standalone`) |
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
- `proxy8081` / `proxy8083`: host `127.0.0.1` → CT loopback
- `proxy3128`: CT → host `nm-warp-egress-proxy` `:13128` (license path; mark → WARP) — **not** mesh egress NAT
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
  +-- gRPC / cognitive: 10.200.0.2:8090 (grpc0) — not :50051
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

- `op-of-controller` listens on **`10.200.0.1:6653`**.
- Bridge: `fail_mode=standalone`, **`controller: []`** (not attached).
- Live flows: `priority=0 actions=NORMAL`.
- Reserved for future tag routing — not required for UDS/svc0/grpc0 correctness today.

---

## 10. WireGuard mesh (`netmaker`)

- Host: `100.69.0.1/24` on iface **`netmaker`**, listen `:51821` (netclient).
- Peers (example): `100.69.0.2` (decoy), `100.69.0.3` (wrt).
- **Must stay off `ovsbr0`.** If mesh ICMP dies while handshakes work, check `ip -d link show netmaker` for `openvswitch_slave` / `master ovs-system` and `ovs-vsctl del-port ovsbr0 netmaker`.

---

## 11. Intent vs live (checklist)

| Topic | Intent | Live 2026-08-10 |
| --- | --- | --- |
| Supervisor | runit | ✓ |
| Single OVS bridge | `ovsbr0` | ✓ |
| Host L3 | pub0 / svc0 / grpc0 (+ bridge LOCAL) | ✓ |
| svc0 = tonic entrance `10.0.0.2` | yes | ✓ NM + mail |
| grpc0 = gRPC `10.200.0.2:8090` | yes | ✓ |
| ovsbr0 LOCAL = fabric `10.200.0.1` | yes | ✓ addr; OF not attached |
| gRPC port | `:8090` | ✓ — **not** `:50051` |
| Netmaker tonic on svc0 | yes | ✓ `fwd-nm-*` |
| WG off OVS | yes | ✓ |
| CT NICs | xray-only historically | **none** today (all nic-less) |
| Shared socks | ghostbridge | ✓ populated |
| OpenFlow policy | future | standalone + NORMAL |

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
2. Keep mesh on `netmaker`; never OVS-enslave WG.
3. Publish tonic (NM, mail helpers) on **svc0**; keep gRPC/cognitive on **grpc0 `:8090`**.
4. Do not revive `10.200.0.1:50051` or put NM API on `10.200.*`.
5. Prefer Netmaker REST egress API for mesh→svc0 NAT; do not fight OF with `nmctl` for that path.
6. License egress stays WARP/mark path — separate from mesh `direct_nat`.
