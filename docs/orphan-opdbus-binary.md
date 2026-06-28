# Orphan Binary: `/usr/local/bin/opdbus`

## Status

This document identifies the orphan binary `/usr/local/bin/opdbus` as a required cleanup task.

## Description

The binary `/usr/local/bin/opdbus` exists on the system but is not associated with any active crate or s6 service in the current codebase. It was likely a legacy entry point from an earlier architecture iteration.

## Identification Evidence

- No source file in `crates/` produces a binary named `opdbus`
- No s6 service definition references this binary
- The bridge functionality is now provided by `op-grpc-bridge`
- The plugin projection functionality is now provided by `op-projection`

## Required Action

**Option A: Remove Orphan Binary** (Recommended)

Delete the binary during the next deployment cycle:
```bash
sudo rm -f /usr/local/bin/opdbus
```

**Option B: Replace with Bridge Binary**

If backward compatibility is required, create a symlink to the bridge binary:
```bash
sudo ln -sf /usr/local/bin/op-grpc-bridge /usr/local/bin/opdbus
```

## Timeline

This cleanup should be performed after the WS5 trim-registrars refactor is complete and all services have been transitioned to the new architecture where:
- `op-grpc-bridge` owns `org.opdbus.v1`
- `op-openvswitch-daemon` owns `org.opdbus.v1.plugins.ovsdb`
- No other processes claim the canonical bus name

## Related Changes

- See WS5 trim-registrars milestone for the full refactor
- See `op-dbus-mirror` changes (now uses `org.opdbus.v1.mirror`)
- See `op-openvswitch-daemon` changes (now uses `org.opdbus.v1.plugins.ovsdb`)
