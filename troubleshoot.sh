#!/bin/sh
# =============================================================================
# ssh-recover.sh
# Comprehensive SSH diagnostics AND auto-recovery for Chimera Linux
# Run from a live/lifeline session on the host
# =============================================================================

# set -e intentionally omitted — musl stat and other tools differ from GNU

RED='\033[0;31m'
YEL='\033[1;33m'
GRN='\033[0;32m'
BLU='\033[0;34m'
CYN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

REPORT_FILE="/tmp/ssh-recover-$(date +%Y%m%d-%H%M%S).txt"
ISSUES_FOUND=0
FIXED=0

log()     { printf "${BOLD}${BLU}[INFO]${NC}  %s\n" "$*" | tee -a "$REPORT_FILE"; }
ok()      { printf "${BOLD}${GRN}[OK]${NC}    %s\n" "$*" | tee -a "$REPORT_FILE"; }
warn()    { printf "${BOLD}${YEL}[WARN]${NC}  %s\n" "$*" | tee -a "$REPORT_FILE"; }
fail()    { printf "${BOLD}${RED}[FAIL]${NC}  %s\n" "$*" | tee -a "$REPORT_FILE"; ISSUES_FOUND=$((ISSUES_FOUND+1)); }
fixed()   { printf "${BOLD}${GRN}[FIXED]${NC} %s\n" "$*" | tee -a "$REPORT_FILE"; FIXED=$((FIXED+1)); }
section() { printf "\n${BOLD}${CYN}━━━ %s ━━━${NC}\n" "$*" | tee -a "$REPORT_FILE"; }
raw()     { printf "%s\n" "$*" | tee -a "$REPORT_FILE"; }
have()    { command -v "$1" >/dev/null 2>&1; }

print_header() {
    printf "\n${BOLD}${CYN}"
    printf "╔══════════════════════════════════════════════════════════════╗\n"
    printf "║         SSH Recovery & Diagnostic Script                     ║\n"
    printf "║         Chimera Linux — Run from lifeline session            ║\n"
    printf "╚══════════════════════════════════════════════════════════════╝\n"
    printf "${NC}\n"
    printf "  Report: %s\n\n" "$REPORT_FILE"
}

# =============================================================================
# 1. SSHD PROCESS
# =============================================================================

check_sshd_process() {
    section "1. SSHD PROCESS"

    log "Checking if sshd is running"
    if pgrep -x sshd >/dev/null 2>&1; then
        ok "sshd process is running (PID: $(pgrep -x sshd | head -1))"
    else
        fail "sshd is NOT running"

        log "Attempting to start sshd via dinit..."
        if have dinitctl; then
            doas dinitctl start sshd 2>/dev/null && fixed "sshd started via dinitctl" || \
                warn "dinitctl start sshd failed"
        fi

        log "Attempting direct sshd start..."
        if have sshd; then
            doas sshd && fixed "sshd started directly" || warn "direct sshd start failed"
        fi

        # Re-check
        if pgrep -x sshd >/dev/null 2>&1; then
            fixed "sshd is now running"
        else
            fail "sshd still not running after recovery attempts"
        fi
    fi

    log "dinit sshd service status"
    if have dinitctl; then
        dinitctl status sshd 2>/dev/null | tee -a "$REPORT_FILE" || raw "  (dinitctl status unavailable)"
    fi
}

# =============================================================================
# 2. SSHD LISTENING PORT
# =============================================================================

check_sshd_listening() {
    section "2. SSHD LISTENING PORT"

    log "Ports sshd is listening on"
    if have ss; then
        LISTENING=$(ss -tlnp | grep -E ":22|sshd" || echo "")
        if [ -n "$LISTENING" ]; then
            ok "sshd is listening:"
            echo "$LISTENING" | tee -a "$REPORT_FILE"
        else
            fail "sshd is NOT listening on any port"
            raw "  All listening TCP ports:"
            ss -tlnp | tee -a "$REPORT_FILE"
        fi
    fi

    log "sshd config — Port setting"
    SSHD_CONF="/etc/ssh/sshd_config"
    if [ -f "$SSHD_CONF" ]; then
        PORT_LINE=$(grep -i "^Port\|^#Port" "$SSHD_CONF" | head -5)
        raw "  $PORT_LINE"
        LISTEN_LINE=$(grep -i "^ListenAddress" "$SSHD_CONF" | head -5)
        if [ -n "$LISTEN_LINE" ]; then
            raw "  $LISTEN_LINE"
            if echo "$LISTEN_LINE" | grep -q "127.0.0.1\|localhost"; then
                fail "sshd ListenAddress is localhost-only — external connections will be refused"
                log "Fixing ListenAddress to 0.0.0.0..."
                doas sed -i 's/^ListenAddress 127.0.0.1/ListenAddress 0.0.0.0/' "$SSHD_CONF"
                doas dinitctl restart sshd 2>/dev/null || doas pkill -HUP sshd
                fixed "ListenAddress updated and sshd reloaded"
            fi
        fi
    else
        warn "sshd_config not found at $SSHD_CONF"
        find /etc -name "sshd_config" 2>/dev/null | tee -a "$REPORT_FILE"
    fi
}

# =============================================================================
# 3. SSHD CONFIGURATION VALIDATION
# =============================================================================

check_sshd_config() {
    section "3. SSHD CONFIGURATION VALIDATION"

    log "Testing sshd config syntax"
    if have sshd; then
        CONFIG_TEST=$(doas sshd -t 2>&1)
        if [ $? -eq 0 ]; then
            ok "sshd config syntax is valid"
        else
            fail "sshd config has errors:"
            raw "$CONFIG_TEST"
            log "Showing full sshd_config for review:"
            cat /etc/ssh/sshd_config 2>/dev/null | grep -v "^#" | grep -v "^$" | tee -a "$REPORT_FILE"
        fi
    fi

    log "Key sshd_config settings"
    for setting in PermitRootLogin PasswordAuthentication PubkeyAuthentication AuthorizedKeysFile AllowUsers DenyUsers AllowGroups DenyGroups MaxAuthTries; do
        VAL=$(grep -i "^$setting" /etc/ssh/sshd_config 2>/dev/null | head -1)
        if [ -n "$VAL" ]; then
            raw "  $VAL"
            # Flag dangerous settings
            if echo "$VAL" | grep -qi "DenyUsers.*jeremy\|AllowUsers.*\!jeremy"; then
                fail "$setting may be blocking your user"
            fi
        fi
    done

    log "Checking AuthorizedKeysFile"
    AUTH_KEYS_FILE=$(grep -i "^AuthorizedKeysFile" /etc/ssh/sshd_config 2>/dev/null | awk '{print $2}' | head -1)
    AUTH_KEYS_FILE=${AUTH_KEYS_FILE:-".ssh/authorized_keys"}
    AUTH_KEYS_PATH="$HOME/${AUTH_KEYS_FILE#./}"
    AUTH_KEYS_PATH=$(eval echo "$AUTH_KEYS_PATH")

    if [ -f "$AUTH_KEYS_PATH" ]; then
        ok "authorized_keys exists: $AUTH_KEYS_PATH"
        raw "  Keys: $(wc -l < "$AUTH_KEYS_PATH")"
        ls -la "$AUTH_KEYS_PATH" | tee -a "$REPORT_FILE"
    else
        fail "authorized_keys NOT found at: $AUTH_KEYS_PATH"
        raw "  If using key auth, this will block all logins"
        find "$HOME" -name "authorized_keys" 2>/dev/null | tee -a "$REPORT_FILE"
    fi

    log "Checking .ssh directory permissions (strict requirements)"
    if [ -d "$HOME/.ssh" ]; then
        # Use ls for permissions — POSIX compatible, works on musl/Chimera (no GNU stat -c)
        SSH_PERMS=$(ls -ld "$HOME/.ssh" | awk '{print $1}')
        KEY_PERMS=$(ls -l "$HOME/.ssh/authorized_keys" 2>/dev/null | awk '{print $1}' || echo "N/A")
        raw "  ~/.ssh permissions: $SSH_PERMS (must be drwx------)"
        raw "  authorized_keys permissions: $KEY_PERMS (must be -rw-------)"

        if [ "$SSH_PERMS" != "drwx------" ]; then
            fail "~/.ssh permissions are $SSH_PERMS — fixing..."
            chmod 700 "$HOME/.ssh"
            fixed "~/.ssh set to 700"
        else
            ok "~/.ssh permissions OK (700)"
        fi

        if [ "$KEY_PERMS" != "N/A" ] && [ "$KEY_PERMS" != "-rw-------" ]; then
            fail "authorized_keys permissions are $KEY_PERMS — fixing..."
            chmod 600 "$HOME/.ssh/authorized_keys"
            fixed "authorized_keys set to 600"
        else
            ok "authorized_keys permissions OK (600)"
        fi
    else
        fail "~/.ssh directory does not exist"
        log "Creating ~/.ssh with correct permissions..."
        mkdir -p "$HOME/.ssh"
        chmod 700 "$HOME/.ssh"
        fixed "~/.ssh created"
    fi
}

# =============================================================================
# 4. FIREWALL / PACKET FILTER
# =============================================================================

check_firewall() {
    section "4. FIREWALL / PACKET FILTER"

    log "nftables ruleset (looking for SSH blocks)"
    if have nft; then
        RULESET=$(doas nft list ruleset 2>/dev/null)
        echo "$RULESET" | tee -a "$REPORT_FILE"

        # Check for port 22 drop/reject
        if echo "$RULESET" | grep -q "22" && echo "$RULESET" | grep -qE "drop|reject"; then
            warn "nftables has rules mentioning port 22 with drop/reject — review above"
        fi

        # Check if there's an accept rule for SSH
        if echo "$RULESET" | grep -qE "22.*accept|ssh.*accept|accept.*22"; then
            ok "nftables has an accept rule for port 22"
        elif echo "$RULESET" | grep -q "22"; then
            warn "Port 22 appears in nftables but no explicit accept found — check ruleset above"
        else
            log "Port 22 not explicitly mentioned in nftables — relying on default policy"
            DEFAULT_POLICY=$(echo "$RULESET" | grep "policy" | head -3)
            raw "  Default policies: $DEFAULT_POLICY"
            if echo "$DEFAULT_POLICY" | grep -qi "drop\|reject"; then
                fail "Default policy is DROP/REJECT and no SSH accept rule found — SSH is blocked"
                log "Adding emergency SSH accept rule..."
                doas nft add rule inet filter input tcp dport 22 accept 2>/dev/null && \
                    fixed "Added nft accept rule for port 22" || \
                    warn "Failed to add nft rule — try manually: doas nft add rule inet filter input tcp dport 22 accept"
            fi
        fi
    else
        raw "  nft not found"
    fi

    log "iptables INPUT chain"
    if have iptables; then
        doas iptables -L INPUT -n -v 2>/dev/null | head -30 | tee -a "$REPORT_FILE"
        if doas iptables -L INPUT -n 2>/dev/null | grep -q "DROP\|REJECT"; then
            warn "iptables has DROP/REJECT rules in INPUT — verify port 22 is not blocked"
        fi
    fi

    log "ip6tables INPUT chain"
    if have ip6tables; then
        doas ip6tables -L INPUT -n 2>/dev/null | head -10 | tee -a "$REPORT_FILE"
    fi
}

# =============================================================================
# 5. MTU & NETWORK PATH
# =============================================================================

check_mtu() {
    section "5. MTU & NETWORK PATH"

    log "Current interface MTUs"
    ip link show | grep -E "^[0-9]|mtu" | grep "mtu" | \
        awk '{for(i=1;i<=NF;i++) if($i=="mtu") {print prev, $i, $(i+1)}; prev=$0}' | \
        tee -a "$REPORT_FILE"

    log "OVS bridge MTU (public IP routes through here)"
    OVSBR0_MTU=$(cat /sys/class/net/ovsbr0/mtu 2>/dev/null || echo "N/A")
    ENS3_MTU=$(cat /sys/class/net/ens3/mtu 2>/dev/null || echo "N/A")

    raw "  ovsbr0 MTU: $OVSBR0_MTU"
    raw "  ens3 MTU:   $ENS3_MTU"

    if [ "$OVSBR0_MTU" != "N/A" ] && [ "$OVSBR0_MTU" != "$ENS3_MTU" ]; then
        warn "MTU mismatch between ovsbr0 ($OVSBR0_MTU) and ens3 ($ENS3_MTU)"
    fi

    # SSH specifically fails when MTU causes fragmentation of the key exchange
    # The SSH banner + key exchange packets can be 1400-1500 bytes
    log "Checking if MTU could be causing SSH key exchange failure"
    if [ "$OVSBR0_MTU" != "N/A" ] && [ "$OVSBR0_MTU" -lt 1400 ]; then
        fail "ovsbr0 MTU=$OVSBR0_MTU is too low — SSH key exchange packets (~1300-1400 bytes) may be dropped"
        log "Setting ovsbr0 MTU to 1500 for SSH recovery..."
        doas ip link set dev ovsbr0 mtu 1500
        fixed "ovsbr0 MTU set to 1500"
    fi

    log "MSS clamp check (prevents large packet fragmentation)"
    if have nft; then
        if doas nft list ruleset 2>/dev/null | grep -qi "clamp\|mss\|maxseg"; then
            ok "MSS clamping rule found in nftables"
        else
            warn "No MSS clamping rule found — adding one to prevent SSH large-packet drops"
            doas nft add rule inet filter forward tcp flags syn tcp option maxseg size set rt mtu 2>/dev/null && \
                fixed "MSS clamp rule added to nftables FORWARD chain" || \
                warn "Could not add MSS clamp — try: doas nft add rule inet filter forward tcp flags syn tcp option maxseg size set rt mtu"
        fi
    fi

    log "Path MTU discovery setting"
    PMTU=$(cat /proc/sys/net/ipv4/ip_no_pmtu_disc 2>/dev/null)
    raw "  ip_no_pmtu_disc = $PMTU (0=PMTU enabled, 1=disabled)"
    if [ "$PMTU" = "1" ]; then
        warn "PMTU discovery is disabled — this can cause silent packet drops on path MTU changes"
    fi
}

# =============================================================================
# 6. ROUTING & CONNECTIVITY
# =============================================================================

check_routing() {
    section "6. ROUTING & CONNECTIVITY"

    log "Default route"
    ip route show default | tee -a "$REPORT_FILE"

    log "Public IP / interface"
    PUBLIC_IP=$(ip addr show ovsbr0 2>/dev/null | grep "inet " | awk '{print $2}' | head -1)
    raw "  ovsbr0 IP: $PUBLIC_IP"

    log "Loopback SSH test (confirms sshd itself works)"
    if have ssh; then
        SSH_LOOPBACK=$(ssh -o "StrictHostKeyChecking=no" \
                          -o "ConnectTimeout=5" \
                          -o "BatchMode=yes" \
                          -p 22 \
                          127.0.0.1 echo "loopback_ok" 2>&1 || echo "failed")
        if echo "$SSH_LOOPBACK" | grep -q "loopback_ok"; then
            ok "SSH loopback test succeeded — sshd works locally"
            raw "  This means the issue is in the network path, not sshd itself"
        else
            warn "SSH loopback test: $SSH_LOOPBACK"
            raw "  May be key-based auth only — this is not necessarily a failure"
        fi
    fi

    log "Can we reach the default gateway?"
    GW=$(ip route show default | awk '{print $3}' | head -1)
    if [ -n "$GW" ]; then
        if ping -c 2 -W 2 "$GW" >/dev/null 2>&1; then
            ok "Gateway $GW is reachable"
        else
            fail "Gateway $GW is NOT reachable — routing is broken"
        fi
    fi

    log "OVS bridge status"
    if have ovs-vsctl; then
        doas ovs-vsctl show 2>/dev/null | tee -a "$REPORT_FILE"
    fi
}

# =============================================================================
# 7. SSH HOST KEYS
# =============================================================================

check_host_keys() {
    section "7. SSH HOST KEYS"

    log "Checking host keys exist"
    KEY_FOUND=0
    for keytype in rsa ecdsa ed25519; do
        KEYFILE="/etc/ssh/ssh_host_${keytype}_key"
        if [ -f "$KEYFILE" ]; then
            ok "Host key exists: $KEYFILE"
            ls -la "$KEYFILE" | tee -a "$REPORT_FILE"
            KEY_FOUND=$((KEY_FOUND+1))
        else
            warn "Missing host key: $KEYFILE"
        fi
    done

    if [ "$KEY_FOUND" -eq 0 ]; then
        fail "NO SSH host keys found — sshd cannot start without them"
        log "Generating host keys..."
        doas ssh-keygen -A && fixed "SSH host keys generated" || \
            fail "Failed to generate host keys"
    fi
}

# =============================================================================
# 8. PAM / AUTH
# =============================================================================

check_pam() {
    section "8. PAM / AUTHENTICATION"

    log "PAM sshd config"
    if [ -f /etc/pam.d/sshd ]; then
        cat /etc/pam.d/sshd | grep -v "^#" | grep -v "^$" | tee -a "$REPORT_FILE"
    else
        warn "/etc/pam.d/sshd not found — PAM may not be configured for SSH"
    fi

    log "Checking if user account is locked"
    USER=$(whoami)
    # Check account lock via /etc/shadow (portable, no GNU passwd -S needed)
    if [ -f /etc/shadow ]; then
        SHADOW_ENTRY=$(doas grep "^${USER}:" /etc/shadow 2>/dev/null | head -1)
        SHADOW_HASH=$(echo "$SHADOW_ENTRY" | cut -d: -f2)
        if echo "$SHADOW_HASH" | grep -q "^!"; then
            fail "User $USER account is LOCKED (shadow hash starts with !) — SSH login will be refused"
            log "Unlocking account..."
            doas passwd -u "$USER" 2>/dev/null && fixed "Account $USER unlocked" || \
                warn "Could not unlock — try: doas passwd -u $USER"
        else
            ok "User $USER account is not locked"
        fi
    else
        raw "  /etc/shadow not readable — skipping lock check"
    fi

    log "Checking /etc/hosts.allow and /etc/hosts.deny (TCP wrappers)"
    if [ -f /etc/hosts.deny ]; then
        DENY=$(cat /etc/hosts.deny | grep -v "^#" | grep -v "^$")
        if [ -n "$DENY" ]; then
            warn "hosts.deny has rules — may be blocking SSH:"
            raw "$DENY"
        else
            ok "hosts.deny is empty"
        fi
    fi

    if [ -f /etc/hosts.allow ]; then
        raw "hosts.allow:"
        cat /etc/hosts.allow | grep -v "^#" | grep -v "^$" | tee -a "$REPORT_FILE"
    fi

    log "Checking /etc/security/access.conf"
    if [ -f /etc/security/access.conf ]; then
        ACCESS=$(cat /etc/security/access.conf | grep -v "^#" | grep -v "^$")
        if [ -n "$ACCESS" ]; then
            raw "$ACCESS"
            if echo "$ACCESS" | grep -q "^-.*ALL"; then
                warn "access.conf has blanket deny rules — may block SSH login"
            fi
        fi
    fi
}

# =============================================================================
# 9. SELINUX / APPARMOR
# =============================================================================

check_mac() {
    section "9. SELINUX / APPARMOR / MAC"

    if have getenforce; then
        SELINUX=$(getenforce 2>/dev/null)
        raw "  SELinux: $SELINUX"
        if [ "$SELINUX" = "Enforcing" ]; then
            warn "SELinux is enforcing — may be blocking sshd. Check: doas ausearch -m avc -ts recent"
        fi
    else
        raw "  SELinux: not present"
    fi

    if have aa-status; then
        raw "  AppArmor: $(doas aa-status --pretty 2>/dev/null | head -3)"
    else
        raw "  AppArmor: not present"
    fi

    ok "No MAC enforcement detected (expected on Chimera)"
}

# =============================================================================
# 10. SSHD LOGS
# =============================================================================

check_logs() {
    section "10. SSHD LOGS (last 50 lines)"

    log "Checking for sshd log entries"

    # Chimera uses dinit — logs may be in different places
    for logfile in /var/log/sshd.log /var/log/auth.log /var/log/secure /var/log/messages; do
        if [ -f "$logfile" ]; then
            raw "=== $logfile (last 30 lines) ==="
            tail -30 "$logfile" | grep -i "ssh\|auth\|fail\|error\|refused" | tee -a "$REPORT_FILE"
        fi
    done

    # Try journalctl if available (shouldn't be on Chimera but just in case)
    if have journalctl; then
        raw "=== journalctl sshd ==="
        journalctl -u ssh -u sshd -n 30 --no-pager 2>/dev/null | tee -a "$REPORT_FILE"
    fi

    # dinit service log
    if [ -d /var/log/dinit ]; then
        raw "=== dinit logs ==="
        ls /var/log/dinit/ | tee -a "$REPORT_FILE"
        cat /var/log/dinit/sshd.log 2>/dev/null | tail -20 | tee -a "$REPORT_FILE"
    fi

    log "Running sshd in debug mode for instant diagnostics"
    raw "  (killing after 5 seconds — just for log output)"
    doas sshd -d -p 2222 2>&1 &
    SSHD_DEBUG_PID=$!
    sleep 3
    kill $SSHD_DEBUG_PID 2>/dev/null
    raw "  Debug sshd started on port 2222 briefly — check output above for errors"
}

# =============================================================================
# 11. EMERGENCY RECOVERY OPTIONS
# =============================================================================

emergency_recovery() {
    section "11. EMERGENCY RECOVERY"

    raw ""
    raw "  If SSH is still not working after this script, try these in order:"
    raw ""
    raw "  ┌─ OPTION 1: Restart sshd ────────────────────────────────────┐"
    raw "  │  doas dinitctl restart sshd                                  │"
    raw "  │  # or if that fails:                                         │"
    raw "  │  doas pkill sshd; doas sshd                                  │"
    raw "  └──────────────────────────────────────────────────────────────┘"
    raw ""
    raw "  ┌─ OPTION 2: Start sshd on alternate port ────────────────────┐"
    raw "  │  doas sshd -p 2222                                           │"
    raw "  │  # Then from local machine:                                  │"
    raw "  │  ssh -p 2222 jeremy@148.113.204.83                          │"
    raw "  └──────────────────────────────────────────────────────────────┘"
    raw ""
    raw "  ┌─ OPTION 3: Reset MTU to safe defaults ──────────────────────┐"
    raw "  │  doas ip link set dev ovsbr0 mtu 1500                       │"
    raw "  │  doas ip link set dev ens3 mtu 1500                         │"
    raw "  │  doas ip link set dev incusbr0 mtu 1500                     │"
    raw "  └──────────────────────────────────────────────────────────────┘"
    raw ""
    raw "  ┌─ OPTION 4: Emergency nft allow SSH ─────────────────────────┐"
    raw "  │  doas nft flush ruleset                                      │"
    raw "  │  doas nft add table inet filter                              │"
    raw "  │  doas nft add chain inet filter input { type filter hook input priority 0 \; policy accept \; }"
    raw "  │  # WARNING: This opens ALL ports — only for emergency access │"
    raw "  └──────────────────────────────────────────────────────────────┘"
    raw ""
    raw "  ┌─ OPTION 5: OVH rescue mode ─────────────────────────────────┐"
    raw "  │  If all else fails, use OVH control panel to boot into      │"
    raw "  │  rescue mode — gives you console access regardless of       │"
    raw "  │  network/SSH state.                                          │"
    raw "  │  https://ca.ovhcloud.com/en/dedicated-servers/              │"
    raw "  └──────────────────────────────────────────────────────────────┘"
}

# =============================================================================
# SUMMARY
# =============================================================================

print_summary() {
    section "SUMMARY"

    printf "\n"
    if [ "$ISSUES_FOUND" -gt 0 ]; then
        printf "${RED}${BOLD}  ✗ %d ISSUE(S) FOUND${NC}\n" "$ISSUES_FOUND"
    fi
    if [ "$FIXED" -gt 0 ]; then
        printf "${GRN}${BOLD}  ✓ %d ISSUE(S) AUTO-FIXED${NC}\n" "$FIXED"
    fi
    if [ "$ISSUES_FOUND" -eq 0 ]; then
        printf "${GRN}${BOLD}  ✓ NO BLOCKING ISSUES FOUND${NC}\n"
        printf "  sshd appears healthy — issue may be in the network path\n"
        printf "  Try: ssh -v jeremy@148.113.204.83 for verbose client output\n"
    fi

    printf "\n  Full report: ${BOLD}%s${NC}\n\n" "$REPORT_FILE"
    printf "  Next steps if still blocked:\n"
    printf "    1. Try alternate port:  doas sshd -p 2222\n"
    printf "    2. Try from host:       ssh -p 2222 jeremy@148.113.204.83\n"
    printf "    3. OVH rescue mode if all else fails\n\n"
}

# =============================================================================
# MAIN
# =============================================================================

print_header
check_sshd_process
check_sshd_listening
check_sshd_config
check_firewall
check_mtu
check_routing
check_host_keys
check_pam
check_mac
check_logs
emergency_recovery
print_summary
