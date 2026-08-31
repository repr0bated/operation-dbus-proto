# ZeroClaw Router Wiring — Technical Design

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│ Router (OpenWrt)                                                │
│                                                                 │
│  LAN Clients ──► zeroclaw gateway :42617                       │
│                      │                                          │
│                      ▼                                          │
│              OpenAI-compatible                                  │
│              provider "odbus"                                   │
│                      │                                          │
│                      │ + x-ghostbridge-footprint                │
│                      │ + x-ghostbridge-trace-id                 │
│                      ▼                                          │
│              netmaker (10.0.0.2)                                │
└──────────────────────┼──────────────────────────────────────────┘
                       │
                       ▼
┌──────────────────────────────────────────────────────────────────┐
│ Host (artix)                                                     │
│                                                                  │
│  op-grpc-bridge :8090 ◄── svc0 (10.0.0.2)                       │
│       │                                                          │
│       ▼                                                          │
│  zeroclaw plugin ──► provider routing ──► upstream LLM APIs     │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

## Configuration Files

### /fast/zeroclaw/config/config.toml

```toml
schema_version = 1
default_provider = "odbus"

[gateway]
port = 42617
host = "0.0.0.0"
require_pairing = false
allow_public_bind = true

[providers.models.openai.odbus]
uri = "http://10.0.0.2:8090/v1"
api_key = "unused"
model = "auto"

[providers.models.openai.odbus.extra_headers]
x-ghostbridge-footprint = "4a99dfe92638818966af0687189b6f2c85c7807e663f15cade14d2308b848e0e"
x-ghostbridge-trace-id = "8423bb899e754ded9673ac45752b5bc9"

[shell-tool]
enabled = false

[browser]
enabled = false

[web-search]
enabled = false

[mcp]
enabled = false
```

### /etc/init.d/zeroclaw (procd init)

Should contain:
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

## Implementation Steps

### Step 1: Create config directory and file
```bash
# On router via LAN SSH (192.168.1.1)
mkdir -p /fast/zeroclaw/config /fast/zeroclaw/state
cat > /fast/zeroclaw/config/config.toml << 'EOF'
# ... config content above ...
EOF
```

### Step 2: Verify init script
```bash
cat /etc/init.d/zeroclaw
# Ensure it points to correct paths and uses gateway start
```

### Step 3: Enable and start service
```bash
/etc/init.d/zeroclaw enable
/etc/init.d/zeroclaw start
```

### Step 4: Verify
```bash
# Check process
ps | grep zeroclaw

# Check listening
netstat -tlnp | grep 42617

# Test endpoint
wget -q -O- http://127.0.0.1:42617/health || curl http://127.0.0.1:42617/health
```

## Verification Commands (from router)

```bash
# 1. Verify bridge reachability
wget -q -O- http://10.0.0.2:8090/api/health

# 2. Test models endpoint through gateway
wget -q -O- http://127.0.0.1:42617/v1/models

# 3. Test chat completion (requires valid model)
wget -q -O- --post-data='{"model":"auto","messages":[{"role":"user","content":"hi"}]}' \
  --header='Content-Type: application/json' \
  http://127.0.0.1:42617/v1/chat/completions
```

## Firewall Notes

Router firewall verified:
- `wg0` is in `lan` zone with `input=ACCEPT`, `output=ACCEPT`, `forward=ACCEPT`
- No additional firewall rules needed for outbound to 10.0.0.2

## Rollback

```bash
/etc/init.d/zeroclaw stop
/etc/init.d/zeroclaw disable
rm -rf /fast/zeroclaw/config/config.toml
```
