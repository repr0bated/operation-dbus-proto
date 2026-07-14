#!/bin/sh
# Make Caddy's Netmaker API socket a host-visible Incus disk mount.
set -eu

CONTAINER=${NETMAKER_CONTAINER:-netmaker}
HOST_DIR=${NETMAKER_RUNTIME_DIR:-/var/lib/netmaker-runtime}
CONTAINER_DIR=${NETMAKER_CONTAINER_RUNTIME_DIR:-/var/lib/netmaker-runtime}
DEVICE=${NETMAKER_RUNTIME_DEVICE:-netmaker-runtime}

[ "$(id -u)" -eq 0 ] || {
    echo "run as root" >&2
    exit 1
}

install -d -m 0755 "$HOST_DIR"

current=$(incus config device get "$CONTAINER" "$DEVICE" source 2>/dev/null || true)
target=$(incus config device get "$CONTAINER" "$DEVICE" path 2>/dev/null || true)
if [ "$current" != "$HOST_DIR" ] || [ "$target" != "$CONTAINER_DIR" ]; then
    incus stop "$CONTAINER" --force
    incus config device remove "$CONTAINER" "$DEVICE" >/dev/null 2>&1 || true
    incus config device add "$CONTAINER" "$DEVICE" disk \
        source="$HOST_DIR" path="$CONTAINER_DIR" shift=true
    incus start "$CONTAINER"
fi

for _ in $(seq 1 60); do
    incus exec "$CONTAINER" -- true >/dev/null 2>&1 && break
    sleep 1
done

incus exec "$CONTAINER" -- sh -eu -c '
    old=/run/netmaker/netmaker.sock
    new=/var/lib/netmaker-runtime/netmaker.sock
    if grep -q "$old" /etc/caddy/Caddyfile; then
        sed -i "s|$old|$new|g" /etc/caddy/Caddyfile
    fi
    if grep -q "^NETMAKER_SOCKET=" /etc/netmaker/netmaker.env; then
        sed -i "s|^NETMAKER_SOCKET=.*|NETMAKER_SOCKET=$new|" /etc/netmaker/netmaker.env
    else
        printf "NETMAKER_SOCKET=%s\n" "$new" >> /etc/netmaker/netmaker.env
    fi
    systemctl restart caddy.service
'

for _ in $(seq 1 60); do
    [ -S "$HOST_DIR/netmaker.sock" ] && exit 0
    sleep 1
done

echo "Netmaker API socket did not appear at $HOST_DIR/netmaker.sock" >&2
exit 1
