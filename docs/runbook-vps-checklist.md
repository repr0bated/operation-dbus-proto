# VPS checklist — avoiding known incompatibilities

Distilled from the 2026-08 flooding/mesh outage chain. Split by what you control
at **purchase time** (provider choice) and what must hold on **any host** you
run (design rules). Each item cites the incident that earned it.

---

## Part 1 — choosing a provider / box

| check | why | incident |
|---|---|---|
| VRRP / proto 112 / multicast **not filtered** on customer ports | If filtered, the gateway MAC can never be learned → `actions=NORMAL` floods every gateway-bound frame to all ports. Forces static FDB pinning forever. | netcup filters VRRP; root of the flooding saga |
| KVM/dedicated with `/dev/net/tun`, cgroup2, nftables, loadable modules | OVS userspace datapath, WireGuard, and firewalling all need them. Containers/shared kernels break OVS in subtle ways. | — |
| UDP hairpin to own public IP works (`nc -u <own-ip> <own-port>`) | Some clouds (OCI) blackhole an instance talking to its own public endpoint. Self-referential WG peers then look "up" but carry 0 bytes. | decoy-wg2 self-peers, `10.0.0.1` unreachable |
| Public NIC MTU ≥ 1420 | Stack is tunnels-within-tunnels (WG inside WARP). Every missing byte costs MSS-clamp hacks per-path. | `mangle` clamp 1240 for netmaker 1280 |
| rDNS/PTR settable; IPv4 **and** IPv6 delegated | Mail legitimacy; REALITY `dest` credibility. | — |

## Part 2 — design rules for any host we run

1. **Never persist an OpenFlow rule with a discovered ofport number.** The wire
   format carries numbers, but release configuration should name ports.
   `op-of-controller` resolves those names from an OpenFlow 1.5 `PORT_DESC`
   reply on every connection. This closes the failure that occurred when
   netclient recreated an interface and its ofport changed from 88 to 199
   while WireGuard handshakes still looked healthy.
2. **Nothing critical lives only in tmpfs without a boot-time restorer.**
   The sealed blob catalog lives in `/dev/shm`; when the stager regressed
   (`opblob stage-shm` missing from the installed binary), op-grpc-bridge
   crash-looped all boot and took the mesh 8090 door down with it.
3. **Every address has an owner service with a watchdog.** "netclient will
   assign it" is not ownership. During the retired Netmaker path,
   `100.69.0.1` vanished mid-run and proved that a healthy handshake is not an
   address-reconciliation mechanism. Apply that lesson to any replacement
   mesh service.
4. **One flow-table mutation path per flow class.** Durable release flows come
   from `/etc/op-dbus/openflow-static-flows.json`; runtime schema mutations go
   through `op-of-controller`. Do not add a watchdog that installs a second
   copy. The former Netmaker PTAP rule and watchdog are retired from the
   current runit path.
5. **After any control-plane restart, verify the data plane.** WG handshakes
   prove only the outer tunnel. Inner packets can be black-holed at the bridge,
   firewall, or listener while every status looks green. The 60-second ritual:
   ```sh
   ovs-vsctl --columns=target,is_connected,status list Controller
   ovs-ofctl -O OpenFlow15 dump-flows ovsbr0 | grep 'actions=NORMAL'
   ovs-appctl fdb/show ovsbr0 | grep -i '00:00:5e:00:01:0a'
   ip route get 188.68.56.1
   ping -c1 -W2 188.68.56.1
   ```

## Known stacked firewalls on this host (check both after edits)

- `/etc/nftables.conf` — inet filter, policy drop, mesh allowlist by source subnet.
- `/etc/iptables/iptables.rules` — legacy iptables-nft table, restored at boot;
  its `ssh-public` DROP is scoped to `-i pub0`. A blanket port rule here silently
  shadows nft accepts because both tables hook input.

## Open items that keep biting

- ~~`opblob` binary lacks `stage-shm`/`persist`~~ FIXED 2026-08-25 (commit
  `22f44859`): subcommands restored from c4db14ae, binary rebuilt+deployed,
  persist→stage round trip verified. The hand-copy fallback in
  `opdbus-rundirs-up` remains as belt-and-suspenders.
- netclient v1.5 client vs v1.6 server (decoy) — MOOT for the mesh: netmaker
  broker retired 2026-08-25, both WG links are static confs now. Decoy's own
  egress is its self-contained wgcf-ingress (WARP), policy asserted by
  `mesh-policy.service`; its underlay to Cloudflare rides the VPS link
  (mark `0x7777` → table 51822 → MASQ out pub0).
- OpenFlow attachment is not boot-owned yet:
  `/etc/op-dbus/network.conf` declares `OPENFLOW_CONTROLLER`, but the current
  `op-of-controller` run script does not consume it and no runit helper invokes
  the safe attach method. Existing hosts retain the Controller row in OVSDB;
  a fresh database can leave the listener healthy but disconnected. Verify
  `ovs-vsctl get-controller ovsbr0` after provisioning.
- PTAP history: vendored `rovs-openflow` gained OXM_OF_PACKET_TYPE and
  OFPAT_ENCAP support, but the affected Netmaker port was retired afterward.
  The current `openflow-static-flows.json` has no PTAP entry and no runit
  service calls the old `ensure-ptap-flow` helper. Re-test the encoder against
  OVS before reintroducing that flow class; its last live attempt returned
  BAD_ACTION while equivalent `ovs-ofctl` syntax worked.
- Router (wrt-router) intentionally disconnected from this host: peer removed
  from static conf; its rc.local still dials a dead endpoint (harmless failed
  keepalives). Reconnect = re-add peer entry with pubkey
  `ppaYyM0y…`, or wipe its `/etc/rc.local` netmaker block.
