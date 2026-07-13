#!/bin/sh
# Install the socket-owning Rust network manager into an existing Incus
# identity container.  This project is not installable by crate name alone;
# the pinned Cargo Git source is part of the provisioning contract.
set -eu

CONTAINER="${1:?Incus container name required}"
REPO_ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
GIT_URL=https://github.com/sparesparrow/rust-network-mgr
GIT_REV=117087cb1bf99cb55ba3e8e40b9e27752cd08f46
TARGET=x86_64-unknown-linux-musl
CACHE_ROOT=${RUST_NETWORK_MGR_INSTALL_ROOT:-/var/cache/operation-dbus/rust-network-mgr-$GIT_REV}

# Identity containers are deliberately NIC-less, so dependency resolution must
# happen on the host.  The resulting artifact still comes from the required
# `cargo install --git` path and is cached by immutable Git revision.
if [ ! -x "$CACHE_ROOT/bin/rust-network-mgr" ]; then
    install -d -m 0755 "$CACHE_ROOT"
    cargo install --git "$GIT_URL" --rev "$GIT_REV" \
        --root "$CACHE_ROOT" --locked --target "$TARGET" rust-network-mgr
fi

incus exec "$CONTAINER" -- install -d -m 0755 /etc/rust-network-mgr
incus file push "$CACHE_ROOT/bin/rust-network-mgr" \
    "$CONTAINER/usr/local/bin/rust-network-mgr" --uid 0 --gid 0 --mode 0755
incus file push "$REPO_ROOT/deploy/rust-network-mgr/config-loopback.yaml" \
    "$CONTAINER/etc/rust-network-mgr/config.yaml" --uid 0 --gid 0 --mode 0644
incus file push "$REPO_ROOT/deploy/rust-network-mgr/rust-network-mgr.service" \
    "$CONTAINER/etc/systemd/system/rust-network-mgr.service" --uid 0 --gid 0 --mode 0644

incus exec "$CONTAINER" -- systemctl daemon-reload
incus exec "$CONTAINER" -- systemctl enable --now rust-network-mgr.service

incus exec "$CONTAINER" -- sh -lc '
    set -eu
    for _ in $(seq 1 50); do
        [ -S /run/rust-network-manager/rust-network-manager.sock ] && exit 0
        sleep 0.2
    done
    systemctl status rust-network-mgr.service --no-pager -l >&2 || true
    exit 1
'

echo "$CONTAINER: rust-network-mgr socket is ready"
