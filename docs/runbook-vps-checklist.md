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

1. **Never pin OpenFlow flows to an ofport.** The wire format carries port
   *numbers*; netclient recreates interfaces and numbers churn (88→199 on
   2026-08-24 overnight, mesh SSH died while handshakes stayed green).
   Re-bind by interface name at runtime; a watchdog must do this, not a human.
   Implemented: `/usr/local/libexec/3tched/ensure-ptap-flow.sh`, called from
   the `wg-3tched` loop.
2. **Nothing critical lives only in tmpfs without a boot-time restorer.**
   The sealed blob catalog lives in `/dev/shm`; when the stager regressed
   (`opblob stage-shm` missing from the installed binary), op-grpc-bridge
   crash-looped all boot and took the mesh 8090 door down with it.
3. **Every address has an owner service with a watchdog.** "netclient will
   assign it" is not ownership. `100.69.0.1` vanished mid-run once already;
   `wg-3tched` re-adds it every 10s and recreates the whole interface if the
   link disappears.
4. **One flow-table mutation path per flow class.** Controller-installed or
   watchdog-installed, never both drifting. Today's PTAP flow is explicitly a
   *stopgap* owned by the watchdog until `rovs-openflow` gains
   `packet_type` + `Encap` and the controller installs it itself.
5. **After any control-plane restart, verify the data plane.** WG handshakes
   prove only the outer tunnel. Inner packets can be black-holed at the bridge,
   firewall, or listener while every status looks green. The 60-second ritual:
   ```sh
   ovs-ofctl dump-flows -O OpenFlow15 ovsbr0 | grep packet_type  # counter moves?
   ip -s link show netmaker            # tx errors not climbing?
   ss -tln | grep '100.69.0.1:22'      # sshd still bound to the mesh IP?
   # from a peer:
   timeout 4 bash -c "</dev/tcp/100.69.0.1/22" && echo mesh-ssh-ok
   ```

## Known stacked firewalls on this host (check both after edits)

- `/etc/nftables.conf` — inet filter, policy drop, mesh allowlist by source subnet.
- `/etc/iptables/iptables.rules` — legacy iptables-nft table, restored at boot;
  its `ssh-public` DROP is scoped to `-i pub0`. A blanket port rule here silently
  shadows nft accepts because both tables hook input.

## Open items that keep biting

- `opblob` binary lacks `stage-shm`/`persist` — next reboot repeats the blob
  outage until rebuilt/redeployed from the workspace.
- netclient v1.5 client vs v1.6 server (decoy) — version-mismatch warnings on
  every pull.
- Stale self-referential WG peers on both boxes (decoy lists its own keys as
  peers; VPS carries dead `qDDvV3S…`/`6mx4y…` entries). Cosmetic until netmaker
  egress for `10.0.0.x` is configured properly — then they become routing traps.
