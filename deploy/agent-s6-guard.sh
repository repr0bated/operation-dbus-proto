#!/bin/sh
# Keep agent sessions on the service6 management surface.  Boot and service6
# execute these programs as root and are therefore unaffected.
set -eu

for command in \
    /usr/bin/s6 \
    /usr/bin/s6-rc /usr/bin/s6-rc-bundle /usr/bin/s6-rc-compile \
    /usr/bin/s6-rc-db /usr/bin/s6-rc-dryrun /usr/bin/s6-rc-format-upgrade \
    /usr/bin/s6-rc-init /usr/bin/s6-rc-repo-init /usr/bin/s6-rc-repo-list \
    /usr/bin/s6-rc-repo-sync /usr/bin/s6-rc-set-change \
    /usr/bin/s6-rc-set-commit /usr/bin/s6-rc-set-copy \
    /usr/bin/s6-rc-set-delete /usr/bin/s6-rc-set-fix \
    /usr/bin/s6-rc-set-install /usr/bin/s6-rc-set-new \
    /usr/bin/s6-rc-set-status /usr/bin/s6-rc-update \
    /usr/bin/s6-db-reload /usr/local/bin/s6d \
    /usr/bin/s6-instance-control /usr/bin/s6-instance-create \
    /usr/bin/s6-instance-delete /usr/bin/s6-instance-maker \
    /usr/bin/s6-svc /usr/bin/s6-svscanctl
do
    if [ -e "$command" ]; then
        chown root:root "$command"
        chmod 0750 "$command"
    fi
done

# Remove the executable bypass left by the emergency restoration copies. These
# are retained for console recovery but are root-only.
for command in /usr/bin/s6.real /usr/bin/s6-*.real
do
    if [ -e "$command" ]; then
        chown root:root "$command"
        chmod 0700 "$command"
    fi
done
