# Artix s6 Bootdb Recovery

## Bootdb Recovery Procedure

### Symptom
System becomes unbootable when `/etc/s6/rc/compiled` (bootdb) is left pointing to a deleted database after an interrupted recompile/update operation.

### Recovery Steps

1. Boot from live rescue media
2. Mount the root filesystem (e.g., `/dev/sda3`) to `/mnt`
3. Chroot into the mounted system:
   ```bash
   mount /dev/sda3 /mnt
   mount --bind /dev /mnt/dev
   mount --bind /proc /mnt/proc
   mount --bind /sys /mnt/sys
   chroot /mnt
   ```

4. Identify the last known-stable compiled database:
   ```bash
   ls -la /etc/s6/rc/compiled.*
   ```
   Look for timestamped databases with the `.current:@<timestamp>:<identifier>` pattern.

5. Verify the candidate database:
   ```bash
   s6-rc-db check /etc/s6/rc/compiled.<candidate>
   ```
   Must exit with status 0 and contain both `default` and `boot` bundles.

6. Repoint the bootdb symlink:
   ```bash
   rm /etc/s6/rc/compiled
   ln -s /etc/s6/rc/compiled.<stable-candidate> /etc/s6/rc/compiled
   ```

7. Verify and exit:
   ```bash
   s6-rc-db check /etc/s6/rc/compiled  # should exit 0
   exit
   umount /mnt/dev /mnt/proc /mnt/sys /mnt
   reboot
   ```

### Example Recovery (2026-07-02)
Recovered from interrupted daemon reload by repointing bootdb to `.current:@400000006a369bdb346d3c96:YESXfi` (dated 2026-06-20), validated to have both bundles and to be the specific post-fix database from the documented op-web-srv notification-fd wedge fix.

## s6-apply Safety Notes

**Always use `/usr/local/bin/s6-apply` for service database updates.** This wrapper provides:
- Atomic sync → commit → live-install → bootdb-sync sequence
- Automatic rollback on failure
- Safe handling of the s6-rc global lock

**Never manually run** `s6 set commit` or `s6 live install` in isolation. An interrupted sequence can orphan the bootdb pointer.

### If s6-apply Gets Stuck

**DO NOT kill the process tree carelessly.** If killed mid-transition, rollback logic won't trigger and bootdb may be left in an inconsistent state. The rollback mechanism only activates on a clean non-zero exit, not on signals.

If s6-apply hangs during the "s6 live install" step, investigate the blocking service (see notification-fd wedge below) and resolve that root cause before proceeding.

## op-web-srv Notification-fd Wedge

### Symptom
`s6-apply` (or any `s6 live install` operation) hangs indefinitely during service transitions. Process tree shows:
```
s6-apply
 └ s6-rc-set-install
    └ s6-rc -u -- change [services including rovs plugins, opdbus, op-web-srv, etc.]
       └ s6-svlisten -U
          └ s6-ftrigrd [blocked reading notification fifo]
```

### Root Cause
Services with `notification-fd=3` configured but whose binaries never write the readiness byte will block `s6-svlisten -U` indefinitely, holding the global s6-rc lock.

Known affected services:
- `op-web-srv`
- `op-assistant-grpc-srv`

### Permanent Fix
1. Remove the `notification-fd` file from service directories:
   ```bash
   rm /etc/s6/sv/op-web-srv/notification-fd
   rm /etc/s6/sv/op-assistant-grpc-srv/notification-fd
   ```

2. Update the git source to prevent reintroduction:
   ```bash
   rm deploy/s6/op-web-srv/notification-fd
   rm deploy/s6/op-assistant-grpc-srv/notification-fd
   ```

3. Commit via `s6-apply` to rebuild the database without the broken notification-fd declarations.

### Emergency Unstick (Non-destructive)
If the wedge occurs during a live transition and you need to unstick it without killing the process tree:

1. Identify the blocked service's PID (e.g., `op-web-srv` at PID 20608)
2. Find its notification fd: `ls -l /proc/20608/fd/3`
3. Write the readiness byte directly:
   ```bash
   echo -n '\0' > /proc/20608/fd/3
   ```
This allows the current transition to complete; still apply the permanent fix afterward.

## gemma Failure-Masking Bug

### Symptom
The `gemma` oneshot service always reports success even when its actual work fails. Downstream services (like xray config generation) that depend on gemma output may crash-loop or fail to start.

### Root Cause
The execline `up` script at `/etc/s6/sv/gemma/up` unconditionally exits 0:
```
foreground { sh /etc/s6/sv/gemma/shell_up }
exit 0
```
Even though `shell_up` has `set -eu` and would correctly propagate failures internally, s6 never sees the real exit code.

### Impact
- xray crash-loops post-reboot if gemma-generated config (`/dev/shm/xray-ghostbridge.json`) is missing
- The failure is silent; logs show gemma as "active" even when the underlying binary failed
- Operators may not realize the generation chain (`op-gemma` → `op-identity-shuttle`) never completed

### Fix Required (Not Yet Applied)
Remove the unconditional `exit 0` from `/etc/s6/sv/gemma/up` and allow the script's own exit code from `shell_up` to propagate. Alternatively, capture and explicitly forward the exit code:
```
backtick -n EXITCODE { sh /etc/s6/sv/gemma/shell_up }
exit ${EXITCODE}
```

---

<!-- Extracted from /mnt/opt-inspect/home/git/operation-dbus-proto/docs/s6-boot-recovery-gemma-ollama-handoff.md on 2026-07-20 -->
