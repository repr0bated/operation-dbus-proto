#!/bin/sh
# Safely compile and activate an s6-rc source tree.
#
# Never repoints /etc/s6/rc/compiled before s6-rc-update succeeds. Doing so
# leaves /run/s6-rc/state from the old database paired with a new compiled DB,
# which makes future s6-rc commands fail with "unable to read valid state".
set -eu

S6_SV=${S6_SV:-/etc/s6/sv}
S6_ADMIN=${S6_ADMIN:-/etc/s6/adminsv}
S6_COMPILED_LINK=${S6_COMPILED_LINK:-/etc/s6/rc/compiled}
S6_DB_ROOT=${S6_DB_ROOT:-/etc/s6/rc}
LABEL=${1:-local}

if ! command -v s6-rc-compile >/dev/null 2>&1; then
    echo "s6-rc-compile not found" >&2
    exit 127
fi

if ! command -v s6-rc-update >/dev/null 2>&1; then
    echo "s6-rc-update not found" >&2
    exit 127
fi

if [ ! -d "$S6_SV" ]; then
    echo "service source directory not found: $S6_SV" >&2
    exit 1
fi

if [ ! -d "$S6_ADMIN" ]; then
    echo "admin source directory not found: $S6_ADMIN" >&2
    exit 1
fi

old_db="$(readlink -f "$S6_COMPILED_LINK" 2>/dev/null || true)"
new_db="${S6_DB_ROOT}/compiled-${LABEL}-$(date +%s%N)"

echo "Compiling s6-rc database: $new_db"
s6-rc-compile "$new_db" "$S6_SV" "$S6_ADMIN"
s6-rc-db -c "$new_db" check

echo "Updating live s6-rc state to: $new_db"
if ! s6-rc-update "$new_db"; then
    echo "s6-rc-update failed; leaving $S6_COMPILED_LINK unchanged" >&2
    [ -n "$old_db" ] && echo "current compiled DB remains: $old_db" >&2
    exit 1
fi

ln -sfnT "$new_db" "$S6_COMPILED_LINK"
echo "Activated s6-rc database: $new_db"

