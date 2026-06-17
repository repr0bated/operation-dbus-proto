# Implementation Log — GhostBridge Host Consolidation

## Status
- T1: ✅ COMPLETE — xray REALITY secrets extracted to `/etc/ghostbridge/xray.env`
- T2: ✅ COMPLETE — `cargo check -p op-plugins` clean; `cargo check -p op-identity` clean
- T3: ✅ COMPLETE — dynamic xray cutover to `/dev/shm/xray-ghostbridge.json`; host gbr-xray service up
- T4: ✅ COMPLETE — DNS split-horizon fixed; NextDNS forwarder; `*.ghostbridge.tech` → 10.200.0.2
- T5: ✅ COMPLETE — Gemma routing brain: subid → tag → xray + OpenFlow rules
- T6: ✅ COMPLETE — derived session_id from WireGuard pubkey; persistent identity vault
- T7: ⏮️ REVERTED — op-web zeroclaw route alias, openclaw retirement, and AccountabilityPage chat wiring rolled back per user request (zeroclaw refactor)
- T8: ⏭️ SKIPPED — Voyage chunking (per user instruction)

## Live verification
- `gbr-xray` up; TLS handshake on 443 succeeds; HTTPS fallback returns 200
- `zeroclaw` s6-supervised and active
- `gemma` oneshot runs before gbr-xray and regenerates routes + config
- `op-web-srv` s6 service started; notification-fd restored
- T7 op-web changes reverted, so `openclaw` routes are restored in source

## Notes for next session
- One pre-existing environmental unit test fails: `anna_scribe::tests::test_notarize_arrival_rejects_missing_schema` (depends on `/dev/shm/live-schema.json` not existing). Not introduced by this work.
- One pre-existing op-web test fails: `privacy_container::tests::desired_instance_publishes_route_without_bridged_nic_by_default`. Not introduced by this work.
- End-to-end WG client → cognitive-mcp/netmaker test still pending per handoff verification gates.
- Zeroclaw refactor files are present in the working tree (untracked) and should be integrated in a follow-up session.
