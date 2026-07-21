#!/bin/sh
# ⚖️ 🟢 🛷 Build gravitl/netclient with s6d (D-Bus) lifecycle support.
#
# Requirements (AGENTS.md):
# - Lifecycle ops route ONLY through s6d -> org.opdbus.v1.S6.Systemctl (D-Bus).
#   Never s6-rc/s6-svc/systemctl directly. The existing s6d already maps Artix s6.
# - The live s6 service is `netclient` (servicedir in /run/s6-rc). netclient only
#   enables/starts/stops/restarts it via s6d; override with NETCLIENT_S6_SERVICE.
#
# Design:
# - Clones netclient at a pinned tag, appends an S6 InitType, routes the daemon
#   init handlers to s6d, then compiles with the local Go toolchain.
# - Patches are applied with anchored perl substitutions and verified, so an
#   upstream layout change fails loudly instead of silently producing a stock
#   binary.
#
# Usage:
#   sh build-netclient-s6.sh            # clone + patch + compile
#   sudo sh build-netclient-s6.sh --install   # install built binary + wire s6d

set -eu

NETCLIENT_VERSION="${NETCLIENT_VERSION:-v1.5.1}"
WORKDIR="${WORKDIR:-/tmp/netclient-s6-build}"
NETCLIENT_S6_SERVICE_NAME="${NETCLIENT_S6_SERVICE_NAME:-netclient}"
S6_INITTYPE_VALUE="${S6_INITTYPE_VALUE:-6}"

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
src_s6="${script_dir}/s6_linux.go"

log() { printf '%s\n' "[netclient-s6] $*"; }
die() { printf '%s\n' "[netclient-s6] ERROR: $*" >&2; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || die "$1 is required"; }

do_install="0"
[ "${1:-}" = "--install" ] && do_install="1"

install_phase() {
  [ "$(id -u)" -eq 0 ] || die "run with sudo for --install"
  need s6d
  bin="${WORKDIR}/netclient"
  [ -x "$bin" ] || die "no built binary at ${bin}; run the build first"

  log "stopping ${NETCLIENT_S6_SERVICE_NAME} via s6d"
  s6d stop "$NETCLIENT_S6_SERVICE_NAME" 2>/dev/null || true

  cfgjson=/etc/netclient/netclient.json
  if [ -f "$cfgjson" ]; then
    cp -a "$cfgjson" "${cfgjson}.pre-s6.bak"
    perl -pi -e 's/"inittype":\s*\d+/"inittype": '"$S6_INITTYPE_VALUE"'/' "$cfgjson"
    log "set inittype=${S6_INITTYPE_VALUE} (s6) in ${cfgjson}"
  fi

  install -m 0755 "$bin" /sbin/netclient
  install -m 0755 "$bin" /usr/local/bin/netclient
  log "installed netclient -> /sbin/netclient and /usr/local/bin/netclient"

  s6d daemon-reload || true
  s6d enable "$NETCLIENT_S6_SERVICE_NAME" || true
  s6d start "$NETCLIENT_S6_SERVICE_NAME" || true
  log "status: ${NETCLIENT_S6_SERVICE_NAME}"
  s6d status "$NETCLIENT_S6_SERVICE_NAME" || true
  return 0
}

if [ "$do_install" = "1" ]; then
  install_phase
  exit 0
fi

need git
need go
need perl
[ -f "$src_s6" ] || die "missing ${src_s6}"

log "cloning netclient ${NETCLIENT_VERSION}"
rm -rf "$WORKDIR"
git clone --depth 1 --branch "$NETCLIENT_VERSION" https://github.com/gravitl/netclient "$WORKDIR"

cfg="${WORKDIR}/config/config.go"
cmn="${WORKDIR}/daemon/common_linux.go"
[ -f "$cfg" ] || die "config/config.go not found (upstream layout changed?)"
[ -f "$cmn" ] || die "daemon/common_linux.go not found (upstream layout changed?)"

log "patching config InitType enum + String()"
perl -0pi -e 's/(\tInitd\n)\)/${1}\tS6\n)/' "$cfg"
perl -pi  -e 's/("openrc", "initd")\}\[i\]/${1}, "s6"}[i]/' "$cfg"
grep -qE '^[[:space:]]+S6$' "$cfg" || die "enum patch failed"
grep -qF '"s6"}[i]' "$cfg"         || die "String() patch failed"

log "patching daemon dispatch + s6 detection"
perl -0pi -e 's/(\tcase config\.Initd:\n\t\treturn setupInitd\(\)\n)/${1}\tcase config.S6:\n\t\treturn setupS6()\n/' "$cmn"
perl -0pi -e 's/(\tcase config\.Initd:\n\t\treturn startInitd\(\)\n)/${1}\tcase config.S6:\n\t\treturn startS6()\n/' "$cmn"
perl -0pi -e 's/(\tcase config\.Initd:\n\t\treturn stopInitd\(\)\n)/${1}\tcase config.S6:\n\t\treturn stopS6()\n/' "$cmn"
perl -0pi -e 's/(\tcase config\.Initd:\n\t\treturn restartInitd\(\)\n)/${1}\tcase config.S6:\n\t\treturn restartS6()\n/' "$cmn"
perl -0pi -e 's/(\t\tcase config\.Initd:\n\t\t\terr = removeInitd\(\)\n)/${1}\t\tcase config.S6:\n\t\t\terr = removeS6()\n/' "$cmn"
perl -pi  -e 's/(host\.InitType == config\.Systemd \|\| host\.InitType == config\.Initd) \{/${1} || host.InitType == config.S6 {/' "$cmn"
perl -0pi -e 's/(\tslog\.Debug\("checking \/sbin\/init", "output ", out\)\n)/${1}\tif strings.Contains(out, "s6") {\n\t\t\/\/ artix\/chimera: s6 init, lifecycle driven through s6d (D-Bus)\n\t\treturn config.S6\n\t}\n/' "$cmn"

for tok in 'return setupS6()' 'return startS6()' 'return stopS6()' 'return restartS6()' 'err = removeS6()' 'return config.S6'; do
  grep -qF "$tok" "$cmn" || die "dispatch patch failed: ${tok}"
done

log "installing daemon/s6_linux.go"
cp "$src_s6" "${WORKDIR}/daemon/s6_linux.go"

cd "$WORKDIR"
log "compiling (go build)"
if ! CGO_ENABLED="${CGO_ENABLED:-0}" go build -o netclient . ; then
  log "CGO_ENABLED=0 build failed; retrying with CGO_ENABLED=1"
  CGO_ENABLED=1 go build -o netclient .
fi
log "built: ${WORKDIR}/netclient"
log "next: sudo sh $0 --install"
