# 3tched Control Plane + ghostbridge Mesh Identity — Requirements

**Version:** 1.0  
**Status:** Draft  
**Domains:** 3tched.com, ghostbridge.tech

---

## 1. Public Surface

### REQ-PUB-001: Marketing Site Orange-Proxied Only
**Statement:** The public-facing marketing site for 3tched.com and ghostbridge.tech SHALL be served exclusively through Cloudflare orange-proxied DNS records.

**Rationale:** Hides origin IP; leverages CF DDoS/WAF; ensures public users never connect directly to VPS.

**Acceptance Criteria:**
- [ ] DNS A/AAAA records for marketing subdomains show CF proxy IPs (not 188.68.58.237)
- [ ] Direct connection to VPS IP on :80/:443 does NOT serve marketing content
- [ ] CF Analytics shows traffic flowing through proxy

### REQ-PUB-002: Subscribe/Registration UI Public Only
**Statement:** The subscribe/registration form SHALL be the ONLY control-plane-adjacent functionality exposed on the public Cloudflare surface.

**Rationale:** Minimizes attack surface; registration is async via email, not live API.

**Acceptance Criteria:**
- [ ] Registration form submits to CF-hosted/proxied endpoint only
- [ ] No public URLs expose dashboard, API, gRPC, or control-plane endpoints
- [ ] Form submission triggers email emission, NOT direct control-plane call

### REQ-PUB-003: No Public Control-Plane URLs
**Statement:** Anonymous internet users MUST NOT have a discoverable or functional path to private control-plane services (dashboard, gRPC, API, broker, qdrant, assistant).

**Rationale:** Control plane is mesh-private by design.

**Acceptance Criteria:**
- [ ] Port scan of VPS public IP shows only :443 (REALITY) and mail ports
- [ ] No DNS records resolve private service names to public IP
- [ ] Attempting to access control-plane paths from public internet returns connection refused or REALITY decoy

---

## 2. Email Control Channel

### REQ-EMAIL-001: Registration via Email-Triggered Channel
**Statement:** Cloudflare-to-control-plane communication for registration MUST use email as the transport, NOT live data streams (Tunnel, Workers TCP, REST/gRPC APIs).

**Rationale:** Async email avoids persistent public→private tunnels; registration is low-volume.

**Acceptance Criteria:**
- [ ] Registration form submission causes email to be sent to control-plane ingest address
- [ ] No CF Tunnel, Workers TCP proxy, or live WebSocket connects to control plane for registration
- [ ] Control plane has no public-facing registration API endpoint

### REQ-EMAIL-002: Email Hop Opaque to End User
**Statement:** End users MUST NOT see or know about the CF→control-plane email hop; it is opaque infrastructure.

**Rationale:** Implementation detail; user sees only "check email for join steps."

**Acceptance Criteria:**
- [ ] Registration confirmation UI says "check your email" (or equivalent), no mention of internal routing
- [ ] Email headers received by user do not expose ingest@ addresses or internal routing
- [ ] No error messages leak internal email addresses to users

### REQ-EMAIL-003: Structured Registration Payload
**Statement:** On subscribe submit, the system SHALL emit a structured registration email/event to a control-plane ingest address containing: email, timestamp, request metadata.

**Rationale:** Enables automated parsing and provisioning.

**Acceptance Criteria:**
- [ ] Ingest email contains machine-parseable payload (JSON in body or structured headers)
- [ ] Payload includes: subscriber email, submission timestamp, source domain
- [ ] Mail CT can filter and route these to provisioning pipeline

### REQ-EMAIL-004: Async Latency Acceptable
**Statement:** Registration latency of seconds to minutes SHALL be acceptable; the system MUST NOT require real-time response.

**Rationale:** Email delivery is inherently async; design embraces this.

**Acceptance Criteria:**
- [ ] User-facing copy sets expectation of "within minutes"
- [ ] No timeout errors if provisioning takes up to 5 minutes
- [ ] System functions correctly with 30-second to 5-minute email delivery delay

---

## 3. Mail Hosting (Half-and-Half)

### REQ-MAIL-001: CF Email Routing Owns Public MX
**Statement:** Cloudflare Email Routing SHALL own the public MX records for 3tched.com and ghostbridge.tech and forward inbound mail to the self-hosted mail CT.

**Rationale:** Hides mail server IP; leverages CF spam filtering on inbound.

**Acceptance Criteria:**
- [ ] MX records for both domains point to CF Email Routing endpoints
- [ ] CF Email Routing rules forward to mail CT destination
- [ ] Inbound mail arrives at mail CT via CF forward

### REQ-MAIL-002: Mail CT is Real Mailbox
**Statement:** The self-hosted mail CT (postfix/dovecot) SHALL be the authoritative mailbox store (IMAP) for both domains.

**Rationale:** Operators need real mailboxes, not just forwarding.

**Acceptance Criteria:**
- [ ] IMAP client can connect to mail CT and retrieve mail for @3tched.com and @ghostbridge.tech
- [ ] Mail persists on mail CT storage, not forwarded elsewhere
- [ ] Multiple mailboxes supported per domain

### REQ-MAIL-003: Mail CT is Registered Outbound SMTP
**Statement:** The mail CT SHALL be the sole registered outbound SMTP sender for 3tched.com and ghostbridge.tech branded From addresses.

**Rationale:** Single sender simplifies SPF/DKIM/DMARC alignment.

**Acceptance Criteria:**
- [ ] SPF records for both domains authorize only mail CT IP (and CF for inbound routing)
- [ ] DKIM signatures on outbound mail originate from mail CT
- [ ] DMARC policy aligns with mail CT as sole sender

### REQ-MAIL-004: Send/Reply as Branded Domains
**Statement:** Users and operators MUST be able to send and reply as @3tched.com and @ghostbridge.tech from the mail CT.

**Rationale:** Core functionality for branded communication.

**Acceptance Criteria:**
- [ ] Outbound mail with From: user@3tched.com delivers successfully
- [ ] Outbound mail with From: user@ghostbridge.tech delivers successfully
- [ ] Reply-to works correctly for both domains

### REQ-MAIL-005: No Gmail as Branded Sender
**Statement:** Gmail MUST NOT be configured as a sender or From identity for 3tched.com or ghostbridge.tech (no Gmail Send-mail-as; no Gmail in SPF as sender for branded From).

**Rationale:** Single authoritative sender; avoid SPF/DKIM conflicts.

**Acceptance Criteria:**
- [ ] SPF records do not include Gmail servers for branded domains
- [ ] No "Send mail as" configured in any Gmail account for these domains
- [ ] DMARC reports show no Gmail-originated mail for branded domains

### REQ-MAIL-006: Multi-Domain Demux
**Statement:** CF Email Routing rules and/or mail CT recipient/domain maps SHALL demux mail for multiple domains correctly.

**Rationale:** Two domains, one infrastructure.

**Acceptance Criteria:**
- [ ] Mail to user@3tched.com routes to correct mailbox
- [ ] Mail to user@ghostbridge.tech routes to correct mailbox
- [ ] Machine addresses (register@, ingest@) route per domain

### REQ-MAIL-007: Machine Addresses Trigger Provisioning
**Statement:** Machine addresses (e.g., register@, ingest@) SHALL land on mail CT and trigger control-plane provisioning pipeline.

**Rationale:** Email-driven enrollment automation.

**Acceptance Criteria:**
- [ ] Mail to register@3tched.com triggers provisioning
- [ ] Mail to ingest@ghostbridge.tech triggers provisioning
- [ ] Trigger mechanism is reliable (procmail/sieve/pipe)

### REQ-MAIL-008: Mail TLS on Standard Ports
**Statement:** Mail CT SHALL serve TLS on ports 465 (SMTPS), 587 (submission), and 993 (IMAPS) using ACME-provisioned certificates.

**Rationale:** Standard secure mail ports; automated cert renewal.

**Acceptance Criteria:**
- [ ] TLS handshake succeeds on :465, :587, :993
- [ ] Certificates are valid for mail.3tched.com / mail.ghostbridge.tech
- [ ] ACME renewal is automated (certbot/acme.sh)

### REQ-MAIL-009: Mail Not on REALITY :443
**Statement:** Mail TLS MUST NOT be terminated on the REALITY :443 listener.

**Rationale:** REALITY is for camouflage only; mail has separate ports.

**Acceptance Criteria:**
- [ ] REALITY config does not reference mail ports or mail domains
- [ ] Mail clients connect on :465/:587/:993, not :443
- [ ] xray config shows no mail-related inbounds

---

## 4. Mesh Privacy

### REQ-MESH-001: Private Services Mesh-Only
**Statement:** After subscribe/join, dashboard, control plane, gRPC, API, broker, qdrant, and assistant SHALL be accessible ONLY on the mesh (e.g., 10.0.0.0/8), NOT via public browser TLS on VPS :443.

**Rationale:** Core security model; mesh is authenticated perimeter.

**Acceptance Criteria:**
- [ ] Services bind to mesh IP only (e.g., 10.0.0.2), not 0.0.0.0
- [ ] Connection from public internet to these services fails
- [ ] Connection from mesh IP succeeds

### REQ-MESH-002: No Public SNI Demux for Private Names
**Statement:** Private service names MUST NOT be demuxed via public SNI on VPS :443.

**Rationale:** Avoids exposing private names on public interface.

**Acceptance Criteria:**
- [ ] No nginx/haproxy/xray SNI routing for private names on :443
- [ ] Private names not in any public TLS config
- [ ] SNI probe for private names on public :443 returns REALITY decoy or connection reset

### REQ-MESH-003: gRPC-Web UI Mesh-Private
**Statement:** The gRPC-web UI SHALL be mesh-private, linked from the private dashboard, targeting op-grpc-bridge on mesh (e.g., 10.0.0.2:8090).

**Rationale:** gRPC UI is operator tooling, not public.

**Acceptance Criteria:**
- [ ] gRPC-web UI binds to mesh IP only
- [ ] Dashboard link to gRPC UI uses mesh-internal URL
- [ ] Public access to gRPC-web UI returns connection refused

---

## 5. OpenFlow Routing

### REQ-OVS-001: Mesh Service Demux via OpenFlow
**Statement:** Known mesh services SHALL be demuxed by IP:port via OpenFlow managed flows on Open vSwitch.

**Rationale:** Fine-grained traffic control without public SNI exposure.

**Acceptance Criteria:**
- [ ] OpenFlow rules route mesh traffic to correct service by IP:port
- [ ] Rules are present in `ovs-ofctl dump-flows`
- [ ] Traffic to mesh services traverses OVS bridge

### REQ-OVS-002: Cookied Managed Flows
**Statement:** OpenFlow rules SHALL use cookies for identification and safe controller attach; rules MUST be managed (add/modify specific flows), never bulk delete-all.

**Rationale:** Prevents accidental network outage; enables safe incremental updates.

**Acceptance Criteria:**
- [ ] All managed flows have consistent cookie prefix
- [ ] Flow updates use `ovs-ofctl add-flow` or `mod-flows`, not `del-flows` without cookie filter
- [ ] No scripts contain `ovs-ofctl del-flows <bridge>` without cookie constraint

### REQ-OVS-003: Safe Controller Attach
**Statement:** OVS controller attachment SHALL be safe (fail-standalone or equivalent); control-plane failure MUST NOT break existing flows.

**Rationale:** Network resilience.

**Acceptance Criteria:**
- [ ] `ovs-vsctl get-fail-mode <bridge>` returns `standalone` or `secure` with appropriate config
- [ ] Existing flows persist if controller disconnects
- [ ] Manual verification: disconnect controller, verify traffic still flows

---

## 6. REALITY Camouflage

### REQ-REALITY-001: Hostile-Underlay Camouflage Only
**Statement:** REALITY on public :443 SHALL serve ONLY as hostile-underlay camouflage, NOT as a browser HTTPS endpoint for owned services.

**Rationale:** REALITY masquerades as another site; not for legitimate service hosting.

**Acceptance Criteria:**
- [ ] Browser connecting to VPS:443 with any owned domain SNI gets REALITY decoy, not owned service
- [ ] xray config inbounds show REALITY with external decoy serverName
- [ ] No owned service TLS certificates on REALITY listener

### REQ-REALITY-002: Single Innocuous Decoy
**Statement:** REALITY SHALL use a single innocuous decoy serverName (e.g., www.microsoft.com); owned service names MUST NOT appear in REALITY serverNames.

**Rationale:** Decoy must look legitimate; owned names would expose intent.

**Acceptance Criteria:**
- [ ] xray config `serverNames` contains only decoy domain(s)
- [ ] No 3tched.com, ghostbridge.tech, or related names in serverNames
- [ ] TLS fingerprint matches decoy site

### REQ-REALITY-003: Authorized Path Through Tunnel
**Statement:** Authorized access path SHALL be: REALITY tunnel → mesh → OpenFlow → services.

**Rationale:** Ensures all access traverses authenticated mesh.

**Acceptance Criteria:**
- [ ] Subscribers connect via xray/REALITY client
- [ ] Client traffic emerges on mesh interface
- [ ] OpenFlow routes mesh traffic to services

---

## 7. Identity / Enrollment

### REQ-ID-001: NetMaker for WireGuard Identity
**Statement:** NetMaker SHALL be the identity provider; all subscribers receive WireGuard identity upon enrollment.

**Rationale:** Existing infrastructure; WG provides mesh access.

**Acceptance Criteria:**
- [ ] Enrollment creates NetMaker node for subscriber
- [ ] Subscriber receives WG config (keys, endpoint, allowed IPs)
- [ ] WG connection grants mesh access

### REQ-ID-002: Provisioning Pipeline
**Statement:** Control plane mail consumer SHALL parse registration triggers and provision: NetMaker enrollment, WG grant, join instructions.

**Rationale:** Automated enrollment flow.

**Acceptance Criteria:**
- [ ] Ingest mail triggers provisioning script/service
- [ ] NetMaker API called to create enrollment
- [ ] Join instructions emailed to subscriber

### REQ-ID-003: Join Instructions to Subscriber
**Statement:** Upon successful provisioning, subscriber SHALL receive email with join instructions (WG config or enrollment link).

**Rationale:** Completes enrollment loop.

**Acceptance Criteria:**
- [ ] Subscriber receives email from @3tched.com or @ghostbridge.tech
- [ ] Email contains actionable join instructions
- [ ] Instructions work to establish mesh access

---

## 8. Observability / Ops

### REQ-OBS-001: Enrollment Event Logging
**Statement:** All enrollment events (request received, provisioned, failed) SHALL be logged with timestamp and subscriber identifier.

**Rationale:** Audit trail; troubleshooting.

**Acceptance Criteria:**
- [ ] Logs contain enrollment events
- [ ] Each event has timestamp and identifier
- [ ] Logs are queryable (journalctl or log file)

### REQ-OBS-002: Mail Delivery Monitoring
**Statement:** Inbound and outbound mail delivery SHALL be monitored for failures/delays.

**Rationale:** Email is critical path; failures must be detected.

**Acceptance Criteria:**
- [ ] Mail logs show delivery status
- [ ] Alerting on delivery failures (optional but recommended)
- [ ] Queue monitoring for stuck mail

### REQ-OBS-003: Mesh Connectivity Health
**Statement:** Mesh connectivity health SHALL be observable (WG handshake status, peer count).

**Rationale:** Mesh is core infrastructure.

**Acceptance Criteria:**
- [ ] `wg show` or NetMaker API shows peer status
- [ ] Health check endpoint or script exists
- [ ] Stale/disconnected peers identifiable

---

## 9. Security / Trust

### REQ-SEC-001: No CF Tunnel/Workers TCP for Control Plane
**Statement:** Cloudflare Tunnel, Workers TCP proxy, or durable data streams MUST NOT be used as the primary path into the control plane.

**Rationale:** Avoids persistent public→private tunnels; email is the control channel.

**Acceptance Criteria:**
- [ ] No cloudflared tunnel daemon running for control-plane ingress
- [ ] No Workers TCP proxy configured for control-plane endpoints
- [ ] CF dashboard shows no tunnels to control-plane services

### REQ-SEC-002: Trust Boundary at Mesh Edge
**Statement:** Trust boundary SHALL be at the mesh edge; unauthenticated traffic MUST NOT reach control-plane services.

**Rationale:** Defense in depth; mesh is authenticated perimeter.

**Acceptance Criteria:**
- [ ] Services reject connections from non-mesh IPs
- [ ] Firewall rules block non-mesh access to service ports
- [ ] Penetration test from public IP fails to reach services

### REQ-SEC-003: Enrollment Token Expiry
**Statement:** Enrollment tokens/links SHALL have expiry (recommended: 24-72 hours).

**Rationale:** Limits exposure window for leaked tokens.

**Acceptance Criteria:**
- [ ] Enrollment links contain expiry or are single-use
- [ ] Expired tokens rejected by provisioning system
- [ ] Expiry time is configurable

---

## 10. Verification

### REQ-VER-001: End-to-End Registration Test
**Statement:** End-to-end registration flow SHALL be verifiable: form submit → email → provisioning → join.

**Rationale:** Ensures full flow works.

**Acceptance Criteria:**
- [ ] Test subscriber can complete full flow
- [ ] Each step produces expected artifacts (email, NetMaker node, WG config)
- [ ] Subscriber gains mesh access

### REQ-VER-002: Mail Delivery Verification
**Statement:** Inbound and outbound mail delivery SHALL be verified for both domains.

**Rationale:** Mail is critical; must work end-to-end.

**Acceptance Criteria:**
- [ ] External sender can deliver to @3tched.com
- [ ] External sender can deliver to @ghostbridge.tech
- [ ] Outbound mail from both domains delivers to external recipients

### REQ-VER-003: Mesh Isolation Verification
**Statement:** Mesh isolation SHALL be verified: public internet cannot reach mesh-private services.

**Rationale:** Core security property.

**Acceptance Criteria:**
- [ ] External port scan shows no mesh service ports
- [ ] External connection attempts to mesh services fail
- [ ] Internal (mesh) connections succeed

---

## 11. Explicit Non-Goals

### NG-001: No nginx/haproxy SNI Front-Splitting
**Description:** Do NOT implement nginx/haproxy SNI front-splitting between dashboard and REALITY on public :443.

**Rationale:** Would expose private names on public interface.

### NG-002: No Public Wildcard Certs for Private Names
**Description:** Do NOT provision public browser-trusted wildcard certificates for private dashboard/API names on VPS edge.

**Rationale:** Private names should not be publicly resolvable or cert-backed.

### NG-003: No CF Data Streams for Subscribe
**Description:** Do NOT use CF Tunnel, Workers TCP, durable data streams, or live tunnels into control plane for subscribe flow.

**Rationale:** Email-triggered async is the design; no persistent tunnels.

### NG-004: No Gmail as Branded Sender
**Description:** Do NOT configure Gmail as a sender (Send-mail-as, SPF include) for 3tched.com or ghostbridge.tech.

**Rationale:** Single authoritative sender (mail CT) simplifies deliverability.

### NG-005: No Private Names in REALITY serverNames
**Description:** Do NOT add private service names to REALITY serverNames to "fix" browser cert issues.

**Rationale:** Would defeat camouflage purpose; private names stay private.

### NG-006: No Public REST/gRPC Registration API
**Description:** Do NOT expose a public REST or gRPC API endpoint for registration.

**Rationale:** Registration is email-triggered; no live API.

---

## Assumptions

### ASSUMPTION-001: Single VPS IP Stable
**Statement:** The public VPS IP (188.68.58.237) is assumed stable and will not change during implementation.

**Impact if wrong:** DNS records, firewall rules, and SPF records need update.

### ASSUMPTION-002: NetMaker API Available
**Statement:** NetMaker API is available and functional for programmatic enrollment.

**Impact if wrong:** Manual enrollment or alternative automation required.

### ASSUMPTION-003: CF Email Routing Reliable
**Statement:** Cloudflare Email Routing forwards mail reliably with minimal delay (< 60s typical).

**Impact if wrong:** Registration delays; may need direct MX fallback.

---

*Document generated for Kiro spec workflow. All REQ-* IDs are stable for task linkage.*
