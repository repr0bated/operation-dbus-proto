#!/usr/bin/env bash
# deploy-network.sh - socket/OpenFlow network installer for operation-dbus-proto
#
# Current model:
#   - wg0 is the host WireGuard identity/session server. The script verifies
#     it by default and only rewrites it when --update-wgconf is provided.
#   - wgcf is the bridge-facing WireGuard/WARP tunnel, managed by wg-quick.
#     Its config is only generated/refreshed when --update-wgcf is provided.
#   - ovsbr0 and internal privacy ports are installed through netplan plus
#     native OVSDB/OpenFlow attach scripts.
#   - Socket-mode Incus containers have no veth, no eth0, and no IP address.
#   - Host nginx reaches system services through /run/services0/*.sock.
#   - NextDNS split DNS is managed through the NextDNS API, not dnsmasq.

set -euo pipefail

SCRIPT_DIR="$(CDPATH='' cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

LOG_FILE="${LOG_FILE:-/var/log/deploy-network.log}"
OP_DBUS_ENV_FILE="${OP_DBUS_ENV_FILE:-/etc/op-dbus/environment}"

SERVICES_CONTAINER="${OP_DBUS_SERVICES_CONTAINER:-services}"
SOCKET_DIR="${SERVICES0_SOCKET_DIR:-/run/services0}"
SOCKET_GROUP="${SERVICES0_SOCKET_GROUP:-_nginx}"
SOCKET_GROUP_ID="${SERVICES0_SOCKET_GID:-980}"

WG_SERVER_INTERFACE="${WG_SERVER_INTERFACE:-${WG_INTERFACE:-wg0}}"
WG_SERVER_ADDRESS="${WG_SERVER_ADDRESS:-10.0.0.1/24}"
WG_SERVER_LISTEN_PORT="${WG_SERVER_LISTEN_PORT:-51820}"
WG_SERVER_PRIVATE_KEY_FILE="${WG_SERVER_PRIVATE_KEY_FILE:-/etc/wireguard/${WG_SERVER_INTERFACE}.netplan.key}"
WG_SERVER_NETPLAN_FILE="${WG_SERVER_NETPLAN_FILE:-/etc/netplan/02-${WG_SERVER_INTERFACE}.yaml}"
WG_SERVER_PEERS="${WG_SERVER_PEERS:-}"

WGCF_INTERFACE="${OP_DBUS_WGCF_INTERFACE:-wgcf}"
WGCF_CONF="/etc/wireguard/${WGCF_INTERFACE}.conf"
WGCF_ACCOUNT_FILE="${WGCF_ACCOUNT_FILE:-/etc/wireguard/wgcf-account.toml}"
OVS_BRIDGE="${OP_DBUS_OVS_BRIDGE:-ovsbr0}"
GRPC_BRIDGE_IFACE="${OP_DBUS_GRPC_BRIDGE_IFACE:-grpc-bridge}"
MGMT_IFACE="${OP_DBUS_MGMT_IFACE:-ovsbr0-mgmt}"
GRPC_BRIDGE_CIDR="10.200.0.2/24"
MGMT_CIDR="10.200.0.1/24"

DINIT_D="${DINIT_D:-/etc/dinit.d}"
NEXTDNS_PROFILE="${NEXTDNS_PROFILE:-689ec7}"
NEXTDNS_REWRITE_NAME="${NEXTDNS_REWRITE_NAME:-dashboard.3tched.com}"
NEXTDNS_REWRITE_CONTENT="${NEXTDNS_REWRITE_CONTENT:-10.0.0.1}"

APPLY_NEXTDNS_REWRITE=true
INSTALL_DASHBOARD_NGINX=false
INSTALL_SERVICES_GATEWAY_SOCKET=true
UPDATE_WGCONF=false
UPDATE_WGCF=false
VERIFY_ONLY=false

usage() {
  cat <<EOF
Usage: sudo $(basename "$0") [OPTIONS]

Options:
  --verify-only              Run verification only, do not install/start services.
  --no-nextdns-rewrite       Do not apply the NextDNS dashboard rewrite.
  --install-dashboard-nginx  Render deploy/nginx/dashboard-3tched-socket.conf.template.
                             Requires OPENCLAW_GATEWAY_TOKEN in the environment.
  --no-services-gateway-socket
                             Do not install/restart the services gateway socket bridge.
  --update-wgconf            Rewrite the netplan wg0 tunnel from WG_SERVER_* values.
  --update-wgcf              Refresh/generate WGCF_CONF from WGCF_ACCOUNT_FILE.
  --help                     Show this help message.

Environment:
  WG_SERVER_INTERFACE         Defaults to ${WG_SERVER_INTERFACE}.
  WG_SERVER_ADDRESS           Defaults to ${WG_SERVER_ADDRESS}.
  WG_SERVER_LISTEN_PORT       Defaults to ${WG_SERVER_LISTEN_PORT}.
  WG_SERVER_PRIVATE_KEY       Optional; written to WG_SERVER_PRIVATE_KEY_FILE.
  WG_SERVER_PRIVATE_KEY_FILE  Defaults to ${WG_SERVER_PRIVATE_KEY_FILE}.
  WG_SERVER_PEERS             Semicolon-separated public-key|allowed-ips records.
  WGCF_ACCOUNT_FILE           Defaults to ${WGCF_ACCOUNT_FILE}.
  WGCF_CONF                   Derived as /etc/wireguard/<WGCF_INTERFACE>.conf.
  WGCF_LICENSE_KEY            Optional license key for --update-wgcf.
  WGCF_DEVICE_NAME            Optional device name for --update-wgcf.
  NEXTDNS_API_KEY            Required to upsert the NextDNS rewrite.
  NEXTDNS_PROFILE            Defaults to ${NEXTDNS_PROFILE}.
  SERVICES_GATEWAY_TOKEN     Required only with --install-dashboard-nginx.
EOF
}

for arg in "$@"; do
  case "$arg" in
    --verify-only)
      VERIFY_ONLY=true
      ;;
    --no-nextdns-rewrite)
      APPLY_NEXTDNS_REWRITE=false
      ;;
    --install-dashboard-nginx)
      INSTALL_DASHBOARD_NGINX=true
      ;;
    --no-services-gateway-socket)
      INSTALL_SERVICES_GATEWAY_SOCKET=false
      ;;
    --no-openclaw-socket)
      INSTALL_SERVICES_GATEWAY_SOCKET=false
      ;;
    --update-wgconf)
      UPDATE_WGCONF=true
      ;;
    --update-wgcf)
      UPDATE_WGCF=true
      ;;
    --help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $arg" >&2
      usage >&2
      exit 1
      ;;
  esac
done

log() {
  local ts msg
  ts="$(date '+%Y-%m-%d %H:%M:%S')"
  msg="[${ts}] $*"
  echo "$msg"
  echo "$msg" >> "$LOG_FILE" 2>/dev/null || true
}

die() {
  log "ERROR: $*"
  exit 1
}

run() {
  log "+ $*"
  "$@"
}

yaml_quote() {
  local value="${1//\'/\'\'}"
  printf "'%s'" "$value"
}

need_root() {
  [[ "$(id -u)" -eq 0 ]] || die "Run as root: sudo $0 $*"
}

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || die "Missing required command: $1"
}

start_service() {
  local service="$1"

  if command -v gdbus >/dev/null 2>&1; then
    local output
    if output="$(gdbus call \
      --system \
      --dest org.chimera.dinit \
      --object-path /org/chimera/dinit \
      --method org.chimera.dinit.Manager.StartService \
      "$service" false 2>&1)"; then
      return 0
    fi
    grep -q 'org.chimera.dinit.Error.ServiceAlready' <<<"$output" && return 0
  fi

  if command -v dinitctl >/dev/null 2>&1; then
    dinitctl start "$service" >/dev/null 2>&1 && return 0
  fi

  return 1
}

restart_service() {
  local service="$1"
  if command -v dinitctl >/dev/null 2>&1; then
    dinitctl restart "$service" >/dev/null 2>&1 && return 0
  fi
  start_service "$service"
}

enable_boot() {
  local service="$1"
  install -d "$DINIT_D/boot.d"
  ln -sfn "../$service" "$DINIT_D/boot.d/$service"
}

ensure_depends_on() {
  local file="$1"
  local dependency="$2"

  [[ -f "$file" ]] || return 0
  grep -qx "depends-on = ${dependency}" "$file" && return 0
  printf 'depends-on = %s\n' "$dependency" >> "$file"
  log "Added depends-on = ${dependency} to ${file}"
}

wait_for_iface() {
  local iface="$1"
  local timeout="${2:-30}"
  local waited=0

  while ! ip link show "$iface" >/dev/null 2>&1; do
    sleep 1
    waited=$((waited + 1))
    [[ "$waited" -ge "$timeout" ]] && die "Interface ${iface} did not appear after ${timeout}s"
  done
  log "Interface ${iface} exists"
}

stat_mode_owner_group() {
  local path="$1"
  if stat -c '%a %U:%G' "$path" >/dev/null 2>&1; then
    stat -c '%a %U:%G' "$path"
  else
    stat -f '%Lp %Su:%Sg' "$path"
  fi
}

container_status() {
  incus info "$1" 2>/dev/null | awk -F': ' '$1 == "Status" { print toupper($2); exit }' || true
}

wait_for_container() {
  local name="$1"
  local timeout="${2:-60}"
  local waited=0

  while [[ "$(container_status "$name")" != "RUNNING" ]]; do
    sleep 2
    waited=$((waited + 2))
    [[ "$waited" -ge "$timeout" ]] && die "Container ${name} did not reach RUNNING after ${timeout}s"
  done
}

source_environment() {
  if [[ -f "$OP_DBUS_ENV_FILE" ]]; then
    set -a
    # shellcheck disable=SC1090
    . "$OP_DBUS_ENV_FILE"
    set +a
    log "Loaded ${OP_DBUS_ENV_FILE}"
  fi
}

load_configuration() {
  SERVICES_CONTAINER="${OP_DBUS_SERVICES_CONTAINER:-${SERVICES_CONTAINER:-services}}"
  SOCKET_DIR="${SERVICES0_SOCKET_DIR:-${SOCKET_DIR:-/run/services0}}"
  SOCKET_GROUP="${SERVICES0_SOCKET_GROUP:-${SOCKET_GROUP:-_nginx}}"
  SOCKET_GROUP_ID="${SERVICES0_SOCKET_GID:-${SOCKET_GROUP_ID:-980}}"

  WG_SERVER_INTERFACE="${WG_SERVER_INTERFACE:-${WG_INTERFACE:-wg0}}"
  WG_SERVER_ADDRESS="${WG_SERVER_ADDRESS:-10.0.0.1/24}"
  WG_SERVER_LISTEN_PORT="${WG_SERVER_LISTEN_PORT:-51820}"
  WG_SERVER_PRIVATE_KEY_FILE="${WG_SERVER_PRIVATE_KEY_FILE:-/etc/wireguard/${WG_SERVER_INTERFACE}.netplan.key}"
  WG_SERVER_NETPLAN_FILE="${WG_SERVER_NETPLAN_FILE:-/etc/netplan/02-${WG_SERVER_INTERFACE}.yaml}"
  WG_SERVER_PEERS="${WG_SERVER_PEERS:-}"

  WGCF_INTERFACE="${OP_DBUS_WGCF_INTERFACE:-${WGCF_INTERFACE:-wgcf}}"
  WGCF_CONF="/etc/wireguard/${WGCF_INTERFACE}.conf"
  WGCF_ACCOUNT_FILE="${WGCF_ACCOUNT_FILE:-/etc/wireguard/wgcf-account.toml}"
  OVS_BRIDGE="${OP_DBUS_OVS_BRIDGE:-${OVS_BRIDGE:-ovsbr0}}"
  GRPC_BRIDGE_IFACE="${OP_DBUS_GRPC_BRIDGE_IFACE:-${GRPC_BRIDGE_IFACE:-grpc-bridge}}"
  MGMT_IFACE="${OP_DBUS_MGMT_IFACE:-${MGMT_IFACE:-ovsbr0-mgmt}}"
  GRPC_BRIDGE_CIDR="10.200.0.2/24"
  MGMT_CIDR="10.200.0.1/24"

  NEXTDNS_PROFILE="${NEXTDNS_PROFILE:-689ec7}"
  NEXTDNS_REWRITE_NAME="${NEXTDNS_REWRITE_NAME:-dashboard.3tched.com}"
  NEXTDNS_REWRITE_CONTENT="${NEXTDNS_REWRITE_CONTENT:-10.0.0.1}"
}

preflight() {
  need_root "$@"
  source_environment
  load_configuration

  need_cmd incus
  need_cmd ip
  need_cmd install
  need_cmd netplan
  need_cmd wg-quick
  need_cmd curl
  need_cmd python3
  need_cmd ovsdb-client
  [[ "$INSTALL_DASHBOARD_NGINX" != "true" ]] || need_cmd nginx

  if [[ ! -f "$WGCF_CONF" ]]; then
    [[ "$UPDATE_WGCF" == "true" ]] \
      || die "${WGCF_CONF} is required for WARP transport. Use --update-wgcf for one-off deployment generation."
  fi

  if [[ "$UPDATE_WGCF" == "true" ]]; then
    need_cmd wgcf
    [[ -f "$WGCF_ACCOUNT_FILE" ]] \
      || die "${WGCF_ACCOUNT_FILE} is required with --update-wgcf"
  fi

  if [[ "$UPDATE_WGCONF" == "true" ]]; then
    [[ -n "${WG_SERVER_PRIVATE_KEY:-}" || -f "$WG_SERVER_PRIVATE_KEY_FILE" ]] \
      || die "Set WG_SERVER_PRIVATE_KEY or create ${WG_SERVER_PRIVATE_KEY_FILE} before --update-wgconf"
    [[ -n "$WG_SERVER_PEERS" ]] \
      || die "WG_SERVER_PEERS is required with --update-wgconf"
  fi
}

write_wg_server_netplan() {
  [[ "$UPDATE_WGCONF" == "true" ]] || return 0
  log "--- Write ${WG_SERVER_INTERFACE} netplan tunnel ---"

  install -d -m 0700 "$(dirname "$WG_SERVER_PRIVATE_KEY_FILE")"
  install -d -m 0755 "$(dirname "$WG_SERVER_NETPLAN_FILE")"

  if [[ -n "${WG_SERVER_PRIVATE_KEY:-}" ]]; then
    umask 077
    printf '%s\n' "$WG_SERVER_PRIVATE_KEY" > "$WG_SERVER_PRIVATE_KEY_FILE"
    unset WG_SERVER_PRIVATE_KEY
  fi
  chmod 0600 "$WG_SERVER_PRIVATE_KEY_FILE"

  local tmp peer_spec public_key allowed_ips endpoint keepalive psk_file allowed_ip
  local -a peer_specs allowed_ip_list
  tmp="$(mktemp)"

  {
    printf '# Managed by deploy-network.sh. Do not edit manually.\n'
    printf 'network:\n'
    printf '  version: 2\n'
    printf '  renderer: networkd\n'
    printf '  tunnels:\n'
    printf '    %s:\n' "$WG_SERVER_INTERFACE"
    printf '      mode: wireguard\n'
    printf '      addresses:\n'
    printf '        - %s\n' "$(yaml_quote "$WG_SERVER_ADDRESS")"
    printf '      port: %s\n' "$WG_SERVER_LISTEN_PORT"
    printf '      key: %s\n' "$(yaml_quote "$WG_SERVER_PRIVATE_KEY_FILE")"
    printf '      peers:\n'

    IFS=';' read -r -a peer_specs <<< "$WG_SERVER_PEERS"
    for peer_spec in "${peer_specs[@]}"; do
      [[ -n "$peer_spec" ]] || continue
      IFS='|' read -r public_key allowed_ips endpoint keepalive psk_file <<< "$peer_spec"
      [[ -n "$public_key" && -n "$allowed_ips" ]] \
        || die "Invalid WG_SERVER_PEERS record: ${peer_spec}. Expected public-key|allowed-ips[|endpoint|keepalive|preshared-key-file]"

      printf '        - keys:\n'
      printf '            public: %s\n' "$(yaml_quote "$public_key")"
      if [[ -n "${psk_file:-}" ]]; then
        printf '            shared: %s\n' "$(yaml_quote "$psk_file")"
      fi
      printf '          allowed-ips:\n'
      IFS=',' read -r -a allowed_ip_list <<< "$allowed_ips"
      for allowed_ip in "${allowed_ip_list[@]}"; do
        allowed_ip="${allowed_ip//[[:space:]]/}"
        [[ -n "$allowed_ip" ]] && printf '            - %s\n' "$(yaml_quote "$allowed_ip")"
      done
      [[ -z "${endpoint:-}" ]] || printf '          endpoint: %s\n' "$(yaml_quote "$endpoint")"
      [[ -z "${keepalive:-}" ]] || printf '          keepalive: %s\n' "$keepalive"
    done
  } > "$tmp"

  install -m 0600 "$tmp" "$WG_SERVER_NETPLAN_FILE"
  rm -f "$tmp"
  rm -f "$DINIT_D/wg-quick-${WG_SERVER_INTERFACE}" "$DINIT_D/boot.d/wg-quick-${WG_SERVER_INTERFACE}"
  log "Wrote ${WG_SERVER_NETPLAN_FILE}; ${WG_SERVER_INTERFACE} will be applied by netplan"
}

ensure_wgcf_config() {
  [[ "$UPDATE_WGCF" == "true" ]] || return 0
  log "--- Generate ${WGCF_CONF} from ${WGCF_ACCOUNT_FILE} ---"

  need_cmd wgcf
  [[ -f "$WGCF_ACCOUNT_FILE" ]] \
    || die "${WGCF_ACCOUNT_FILE} is required to generate ${WGCF_CONF}"

  local tmp
  local -a update_args
  tmp="$(mktemp)"
  update_args=()
  [[ -z "${WGCF_LICENSE_KEY:-}" ]] || update_args+=("--license-key" "$WGCF_LICENSE_KEY")
  [[ -z "${WGCF_DEVICE_NAME:-}" ]] || update_args+=("--name" "$WGCF_DEVICE_NAME")

  if [[ "$UPDATE_WGCF" == "true" ]]; then
    wgcf --config "$WGCF_ACCOUNT_FILE" update "${update_args[@]}"
  fi
  wgcf --config "$WGCF_ACCOUNT_FILE" generate --profile "$tmp"
  install -m 0600 "$tmp" "$WGCF_CONF"
  rm -f "$tmp"
}

install_network_artifacts() {
  log "--- Install network artifacts ---"

  ensure_wgcf_config

  install -d "$DINIT_D" "$DINIT_D/boot.d" "$DINIT_D/scripts" /etc/netplan /etc/systemd/network

  install -m 0644 "$SCRIPT_DIR/dinit/services0-sockets" "$DINIT_D/services0-sockets"
  install -m 0755 "$SCRIPT_DIR/dinit/scripts/services0-sockets.sh" "$DINIT_D/scripts/services0-sockets.sh"
  install -m 0644 "$SCRIPT_DIR/dinit/wg-quick-all" "$DINIT_D/wg-quick-all"
  install -m 0755 "$SCRIPT_DIR/dinit/scripts/wg-quick-all-up.sh"   /usr/local/sbin/wg-quick-all-up.sh
  install -m 0755 "$SCRIPT_DIR/dinit/scripts/wg-quick-all-down.sh" /usr/local/sbin/wg-quick-all-down.sh
  install -m 0644 "$SCRIPT_DIR/dinit/op-ovs-services" "$DINIT_D/op-ovs-services"
  install -m 0755 "$SCRIPT_DIR/dinit/op-ovs-services-start.sh" "$DINIT_D/scripts/op-ovs-services-start.sh"
  install -m 0644 "$SCRIPT_DIR/dinit/systemd-networkd" "$DINIT_D/systemd-networkd"
  install -m 0644 "$SCRIPT_DIR/dinit/netplan-apply" "$DINIT_D/netplan-apply"
  install -m 0644 "$SCRIPT_DIR/dinit/ovs-attach-ports" "$DINIT_D/ovs-attach-ports"
  install -m 0755 "$SCRIPT_DIR/dinit/scripts/ovs-attach-ports.sh" "$DINIT_D/scripts/ovs-attach-ports.sh"

  install -m 0600 "$SCRIPT_DIR/netplan/01-ovsbr0.yaml" /etc/netplan/01-ovsbr0.yaml
  write_wg_server_netplan
  find "$SCRIPT_DIR/systemd/networkd" -maxdepth 1 -type f | while read -r file; do
    install -m 0644 "$file" "/etc/systemd/network/$(basename "$file")"
  done

  ensure_depends_on "$DINIT_D/incus" services0-sockets
  ensure_depends_on "$DINIT_D/nginx" services0-sockets
  ensure_depends_on "$DINIT_D/ovs-attach-ports" wg-quick-all
  ensure_depends_on "$DINIT_D/ovs-attach-ports" netplan-apply
  ensure_depends_on "$DINIT_D/ovs-attach-ports" systemd-networkd

  enable_boot services0-sockets
  enable_boot wg-quick-all
  enable_boot op-ovs-services
  enable_boot systemd-networkd
  enable_boot netplan-apply
  enable_boot ovs-attach-ports
}

start_host_network() {
  log "--- Start host network services ---"

  start_service services0-sockets || run "$DINIT_D/scripts/services0-sockets.sh"

  if [[ "$UPDATE_WGCF" == "true" ]]; then
    restart_service wg-quick-all || {
      wg-quick down "$WGCF_INTERFACE" >/dev/null 2>&1 || true
      wg-quick up "$WGCF_INTERFACE" || true
    }
  else
    start_service wg-quick-all || wg-quick up "$WGCF_INTERFACE" || true
  fi
  wait_for_iface "$WGCF_INTERFACE" 30

  start_service op-ovs-services || true
  start_service systemd-networkd || true

  log "Applying netplan OVS bridge config"
  netplan apply
  wait_for_iface "$OVS_BRIDGE" 30

  "$DINIT_D/scripts/ovs-attach-ports.sh"
  start_service ovs-attach-ports || true

  for iface in "$MGMT_IFACE" "$GRPC_BRIDGE_IFACE" ovsbr0-sock priv_wg priv_warp priv_xray; do
    wait_for_iface "$iface" 20
  done
}

ensure_default_profile_socket_only() {
  if incus profile device get default eth0 type >/dev/null 2>&1; then
    log "Removing eth0 from Incus default profile"
    incus profile device remove default eth0
  else
    log "Incus default profile has no eth0"
  fi
}

ensure_services_container() {
  log "--- Ensure ${SERVICES_CONTAINER} socket container ---"

  if ! incus info "$SERVICES_CONTAINER" >/dev/null 2>&1; then
    log "Creating ${SERVICES_CONTAINER} without a NIC"
    incus init images:debian/trixie "$SERVICES_CONTAINER"
    incus config set "$SERVICES_CONTAINER" security.privileged true
    incus config set "$SERVICES_CONTAINER" security.nesting true
  fi

  incus config set "$SERVICES_CONTAINER" user.function system-services

  incus config device remove "$SERVICES_CONTAINER" eth0 >/dev/null 2>&1 || true
  incus config device remove "$SERVICES_CONTAINER" smtp25 >/dev/null 2>&1 || true

  current_source="$(incus config device get "$SERVICES_CONTAINER" services0 source 2>/dev/null || true)"
  current_path="$(incus config device get "$SERVICES_CONTAINER" services0 path 2>/dev/null || true)"
  if [[ "$current_source" != "$SOCKET_DIR" || "$current_path" != "$SOCKET_DIR" ]]; then
    incus config device remove "$SERVICES_CONTAINER" services0 >/dev/null 2>&1 || true
    incus config device add "$SERVICES_CONTAINER" services0 disk source="$SOCKET_DIR" path="$SOCKET_DIR"
  fi

  if [[ "$(container_status "$SERVICES_CONTAINER")" != "RUNNING" ]]; then
    incus start "$SERVICES_CONTAINER"
    wait_for_container "$SERVICES_CONTAINER" 60
  fi

  if incus exec "$SERVICES_CONTAINER" -- test -e /sys/class/net/eth0 >/dev/null 2>&1; then
    log "${SERVICES_CONTAINER} still has eth0 after device removal; restarting"
    incus restart "$SERVICES_CONTAINER"
    wait_for_container "$SERVICES_CONTAINER" 60
  fi

  incus exec "$SERVICES_CONTAINER" -- sh -lc 'wg-quick down wg0 >/dev/null 2>&1 || true'
  incus exec "$SERVICES_CONTAINER" -- sh -lc 'systemctl disable --now wg-quick@wg0.service wg-quick@wg0 >/dev/null 2>&1 || true'

  verify_container_loopback_only "$SERVICES_CONTAINER"
}

verify_container_loopback_only() {
  local container="$1"
  local non_loopback

  non_loopback="$(incus exec "$container" -- sh -lc "awk -F: 'NR>2 { gsub(/^[ \t]+|[ \t]+$/, \"\", \$1); if (\$1 != \"lo\") print \$1 }' /proc/net/dev" 2>/dev/null || true)"
  [[ -z "$non_loopback" ]] || die "${container} has non-loopback interfaces: ${non_loopback}"
  log "${container}: loopback-only"
}

install_services_gateway_socket_bridge() {
  [[ "$INSTALL_SERVICES_GATEWAY_SOCKET" == "true" ]] || return 0
  log "--- Install services gateway socket bridge ---"

  if ! incus info "$SERVICES_CONTAINER" >/dev/null 2>&1; then
    log "Skipping services gateway socket bridge; ${SERVICES_CONTAINER} does not exist"
    return 0
  fi

  incus file push "$SCRIPT_DIR/systemd/gateway-socket.service" \
    "$SERVICES_CONTAINER/etc/systemd/system/gateway-socket.service"

  incus exec "$SERVICES_CONTAINER" -- systemctl daemon-reload
  incus exec "$SERVICES_CONTAINER" -- systemctl disable --now openclaw-socket.service >/dev/null 2>&1 || true
  incus exec "$SERVICES_CONTAINER" -- systemctl enable gateway-socket.service >/dev/null 2>&1 || true

  if incus exec "$SERVICES_CONTAINER" -- systemctl is-active --quiet openclaw-gateway.service; then
    incus exec "$SERVICES_CONTAINER" -- systemctl restart gateway-socket.service
  else
    log "Gateway service is not active; socket bridge installed but not restarted"
  fi
}

install_dashboard_nginx() {
  [[ "$INSTALL_DASHBOARD_NGINX" == "true" ]] || return 0
  log "--- Install dashboard nginx socket config ---"

  if [[ -z "${SERVICES_GATEWAY_TOKEN:-}" && -n "${OPENCLAW_GATEWAY_TOKEN:-}" ]]; then
    SERVICES_GATEWAY_TOKEN="$OPENCLAW_GATEWAY_TOKEN"
  fi
  [[ -n "${SERVICES_GATEWAY_TOKEN:-}" ]] \
    || die "SERVICES_GATEWAY_TOKEN is required with --install-dashboard-nginx"

  install -d /etc/nginx/http.d
  SERVICES_GATEWAY_TOKEN="$SERVICES_GATEWAY_TOKEN" python3 - \
    "$SCRIPT_DIR/nginx/dashboard-3tched-socket.conf.template" \
    /etc/nginx/http.d/dashboard-3tched.conf <<'PY'
import os
import pathlib
import sys

template = pathlib.Path(sys.argv[1])
target = pathlib.Path(sys.argv[2])
target.write_text(
    template.read_text().replace("<OPENCLAW_GATEWAY_TOKEN>", os.environ["SERVICES_GATEWAY_TOKEN"])
)
PY

  nginx -t
  nginx -s reload
}

apply_nextdns_rewrite() {
  [[ "$APPLY_NEXTDNS_REWRITE" == "true" ]] || return 0
  log "--- NextDNS split DNS rewrite ---"

  if [[ -z "${NEXTDNS_API_KEY:-}" ]]; then
    log "NEXTDNS_API_KEY not set; skipped rewrite ${NEXTDNS_REWRITE_NAME} -> ${NEXTDNS_REWRITE_CONTENT}"
    return 0
  fi

  NEXTDNS_PROFILE="$NEXTDNS_PROFILE" NEXTDNS_API_KEY="$NEXTDNS_API_KEY" \
    "$SCRIPT_DIR/nextdns/upsert-rewrite.py" "$NEXTDNS_REWRITE_NAME" "$NEXTDNS_REWRITE_CONTENT"
}

verify_network() {
  log "--- Verify socket/OpenFlow network ---"

  if ip link show "$WG_SERVER_INTERFACE" >/dev/null 2>&1; then
    log "Interface ${WG_SERVER_INTERFACE} exists"
  else
    log "Interface ${WG_SERVER_INTERFACE} is absent; dashboard WG access depends on external provisioning or --update-wgconf"
  fi
  wait_for_iface "$WGCF_INTERFACE" 1
  wait_for_iface "$OVS_BRIDGE" 1
  wait_for_iface ovsbr0-sock 1
  wait_for_iface priv_xray 1
  wait_for_iface "$GRPC_BRIDGE_IFACE" 1

  [[ -d "$SOCKET_DIR" ]] || die "${SOCKET_DIR} missing"
  local socket_mode sock_mode
  socket_mode="$(stat_mode_owner_group "$SOCKET_DIR")"
  [[ "$socket_mode" == "770 root:${SOCKET_GROUP}" || "$socket_mode" == "770 root:${SOCKET_GROUP_ID}" ]] \
    || die "${SOCKET_DIR} has unexpected ownership/mode: ${socket_mode}"

  if incus info "$SERVICES_CONTAINER" >/dev/null 2>&1; then
    verify_container_loopback_only "$SERVICES_CONTAINER"
  fi

  if [[ -S "$SOCKET_DIR/gateway.sock" ]]; then
    sock_mode="$(stat_mode_owner_group "$SOCKET_DIR/gateway.sock")"
    [[ "$sock_mode" == "660 root:${SOCKET_GROUP}" || "$sock_mode" == "660 root:${SOCKET_GROUP_ID}" ]] \
      || die "${SOCKET_DIR}/gateway.sock has unexpected ownership/mode: ${sock_mode}"

    if command -v doas >/dev/null 2>&1; then
      doas -u "$SOCKET_GROUP" sh -c \
        "curl -s -o /dev/null -w '%{http_code}' --unix-socket '$SOCKET_DIR/gateway.sock' http://localhost/" \
        | grep -qx '200' || die "nginx user cannot reach gateway.sock"
    fi
  else
    log "gateway.sock is not present yet; services gateway socket bridge may not be active"
  fi

  log "Verification complete"
}

summary() {
  cat <<EOF

=== SOCKET/OPENFLOW NETWORK SUMMARY ===
wg identity:        ${WG_SERVER_INTERFACE} ($(ip -4 -brief addr show "$WG_SERVER_INTERFACE" 2>/dev/null | awk '{ print $3 }'))
bridge wg tunnel:   ${WGCF_INTERFACE} ($(ip -4 -brief addr show "$WGCF_INTERFACE" 2>/dev/null | awk '{ print $3 }'))
ovs bridge:         ${OVS_BRIDGE} ($(ip -4 -brief addr show "$OVS_BRIDGE" 2>/dev/null | awk '{ print $3 }'))
socket dir:         ${SOCKET_DIR} ($(stat_mode_owner_group "$SOCKET_DIR" 2>/dev/null || echo missing))
services container: $(container_status "$SERVICES_CONTAINER")
NextDNS rewrite:    ${NEXTDNS_REWRITE_NAME} -> ${NEXTDNS_REWRITE_CONTENT} (${APPLY_NEXTDNS_REWRITE})
=======================================
EOF
}

main() {
  log "=== deploy-network.sh socket/OpenFlow installer starting ==="
  preflight "$@"

  if [[ "$VERIFY_ONLY" != "true" ]]; then
    install_network_artifacts
    start_host_network
    ensure_default_profile_socket_only
    ensure_services_container
    install_services_gateway_socket_bridge
    install_dashboard_nginx
    apply_nextdns_rewrite
  fi

  verify_network
  summary
  log "deploy-network.sh completed successfully"
}

main "$@"

# ---------------------------------------------------------------------------
# ENVIRONMENT DETAILS (2026-04-15T02:25:03+00:00)
# ---------------------------------------------------------------------------
# Socket Migration Complete:
# - services container: loopback-only (lan0-service = lo renamed, no veth)
# - ovsbr0 ports: grpc-bridge, wgcf, priv_warp, priv_xray, ovsbr0-sock anchor
# - OpenFlow: DNS pri 200 udp:53 → ovsbr0-sock → lan0-service nextdns
# - Uplink: ovsbr0 10.88.88.1/24 NAT → ens3 (148.113.204.83)
# - Persistence: raw.lxc post-start hook for lan0-service dummy
# Verify: incus exec services -- ping 1.1.1.1
