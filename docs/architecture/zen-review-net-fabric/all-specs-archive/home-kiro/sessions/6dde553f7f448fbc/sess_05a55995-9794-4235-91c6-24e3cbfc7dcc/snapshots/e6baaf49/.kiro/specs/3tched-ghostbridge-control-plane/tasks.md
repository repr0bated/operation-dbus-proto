# 3tched Control Plane + ghostbridge Mesh Identity — Implementation Tasks

**Version:** 1.0  
**Status:** Draft  
**Traces to:** requirements.md, design.md

---

## Task Execution Order

Tasks are grouped by phase and ordered by dependency. Complete each phase before proceeding.

---

## Phase 1: DNS and Cloudflare Public Surface

### TASK-001: Configure CF DNS Orange-Proxy Records
**Description:** Set up Cloudflare DNS A/AAAA records for marketing domains with orange-cloud proxy enabled.

**Linked Requirements:** REQ-PUB-001, REQ-PUB-003

**Design Reference:** Section 6.1 DNS Records

**Dependencies:** None (starting task)

**Steps:**
1. In CF dashboard, add A record for `3tched.com` → CF proxy IP (orange)
2. Add CNAME `www.3tched.com` → `3tched.com` (orange)
3. Add A record for `ghostbridge.tech` → CF proxy IP (orange)
4. Add CNAME `www.ghostbridge.tech` → `ghostbridge.tech` (orange)
5. Add grey-cloud A records for `mail.3tched.com` and `mail.ghostbridge.tech` → 188.68.58.237

**Definition of Done:**
- [ ] `dig 3tched.com` returns CF proxy IP, not VPS IP
- [ ] `dig mail.3tched.com` returns 188.68.58.237
- [ ] Direct curl to VPS IP:443 does NOT serve marketing content



---

### TASK-002: Deploy Marketing Site to CF Pages/Workers
**Description:** Deploy static marketing site for both domains via Cloudflare Pages or Workers.

**Linked Requirements:** REQ-PUB-001, REQ-PUB-002

**Design Reference:** Section 1.1 System Components

**Dependencies:** TASK-001

**Steps:**
1. Create CF Pages project or Workers site
2. Deploy marketing content with registration form
3. Configure custom domains (3tched.com, ghostbridge.tech)
4. Verify HTTPS works via CF edge

**Definition of Done:**
- [ ] Marketing site accessible at https://3tched.com and https://ghostbridge.tech
- [ ] Registration form visible and functional
- [ ] CF Analytics shows traffic

---

### TASK-003: Implement Registration Form Email Emission
**Description:** Configure registration form to emit structured email to ingest address on submit.

**Linked Requirements:** REQ-EMAIL-001, REQ-EMAIL-002, REQ-EMAIL-003

**Design Reference:** Section 3.1 Subscribe Sequence (steps 3-6)

**Dependencies:** TASK-002

**Steps:**
1. Create form handler (CF Workers or Pages Function)
2. On submit: construct JSON payload (email, timestamp, source domain)
3. Send email to ingest@3tched.com (or appropriate ingest address)
4. Return user-facing "check your email" response
5. Ensure no internal addresses exposed in response

**Definition of Done:**
- [ ] Form submit triggers email to ingest address
- [ ] Email contains machine-parseable JSON payload
- [ ] User sees only "check your email" message
- [ ] No CF Tunnel or live API call to control plane



---

## Phase 2: Cloudflare Email Routing

### TASK-004: Configure CF Email Routing for Both Domains
**Description:** Set up Cloudflare Email Routing to forward inbound mail to self-hosted mail CT.

**Linked Requirements:** REQ-MAIL-001, REQ-MAIL-006

**Design Reference:** Section 3.2 Inbound Mail Flow

**Dependencies:** TASK-001 (DNS must be on CF)

**Steps:**
1. Enable Email Routing for 3tched.com in CF dashboard
2. Enable Email Routing for ghostbridge.tech in CF dashboard
3. Add destination address (mail CT forward endpoint)
4. Create catch-all or specific routing rules per domain
5. Create rules for machine addresses (register@, ingest@)

**Definition of Done:**
- [ ] MX records for both domains point to CF Email Routing
- [ ] Test email to user@3tched.com forwards to mail CT
- [ ] Test email to user@ghostbridge.tech forwards to mail CT
- [ ] Machine address routing rules in place

---

### TASK-005: Verify CF Email Routing Delivery
**Description:** End-to-end test CF Email Routing to mail CT delivery.

**Linked Requirements:** REQ-MAIL-001, REQ-EMAIL-001

**Design Reference:** Section 5.2 CF Forward Failure

**Dependencies:** TASK-004, TASK-007 (mail CT must be running)

**Steps:**
1. Send test email from external address to test@3tched.com
2. Verify arrival in mail CT logs
3. Send test email to test@ghostbridge.tech
4. Verify multi-domain demux works correctly
5. Check delivery latency (should be < 60s typical)

**Definition of Done:**
- [ ] External emails arrive at mail CT for both domains
- [ ] Delivery latency < 60 seconds
- [ ] CF Email Routing dashboard shows successful deliveries



---

## Phase 3: Mail CT Setup

### TASK-006: Provision Mail CT Container
**Description:** Create Incus container for mail services (Postfix/Dovecot).

**Linked Requirements:** REQ-MAIL-002, REQ-MAIL-003

**Design Reference:** Section 1.1 System Components

**Dependencies:** None (can parallel with Phase 1)

**Steps:**
1. Create Incus container (Artix or compatible base)
2. Configure network (mesh IP + ability to receive forwarded mail)
3. Install postfix, dovecot, opendkim
4. Configure runit services for postfix, dovecot, opendkim

**Definition of Done:**
- [ ] Container running with runit supervision
- [ ] Postfix, Dovecot, OpenDKIM services defined
- [ ] Network connectivity verified

---

### TASK-007: Configure Postfix Multi-Domain
**Description:** Set up Postfix to handle mail for both 3tched.com and ghostbridge.tech.

**Linked Requirements:** REQ-MAIL-002, REQ-MAIL-004, REQ-MAIL-006

**Design Reference:** Section 9.2 Postfix Main Config Points

**Dependencies:** TASK-006

**Steps:**
1. Configure `mydestination` for both domains
2. Set up virtual alias maps for domain routing
3. Configure mailbox locations
4. Create machine address aliases (register@, ingest@ → pipeline)
5. Test local delivery

**Definition of Done:**
- [ ] Mail to user@3tched.com delivers locally
- [ ] Mail to user@ghostbridge.tech delivers locally
- [ ] Machine addresses configured in virtual maps

---

### TASK-008: Configure Mail CT TLS/ACME
**Description:** Set up ACME certificates for mail ports (465/587/993).

**Linked Requirements:** REQ-MAIL-008, REQ-MAIL-009

**Design Reference:** Section 6.2 Certificate Strategy

**Dependencies:** TASK-006, TASK-001 (DNS grey-cloud records)

**Steps:**
1. Install certbot or acme.sh
2. Obtain certificates for mail.3tched.com and mail.ghostbridge.tech
3. Configure Postfix to use certificates (:465, :587)
4. Configure Dovecot to use certificates (:993)
5. Set up automatic renewal cron/timer

**Definition of Done:**
- [ ] TLS handshake succeeds on :465, :587, :993
- [ ] Certificate valid for mail hostnames
- [ ] Renewal automation configured
- [ ] Mail NOT on REALITY :443 (separate ports only)



---

### TASK-009: Configure SPF Records
**Description:** Set up SPF DNS records authorizing mail CT as sole sender.

**Linked Requirements:** REQ-MAIL-003, REQ-MAIL-005

**Design Reference:** Section 9.3 SPF/DKIM/DMARC Records

**Dependencies:** TASK-001

**Steps:**
1. Add TXT record for 3tched.com: `v=spf1 ip4:188.68.58.237 -all`
2. Add TXT record for ghostbridge.tech: `v=spf1 ip4:188.68.58.237 -all`
3. Verify NO Gmail include in SPF
4. Test with SPF checker tool

**Definition of Done:**
- [ ] SPF records present for both domains
- [ ] Records authorize only VPS IP (mail CT)
- [ ] No Gmail servers in SPF
- [ ] SPF checker passes

---

### TASK-010: Configure DKIM Signing
**Description:** Set up OpenDKIM for outbound mail signing.

**Linked Requirements:** REQ-MAIL-003

**Design Reference:** Section 9.3 SPF/DKIM/DMARC Records

**Dependencies:** TASK-006, TASK-007

**Steps:**
1. Generate DKIM keys for both domains
2. Configure OpenDKIM with key table and signing table
3. Add DKIM TXT records to DNS for both domains
4. Configure Postfix milter integration
5. Test outbound signing

**Definition of Done:**
- [ ] DKIM keys generated and configured
- [ ] DNS TXT records for _domainkey published
- [ ] Outbound mail shows valid DKIM signature
- [ ] DKIM checker passes for both domains

---

### TASK-011: Configure DMARC Policy
**Description:** Set up DMARC records for both domains.

**Linked Requirements:** REQ-MAIL-003, REQ-MAIL-005

**Design Reference:** Section 9.3 SPF/DKIM/DMARC Records

**Dependencies:** TASK-009, TASK-010

**Steps:**
1. Add DMARC TXT record for _dmarc.3tched.com
2. Add DMARC TXT record for _dmarc.ghostbridge.tech
3. Set policy to reject (after testing with none/quarantine)
4. Configure RUA address for reports

**Definition of Done:**
- [ ] DMARC records present for both domains
- [ ] Policy set appropriately (start with p=none, migrate to p=reject)
- [ ] Reports configured to aggregate address
- [ ] DMARC checker passes



---

### TASK-012: Test Outbound Mail Delivery
**Description:** Verify outbound mail from both domains delivers successfully.

**Linked Requirements:** REQ-MAIL-004, REQ-VER-002

**Design Reference:** Section 3.3 Outbound Mail Flow

**Dependencies:** TASK-008, TASK-009, TASK-010, TASK-011

**Steps:**
1. Send test email from user@3tched.com to external recipient
2. Verify delivery and check headers (SPF, DKIM, DMARC pass)
3. Send test email from user@ghostbridge.tech to external recipient
4. Verify delivery and check headers
5. Test reply flow works

**Definition of Done:**
- [ ] Outbound mail from both domains delivers to external recipients
- [ ] SPF, DKIM, DMARC all pass in headers
- [ ] Reply-to works correctly
- [ ] No Gmail involved in send path

---

## Phase 4: Email Ingest Pipeline

### TASK-013: Create Ingest Mailbox Trigger
**Description:** Configure machine addresses to trigger provisioning pipeline on mail arrival.

**Linked Requirements:** REQ-MAIL-007, REQ-EMAIL-001

**Design Reference:** Section 3.1 Subscribe Sequence (steps 7-9)

**Dependencies:** TASK-007

**Steps:**
1. Create ingest@ mailbox or alias
2. Configure Postfix pipe transport or Dovecot sieve for trigger
3. Implement trigger script that invokes provisioning
4. Test trigger fires on mail arrival

**Definition of Done:**
- [ ] Mail to ingest@3tched.com triggers script
- [ ] Mail to register@ghostbridge.tech triggers script (if used)
- [ ] Trigger reliable (logged, error handling)

---

### TASK-014: Implement Registration Payload Parser
**Description:** Create parser for structured registration email payload.

**Linked Requirements:** REQ-EMAIL-003, REQ-ID-002

**Design Reference:** Section 3.1 Subscribe Sequence (step 10)

**Dependencies:** TASK-013

**Steps:**
1. Define payload schema (JSON: email, timestamp, source_domain)
2. Implement parser (shell/python script)
3. Validate required fields
4. Handle malformed payloads gracefully (log and discard)
5. Output parsed data for provisioning

**Definition of Done:**
- [ ] Parser extracts email, timestamp, source_domain from payload
- [ ] Invalid payloads logged and rejected
- [ ] Parser output consumable by provisioning step



---

### TASK-015: Implement NetMaker Enrollment Grant
**Description:** Create provisioning logic to enroll subscriber in NetMaker and generate WG config.

**Linked Requirements:** REQ-ID-001, REQ-ID-002

**Design Reference:** Section 3.1 Subscribe Sequence (steps 11-13)

**Dependencies:** TASK-014, NetMaker running

**Steps:**
1. Implement NetMaker API client
2. Create enrollment function: generate node, get WG config
3. Generate enrollment token with expiry (24-72h)
4. Store enrollment state for tracking
5. Handle API failures gracefully

**Definition of Done:**
- [ ] NetMaker API call creates enrollment
- [ ] WG config or enrollment link generated
- [ ] Token has expiry
- [ ] Failures logged, don't crash pipeline

---

### TASK-016: Implement Join Instructions Emailer
**Description:** Send enrollment completion email with WG config/instructions to subscriber.

**Linked Requirements:** REQ-ID-003

**Design Reference:** Section 3.1 Subscribe Sequence (steps 14-15)

**Dependencies:** TASK-015, TASK-007 (outbound mail working)

**Steps:**
1. Create email template for join instructions
2. Include WG config or enrollment link
3. Send from appropriate @domain address
4. Log send success/failure

**Definition of Done:**
- [ ] Subscriber receives email with join instructions
- [ ] Email sent from branded address (@3tched.com or @ghostbridge.tech)
- [ ] Instructions actionable (WG config works, or link valid)

---

## Phase 5: Mesh Service Binding

### TASK-017: Bind Control Plane Services to Mesh IP
**Description:** Configure dashboard, API, broker, qdrant, assistant to bind to mesh IP only.

**Linked Requirements:** REQ-MESH-001, REQ-MESH-003

**Design Reference:** Section 2.3 What is Public vs Mesh

**Dependencies:** Mesh network functional

**Steps:**
1. Audit each service's bind address configuration
2. Change bind from 0.0.0.0 to mesh IP (e.g., 10.0.0.x)
3. Restart services
4. Verify services reject non-mesh connections

**Definition of Done:**
- [ ] Dashboard binds to mesh IP only
- [ ] API binds to mesh IP only
- [ ] gRPC bridge binds to 10.0.0.2:8090
- [ ] Connection from public IP refused
- [ ] Connection from mesh IP succeeds



---

### TASK-018: Configure Firewall for Mesh Isolation
**Description:** Set up firewall rules to block non-mesh access to private services.

**Linked Requirements:** REQ-MESH-001, REQ-SEC-002

**Design Reference:** Section 2.2 Trust Boundary Definitions

**Dependencies:** TASK-017

**Steps:**
1. Define allowed ports from public (443 REALITY, 465/587/993 mail)
2. Block all other ports from public interface
3. Allow mesh range (10.0.0.0/8) to access service ports
4. Log blocked attempts (optional)

**Definition of Done:**
- [ ] `nmap` from external shows only :443 and mail ports
- [ ] Service ports unreachable from public IP
- [ ] Mesh clients can reach services

---

## Phase 6: OpenFlow Configuration

### TASK-019: Define OpenFlow Cookie Convention
**Description:** Establish cookie prefix and management rules for OVS flows.

**Linked Requirements:** REQ-OVS-002

**Design Reference:** Section 4.2 Cookie Convention

**Dependencies:** None (documentation task)

**Steps:**
1. Choose cookie prefix (e.g., 0x3tched or numeric)
2. Document in ops runbook
3. Create helper scripts for safe flow operations
4. Add safeguard: script refuses del-flows without cookie

**Definition of Done:**
- [ ] Cookie convention documented
- [ ] Helper scripts created (add-flow, del-flow with cookie)
- [ ] No scripts contain unfiltered del-flows

---

### TASK-020: Implement Mesh Service OpenFlow Rules
**Description:** Add OpenFlow rules to route mesh traffic to services by IP:port.

**Linked Requirements:** REQ-OVS-001, REQ-OVS-002

**Design Reference:** Section 4.1 Flow Architecture

**Dependencies:** TASK-019, TASK-017 (services bound)

**Steps:**
1. Backup existing flows: `ovs-ofctl dump-flows br-mesh > flows.backup`
2. Add flow for op-grpc-bridge (10.0.0.2:8090)
3. Add flows for other mesh services
4. Verify with `ovs-ofctl dump-flows`
5. Test traffic routing

**Definition of Done:**
- [ ] Flows present with correct cookie
- [ ] Traffic to mesh IPs routes to correct output port
- [ ] `ovs-ofctl dump-flows` shows managed flows



---

### TASK-021: Configure OVS Fail-Mode Standalone
**Description:** Set OVS bridge fail-mode to standalone for resilience.

**Linked Requirements:** REQ-OVS-003

**Design Reference:** Section 4.3 Safe Controller Attach

**Dependencies:** OVS bridge exists

**Steps:**
1. Check current fail-mode: `ovs-vsctl get-fail-mode br-mesh`
2. Set to standalone: `ovs-vsctl set-fail-mode br-mesh standalone`
3. Verify setting persists
4. Test: disconnect controller, verify flows persist

**Definition of Done:**
- [ ] `ovs-vsctl get-fail-mode br-mesh` returns `standalone`
- [ ] Flows persist when controller disconnects
- [ ] Traffic continues to flow during controller outage

---

## Phase 7: REALITY Configuration

### TASK-022: Audit REALITY serverNames
**Description:** Verify REALITY config uses only innocuous decoy, no owned names.

**Linked Requirements:** REQ-REALITY-001, REQ-REALITY-002

**Design Reference:** Section 9.1 xray/REALITY Config

**Dependencies:** xray config exists

**Steps:**
1. Read /etc/xray/xray_config.json (in container)
2. Check `serverNames` array in REALITY settings
3. Verify contains ONLY decoy domain (e.g., www.microsoft.com)
4. Remove any 3tched.com, ghostbridge.tech, or related names
5. Verify no owned names in `dest` or related fields

**Definition of Done:**
- [ ] `serverNames` contains only decoy domain(s)
- [ ] No 3tched/ghostbridge names anywhere in REALITY config
- [ ] Browser probe to VPS:443 with owned SNI gets decoy response

---

### TASK-023: Trim REALITY serverNames to Single Decoy
**Description:** Reduce REALITY serverNames to single innocuous decoy if multiple exist.

**Linked Requirements:** REQ-REALITY-002

**Design Reference:** Section 9.1 xray/REALITY Config

**Dependencies:** TASK-022

**Steps:**
1. If multiple serverNames, choose single best decoy
2. Update xray_config.json
3. Restart xray: `sudo sv restart xray`
4. Verify tunnel still works for authorized clients
5. Verify decoy response for unauthorized probes

**Definition of Done:**
- [ ] Single serverName in config
- [ ] Authorized REALITY clients can still connect
- [ ] Unauthorized probes get decoy response

---

## Phase 8: Private gRPC UI

### TASK-024: Configure gRPC-Web UI Mesh Binding
**Description:** Ensure gRPC-web UI binds to mesh IP and is linked from private dashboard.

**Linked Requirements:** REQ-MESH-003

**Design Reference:** Section 2.3 What is Public vs Mesh

**Dependencies:** TASK-017

**Steps:**
1. Verify gRPC-web UI binds to mesh IP (not 0.0.0.0)
2. Configure to target op-grpc-bridge at 10.0.0.2:8090
3. Add link to gRPC UI from private dashboard
4. Test access from mesh client

**Definition of Done:**
- [ ] gRPC-web UI accessible only from mesh
- [ ] UI targets op-grpc-bridge correctly
- [ ] Dashboard has working link to gRPC UI
- [ ] Public access returns connection refused
