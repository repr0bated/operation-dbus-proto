# Dropped Excerpts: zenflow-byom-profile.md

**Source:** `/mnt/opt-inspect/home/git/operation-dbus-proto/docs/zenflow-byom-profile.md`  
**Extraction Date:** 2026-07-20

## Port 18789 References (replaced with 8090)

### Excerpt 1: Setup Step 3
**Location:** Setup Steps, item 3  
**Reason:** Port standardization - OpenClaw gateway now runs on 8090

Original:
```bash
export ZENFLOW_OPENCLAW_BASE_URL=http://127.0.0.1:18789
```

Replaced with:
```bash
export ZENFLOW_OPENCLAW_BASE_URL=http://127.0.0.1:8090
```

### Excerpt 2: Verification Section
**Location:** Verification, item 2  
**Reason:** Port standardization - consistent with updated gateway port

Original:
```
Watch the OpenClaw/log output to confirm it connects to `http://127.0.0.1:18789/v1/chat/completions` with the configured model.
```

Replaced with:
```
Watch the OpenClaw/log output to confirm it connects to `http://127.0.0.1:8090/v1/chat/completions` with the configured model.
```

---

**Note:** These excerpts document the original port references that were systematically replaced during consolidation. The port change from 18789 to 8090 aligns with the standardized OpenClaw gateway configuration across the codebase.
