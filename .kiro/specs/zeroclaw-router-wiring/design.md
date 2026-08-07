# ZeroClaw Router Wiring — Technical Design

## Status: Final

See `requirements.md` for locked decisions D1–D5.

## Architecture

```
┌──────────────────────────────────────────────────────────────┐
│ Router (OpenWrt)                                             │
│                                                              │
│  Clients ──► zeroclaw gateway :42617                         │
│   (loopback default;          │                              │
│    LAN only if paired)        ▼                              │
│                    OpenAI-compatible provider "odbus"        │
│                               │                              │
│                               │ + x-ghostbridge-footprint    │
│                               │ + trace-id and/or wg pubkey  │
│                               │   (ops-provisioned)          │
│                               ▼                              │
│                    netmaker → 10.0.0.2                       │
└───────────────────────────────┼──────────────────────────────┘
                                │
                ┌───────────────┴────────────────┐
                ▼                                ▼
┌───────────────────────────┐    ┌───────────────────────────────┐
│ op-web :8080 (HTTP)       │    │ op-grpc-bridge :8090          │
│  /v1/models               │───►│ gRPC / gRPC-Web (mesh)        │
│  /v1/chat/completions     │    │ sole PluginService router     │
│  requires Ghostbridge hdrs│    │ enforce_bridge_capability     │
└───────────────────────────┘    └───────────────────────────────┘
```

ZeroClaw on the router speaks **OpenAI HTTP** to op-web. op-web adapts each
call into a schema-declared bridge method on `:8090` and forwards the same
Ghostbridge identity (`crates/op-web/src/handlers/zeroclaw.rs`).

## Auth model (machine mesh client)

1. Ops creates/registers a **machine** Ghostbridge footprint on the host
   (sled / grants — existing path; not HumanPrincipal).
2. Ops grants that footprint the capabilities required for zeroclaw methods
   used by `/v1` adapters.
3. Values are copied to the router over SSH (or equivalent), never via git.
4. ZeroClaw injects them as `extra_headers` on every upstream request.
5. Missing/wrong identity ⇒ op-web 401 / bridge unauthenticated or
   permission denied — fail-closed.

**Not used here:** `OracleIdentityAssertion`, decoy signing, HumanPrincipal
resolve. Those are human-only (identity-handoff).

**Hygiene:** treat provisioned headers as rotatable credentials; document
expiry/rotation in ops notes outside the repo if desired. This is constrained
residual-risk use for mesh machines, not the human product path.

## Configuration template

`/fast/zeroclaw/config/config.toml` — placeholders only:

```toml
schema_version = 1
default_provider = "odbus"

[gateway]
port = 42617
# Default: loopback. Non-loopback requires pairing (D3).
host = "127.0.0.1"
require_pairing = true

[providers.models.openai.odbus]
# OpenAI HTTP surface is op-web, not the gRPC bridge.
uri = "http://10.0.0.2:8080/v1"
api_key = "unused"
model = "auto"

[providers.models.openai.odbus.extra_headers]
# OPS-PROVISIONED — never commit real values
x-ghostbridge-footprint = "<OPS_PROVISIONED_FOOTPRINT>"
x-ghostbridge-trace-id = "<OPS_PROVISIONED_TRACE_OR_OMIT_IF_USING_WG>"
# Optional alternative / complement per op-web ghostbridge_metadata():
# x-wireguard-pubkey = "<OPS_PROVISIONED_PUBKEY>"

[shell-tool]
enabled = false

[browser]
enabled = false

[web-search]
enabled = false

[mcp]
enabled = false
```

## Ops provisioning (host → router)

1. On host, obtain or mint the machine footprint used for this router peer
   (existing identity/sled tooling — exact command is host-ops, not fixed here).
2. Ensure `/dev/shm/opdbus/capability-grants.json` (or `OP_GRANTS_PATH`) grants
   that footprint the zeroclaw capabilities required by op-web adapters.
3. SSH to router `192.168.1.1`; write config with real header values.
4. Confirm values do not appear in any git-tracked file.

## Init script

`/etc/init.d/zeroclaw`:

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

## Surface reconciliation

| Endpoint | Role |
|---|---|
| `10.0.0.2:8090` | Bridge gRPC / gRPC-Web; topology lock; not OpenAI `/v1` |
| `10.0.0.2:8080/v1/*` | op-web OpenAI adapters → bridge |
| Router `:42617` | Local ZeroClaw gateway for paired/loopback clients |

`wget http://10.0.0.2:8090/` succeeding only proves bridge HTTP `/` — it does
**not** prove OpenAI compatibility.

## Firewall

- Outbound mesh to `10.0.0.2:8080` and `:8090` must be allowed (netmaker path).
- LAN access to `:42617` only if bind/policy explicitly enabled with pairing.

## Rollback

```bash
/etc/init.d/zeroclaw stop
/etc/init.d/zeroclaw disable
rm -f /fast/zeroclaw/config/config.toml
```
