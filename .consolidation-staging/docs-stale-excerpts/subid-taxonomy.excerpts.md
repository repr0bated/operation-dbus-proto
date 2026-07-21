# Stale Excerpts from subid-taxonomy.md

## Dropped: Incorrect Port Reference

**Location:** Routing tags table, `grpc-bridge` row

**Source excerpt:**
```markdown
| `grpc-bridge` | zeroclaw gateway on 127.0.0.1:18789 | `exp.service.zeroclaw-serve@v1` | gRPC |
```

**Reason for dropping:** Port 18789 is incorrect; the correct port is 8090 (already applied in destination).

**Corrected version (already in destination):**
```markdown
| `grpc-bridge` | zeroclaw gateway on 127.0.0.1:8090 | `exp.service.zeroclaw-serve@v1` | gRPC |
```

---

**Note:** All other content from the source file is identical to the destination and required no changes.

<!-- Extracted from /mnt/opt-inspect/home/git/operation-dbus-proto/docs/subid-taxonomy.md on 2026-07-20 -->
