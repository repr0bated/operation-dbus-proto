# 3tched Control Plane + ghostbridge Mesh Identity — Tasks

**Version:** 1.2  
**Status:** Draft  
**Traces to:** requirements.md, design.md

---

## Revision 1.2

- **FIXED TASK-004:** Concrete mechanism — MUST use CF Email Workers/Email Routing outbound, MUST NOT HTTP/gRPC/WebSocket to VPS; added REQ-ID-005 link
- **FIXED TASK-018:** Updated for decoy/NetMaker enrollment — human WG terminates at Oracle decoy, not host wg-lan
- **ADDED:** E2E note — HumanPrincipal/assertion path verified under handoff mission, not this task list
- **ADDED:** Reference to adjacent spec for identity topology

## Revision 1.1

- **FIXED TASK-002:** Now links REQ-SEC-001 (added to requirements.md)
- **FIXED TASK-016/021:** WireGuard is primary; REALITY is optional separate check
- **ADDED:** Phase for Identity/Enrollment tasks (form→email→provision→join mail)
- **ADDED:** CF Email Routing rules for human + machine addresses
- **ADDED:** Private gRPC-web UI task
- **ADDED:** E2E verification with correct flow (user receives join email, not CF return path)

---

## Phase 1: Cloudflare DNS/Proxy Surface

### TASK-001: Configure Orange-Proxied DNS
**Description:** Set up CF DNS with orange-cloud proxy for public domains, grey-cloud for mail.

**Linked REQs:** REQ-CF-001, REQ-VPS-002

**Dependencies:** None

**Steps:**
1. Set A record `3tched.com` → CF proxy (orange)
2. Set A record `ghostbridge.tech` → CF proxy (orange)
3. Set CNAME `www.*` → root (orange)
4. Set A record `mail.3tched.com` → 188.68.58.237 (grey)
5. Set A record `mail.ghostbridge.tech` → 188.68.58.237 (grey)

**Verification:**
- [ ] `dig 3tched.com` returns CF IP, not VPS
- [ ] `dig mail.3tched.com` returns VPS IP
- [ ] Browser to 3tched.com goes through CF

---

### TASK-002: Verify No CF Tunnels Exist
**Description:** Confirm no cloudflared tunnels connect to VPS control plane.

**Linked REQs:** REQ-CF-002, REQ-SEC-001

**Dependencies:** None

**Steps:**
1. Check CF dashboard → Zero Trust → Tunnels
2. Verify no tunnels point to VPS or control-plane services
3. Check VPS for cloudflared process: `pgrep cloudflared`

**Verification:**
- [ ] CF dashboard shows no relevant tunnels
- [ ] No cloudflared running on VPS
- [ ] No Workers TCP proxies configured

---

## Phase 2: CF Email Routing

### TASK-003: Configure CF Email Routing Rules
**Description:** Set up CF Email Routing for both domains, routing to mail CT. Distinguish human mailboxes from machine addresses.

**Linked REQs:** REQ-CF-003, REQ-CF-004, REQ-MAIL-005

**Dependencies:** TASK-001

**Steps:**
1. Enable Email Routing for 3tched.com in CF dashboard
2. Enable Email Routing for ghostbridge.tech in CF dashboard
3. Add forwarding destination: mail CT (NOT Gmail)
4. Create rules for human mailboxes (catch-all or specific)
5. Create rules for machine addresses (register@, ingest@)
6. Verify no Gmail addresses in destinations

**Verification:**
- [ ] MX records point to CF Email Routing
- [ ] Test email to user@3tched.com forwards to mail CT
- [ ] Test email to ingest@3tched.com forwards to mail CT
- [ ] CF Email Routing destinations list shows NO Gmail
- [ ] Both domains route correctly

---

### TASK-004: Implement Subscribe Form Email Trigger
**Description:** Configure CF registration form to emit structured email to ingest address (no CP URL).

**Linked REQs:** REQ-ID-001, REQ-CF-002

**Dependencies:** TASK-003

**Steps:**
1. Create form handler (CF Pages Function or Workers)
2. On submit: construct JSON payload (subscriber email, timestamp, source domain)
3. Send email to ingest@3tched.com (or appropriate address)
4. Return user-facing "Thanks, check your email for join steps"
5. Ensure no control-plane URLs in response or form action

**Verification:**
- [ ] Form submit triggers email to ingest address
- [ ] Email contains machine-parseable JSON payload
- [ ] User sees "check your email" confirmation
- [ ] No direct connection to VPS
- [ ] No CP URLs exposed

---

## Phase 3: REALITY/xray Configuration (Optional)

### TASK-005: Audit REALITY serverNames
**Description:** Verify xray config has single decoy serverName, no owned domains.

**Linked REQs:** REQ-REALITY-002, REQ-REALITY-003, REQ-SNI-003

**Dependencies:** xray installed (if REALITY enabled)

**Steps:**
1. Read `/etc/xray/xray_config.json`
2. Check `serverNames` array
3. Grep for owned domains: `grep -iE "3tched|ghostbridge" /etc/xray/xray_config.json`
4. Verify single innocuous decoy (e.g., www.microsoft.com)

**Verification:**
- [ ] serverNames contains exactly one entry
- [ ] Entry is innocuous external site
- [ ] No owned domains anywhere in config

---

### TASK-006: Trim serverNames to Single Decoy
**Description:** If multiple serverNames exist, reduce to single decoy.

**Linked REQs:** REQ-REALITY-002

**Dependencies:** TASK-005

**Steps:**
1. Edit xray_config.json
2. Set `serverNames: ["www.microsoft.com"]` (or chosen decoy)
3. Verify `dest` matches decoy
4. Restart xray: `sudo sv restart xray`
5. Test authorized tunnel still works (for those using REALITY)

**Verification:**
- [ ] Config has single serverName
- [ ] Authorized REALITY clients can connect
- [ ] Probe returns decoy response

---

### TASK-007: Verify No SNI Demux on :443
**Description:** Confirm only xray handles :443 (if enabled), no nginx/haproxy SNI splitting.

**Linked REQs:** REQ-SNI-001, REQ-SNI-002

**Dependencies:** None

**Steps:**
1. Check what listens on :443: `ss -tlnp | grep :443`
2. Verify only xray process (or nothing if REALITY disabled)
3. Check no nginx config for :443: `grep -r "listen.*443" /etc/nginx/`
4. Check no haproxy config for :443

**Verification:**
- [ ] Only xray on :443 (or port closed)
- [ ] No nginx :443 listeners
- [ ] No haproxy :443 listeners

---

### TASK-008: Test REALITY Decoy Response
**Description:** Verify SNI probe with owned domain returns decoy, not owned cert.

**Linked REQs:** REQ-REALITY-001, REQ-REALITY-003, REQ-SNI-001

**Dependencies:** TASK-006

**Steps:**
1. Probe with owned SNI: `openssl s_client -connect 188.68.58.237:443 -servername 3tched.com`
2. Check returned certificate (should be decoy's cert)
3. Probe with decoy SNI: `openssl s_client -connect 188.68.58.237:443 -servername www.microsoft.com`
4. Compare results (should be identical)

**Verification:**
- [ ] Owned SNI returns decoy cert
- [ ] No owned domain cert exposed
- [ ] SNI value doesn't change response

---

## Phase 4: VPS Port Surface

### TASK-009: Audit VPS Public Ports
**Description:** Verify only :443 (if REALITY) and mail ports exposed publicly.

**Linked REQs:** REQ-VPS-001

**Dependencies:** None

**Steps:**
1. External port scan: `nmap -p- 188.68.58.237` (from outside)
2. Check firewall rules: `iptables -L -n` or `nft list ruleset`
3. List listening sockets: `ss -tlnp`

**Verification:**
- [ ] Only :443 (optional), :465, :587, :993 open externally
- [ ] Other ports filtered/closed
- [ ] Firewall rules match intent

---

### TASK-010: Configure Firewall Rules
**Description:** Ensure firewall drops non-essential inbound on public IP.

**Linked REQs:** REQ-VPS-001, REQ-MESH-002

**Dependencies:** TASK-009

**Steps:**
1. Allow :443 (REALITY, if enabled)
2. Allow :465, :587, :993 (mail)
3. Allow mesh range for service ports
4. Drop all other inbound on public interface
5. Persist rules

**Verification:**
- [ ] Rules applied
- [ ] External scan confirms
- [ ] Mesh traffic still flows

---

## Phase 5: Mail CT Configuration

### TASK-011: Configure Mail CT TLS/ACME
**Description:** Set up ACME certs for mail.* subdomains on mail ports.

**Linked REQs:** REQ-MAIL-001, REQ-SNI-003

**Dependencies:** TASK-001 (grey-cloud DNS)

**Steps:**
1. Install certbot or acme.sh
2. Obtain cert for mail.3tched.com
3. Obtain cert for mail.ghostbridge.tech (or SAN cert)
4. Configure postfix to use certs (:465, :587)
5. Configure dovecot to use certs (:993)
6. Set up renewal automation

**Verification:**
- [ ] TLS works on :465, :587, :993
- [ ] Cert valid for mail.* hostnames
- [ ] Renewal cron/timer configured
- [ ] Certs are for mail.* (not web domains on :443)

---

### TASK-012: Configure SPF Records
**Description:** Set SPF to authorize only VPS IP, explicitly exclude Gmail.

**Linked REQs:** REQ-MAIL-003, REQ-MAIL-004

**Dependencies:** TASK-001

**Steps:**
1. Add TXT `3tched.com`: `v=spf1 ip4:188.68.58.237 -all`
2. Add TXT `ghostbridge.tech`: `v=spf1 ip4:188.68.58.237 -all`
3. Verify no Gmail include
4. Test with SPF checker

**Verification:**
- [ ] SPF records present
- [ ] Only VPS IP authorized
- [ ] No Gmail in SPF
- [ ] SPF checker passes

---

### TASK-013: Configure DKIM
**Description:** Set up OpenDKIM for outbound signing.

**Linked REQs:** REQ-MAIL-003

**Dependencies:** TASK-011

**Steps:**
1. Generate DKIM keys for both domains
2. Configure OpenDKIM
3. Add DKIM TXT records to DNS
4. Configure postfix milter
5. Test outbound signing

**Verification:**
- [ ] DKIM keys generated
- [ ] DNS TXT records published
- [ ] Outbound mail has valid DKIM signature

---

### TASK-014: Configure DMARC
**Description:** Set DMARC policy for both domains.

**Linked REQs:** REQ-MAIL-003

**Dependencies:** TASK-012, TASK-013

**Steps:**
1. Add TXT `_dmarc.3tched.com`: `v=DMARC1; p=reject; rua=mailto:dmarc@3tched.com`
2. Add TXT `_dmarc.ghostbridge.tech`: similar
3. Start with p=none, migrate to p=reject after testing

**Verification:**
- [ ] DMARC records present
- [ ] Test mail shows DMARC pass
- [ ] Reports configured

---

### TASK-015: Verify Mail NOT on REALITY
**Description:** Confirm xray config has no mail-related inbounds.

**Linked REQs:** REQ-MAIL-002

**Dependencies:** TASK-005

**Steps:**
1. Check xray config for mail references
2. Verify no :465/:587/:993 in xray inbounds
3. Verify mail.* not in REALITY config

**Verification:**
- [ ] No mail ports in xray config
- [ ] Mail clients connect directly to mail CT ports
- [ ] REALITY doesn't handle mail

---

### TASK-016: Test Human Send/Reply
**Description:** Verify operators can IMAP retrieve and SMTP send/reply as branded domains.

**Linked REQs:** REQ-MAIL-006

**Dependencies:** TASK-011, TASK-012, TASK-013, TASK-014

**Steps:**
1. Configure mail client with IMAP :993 and SMTP :587
2. Send test email from user@3tched.com to external recipient
3. Verify delivery (check SPF/DKIM/DMARC in headers)
4. Reply to received email
5. Repeat for ghostbridge.tech

**Verification:**
- [ ] IMAP retrieval works for both domains
- [ ] SMTP submission works for both domains
- [ ] Outbound mail delivers with SPF/DKIM/DMARC pass
- [ ] Reply-to works correctly

---

## Phase 6: Identity / Enrollment Pipeline

### TASK-017: Implement Mail Ingest Consumer
**Description:** Create pipeline that consumes ingest mail and triggers provisioning.

**Linked REQs:** REQ-ID-002, REQ-MAIL-005

**Dependencies:** TASK-003

**Steps:**
1. Configure postfix pipe transport or dovecot sieve for ingest@/register@
2. Implement trigger script that parses JSON payload
3. Validate required fields (subscriber email, timestamp, source)
4. Pass to provisioning on success; log and reject on failure
5. Test with sample ingest email

**Verification:**
- [ ] Mail to ingest@ triggers script
- [ ] Payload parsed correctly
- [ ] Invalid payloads logged and rejected
- [ ] Valid payloads forwarded to provisioning

---

### TASK-018: Implement NetMaker Enrollment Grant
**Description:** Create provisioning logic to enroll subscriber in NetMaker and generate WG config.

**Linked REQs:** REQ-ID-003, REQ-MESH-003

**Dependencies:** TASK-017, NetMaker running

**Steps:**
1. Implement NetMaker API client
2. Create enrollment function: generate node, get WG config
3. Generate enrollment token with expiry (24-72h)
4. Store enrollment state for tracking
5. Handle API failures gracefully

**Verification:**
- [ ] NetMaker API call creates enrollment
- [ ] WG config or enrollment link generated
- [ ] Token has expiry
- [ ] Failures logged, don't crash pipeline

---

### TASK-019: Implement Join Instructions Emailer
**Description:** Send join instructions email from mail CT to subscriber.

**Linked REQs:** REQ-ID-004, REQ-MAIL-003

**Dependencies:** TASK-018, TASK-016 (outbound mail working)

**Steps:**
1. Create email template for join instructions
2. Include WG config or enrollment link
3. Send from @3tched.com or @ghostbridge.tech (based on source)
4. Log send success/failure

**Verification:**
- [ ] Subscriber receives email with join instructions
- [ ] Email sent from branded address
- [ ] SPF/DKIM/DMARC pass on delivery
- [ ] Instructions actionable (WG config works)

---

## Phase 7: Mesh Service Binding

### TASK-020: Bind Services to Mesh IPs
**Description:** Configure all private services to bind only to mesh interface.

**Linked REQs:** REQ-MESH-001

**Dependencies:** Mesh network functional

**Steps:**
1. Audit each service bind address
2. Change from 0.0.0.0 to 10.0.0.x for each
3. Restart services
4. Verify binding

**Verification:**
- [ ] `ss -tlnp` shows 10.x.x.x binds
- [ ] No 0.0.0.0 binds for private services
- [ ] Public IP connection refused

---

### TASK-021: Configure Private gRPC-Web UI
**Description:** Set up mesh-private gRPC-web UI linked from dashboard.

**Linked REQs:** REQ-MESH-004

**Dependencies:** TASK-020

**Steps:**
1. Deploy gRPC-web UI bound to mesh IP only
2. Configure to target op-grpc-bridge at 10.0.0.2:8090
3. Add link to gRPC UI from private dashboard
4. Test access from mesh client

**Verification:**
- [ ] gRPC-web UI binds to mesh IP only
- [ ] Dashboard has working link to gRPC UI
- [ ] gRPC UI reaches op-grpc-bridge
- [ ] Public access fails

---

### TASK-022: Test Mesh Isolation
**Description:** Verify public internet cannot reach mesh services.

**Linked REQs:** REQ-MESH-002, REQ-SEC-002

**Dependencies:** TASK-020, TASK-010

**Steps:**
1. From external host, try connecting to service ports on VPS IP
2. Verify all fail (connection refused or timeout)
3. From mesh (via WireGuard), verify services reachable

**Verification:**
- [ ] External connections to mesh services fail
- [ ] WireGuard connections succeed
- [ ] Trust boundary enforced

---

## Phase 8: OpenFlow Configuration

### TASK-023: Establish Cookie Convention
**Description:** Document and implement consistent cookie prefix for managed flows.

**Linked REQs:** REQ-OVS-002

**Dependencies:** None

**Steps:**
1. Choose cookie (e.g., 0x3tched = 0x33746368)
2. Document in ops runbook
3. Create helper scripts with cookie enforcement
4. Audit existing flows for cookie

**Verification:**
- [ ] Convention documented
- [ ] Scripts use cookie
- [ ] Existing flows migrated if needed

---

### TASK-024: Implement Service OpenFlow Rules
**Description:** Add OpenFlow rules for mesh service routing by IP:port.

**Linked REQs:** REQ-OVS-001, REQ-OVS-002

**Dependencies:** TASK-023, TASK-020

**Steps:**
1. Backup flows: `ovs-ofctl dump-flows br-mesh > flows.backup`
2. Add flow for op-grpc-bridge (10.0.0.2:8090)
3. Add flows for other services
4. Verify with dump-flows
5. Test traffic routing

**Verification:**
- [ ] Flows present with cookie
- [ ] Traffic routes correctly
- [ ] No flows without cookie

---

### TASK-025: Verify No Unsafe del-flows
**Description:** Audit all scripts/configs for unfiltered bulk flow deletion.

**Linked REQs:** REQ-OVS-003

**Dependencies:** None

**Steps:**
1. Grep codebase: `grep -r "del-flows" .`
2. Check each occurrence for cookie filter
3. Fix any unfiltered del-flows
4. Add CI check if possible

**Verification:**
- [ ] No unfiltered del-flows in scripts
- [ ] All deletions use cookie or match filter

---

### TASK-026: Configure OVS Fail-Mode Standalone
**Description:** Set fail-mode so flows persist during controller disconnect.

**Linked REQs:** REQ-OVS-004

**Dependencies:** OVS bridge exists

**Steps:**
1. Check: `ovs-vsctl get-fail-mode br-mesh`
2. Set: `ovs-vsctl set-fail-mode br-mesh standalone`
3. Verify persistence
4. Test by disconnecting controller

**Verification:**
- [ ] fail-mode is standalone
- [ ] Flows persist on controller disconnect
- [ ] Traffic continues flowing

---

## Phase 9: End-to-End Verification

### TASK-027: E2E Registration Flow Test
**Description:** Full end-to-end test: subscribe → ingest → provision → join mail → WG → mesh.

**Linked REQs:** REQ-ID-001 through REQ-ID-005, REQ-MESH-003

**Dependencies:** All previous tasks

**Steps:**
1. Submit registration form on CF site
2. Verify "check your email" confirmation
3. Verify ingest email arrives at mail CT
4. Verify provisioning creates NetMaker enrollment
5. Verify join email sent to subscriber's external email
6. Import WG config, connect
7. Verify mesh services accessible (dashboard, gRPC UI)
8. Confirm user never directly contacted VPS or saw CP URLs

**Verification:**
- [ ] Form submit works, shows "check email"
- [ ] Ingest mail arrives at mail CT
- [ ] NetMaker enrollment created
- [ ] Subscriber receives join email at their address
- [ ] WG config works, tunnel connects
- [ ] Dashboard accessible via mesh
- [ ] gRPC UI accessible via dashboard link
- [ ] No CP URLs exposed to user at any point

---

### TASK-028: REALITY Optional Path Test (Separate)
**Description:** Test REALITY tunnel path for hostile network scenarios.

**Linked REQs:** REQ-REALITY-001, REQ-REALITY-004

**Dependencies:** TASK-006, TASK-027

**Steps:**
1. Configure xray client with REALITY credentials
2. Connect via REALITY tunnel
3. Verify mesh services accessible through REALITY
4. Confirm this is optional, not required for WG-only users

**Verification:**
- [ ] REALITY tunnel connects
- [ ] Mesh services reachable via REALITY
- [ ] WG-only users unaffected (TASK-027 passed without REALITY)

---

### TASK-029: Full Verification Checklist
**Description:** Execute complete verification of separation architecture.

**Linked REQs:** All

**Dependencies:** All previous tasks

**Checklist:**
- [ ] DNS: public domains → CF, mail.* → VPS
- [ ] CF: no tunnels to control plane
- [ ] CF Email Routing: forwards to mail CT, no Gmail destinations
- [ ] REALITY (if enabled): single decoy, no owned names
- [ ] SNI: probe returns decoy for any SNI (or port closed)
- [ ] VPS ports: only :443 (optional) + mail
- [ ] Mail: ACME certs on :465/:587/:993, NOT on :443
- [ ] SPF/DKIM/DMARC: aligned, no Gmail
- [ ] Services: bind mesh-only
- [ ] Mesh: unreachable from public
- [ ] OpenFlow: cookied, fail-standalone, no unsafe del-flows
- [ ] gRPC UI: mesh-private, linked from dashboard
- [ ] E2E: user submits → checks email → gets join mail → WG works → mesh works
- [ ] User never sees CP URLs or knows about email hop infrastructure

---

*Tasks ordered by dependency. WireGuard is primary access; REALITY is optional. Complete each phase before proceeding.*
