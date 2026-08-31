# ZeroClaw Router Wiring — Implementation Tasks

## Status: Blocked — cannot implement until auth model resolved

See `requirements.md` for blockers.

## Prerequisites

### Verified
- [x] Router reachable via LAN SSH at 192.168.1.1
- [x] Router can reach bridge at 10.0.0.2:8090 via netmaker (ping OK, wget HTTP 200)
- [x] ZeroClaw binary exists at /fast/zeroclaw/bin/zeroclaw
- [x] Init script exists at /etc/init.d/zeroclaw

### Blocked
- [ ] Auth model chosen (assertion vs labeled residual-risk lab mode)
- [ ] If assertion: Oracle signing + bridge validation implemented
- [ ] If lab mode: residual-risk envelope scoped with expiry
- [ ] Identity values provisioned by ops (NOT from git)

## Tasks (When Unblocked)

### Task 1: Create config directory structure
**Execute on router via LAN SSH (192.168.1.1)**

```bash
mkdir -p /fast/zeroclaw/config /fast/zeroclaw/state
```

Verification:
```bash
ls -la /fast/zeroclaw/
```

### Task 2: Deploy config (auth-dependent)

**DO NOT paste secrets from this spec. Obtain from ops provisioning.**

Template config in `design.md`. Actual deployment:

**If assertion auth (product path)**:
1. Confirm assertion mechanism available
2. Configure assertion source in config
3. Deploy config without hardcoded secrets

**If lab mode (residual-risk)**:
1. Obtain identity values from ops (not git): `<ops procedure TBD>`
2. Deploy config with values and expiry comment
3. Label deployment as residual-risk

Verification:
```bash
cat /fast/zeroclaw/config/config.toml
# Confirm: no hardcoded secrets in git history
# Confirm: expiry date if lab mode
```

### Task 3: Verify init script configuration
**Execute on router**

```bash
cat /etc/init.d/zeroclaw
```

Confirm:
- `ZEROCLAW_CONFIG_DIR=/fast/zeroclaw/config`
- `HOME=/fast/zeroclaw/state`
- Command: `zeroclaw gateway start --port 42617`

### Task 4: Enable and start service
**Execute on router**

```bash
/etc/init.d/zeroclaw enable
/etc/init.d/zeroclaw start
sleep 3
```

Verification:
```bash
ps | grep zeroclaw
netstat -tlnp | grep 42617
```

### Task 5: Verify auth rejection (fail-closed)

**Critical**: Confirm unauthorized requests are rejected.

```bash
# Request without credentials should fail
wget -q -O- http://127.0.0.1:42617/v1/models 2>&1
# Expected: 401 or 403, NOT success
```

### Task 6: Verify authorized request

Method depends on auth model:

**If assertion**:
```bash
# Obtain assertion, inject into request
# <mechanism TBD>
```

**If lab mode**:
```bash
# With ops-provisioned credentials in config
wget -q -O- http://127.0.0.1:42617/v1/models
# Expected: model list from bridge
```

### Task 7: Document LAN access policy (if LAN exposed)

If `host` changed from `127.0.0.1`:
1. Document which clients may call gateway
2. Document capability grants per client
3. Ensure `require_pairing = true` or equivalent auth

## Success Criteria

- [ ] ZeroClaw process running on router
- [ ] Port 42617 listening on configured bind address
- [ ] Unauthorized requests rejected (fail-closed)
- [ ] Authorized requests succeed through to bridge
- [ ] No secrets in git history
- [ ] If lab mode: expiry date documented and calendared

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

# Test bridge directly
wget -q -O- http://10.0.0.2:8090/api/health
```

**If auth fails unexpectedly:**
- Verify auth mechanism matches config
- For assertion: check Oracle availability, signature validity
- For lab mode: verify ops-provisioned values correct, not expired
- Check bridge logs for validation errors

**If requests succeed without auth (SECURITY ISSUE):**
- Stop service immediately
- Verify config has auth enabled
- Do not proceed until fail-closed confirmed
