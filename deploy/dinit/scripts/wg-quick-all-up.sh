#!/bin/sh
# Bring up all wg-quick-managed WireGuard interfaces.
# wgcf  — Cloudflare WARP egress tunnel (attaches to OVS as external port)
# wg0   — host VPN server for tablet/laptop/services-container peers
set -eu

wg-quick up wgcf || true
wg-quick up wg0  || true
