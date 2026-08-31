# ZeroClaw Router Wiring — Implementation Tasks

## Prerequisites (Verified ✓)
- [x] Router reachable via LAN SSH at 192.168.1.1 (no password)
- [x] Router can reach bridge at 10.0.0.2:8090 via netmaker
- [x] ZeroClaw binary exists at /fast/zeroclaw/bin/zeroclaw
- [x] Init script exists at /etc/init.d/zeroclaw
- [x] Ghostbridge footprint obtained from host

## Tasks

### Task 1: Create ZeroClaw config
**Execute on router via LAN SSH (192.168.1.1)**

```bash
mkdir -p /fast/zeroclaw/config /fast/zeroclaw/state

cat > /fast/zeroclaw/config/config.toml << 'EOF'
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
EOF
```

### Task 2: Verify init script configuration
**Execute on router**

```bash
cat /etc/init.d/zeroclaw
```

Ensure it includes:
- `ZEROCLAW_CONFIG_DIR=/fast/zeroclaw/config` in environment
- `HOME=/fast/zeroclaw/state` in environment
- Command: `zeroclaw gateway start --port 42617`

If init script needs updating:
```bash
cat > /etc/init.d/zeroclaw << 'EOF'
#!/bin/sh /etc/rc.common
START=99
USE_PROCD=1

start_service() {
    procd_open_instance
    procd_set_param command /fast/zeroclaw/bin/zeroclaw gateway start --port 42617
    procd_set_param env HOME=/fast/zeroclaw/state ZEROCLAW_CONFIG_DIR=/fast/zeroclaw/config
    procd_set_param respawn
    procd_set_param stdout 1
    procd_set_param stderr 1
    procd_close_instance
}
EOF
chmod +x /etc/init.d/zeroclaw
```

### Task 3: Enable and start service
**Execute on router**

```bash
/etc/init.d/zeroclaw enable
/etc/init.d/zeroclaw start
sleep 3
ps | grep zeroclaw
netstat -tlnp | grep 42617
```

### Task 4: Verify end-to-end
**Execute on router**

```bash
# Health check (if endpoint exists)
wget -q -O- http://127.0.0.1:42617/health 2>&1 || echo "No health endpoint"

# Models list
wget -q -O- http://127.0.0.1:42617/v1/models

# If models work, test chat
wget -q -O- --post-data='{"model":"auto","messages":[{"role":"user","content":"ping"}]}' \
  --header='Content-Type: application/json' \
  http://127.0.0.1:42617/v1/chat/completions
```

### Task 5: Verify from LAN client
**Execute from any device on router's LAN (192.168.1.x)**

```bash
curl http://192.168.1.1:42617/v1/models
```

## Success Criteria
- [ ] ZeroClaw process running on router
- [ ] Port 42617 listening
- [ ] /v1/models returns model list from bridge
- [ ] Chat completion request succeeds
- [ ] LAN clients can reach router:42617

## Troubleshooting

**If zeroclaw fails to start:**
```bash
# Check binary
/fast/zeroclaw/bin/zeroclaw --version

# Try manual start with debug
ZEROCLAW_CONFIG_DIR=/fast/zeroclaw/config HOME=/fast/zeroclaw/state \
  /fast/zeroclaw/bin/zeroclaw gateway start --port 42617
```

**If bridge unreachable:**
```bash
# Verify netmaker
wg show netmaker
ip route | grep 10.0.0

# Test bridge directly
wget -q -O- http://10.0.0.2:8090/api/health
```

**If auth fails (401/403):**
- Verify ghostbridge headers in config match current host identity
- Check host identity: `curl http://10.0.0.2:8080/api/identity/sled`
