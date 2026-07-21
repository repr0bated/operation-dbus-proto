# Stale Excerpts: ghostbridge-identity-sled-snowball-pipeline.md

**Source**: `/mnt/opt-inspect/home/git/operation-dbus-proto/docs/ghostbridge-identity-sled-snowball-pipeline.md`

**Extraction Date**: 2026-07-20

**Status**: These excerpts represent outdated content that has been replaced in the current documentation.

## Dropped Section 1: Old wg-xray Datapath Reference

**Context**: The Model (Settled) → Xray section

**Stale Content**:
```markdown
- **Xray** (in the wg-xray datapath):
  - Receives GB_* env vars.
  - Injects into gRPC metadata on outbound:
    - `X-Ghostbridge-Footprint`
    - `X-Ghostbridge-Trace-ID`
    - `X-WireGuard-Pubkey` (when applicable).
```

**Why Dropped**: The `wg-xray datapath` is an obsolete architecture. Current implementation has Xray running on the host, attached to the OVS datapath, not in a separate wg-xray datapath.

**Replaced With**: "Xray (on the host, attached to the OVS datapath)" - reflects the actual network topology where Xray is an OVS-integrated router, not a standalone WireGuard-coupled datapath component.

---

## Dropped Section 2: Port 18789 Reference

**Context**: The Model (Settled) → GhostbridgeInterceptor section

**Stale Content**:
```markdown
- **GhostbridgeInterceptor** (`op-grpc-bridge/src/interceptor.rs` + similar in other crates):
  - Enforces the Accountability Loop on every gRPC ingress (port 18789).
```

**Why Dropped**: Port 18789 is the old gRPC port number. The system has been migrated to port 8090.

**Replaced With**: "port 8090" - the current gRPC ingress port per the active architecture.

**Configuration Impact**: Any client configurations, firewall rules, or service definitions referencing port 18789 must be updated to port 8090.

---

## Summary of Changes

| Old Value | New Value | Reason |
|-----------|-----------|--------|
| `in the wg-xray datapath` | `on the host, attached to the OVS datapath` | Architecture evolution: Xray is now OVS-integrated, not a separate WG datapath |
| `port 18789` | `port 8090` | Port standardization for gRPC ingress |

## Related Files to Check

If merging this documentation, ensure these related files also reflect the updated architecture:

- `crates/op-xray-daemon/` - Xray daemon implementation
- `crates/op-grpc-bridge/src/interceptor.rs` - Should reference port 8090
- `deploy/` - Service definitions and configuration templates
- Any client configuration files or environment variable definitions

<!-- Extracted from /mnt/opt-inspect/home/git/operation-dbus-proto/docs/ghostbridge-identity-sled-snowball-pipeline.md on 2026-07-20 -->
