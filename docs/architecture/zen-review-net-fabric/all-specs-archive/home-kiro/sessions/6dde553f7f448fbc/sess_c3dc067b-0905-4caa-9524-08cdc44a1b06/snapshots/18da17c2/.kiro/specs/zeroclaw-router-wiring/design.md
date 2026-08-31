# ZeroClaw Router Wiring — Technical Design

## Status: Draft — blocked on identity/auth redesign

See `requirements.md` for blockers.

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│ Router (OpenWrt)                                                │
│                                                                 │
│  Authorized Clients ──► zeroclaw gateway :42617                │
│   (loopback / mesh     (127.0.0.1 or mesh bind default)        │
│    or paired LAN)             │                                 │
│                               ▼                                 │
│                    OpenAI-compatible provider                   │
│                               │                                 │
│                               │ + auth (see Auth Model below)  │
│                               ▼                                 │
│                    netmaker → 10.0.0.2:8090                    │
└───────────────────────────────┼─────────────────────────────────┘
                                │
                                ▼
┌──────────────────────────────────────────────────────────────────┐
│ Host (mesh-private, per topology lock)                           │
│                                                                  │
│  op-grpc-bridge :8090                                           │
│       │                                                          │
│       ├─► assertion validation (HumanPrincipal registry)        │
│       │                                                          │
│       ▼                                                          │
│  zeroclaw plugin ──► provider routing ──► upstream              │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

## Auth Model

### Product Path (Preferred)

Per [`netmaker-xray-identity-handoff/`](../netmaker-xray-identity-handoff/):

1. Router obtains `OracleIdentityAssertion` from Oracle decoy WG
2. Assertion injected as request header/metadata
3. Bridge validates assertion signature
4. Bridge resolves `HumanPrincipal` from assertion
5. Request proceeds if principal authorized

**Blocked**: Assertion signing/validation not yet implemented.

### Residual-Risk Lab Mode (If Needed)

If interim lab access required before product path ready:

1. Ops provisions identity values (footprint, trace-id) out-of-band
2. Values injected at deploy time, NOT hardcoded in git
3. Config labeled `residual-risk` with explicit expiry date
4. **NOT** in Active topology — separate lab topology only
5. Fail-closed: no fallback to weaker auth if assertion available

**Not yet scoped**: Residual-risk envelope undefined.

## Configuration Files

### /fast/zeroclaw/config/config.toml (Template)

```toml
schema_version = 1
default_provider = "odbus"

[gateway]
port = 42617
# SECURITY: Default to loopback. Mesh/LAN exposure requires auth.
host = "127.0.0.1"
# SECURITY: Require pairing for any non-loopback bind.
require_pairing = true

[providers.models.openai.odbus]
uri = "http://10.0.0.2:8090/v1"
api_key = "unused"
model = "auto"

# AUTH: Choose ONE of the following auth blocks.
# Do NOT use both. Do NOT hardcode values in git.

# --- Option A: Assertion auth (product path) ---
# [providers.models.openai.odbus.assertion]
# # Assertion obtained from Oracle decoy WG at runtime
# source = "oracle-assertion"  # placeholder: actual mechanism TBD
# # Bridge validates and resolves HumanPrincipal

# --- Option B: Residual-risk lab mode (time-boxed) ---
# [providers.models.openai.odbus.extra_headers]
# # VALUES PROVISIONED BY OPS — never hardcoded
# # x-ghostbridge-footprint = "<OPS_PROVISIONED>"
# # x-ghostbridge-trace-id = "<OPS_PROVISIONED>"
# # EXPIRY: <DATE> — remove after identity-handoff complete
# # SCOPE: Lab topology only, not Active

[shell-tool]
enabled = false

[browser]
enabled = false

[web-search]
enabled = false

[mcp]
enabled = false
```

### Ops Provisioning Steps (When Unblocked)

**For product path (Option A)**:
1. Deploy Oracle decoy WG with assertion signing capability
2. Configure router to obtain assertion at startup/request time
3. Document assertion injection mechanism for zeroclaw
4. Deploy bridge with assertion validation enabled

**For lab mode (Option B, if needed)**:
1. Generate identity values on host: `<document actual command>`
2. Securely transfer to router (SSH, not git)
3. Inject into config with expiry comment
4. Label deployment as `residual-risk`
5. Set calendar reminder for expiry review

### /etc/init.d/zeroclaw (procd init)

```sh
#!/bin/sh /etc/rc.common
START=99
USE_PROCD=1

ZEROCLAW_BIN="/fast/zeroclaw/bin/zeroclaw"
ZEROCLAW_CONFIG="/fast/zeroclaw/config"
ZEROCLAW_STATE="/fast/zeroclaw/state"

start_service() {
    procd_open_instance
    procd_set_param command "$ZEROCLAW_BIN" gateway start --port 42617
    procd_set_param env HOME="$ZEROCLAW_STATE" ZEROCLAW_CONFIG_DIR="$ZEROCLAW_CONFIG"
    procd_set_param respawn
    procd_set_param stdout 1
    procd_set_param stderr 1
    procd_close_instance
}
```

## Bridge Surface Reconciliation

**Active topology lock**: gRPC at 10.0.0.2:8090, mesh-private.

**Current design**: HTTP `/v1` (OpenAI-compatible) to same endpoint.

**Justification**: op-grpc-bridge exposes HTTP `/v1` surface at :8090 alongside gRPC. Both surfaces:
- Share the same auth gate (once implemented)
- Are mesh-private (no public exposure)
- Route through the same backend

**Verification needed**: Confirm bridge auth gate applies to HTTP `/v1` path, not just gRPC.

## Firewall Notes

Router firewall (verified):
- `wg0` in `lan` zone: input/output/forward = ACCEPT
- `netmaker` routes to 10.0.0.2
- No additional rules needed for outbound mesh traffic

LAN exposure (if enabled):
- Requires `require_pairing = true` or assertion auth
- Document which clients may call and with what grants

## Verification Commands (When Unblocked)

```bash
# 1. Verify bridge reachability (already confirmed)
wget -q -O- http://10.0.0.2:8090/api/health

# 2. Verify gateway listening
netstat -tlnp | grep 42617

# 3. Test models endpoint (requires auth working)
# Method depends on auth model chosen

# 4. Verify auth rejection without credentials
# Should fail with 401/403, not succeed
```

## Rollback

```bash
/etc/init.d/zeroclaw stop
/etc/init.d/zeroclaw disable
rm /fast/zeroclaw/config/config.toml
```
