# Tasks — Netclient Container Netns

Tasks are ordered within each phase. **Phase 2 must not begin until Phase 1 is
validated in production. These are independent execution gates, not a continuous
pipeline.**

---

## Phase 1 — NetMaker OVS Internal Port + Netclient Join

### Feasibility investigation (before port provisioning)

- [ ] **T-0** Determine where `wgcf-egress` interface lives: host netns or xray netns? Run `sudo ip link show wgcf-egress` on host and `sudo incus exec xray -- ip link show wgcf-egress` inside xray. Document which netns owns it and verify the WARP policy route (`ip rule show` + `ip route show table 51820`) in that netns. This determines whether §2.3 primary or alternative path applies.
- [ ] **T-0.1** Verify IP forwarding inside xray: `sudo incus exec xray -- sysctl net.ipv4.ip_forward`. If `0`, determine how to persist `=1` inside the container (sysctl.conf or Incus `linux.sysctl` config key).
- [ ] **T-0.2** If `wgcf-egress` is host-only (not in xray netns): design the host-side fwmark path (xray MASQs behind `10.200.0.1`, host marks packets from xray's source IPs, host ip-rule → WARP). Document as §2.3 addendum and update task sequence below accordingly.

### Port provisioning

- [ ] **T-1** Add OVS internal port `netmk` to `ovsbr0` via `rovs_commands.add_port` D-Bus call. Verify with `ovs-vsctl show` (read-only check) that the port appears as `type: internal`.
- [ ] **T-2** Determine `NetMaker` container's init PID (`incus info NetMaker` → Pid field). Move `netmk` interface into that netns via rtnetlink `RTM_NEWLINK` with `IFLA_NET_NS_PID`.
- [ ] **T-3** Inside container netns: assign `10.200.1.1/30` to `netmk`, bring link UP, add default route via `10.200.1.2`. All via rtnetlink D-Bus methods (with netns extension if needed).

### Xray-side gateway & WARP routing (replaces prior host-side SNAT)

- [ ] **T-3.5** Inside xray's netns: assign `10.200.1.2/30` as secondary address on xray's `eth0` via rtnetlink D-Bus (with netns extension targeting xray's PID). This makes xray the ARP-resolvable gateway for `netmk`'s default route. Verify: `incus exec xray -- ip addr show eth0` includes `10.200.1.2/30`.
- [ ] **T-4** Inside xray's netns: ensure `net.ipv4.ip_forward=1`. If not already set, configure via Incus `linux.sysctl.net.ipv4.ip_forward` or a one-shot exec.
- [ ] **T-5** Inside xray's netns: add iptables mangle FORWARD rule:
  `-t mangle -A FORWARD -s 10.200.1.0/30 -j MARK --set-mark 0x51821`
  (marks transit packets from NetMaker with the WARP fwmark).
- [ ] **T-5.1** Inside xray's netns (if `wgcf-egress` is in xray netns): add iptables nat POSTROUTING rule:
  `-t nat -A POSTROUTING -s 10.200.1.0/30 -o wgcf-egress -j MASQUERADE`
  OR (if `wgcf-egress` is host-only): add MASQUERADE behind xray's own IP and let host-side fwmark routing handle WARP. See T-0.2 result.
- [ ] **T-5.2** Verify end-to-end: from inside NetMaker, `ping -c1 1.1.1.1` should succeed AND the packet should exit via WARP (verify with `tcpdump -i wgcf-egress` on the host or xray, depending on T-0 result — should show the ping transit there, not on `pub0`).

### Boot-order dependency chain (runit ready-stamps)

- [ ] **T-5.3** Create runit one-shot service `xray-egress-ready`:
  - Polls for: xray container running + `10.200.1.2/30` on xray `eth0` + forwarding enabled + fwmark rules present + WARP route functional (test with a single UDP probe or route-table check)
  - On success: `touch /run/opdbus/runit-ready/xray-egress-ready`
  - Depends on: `ovsbr0-svc-addr` (existing stamp)
- [ ] **T-5.4** Create runit one-shot service `netmk-port-attach`:
  - Waits for: `/run/opdbus/runit-ready/xray-egress-ready`
  - Executes: T-1 through T-3 (port create, move, configure)
  - On success: `touch /run/opdbus/runit-ready/netmk-port-attach`
- [ ] **T-5.5** Create runit one-shot service `netmk-of-restrict`:
  - Waits for: `/run/opdbus/runit-ready/netmk-port-attach`
  - Executes: T-12.6 (OpenFlow rules)
  - On success: `touch /run/opdbus/runit-ready/netmk-of-restrict`
- [ ] **T-5.6** Create runit one-shot service `netclient-start`:
  - Waits for: `/run/opdbus/runit-ready/netmk-of-restrict`
  - Executes: calls `netmaker_restart("netclient")` via `op-grpc-bridge` (which routes to the in-container `op-grpc-adapters` via the proxy device), OR as fallback if the adapter isn't ready yet, `incus exec NetMaker -- sv start netclient`
  - On success (adapter's `is_active` or pgrep confirms): `touch /run/opdbus/runit-ready/netclient-start`

### Rtnetlink plugin extension (if needed)

- [ ] **T-6** Audit `rtnetlink.rs` for cross-netns support. If `set_link_state`, `add_ipv4_address`, `set_default_route` cannot target a foreign netns: add optional `netns_pid: Option<u32>` field to their input structs, and `setns(CLONE_NEWNET)` + restore in the implementation.
- [ ] **T-7** Add `MoveLinkInput { iface_name: String, netns_pid: u32 }` D-Bus method to rtnetlink plugin if not already exposed. Implementation: `RTM_NEWLINK` with `IFLA_NET_NS_PID` attribute via the `netlink-packet-route` crate.

### Netclient supervision — deploy existing `op-grpc-adapters` inside container

- [ ] **T-8** Verify runit presence inside `NetMaker` container: `incus exec NetMaker -- which sv` and `incus exec NetMaker -- ls /etc/service/`. If runit is not present, install a minimal runit setup (or decide on Option B — see design.md §2.4 open question) and document the chosen path.
- [ ] **T-8.1** If runit is present (or after installing it): create `/etc/service/netclient/run` inside the container (`#!/bin/sh\nexec netclient daemon`). Verify `sv status netclient` works inside the container.
- [ ] **T-8.2** Deploy `op-grpc-adapters` binary into the `NetMaker` container. Cross-compile or build inside the container as appropriate. Place at a stable path (e.g., `/usr/local/bin/op-grpc-adapters`).
- [ ] **T-8.3** Set up process supervision for `op-grpc-adapters` itself inside the container (runit service directory `/etc/service/op-grpc-adapters/run` if using runit, or equivalent). The adapter must start automatically and restart on crash.
- [ ] **T-8.4** Add a third Incus `proxy` device (`grpc-adapter`) to the `NetMaker` container: `incus config device add NetMaker grpc-adapter proxy listen=tcp:127.0.0.1:<host-port> connect=tcp:127.0.0.1:<container-port> bind=host`. Verify: `incus config device show NetMaker` lists all three proxy devices.
- [ ] **T-9** Configure `op-grpc-bridge`'s `netmaker_client()` endpoint to point at the new proxy device's host-side address (`http://127.0.0.1:<host-port>`). Validate: `netmaker_join()` / `netmaker_leave()` / `netmaker_restart()` calls from the host reach the adapter and produce correct responses (can test with a no-op call like `get_server_health` or `list_nodes`).

### OCI plugin schema

- [ ] **T-10** Add `port_attach` declaration for `NetMaker` in the OCI plugin schema: `{ bridge: "ovsbr0", iface_name: "netmk", ip_addrs: ["10.200.1.1/30"], gateway: "10.200.1.2" }`.
- [ ] **T-11** Ensure the OCI plugin lifecycle (boot → loopback → AttachPort → configure) triggers for `NetMaker` on next reconciliation cycle.

### Rule persistence (xray-side)

- [ ] **T-12** Persist the xray-side iptables rules (mangle FORWARD fwmark + nat POSTROUTING MASQUERADE) and the `10.200.1.2/30` secondary address. Options: (a) Incus cloud-init/user-data, (b) a script triggered by `xray-egress-ready` one-shot on every boot, (c) iptables-save/restore inside xray. Idempotent: check `-C` before `-A`.

### OpenFlow egress restriction (Phase 1, required)

- [ ] **T-12.5** Read the current OpenFlow flow table on `ovsbr0` via the `openflow`/`openflow_obfuscation` state plugin's D-Bus surface (not raw `ovs-ofctl`). Confirm nothing there would drop UDP/51822 from `netmk`. (As of this writing, only a single priority=0 `actions=NORMAL` rule exists — document this baseline.)
- [ ] **T-12.6** Install OpenFlow rules on `ovsbr0` restricting `netmk` egress:
  - Allow: UDP from `netmk` port with destination port 51822 (WireGuard listen port, per `/etc/netclient/netclient.json`).
  - Allow: return UDP traffic to `netmk` (source port 51822, or rely on the NORMAL action for inbound).
  - Deny: all other egress originating from `netmk`'s OVS port.
  Implemented via the `openflow` state plugin D-Bus surface. Verify: `ovs-ofctl dump-flows ovsbr0` (read-only check) shows the new rules.
- [ ] **T-12.7** Persist the OpenFlow rules in `op-ovsbr0-setup` (or equivalent) so they survive bridge restarts. Idempotent on re-application.

### Validation

- [ ] **T-13** Verify: `incus exec NetMaker -- ip addr show netmk` → shows `10.200.1.1/30`.
- [ ] **T-14** Verify: `incus exec NetMaker -- ip route show` → default via `10.200.1.2` dev `netmk`.
- [ ] **T-14.5** Verify: `incus exec xray -- ip addr show eth0` → shows `10.200.1.2/30` as secondary.
- [ ] **T-14.6** Verify: `incus exec xray -- sysctl net.ipv4.ip_forward` → `= 1`.
- [ ] **T-14.7** Verify: `incus exec xray -- iptables -t mangle -L FORWARD -n` → shows fwmark rule for `10.200.1.0/30`.
- [ ] **T-15** Verify (egress path correctness): `incus exec NetMaker -- ping -c1 8.8.8.8` → success. Simultaneously monitor: packet should appear on `wgcf-egress` (via tcpdump), NOT on `pub0` directly. This proves the WARP path, not raw SNAT.
- [ ] **T-16** **Primary pass/fail gate**: `netmaker_join()` via `op-grpc-bridge` succeeds (or fallback: `incus exec NetMaker -- netclient join`). `incus exec NetMaker -- wg show` reports a handshake with at least one peer. This proves UDP/51822 egress works end-to-end through the OpenFlow restriction.
- [ ] **T-17** Verify: existing proxy devices still work — `curl -s http://127.0.0.1:8081/api/health` from host returns 200.
- [ ] **T-18** Verify: host's own `3tched` interface unaffected — `wg show 3tched` on host shows unchanged peers/endpoints.

---

## ⛔ PHASE GATE — Do Not Proceed Until Phase 1 Is Stable

Phase 1 must run in production for a minimum observation period (recommend ≥ 48 h)
with:
- netclient maintaining mesh connectivity (no handshake timeouts)
- proxy devices continuously functional
- no OVS port flaps or netns leaks

Phase 2 execution requires:
1. Explicit human approval after observation period
2. Scheduled maintenance window communicated to affected users
3. Rollback procedure tested in a non-production container first

---

## Phase 2 — xray Veth-to-OVS-Internal Migration

### Pre-staging (safe, no traffic impact)

- [ ] **T-19** Create OVS internal port `xray0` on `ovsbr0` via `rovs_commands.add_port`. Do NOT move it into xray's netns yet — it sits idle in host netns.
- [ ] **T-20** Document xray's current network config: `incus exec xray -- ip addr show eth0`, `ip route show`, MAC address. This is the rollback target state.
- [ ] **T-21** Confirm `vethde51090d` peer relationship: check `/sys/class/net/vethde51090d/ifindex` and compare with `incus exec xray -- cat /sys/class/net/eth0/iflink`. Document the mapping.

### Cutover plan

- [ ] **T-22** Write and test a cutover script (idempotent, with timeout):
  1. Move `xray0` into xray's netns by PID
  2. Inside netns: assign xray's IPs + MAC to `xray0`, bring UP, set routes
  3. `incus config device remove xray eth0` (removes veth pair)
  4. Health check: `curl --max-time 5 http://10.200.0.1:<port>` from host
  5. If health check fails within 10 s: trigger rollback

- [ ] **T-23** Write and test rollback script:
  1. `incus config device add xray eth0 nic nictype=bridged parent=ovsbr0`
  2. Wait for Incus to recreate the veth pair and assign it
  3. Inside container: restore documented IP/route/MAC on `eth0`
  4. Remove `xray0` from container netns and OVS bridge
  5. Health check confirms restoration

### Execution (maintenance window only)

- [ ] **T-24** Execute cutover script during scheduled window. Monitor with `tcpdump -i xray0` and external probe to `api.3tched.com`.
- [ ] **T-25** If successful: update OCI plugin schema to declare xray's `port_attach` as `xray0` instead of relying on Incus NIC.
- [ ] **T-26** Remove stale veth configuration references from any remaining scripts/docs.

### Validation

- [ ] **T-27** Verify: `ovs-vsctl show` no longer lists any `veth*` port.
- [ ] **T-28** Verify: `incus config show xray` has no `nic` device — only `port_attach` via OCI plugin.
- [ ] **T-29** Verify: external DNS resolution + HTTPS to `api.3tched.com`, `broker.3tched.com` works end-to-end.
- [ ] **T-30** Verify: `incus exec xray -- ip addr show xray0` shows expected IPs; no `eth0` present.

---

## Open Decisions Before T-1 (Port Provisioning)

- **T-0 is the critical gate**: where `wgcf-egress` lives determines the
  entire forwarding path. If it's host-only (most likely, since `wg-quick`
  runs as a host runit service), the two-hop variant in design §2.3
  alternative applies: xray MASQs behind its own IP, packet returns to host,
  host fwmarks and routes via WARP. T-0.2 must produce the concrete iptables
  + ip-rule recipe for this case before T-3.5 through T-5.2 can execute.

## Open Decisions Before T-6 / T-7

- Confirm whether the current rtnetlink plugin implementation supports
  cross-netns operations or requires the extension described in design §2.2.
  Read `op_network::rtnetlink` module to check for `setns` usage or netns-fd
  parameters.

## Open Decisions Before T-22

- Determine whether Incus supports hot-removing a `nic` device from a running
  container without restarting it. Test with a non-production container first.
  If not supported, T-22 must include a container restart step and the downtime
  estimate increases.
