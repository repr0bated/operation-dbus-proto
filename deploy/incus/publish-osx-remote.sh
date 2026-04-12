#!/usr/bin/env bash
# Publish an Incus macOS VM console remotely:
# 1) Creates/starts a dinit service that proxies VM SPICE socket -> TCP port.
# 2) Upserts Cloudflare A record (DNS-only) for remote client access.
#
# Requires ~/.bash_secrets with:
#   CF_DNS_ZONE_TOKEN
#   CF_ZONEID_3TCHEDCOM (or CF_ZONE_ID_3TCHED / CF_ZONEID_3TCHED)

set -euo pipefail

VM_NAME="macos-incus"
FQDN="osx.3tched.com"
PORT="5905"
SERVICE_NAME="osx-spice-proxy"
SPICE_SOCKET=""
PUBLIC_IP=""
START_VM=1

log() {
    printf '[publish-osx] %s\n' "$*"
}

warn() {
    printf '[publish-osx][warn] %s\n' "$*" >&2
}

die() {
    printf '[publish-osx][error] %s\n' "$*" >&2
    exit 1
}

usage() {
    cat <<EOF
Usage:
  $(basename "$0") [options]

Options:
  --vm-name <name>       Incus VM name (default: ${VM_NAME})
  --fqdn <name>          DNS name to publish (default: ${FQDN})
  --port <port>          Host TCP port for SPICE proxy (default: ${PORT})
  --service-name <name>  dinit service name (default: ${SERVICE_NAME})
  --socket <path>        SPICE unix socket path (default: /run/incus/<vm>/qemu.spice)
  --public-ip <ip>       Override detected public IPv4
  --no-start-vm          Do not auto-start VM if currently stopped
  -h, --help             Show help

Example:
  doas ./deploy/incus/publish-osx-remote.sh --fqdn osx.3tched.com --port 5905
EOF
}

require_cmd() {
    command -v "$1" >/dev/null 2>&1 || die "required command not found: $1"
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --vm-name)
                VM_NAME="$2"
                shift 2
                ;;
            --fqdn)
                FQDN="$2"
                shift 2
                ;;
            --port)
                PORT="$2"
                shift 2
                ;;
            --service-name)
                SERVICE_NAME="$2"
                shift 2
                ;;
            --socket)
                SPICE_SOCKET="$2"
                shift 2
                ;;
            --public-ip)
                PUBLIC_IP="$2"
                shift 2
                ;;
            --no-start-vm)
                START_VM=0
                shift
                ;;
            -h|--help)
                usage
                exit 0
                ;;
            *)
                die "unknown argument: $1"
                ;;
        esac
    done
}

detect_public_ipv4() {
    local ip
    ip="$(ip -4 route get 1.1.1.1 2>/dev/null | awk '/src/ {for (i = 1; i <= NF; i++) if ($i == "src") {print $(i + 1); exit}}')"
    if [[ -n "$ip" ]]; then
        printf '%s\n' "$ip"
        return
    fi

    curl -4fsS https://ifconfig.me || true
}

ensure_root() {
    [[ "${EUID}" -eq 0 ]] || die "run as root (doas/sudo)"
}

load_cloudflare_secrets() {
    local secrets_file=""
    local candidate=""

    if [[ -n "${BASH_SECRETS_FILE:-}" ]]; then
        candidate="${BASH_SECRETS_FILE}"
        [[ -f "${candidate}" ]] || die "BASH_SECRETS_FILE does not exist: ${candidate}"
        secrets_file="${candidate}"
    fi

    if [[ -z "${secrets_file}" && -n "${SUDO_USER:-}" ]]; then
        candidate="$(getent passwd "${SUDO_USER}" | cut -d: -f6)/.bash_secrets"
        [[ -f "${candidate}" ]] && secrets_file="${candidate}"
    fi

    if [[ -z "${secrets_file}" ]]; then
        candidate="$(getent passwd "${USER:-root}" | cut -d: -f6)/.bash_secrets"
        [[ -f "${candidate}" ]] && secrets_file="${candidate}"
    fi

    if [[ -z "${secrets_file}" && -f /root/.bash_secrets ]]; then
        secrets_file="/root/.bash_secrets"
    fi

    [[ -n "${secrets_file}" ]] || die "could not find ~/.bash_secrets (set BASH_SECRETS_FILE=/path/to/.bash_secrets)"

    # shellcheck disable=SC1090
    source "${secrets_file}"
    log "Loaded Cloudflare secrets from: ${secrets_file}"

    CF_TOKEN="${CF_DNS_ZONE_TOKEN:-}"
    ZONE_ID="${CF_ZONEID_3TCHEDCOM:-${CF_ZONE_ID_3TCHED:-${CF_ZONEID_3TCHED:-}}}"

    [[ -n "${CF_TOKEN}" ]] || die "CF_DNS_ZONE_TOKEN missing in /root/.bash_secrets"
    [[ -n "${ZONE_ID}" ]] || die "CF_ZONEID_3TCHEDCOM (or fallback vars) missing in /root/.bash_secrets"
}

ensure_vm_running() {
    incus info "$VM_NAME" >/dev/null 2>&1 || die "VM not found: ${VM_NAME}"

    local status
    status="$(incus info "$VM_NAME" | awk -F': ' '$1 == "Status" {print $2; exit}')"
    if [[ "$status" != "RUNNING" ]]; then
        if [[ "$START_VM" -eq 1 ]]; then
            log "Starting VM ${VM_NAME}..."
            incus start "$VM_NAME"
        else
            die "VM ${VM_NAME} is not running (use without --no-start-vm or start it first)"
        fi
    fi
}

write_dinit_service() {
    local service_file="/etc/dinit.d/${SERVICE_NAME}"
    local socket_path="$1"

    cat > "${service_file}" <<EOF
type = process
command = /usr/bin/socat TCP-LISTEN:${PORT},reuseaddr,fork UNIX-CONNECT:${socket_path}
restart = true
EOF
    log "Wrote dinit service: ${service_file}"
}

restart_dinit_service() {
    local name="$1"
    if dinitctl status "$name" >/dev/null 2>&1; then
        dinitctl restart "$name"
    else
        dinitctl start "$name"
    fi
    log "dinit service active: ${name}"
}

upsert_cloudflare_record() {
    local fqdn="$1"
    local ip="$2"
    local host="${fqdn%.3tched.com}"
    local existing
    local payload

    existing="$(curl -fsS -X GET "https://api.cloudflare.com/client/v4/zones/${ZONE_ID}/dns_records?type=A&name=${fqdn}" \
        -H "Authorization: Bearer ${CF_TOKEN}" \
        -H "Content-Type: application/json" | jq -r '.result[0].id // empty')"

    payload="{\"type\":\"A\",\"name\":\"${host}\",\"content\":\"${ip}\",\"ttl\":1,\"proxied\":false}"

    if [[ -n "$existing" ]]; then
        curl -fsS -X PUT "https://api.cloudflare.com/client/v4/zones/${ZONE_ID}/dns_records/${existing}" \
            -H "Authorization: Bearer ${CF_TOKEN}" \
            -H "Content-Type: application/json" \
            --data "${payload}" >/dev/null
        log "Updated Cloudflare DNS: ${fqdn} -> ${ip} (DNS-only)"
    else
        curl -fsS -X POST "https://api.cloudflare.com/client/v4/zones/${ZONE_ID}/dns_records" \
            -H "Authorization: Bearer ${CF_TOKEN}" \
            -H "Content-Type: application/json" \
            --data "${payload}" >/dev/null
        log "Created Cloudflare DNS: ${fqdn} -> ${ip} (DNS-only)"
    fi
}

main() {
    parse_args "$@"

    require_cmd incus
    require_cmd socat
    require_cmd dinitctl
    require_cmd jq
    require_cmd curl
    require_cmd ip
    ensure_root

    load_cloudflare_secrets
    ensure_vm_running

    if [[ -z "$SPICE_SOCKET" ]]; then
        SPICE_SOCKET="/run/incus/${VM_NAME}/qemu.spice"
    fi
    [[ -S "$SPICE_SOCKET" ]] || die "SPICE socket missing: ${SPICE_SOCKET}"

    if [[ -z "$PUBLIC_IP" ]]; then
        PUBLIC_IP="$(detect_public_ipv4)"
    fi
    [[ -n "$PUBLIC_IP" ]] || die "could not detect public IPv4"

    write_dinit_service "$SPICE_SOCKET"
    restart_dinit_service "$SERVICE_NAME"
    upsert_cloudflare_record "$FQDN" "$PUBLIC_IP"

    cat <<EOF

[publish-osx] Done.
VM:          ${VM_NAME}
SPICE proxy: ${SERVICE_NAME} -> 0.0.0.0:${PORT}
DNS:         ${FQDN} -> ${PUBLIC_IP} (DNS-only)

Connect with:
  remote-viewer spice://${FQDN}:${PORT}
EOF
}

main "$@"
