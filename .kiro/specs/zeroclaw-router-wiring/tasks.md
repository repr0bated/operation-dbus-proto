# ZeroClaw Router Wiring — Implementation Tasks

## Status: Final — implement when gates pass

Do not paste secrets from this repo. Obtain identity material from ops on the host.

## Gates (must be green before Task 4+)

- [ ] **G1** From router: `wget -q -O- http://10.0.0.2:8080/v1/models` without
  identity returns 401/403 (op-web reachable + fail-closed).
- [ ] **G2** Machine footprint minted/known on host; grants allow required
  zeroclaw capabilities for that footprint.
- [ ] **G3** Binary `/fast/zeroclaw/bin/zeroclaw` and init `/etc/init.d/zeroclaw`
  present and sane.

## Prerequisites

- [x] Router SSH via LAN `192.168.1.1` (prior claim; re-check)
- [x] Router reaches `10.0.0.2:8090` (ping / bridge `/` — prior claim)
- [ ] Router reaches `10.0.0.2:8080` (OpenAI surface) — **G1**
- [ ] Ops-provisioned Ghostbridge headers available on implementer machine — **G2**

## Tasks

### Task 1: Config directories
On router:

```bash
mkdir -p /fast/zeroclaw/config /fast/zeroclaw/state
ls -la /fast/zeroclaw/
```

### Task 2: Deploy config (ops values)
Copy template from `design.md`. Replace placeholders with **ops-provisioned**
footprint / trace (and/or WireGuard pubkey). URI must be
`http://10.0.0.2:8080/v1`. Bind default `127.0.0.1`, `require_pairing = true`.

Verify:

```bash
grep -E '8080/v1|127.0.0.1|require_pairing' /fast/zeroclaw/config/config.toml
# Confirm real secrets are present on disk and absent from git
```

### Task 3: Init script
```bash
cat /etc/init.d/zeroclaw
```

Confirm `ZEROCLAW_CONFIG_DIR`, `HOME` state dir, and
`zeroclaw gateway start --port 42617`. Update from `design.md` if needed;
`chmod +x`.

### Task 4: Enable and start
```bash
/etc/init.d/zeroclaw enable
/etc/init.d/zeroclaw start
sleep 3
ps | grep zeroclaw
netstat -tlnp | grep 42617
```

Expect listen on `127.0.0.1:42617` (not `0.0.0.0` unless intentionally paired).

### Task 5: Fail-closed checks
```bash
# Direct to op-web without identity — must fail
wget -q -O- http://10.0.0.2:8080/v1/models 2>&1

# Through local gateway with empty/wrong upstream headers — must fail
# (depends on config; if config has good headers, test by temporarily
# breaking footprint and restarting, then restore)
```

### Task 6: Authorized path
With correct ops headers in config:

```bash
wget -q -O- http://127.0.0.1:42617/v1/models
# Optional chat:
# wget -q -O- --post-data='{"model":"auto","messages":[{"role":"user","content":"ping"}]}' \
#   --header='Content-Type: application/json' \
#   http://127.0.0.1:42617/v1/chat/completions
```

### Task 7: LAN policy (only if exposing beyond loopback)
Document clients + grants; keep `require_pairing = true`; do not commit that
document’s secrets.

## Success criteria

- [ ] Process up; `:42617` on configured bind only
- [ ] Unauthorized op-web `/v1` rejected
- [ ] Authorized gateway `/v1/models` succeeds via op-web → bridge
- [ ] No live footprints/trace-ids in git
- [ ] URI targets `:8080/v1`, not `:8090/v1`

## Troubleshooting

```bash
/fast/zeroclaw/bin/zeroclaw --version
ZEROCLAW_CONFIG_DIR=/fast/zeroclaw/config HOME=/fast/zeroclaw/state \
  /fast/zeroclaw/bin/zeroclaw gateway start --port 42617

wg show netmaker
wget -q -O- http://10.0.0.2:8080/ 2>&1
wget -q -O- http://10.0.0.2:8090/ 2>&1
```

If requests succeed **without** identity: stop service immediately; fix auth
before continuing.
