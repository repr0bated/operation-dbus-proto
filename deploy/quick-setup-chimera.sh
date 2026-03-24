#!/bin/bash
#
# Quick setup for Chimera ICUS container
# Usage: sudo ./quick-setup-chimera.sh --wg-endpoint wg.example.com:51820 --wg-pubkey <SERVER_PUBKEY> --mcp-host mcp.example.com
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --wg-endpoint)
            export WIREGUARD_ENDPOINT="$2"
            shift 2
            ;;
        --wg-pubkey)
            export WIREGUARD_PUBLIC_KEY="$2"
            shift 2
            ;;
        --wg-allowed-ips)
            export WIREGUARD_ALLOWED_IPS="$2"
            shift 2
            ;;
        --mcp-host)
            export MCP_SSH_HOST="$2"
            shift 2
            ;;
        --mcp-port)
            export MCP_SSH_PORT="$2"
            shift 2
            ;;
        --icus-name)
            export ICUS_NAME="$2"
            shift 2
            ;;
        --dhcp-interface)
            export DHCP_INTERFACE="$2"
            shift 2
            ;;
        --help|-h)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Required Options:"
            echo "  --wg-endpoint HOST:PORT    WireGuard server endpoint (e.g., wg.example.com:51820)"
            echo "  --wg-pubkey KEY            WireGuard server public key"
            echo "  --mcp-host HOST            MCP SSH server hostname/IP"
            echo ""
            echo "Optional Options:"
            echo "  --wg-allowed-ips IPS       Allowed IPs (default: 0.0.0.0/0, ::/0)"
            echo "  --mcp-port PORT            MCP SSH port (default: 22)"
            echo "  --icus-name NAME           Container name (default: chimera-icus)"
            echo "  --dhcp-interface IFACE     DHCP interface (default: eth0)"
            echo "  --help                     Show this help"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Validate required args
if [[ -z "${WIREGUARD_ENDPOINT:-}" ]]; then
    echo "ERROR: --wg-endpoint is required"
    exit 1
fi

if [[ -z "${WIREGUARD_PUBLIC_KEY:-}" ]]; then
    echo "ERROR: --wg-pubkey is required"
    exit 1
fi

if [[ -z "${MCP_SSH_HOST:-}" ]]; then
    echo "ERROR: --mcp-host is required"
    exit 1
fi

echo "=========================================="
echo "Chimera Linux ICUS Quick Setup"
echo "=========================================="
echo ""
echo "Configuration:"
echo "  WireGuard Endpoint: $WIREGUARD_ENDPOINT"
echo "  MCP SSH Host:       $MCP_SSH_HOST"
echo "  MCP SSH Port:       ${MCP_SSH_PORT:-22}"
echo "  ICUS Name:          ${ICUS_NAME:-chimera-icus}"
echo "  DHCP Interface:     ${DHCP_INTERFACE:-eth0}"
echo ""
read -p "Continue? [Y/n]: " confirm
if [[ ! "$confirm" =~ ^[Yy]$ ]] && [[ -n "$confirm" ]]; then
    echo "Aborted."
    exit 1
fi

# Run the main setup script
exec "$SCRIPT_DIR/setup-chimera-icus.sh"
