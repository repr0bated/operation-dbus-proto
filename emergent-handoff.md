# Handoff: op-network OpenFlow flooding fix restoration + encap translator support

## Host & repo
- Repo: `/srv/git/odbus`, Rust workspace, current branch tip `ffcb4796` (main line).
- Crate: `op-network` — passive-mode OpenFlow controller for OVS bridge `ovsbr0`.
- Live host runs **runit** (PID 1): services via `sudo sv <cmd> <svc>`, never systemctl/s6.
  Deployment is btrfs send/receive via `deploy/runit/build-golden.sh` — do not hand-copy binaries.
- Bridge ports: `pub0` (ISP uplink), `svc0`, `chatbot-port`, `3tched`, `eth0`,
  `netmaker` (WireGuard, L3-only).

## Problem history (verified, not speculative)

1. **PORT_DESC root cause**: `build_port_desc_request()` in
   `crates/op-network/src/controller.rs` builds an OF1.5 multipart request but leaves
   `port_no=0` in the 8-byte `ofp15_port_desc_request` body (`ofp_multipart_request`
   header = type(2)+flags(2)+pad(4); body starts at offset 8). Port 0 is invalid →
   OVS returns an empty port list with no error → every symbolic port name
   unresolvable → flows never install → `actions=NORMAL` floods gateway-bound frames
   (VRRP MAC `00:00:5e:00:01:0a`) to all ports incl. `pub0`.

   Fix: write `OFPP_ANY (0xFFFF_FFFF)` at `body[8..12]`, correct the doc comment,
   add unit test `test_port_desc_request_asks_for_all_ports`.

2. **Fatal static-flow loop**: loop uses `?`; one unresolvable flow aborts the session
   → 8s reconnect loop that also starves the static FDB pin. Fix: match + `warn!` +
   `static_skipped` counter.

3. **Barrier bail**: barrier reply loop `bail!`s on FlowMod error and would hang once
   demoted (BarrierReply becomes only exit). Fix: `warn!` + 10s `tokio::time::timeout`
   per recv + break when error xid == `barrier_xid`.

4. **FDB pin ordering**: move `ensure_static_fdb_entries` from after the static-flow
   loop/barrier to *before* both (step 5b).

5. **Encap flow silently dropped** (still unfixed):
   `deploy/config/openflow-static-flows.json` and
   `/etc/op-dbus/openflow-static-flows.json` contain a valid PTAP flow —

       priority=200, in_port=netmaker, packet_type=(1,0x800),
       actions=[encap(ethernet), set_field eth_src/dst, output]

   needed because WireGuard is L3 (no Ethernet header, MAC learning impossible,
   ~34k tx errors). `crates/op-network/src/openflow_translate.rs` enum
   `JsonFlowAction` has no `Encap` variant and the match-key parser has no
   `packet_type`, so the flow loads then vanishes at translation.

## Work already done — restore, don't rewrite

Commit `106a0ef6` on branch `genesis-identity-recovery`
("fix(openflow): request all ports in OF1.5 PORT_DESC; degrade static flows",
97 lines changed) implements items 1–4 against an older file revision. It does NOT
apply cleanly to current HEAD (`git apply --3way` conflicts around controller.rs
lines 93/403/434/446/780). Task: hand-port those hunks onto current
`crates/op-network/src/controller.rs` preserving current code style.

Reference material:
- `/srv/git/flooding-fix/controller-fdb.patch`
- `/srv/git/flooding-fix/transcript.md`
- `/srv/git/flooding-fix/memory/PRD.md`

Then implement item 5: add `packet_type` match key +
`JsonFlowAction::Encap { ethertype }` → `encap(ethernet)` action in
`openflow_translate.rs` so the prio-200 netmaker flow survives translation.

## Known follow-ups (do NOT fix now unless trivial)
- Defect "netmaker-ovs-attach never attaches" is contained-not-fixed by design
  (skip+warn stays until attach works).
- Barrier timeout is per-recv, not total budget.
- No Rust tests yet cover skip-and-log arm, non-fatal barrier arm, or timeout —
  add them if cheap.
- `unixctl.rs` supports `OVS_VSWITCHD_CTL` override — verify socket discovery
  isn't pid-baked.

## Verification (must pass)

```sh
cargo build -p op-network
cargo test -p op-network test_port_desc_request_asks_for_all_ports
# unit test or grep proving the prio-200 encap flow now translates
```

Post-deploy runtime checks (operator runs these, agent does not):

```sh
ovs-ofctl -O OpenFlow15 dump-ports-desc ovsbr0 | head   # 6 ports, not empty
ovs-appctl fdb/show ovsbr0 | grep 00:00:5e:00:01:0a     # pin present
ovs-ofctl dump-flows ovsbr0 | grep packet_type          # encap flow finally installed
```

## Hard constraints
- Do not touch the live OVS datapath, restart network-critical runit services,
  deploy, or edit anything under `/etc/xray` or `/run/runit/service`.
- Do not introduce new Python deps; Rust-first per AGENTS.md.
- Commit separately: (1) ported controller fix, (2) encap/packet_type translator
  support + tests.
