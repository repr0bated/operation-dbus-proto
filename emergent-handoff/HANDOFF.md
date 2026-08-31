# Handoff — op-network OpenFlow flooding fix: restore ported commit + encap translator

You are producing patches against Rust files included in this package. You have NO
access to the target machine. Everything you need is in this zip:

```
src/controller.rs            CURRENT target file (the live version to modify)
src/openflow_translate.rs    CURRENT translator to extend
config/openflow-static-flows.json   static flow set incl. the prio-200 encap flow
reference/106a0ef6.diff      prior fix commit diff (older base revision)
reference/controller-fdb.patch  original reviewed 4-hunk patch (superseded by the commit)
HANDOFF.md                   this file
```

## System context (read-only facts)

A single VPS runs an OVS bridge `ovsbr0` with ports: `pub0` (ISP uplink),
`svc0`, `chatbot-port`, `3tched`, `eth0`, `netmaker` (WireGuard, L3-only).
`op-network` is a passive-mode OpenFlow controller: OVS connects to IT over TCP,
it sends HELLO → FEATURES → SET_CONFIG → PortDesc multipart request → installs
NORMAL fallback + configured flows, and pins a static FDB entry
(`ovsbr0:eth0:0:00:00:5e:00:01:0a`, the ISP VRRP virtual MAC) via unixctl.
The ISP filters VRRP off customer ports, so the MAC can never be learned — the
pin is mandatory or `actions=NORMAL` floods gateway-bound frames to every port.

## Defects to fix

### A. PORT_DESC asks for port 0 (root cause)
In `src/controller.rs`, `build_port_desc_request()` allocates an 8-byte body and
sets only bytes [0..2] (`OFPMP_PORT_DESC`). The OF1.5 request body layout is:

```c
struct ofp15_port_desc_request {   /* openflow-1.5.h */
    ovs_be32 port_no;              /* All ports if OFPP_ANY. */
    uint8_t pad[4];
};
```

i.e. after type(2)+flags(2)+pad(4), bytes [8..12] must be `port_no`.
Leaving them zero asks for port 0 → invalid → OVS returns an EMPTY port list
with no error → every symbolic port name unresolvable → flows fail → NORMAL
floods everything.

Fix:
- `body[8..12].copy_from_slice(&OFPP_ANY.to_be_bytes());` where
  `const OFPP_ANY: u32 = 0xFFFF_FFFF;` (already defined in the file).
- Correct the doc comment above the fn to state the real layout
  type(2)+flags(2)+pad(4)+port_no(4)+pad(4).
- Add unit test `test_port_desc_request_asks_for_all_ports`: build the request,
  assert total length 24 and `u32::from_be_bytes(body[8..12]) == OFPP_ANY`.

### B. Static-flow loop is fatal
The "6b" durable-static-flows loop uses `?`; ONE unresolvable flow (netmaker,
see D) aborts the whole session → 8s reconnect loop that also starves the FDB pin.
Fix: match on the result, count successes (`static_installed`) and skips
(`static_skipped`), log each skip with `warn!`, never abort.

### C. Barrier error arm bails / would hang
The barrier-reply loop `bail!`s on any FlowMod OFPT_ERROR. Demoting it to `warn!`
alone introduces an infinite loop (BarrierReply becomes the only exit).
Fix: demote to `warn!` AND add a 10s `tokio::time::timeout` around the recv AND
break when an error message carries `barrier_xid`.

### D. FDB pin ordering
Move the static FDB pin block (`ensure_static_fdb_entries`) to BEFORE the
static-flow loop and its barrier (call it step 5b). L2 reachability must never
gate on symbolic-port resolution. Note `ensure_static_fdb_entries` already
returns a count and logs internally; keep logging at the call site minimal.

### E. Encap/packet_type unsupported in translator (separate change)
`config/openflow-static-flows.json` contains:

```json
{ "priority": 200, "match": { "in_port": "netmaker", "packet_type": "(1,0x800)" },
  "actions": [ {"type":"encap","ethertype":"ethernet"}, ...set_field/output... ] }
```

This is REQUIRED: WireGuard has no Ethernet header, so MAC learning is impossible
on `netmaker`; the flow matches IPv4 arriving on the L3 port and encapsulates an
Ethernet header (PTAP). Today `openflow_translate.rs` silently drops it:
`JsonFlowAction` has no `Encap` variant and the match parser has no `packet_type`
key. The controller logs success while installing only the flows it understands.

Fix (in `src/openflow_translate.rs`):
1. Parse `packet_type: "(ns,type)"` in the match struct; represent as tuple;
   when present pass it through to the flow match builder (crate `rovs` /
   whatever Match API the crate exposes — inspect imports; if the underlying
   library cannot express packet_type, emit the OpenFlow field manually or
   document precisely why and stop after implementing 2–4).
2. Add `JsonFlowAction::Encap { ethertype }` mapping to `encap(ethernet)`
   (`packet_type-aware action`), placed before any set_field of eth_src/eth_dst
   in the emitted ActionList order.
3. Unit tests: prio-200 netmaker JSON translates end-to-end without error;
   unknown action types still produce a clear error (not silent skip).

## Deliverables

Two separate diffs/patches (do not combine):

1. `0001-controller-fix.patch` — items A–D against `src/controller.rs`.
2. `0002-translator-encap.patch` — item E against `src/openflow_translate.rs`.

Unified diffs, valid `git apply` input: explicit hunk ranges (never bare `@@`),
never `old_start <= 1` on hunk 1 unless truly at line 1, always ≥1 line of both
leading and trailing context per hunk (a fragment with zero trailing context can
only apply at EOF), final trailing newline present. Verify each patch applies to
the INCLUDED copies of the files before returning.

## Verification you must run (no hardware needed)

- `cargo build -p op-network` if you have a Rust toolchain; otherwise state
  plainly it was not compiled.
- Structural: apply your own patches to the included sources with
  `git apply --check` at zero drift.
- Do NOT claim runtime verification of OVS behaviour — impossible from here.
  The operator will check post-deploy:
  `dump-ports-desc ovsbr0` lists 6 ports; `fdb/show ovsbr0 | grep 00:00:5e:00:01:0a`;
  `dump-flows ovsbr0 | grep packet_type` shows the prio-200 flow finally installed.

## Constraints

- No new external crates unless strictly necessary; say why if you add one.
- Keep existing code style (thiserror/anyhow patterns, `log::` macros, doc comments).
- Known accepted follow-ups (do NOT fix): netmaker attach itself stays broken
  (skip+warn is intentional until fixed); barrier timeout remains per-recv.
