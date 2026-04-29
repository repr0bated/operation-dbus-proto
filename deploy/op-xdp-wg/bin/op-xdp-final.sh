#!/usr/bin/env bash
set -euo pipefail

CT="wg-xray"
HOST_MAC="fa:16:3e:f1:71:d2"

until incus info $CT >/dev/null 2>&1; do sleep 2; done
incus stop $CT --force 2>/dev/null || true
incus config set $CT volatile.eth0.hwaddr=$HOST_MAC
incus start $CT
