# Project Memories

## Architecture Decisions
- **Unified namespace**: All D-Bus interfaces live under `org.opdbus.v1`.
- **Init System**: Standardized on s6 (no systemd/dinit services on Artix host).
- **1:1 Mirror**: State is mirrored 1:1 using zero-copy shared memory (`/dev/shm`) and SQLite where necessary.
- **Internal Comm**: Use gRPC for internal service communication and D-Bus for system-level bridging.

## Known Issues
- OpenClaw trusted proxies require injecting `X-Ghostbridge-Footprint` and `X-Ghostbridge-Trace-ID`.
- Database locks can occur if `/run/op-dbus/` is unmapped or has incorrect permissions.
