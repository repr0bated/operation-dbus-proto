> Historical input only. This prompt describes the pre-outage checkpoint and
> must not be rerun as current live-state authority. Use the post-outage FR-0
> and R-* gates in the revised requirements/tasks instead.

Update the spec at .kiro/specs/netclient-container-netns based on current live implementation state. Cross-reference design.md, requirements.md, spec.md, and tasks.md against the actual code and live host state below, and produce a revised spec + tasks.md.

Files to review:
- .kiro/specs/netclient-container-netns/{design.md,requirements.md,spec.md,tasks.md}
- crates/op-plugins/src/state_plugins/rtnetlink.rs
- crates/op-plugins/src/state_plugins/rovs_commands.rs
- crates/op-grpc-bridge/src/bin/op-netmk-reconcile.rs
- deploy/runit/netmk-egress-policy/run
- /dev/shm/opdbus/capability-grants.json (live, root-only)

Already implemented (verify against code, mark tasks complete):
- netmk (netmaker) and grpc (xray) OVS internal ports attached into their container netns via native rtnetlink/OVSDB, addressed per spec.md §1 (netmk=10.200.1.1/30, xray gw=10.200.1.2/30, xray existing 10.200.0.1/24 preserved).
- Scoped capability grants added for the operator identity footprint (rtnetlink + ovsdb port capabilities) — confirmed the grant model is identity-scoped, not wildcard.
- xray default route restored (10.200.0.2 via grpc) after its old veth eth0 was removed — xray's own egress confirmed working.
- Ad hoc iptables bypass chains added on host (OP_NETMK_BYPASS_FWD, OP_NETMK_BYPASS_NAT) and on xray (OP_NETMK_BYPASS_FWD), as a stand-in for the spec's full OP_NETMK_* chain set, since wgcf-egress (the spec's assumed upstream WireGuard tunnel) does not exist on this host.

Open problem to diagnose and resolve in the spec:
- Transit traffic from netmaker (10.200.1.1) through xray (grpc) through host (svc0 -> pub0) to the internet is NOT working. Host's own egress works; xray's own egress works; netmaker can reach the xray gateway and host's svc0 address directly. But host's OP_NETMK_BYPASS_FWD/NAT chain counters show exactly 0 packets/0 bytes despite jump rules being correctly placed in FORWARD/POSTROUTING, matching svc0<->pub0 and 10.200.1.1 source/dest, and despite correct-looking return/default routes. Leading unconfirmed hypothesis: rp_filter (reverse path filtering) on svc0 and/or pub0 dropping the asymmetric multi-hop path before it reaches these chains. Diagnose the actual cause and produce concrete tasks to fix it (e.g. rp_filter mode changes scoped to the minimum needed interfaces, or an alternate routing/marking fix if rp_filter isn't the cause).

Hard constraint from the user, must be reflected in the spec: no dropping or downgrading of Netmaker functionality (e.g. disabling EE/Pro licensing) as a workaround for this networking gap. The only acceptable fix is real working internet egress for the netmaker container. Do not propose license/feature downgrades as an option.

Also fold in as still-outstanding, separate tasks (don't block on them, just track):
- crates/op-plugins/src/state_plugins/netmaker.rs schema/dispatch fixes need `cargo check -p op-plugins` verification and an SHM catalog reseal afterward.
- deploy/runit/migrate-netmaker-to-runit.sh is blocked on the /opt/op-dbus/golden tree not existing yet (deploy/runit/build-golden.sh has not been run).

Output: updated design.md/requirements.md/tasks.md reflecting real current state, with the rp_filter/transit-routing investigation broken into concrete verifiable subtasks.
