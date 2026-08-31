# 3tched Control Plane + ghostbridge Mesh Identity — Requirements

**Version:** 1.2  
**Status:** Draft  
**Domains:** 3tched.com, ghostbridge.tech (and related)

---

## Revision 1.2

- **FIXED REQ-CF-001:** Softened overclaim — public *web* hostnames resolve to CF; mail.* and SPF MAY publish VPS IP by design
- **FIXED REQ-SEC-001:** Now absolute — no CF Tunnel/Workers TCP/live streams into CP at all (aligned with REQ-CF-002)
- **ADDED ASSUMPTION-003:** CF Email Workers (or equivalent CF-native outbound mail) available for form→email trigger
- **ADDED:** Adjacent Specs / Identity Topology section — locks human WG termination at Oracle decoy per handoff spec
- **UPDATED REQ-MESH-003/REQ-ID-003:** Clarified that "WG primary" = decoy termination, not host wg-lan; NetMaker = transport layer
- **ADDED NG-008:** Oracle assertion crypto is non-goal (owned by handoff spec)

## Revision 1.1

- **FIXED REQ-BACKEND-002:** User MAY see "check your email for join steps" — opaque means the CF→CP email hop, not the user-facing UX
- **REMOVED ASSUMPTION-001:** No CF Workers polling or live return path — completion is via outbound join email to subscriber
- **FIXED REALITY:** Now optional hostile-underlay camouflage, not mandatory subscriber path
- **CLARIFIED mail certs:** ACME for mail.* on :465/:587/:993 is required; forbidden is owned web domain certs on :443
- **ADDED:** Identity/enrollment section, mail demux, human send/reply, private gRPC UI, Gmail routing negative
- **ADDED:** REQ-SEC-001 (was missing, TASK-002 referenced it)

---

## 1. Public Surface Isolation (Cloudflare)

### REQ-CF-001: Cloudflare as Sole Public Interface
**Statement:** All public-facing web content (marketing, registration UI) SHALL be served exclusively through Cloudflare orange-proxied infrastructure.

**Rationale:** Users interact only with CF; VPS IP is never exposed for web traffic.

**Acceptance Criteria:**
- [ ] DNS A/AAAA for public sites return CF proxy IPs, not VPS IP
- [ ] No public web content served from VPS directly
- [ ] Users cannot discover VPS IP from normal web interaction

### REQ-CF-002: No Live Backend Connections
**Statement:** Cloudflare infrastructure MUST NOT maintain live connections (Tunnel, Workers TCP, WebSocket, REST/gRPC API) to the VPS control plane.

**Rationale:** CF and VPS backend communicate only via async email; no persistent tunnels.

**Acceptance Criteria:**
- [ ] No cloudflared daemon running for control-plane ingress
- [ ] No Workers TCP proxy to VPS endpoints
- [ ] CF dashboard shows zero tunnels to VPS services
- [ ] Backend communication uses email transport only

### REQ-CF-003: CF Email Routing for Inbound MX
**Statement:** Cloudflare Email Routing SHALL own the public MX records and forward inbound mail to the mail CT on VPS.

**Rationale:** Hides mail server IP; CF handles public MX.

**Acceptance Criteria:**
- [ ] MX records point to CF Email Routing
- [ ] CF forwards to mail CT destination
- [ ] Inbound mail arrives at mail CT via CF forward

### REQ-CF-004: No Gmail as CF Email Routing Destination
**Statement:** Gmail MUST NOT be configured as a destination in CF Email Routing for branded domains.

**Rationale:** Mail CT is sole mailbox; Gmail would bypass mail CT.

**Acceptance Criteria:**
- [ ] CF Email Routing destinations do not include Gmail addresses for @3tched.com or @ghostbridge.tech
- [ ] All inbound mail routes to mail CT, not Gmail

---

## 2. VPS Public Port Exposure

### REQ-VPS-001: Minimal Public Port Surface
**Statement:** VPS public IP SHALL expose ONLY: port 443 (REALITY, optional) and mail ports (465/587/993).

**Rationale:** Minimal attack surface; everything else is mesh-private.

**Acceptance Criteria:**
- [ ] External port scan shows only :443 and mail ports
- [ ] No other services bound to public interface
- [ ] Firewall drops all other inbound on public IP

### REQ-VPS-002: No Web Services on Public IP
**Statement:** VPS MUST NOT serve any owned web content (dashboard, API, marketing) on public IP.

**Rationale:** Web content is CF-only; VPS :443 is REALITY camouflage only.

**Acceptance Criteria:**
- [ ] HTTP/HTTPS to VPS IP returns REALITY decoy or connection refused
- [ ] No owned domain content accessible via VPS public IP
- [ ] Browser to VPS:443 never sees owned service

---

## 3. REALITY/xray Configuration (Optional Camouflage)

### REQ-REALITY-001: Camouflage Only
**Statement:** xray REALITY on VPS :443 SHALL function ONLY as optional hostile-underlay camouflage tunnel, NOT as a browser HTTPS endpoint for owned services, and NOT as the primary subscriber access path.

**Rationale:** REALITY is for hostile networks; primary access is WireGuard mesh.

**Acceptance Criteria:**
- [ ] Browser connecting to VPS:443 gets decoy TLS response
- [ ] No owned service TLS certificates on REALITY listener
- [ ] REALITY serves camouflage, not content
- [ ] Subscribers not required to use REALITY client

### REQ-REALITY-002: Single Innocuous Decoy ServerName
**Statement:** REALITY serverNames configuration SHALL contain ONLY a single innocuous decoy domain (e.g., www.microsoft.com).

**Rationale:** Decoy must look legitimate; multiple or owned names expose intent.

**Acceptance Criteria:**
- [ ] xray config serverNames contains exactly one decoy domain
- [ ] Decoy is a high-traffic innocuous site
- [ ] TLS fingerprint matches decoy site

### REQ-REALITY-003: No Owned Names in ServerNames
**Statement:** Owned domain names (3tched.com, ghostbridge.tech, any related) MUST NOT appear in REALITY serverNames.

**Rationale:** Owned names in serverNames would expose the VPS as related to those domains.

**Acceptance Criteria:**
- [ ] grep of xray config shows no owned domains in serverNames
- [ ] SNI probe for owned names returns decoy, not owned cert
- [ ] No owned domain TLS certs loaded by xray

### REQ-REALITY-004: Optional Tunnel Path
**Statement:** REALITY tunnel MAY be used as an alternative access path for subscribers in hostile network environments; it is NOT required.

**Rationale:** WireGuard is primary; REALITY is optional camouflage for censored networks.

**Acceptance Criteria:**
- [ ] Subscribers can access mesh via WireGuard without REALITY
- [ ] REALITY path available for those who need it
- [ ] Documentation clarifies REALITY is optional

---

## 4. SNI and TLS Constraints

### REQ-SNI-001: No SNI Demux on Public :443
**Statement:** VPS public :443 MUST NOT perform SNI-based demultiplexing to route different domains to different backends.

**Rationale:** SNI demux would expose owned domain names on public interface.

**Acceptance Criteria:**
- [ ] No nginx/haproxy/xray SNI routing rules on :443
- [ ] All :443 traffic handled identically (REALITY decoy)
- [ ] SNI value does not affect routing on public port

### REQ-SNI-002: No nginx/haproxy SNI Front
**Statement:** nginx, haproxy, or similar SNI-splitting proxies MUST NOT be deployed on VPS public :443.

**Rationale:** Would expose private service names on public interface.

**Acceptance Criteria:**
- [ ] No nginx listening on public :443
- [ ] No haproxy listening on public :443
- [ ] Only xray/REALITY handles public :443 (if enabled)

### REQ-SNI-003: No Public Web TLS Certs for Owned Domains
**Statement:** Browser-trusted TLS certificates for owned web domains (dashboard, API, etc.) MUST NOT be deployed on VPS public :443. ACME certs for mail.* subdomains on mail ports ARE required.

**Rationale:** Web domains are CF-only or mesh-private; mail needs proper certs on dedicated ports.

**Acceptance Criteria:**
- [ ] No Let's Encrypt / ACME certs for owned web domains on VPS :443
- [ ] VPS :443 presents decoy cert only (or closed if REALITY disabled)
- [ ] ACME certs for mail.3tched.com / mail.ghostbridge.tech exist on :465/:587/:993

---

## 5. Mail Infrastructure

### REQ-MAIL-001: Mail CT on Dedicated Ports
**Statement:** Mail CT (postfix/dovecot) SHALL serve TLS on ports 465 (SMTPS), 587 (submission), 993 (IMAPS) with ACME certificates for mail.* subdomains.

**Rationale:** Standard mail ports, separate from REALITY; proper certs for mail clients.

**Acceptance Criteria:**
- [ ] TLS works on :465, :587, :993
- [ ] Certs valid for mail.3tched.com, mail.ghostbridge.tech
- [ ] ACME renewal automated

### REQ-MAIL-002: Mail NOT on REALITY :443
**Statement:** Mail services MUST NOT be terminated or proxied through REALITY :443.

**Rationale:** Mail has dedicated ports; REALITY is camouflage only.

**Acceptance Criteria:**
- [ ] xray config has no mail-related inbounds
- [ ] Mail clients connect on :465/:587/:993, not :443
- [ ] REALITY config has no mail domain references

### REQ-MAIL-003: Outbound SMTP Direct from Mail CT
**Statement:** Outbound mail for @3tched.com and @ghostbridge.tech SHALL originate directly from mail CT, NOT via Gmail or CF.

**Rationale:** Single authoritative sender; SPF/DKIM/DMARC alignment.

**Acceptance Criteria:**
- [ ] SPF records authorize only VPS IP for sending
- [ ] DKIM signatures from mail CT keys
- [ ] No Gmail servers in SPF
- [ ] No Gmail "Send mail as" configured

### REQ-MAIL-004: No Gmail as Branded Sender
**Statement:** Gmail MUST NOT be configured as a sender identity for 3tched.com or ghostbridge.tech.

**Rationale:** Avoids SPF/DKIM conflicts; mail CT is sole sender.

**Acceptance Criteria:**
- [ ] No Gmail in SPF records
- [ ] No Gmail "Send mail as" for these domains
- [ ] DMARC reports show no Gmail-originated mail

### REQ-MAIL-005: Multi-Domain Demux
**Statement:** CF Email Routing rules and/or mail CT recipient maps SHALL correctly route mail for both domains and distinguish human mailboxes from machine addresses.

**Rationale:** Two domains, multiple address types, one infrastructure.

**Acceptance Criteria:**
- [ ] Mail to user@3tched.com routes to user mailbox
- [ ] Mail to user@ghostbridge.tech routes to user mailbox
- [ ] Mail to register@/ingest@ routes to provisioning pipeline
- [ ] Human and machine addresses handled appropriately

### REQ-MAIL-006: Human Send/Reply Capability
**Statement:** Operators and users SHALL be able to read mail via IMAP and send/reply via SMTP submission as @3tched.com and @ghostbridge.tech from mail CT.

**Rationale:** Not just SPF theory — actual working send/reply.

**Acceptance Criteria:**
- [ ] IMAP client can connect and retrieve mail for both domains
- [ ] SMTP submission (:587) works for both domains
- [ ] Sent mail delivers successfully with valid SPF/DKIM/DMARC
- [ ] Reply-to works correctly

---

## 6. Mesh Privacy

### REQ-MESH-001: Private Services Mesh-Only Binding
**Statement:** All control-plane services (dashboard, API, gRPC, broker, qdrant, assistant) SHALL bind ONLY to mesh IP addresses (e.g., 10.0.0.0/8).

**Rationale:** Services unreachable from public internet; mesh is trust boundary.

**Acceptance Criteria:**
- [ ] Services bind to 10.x.x.x, not 0.0.0.0
- [ ] Connection from public IP fails
- [ ] Connection from mesh IP succeeds

### REQ-MESH-002: Trust Boundary at Mesh Edge
**Statement:** The security trust boundary SHALL be at mesh edge; unauthenticated traffic MUST NOT reach control-plane services.

**Rationale:** Defense in depth; only WireGuard-authenticated users reach mesh.

**Acceptance Criteria:**
- [ ] Firewall blocks non-mesh source IPs from service ports
- [ ] Services reject connections from outside mesh range
- [ ] Public internet cannot reach mesh services

### REQ-MESH-003: WireGuard/NetMaker as Primary Identity
**Statement:** Mesh identity SHALL be managed by NetMaker; all subscribers receive WireGuard credentials upon enrollment. WireGuard is the PRIMARY access method.

**Rationale:** Existing infrastructure; WG provides authenticated mesh access without requiring REALITY.

**Acceptance Criteria:**
- [ ] Enrollment creates NetMaker node
- [ ] Subscriber receives WG config
- [ ] WG connection grants mesh access
- [ ] REALITY not required for mesh access

### REQ-MESH-004: Private gRPC-Web UI
**Statement:** gRPC-web UI SHALL be mesh-private, bound to mesh IP, and linked from the private dashboard targeting op-grpc-bridge at 10.0.0.2:8090.

**Rationale:** gRPC UI is operator tooling, not public.

**Acceptance Criteria:**
- [ ] gRPC-web UI binds to mesh IP only
- [ ] Dashboard contains link to gRPC UI
- [ ] gRPC UI targets op-grpc-bridge at 10.0.0.2:8090
- [ ] Public access to gRPC-web UI fails

---

## 7. OpenFlow Routing

### REQ-OVS-001: IP:Port Based Demux
**Statement:** Mesh service routing SHALL use OpenFlow rules matching IP:port, NOT SNI or domain names.

**Rationale:** L3/L4 routing; no domain exposure; works with encrypted traffic.

**Acceptance Criteria:**
- [ ] OpenFlow rules match destination IP and port
- [ ] No L7/SNI inspection in flow rules
- [ ] Traffic routes correctly by IP:port

### REQ-OVS-002: Cookied Managed Flows
**Statement:** All managed OpenFlow rules SHALL use a consistent cookie prefix for identification.

**Rationale:** Enables safe incremental updates; prevents accidental bulk deletion.

**Acceptance Criteria:**
- [ ] All managed flows have cookie (e.g., 0x3tched)
- [ ] Scripts use cookie filter for modifications
- [ ] `ovs-ofctl dump-flows` shows consistent cookies

### REQ-OVS-003: No Bulk Flow Deletion
**Statement:** OpenFlow management MUST NOT use unfiltered bulk deletion (no `ovs-ofctl del-flows <bridge>` without cookie constraint).

**Rationale:** Prevents accidental network outage.

**Acceptance Criteria:**
- [ ] No scripts contain unfiltered del-flows
- [ ] All deletions specify cookie or match criteria
- [ ] Code review catches unsafe del-flows

### REQ-OVS-004: Safe Controller Attach
**Statement:** OVS fail-mode SHALL be standalone; controller disconnect MUST NOT break existing flows.

**Rationale:** Network resilience; flows persist during control-plane issues.

**Acceptance Criteria:**
- [ ] `ovs-vsctl get-fail-mode` returns standalone
- [ ] Flows persist when controller disconnects
- [ ] Traffic continues during controller outage

---

## 8. Identity / Enrollment

### REQ-ID-001: Registration via Email Trigger
**Statement:** Subscribe form submission SHALL emit a structured registration email to an ingest address; control-plane SHALL NOT expose a public REST/gRPC registration API.

**Rationale:** Email-triggered async; no live API on VPS edge.

**Acceptance Criteria:**
- [ ] Form submission triggers email to ingest@
- [ ] No public registration API endpoint on VPS
- [ ] Email contains structured payload (subscriber email, timestamp, source)

### REQ-ID-002: Control Plane Mail Consumer
**Statement:** Control plane SHALL consume ingest mail, parse registration payloads, and trigger provisioning.

**Rationale:** Email-driven automation.

**Acceptance Criteria:**
- [ ] Ingest mail triggers provisioning pipeline
- [ ] Payload parsed correctly
- [ ] Errors logged, invalid payloads rejected gracefully

### REQ-ID-003: NetMaker Enrollment Grant
**Statement:** Provisioning SHALL create NetMaker node and generate WireGuard enrollment for subscriber.

**Rationale:** Automated identity provisioning.

**Acceptance Criteria:**
- [ ] NetMaker API called to create enrollment
- [ ] WG config or enrollment link generated
- [ ] Enrollment has expiry (recommended 24-72h)

### REQ-ID-004: Join Instructions via Outbound Mail
**Statement:** Upon successful provisioning, mail CT SHALL send join instructions email to subscriber's email address as @3tched.com or @ghostbridge.tech.

**Rationale:** Completes enrollment loop; no CF return path needed.

**Acceptance Criteria:**
- [ ] Subscriber receives email from branded domain
- [ ] Email contains WG config or enrollment instructions
- [ ] Email delivers successfully (SPF/DKIM/DMARC pass)

### REQ-ID-005: User-Facing UX
**Statement:** End-user UX MAY display "thanks, check your email for join steps" after form submission. The opaque infrastructure is the CF→CP email hop, not the user-facing messaging.

**Rationale:** Async latency (seconds–minutes) is acceptable; user knows to check email.

**Acceptance Criteria:**
- [ ] User sees confirmation after form submit
- [ ] User told to check email for next steps
- [ ] No control-plane URLs or internal addresses exposed
- [ ] CF→CP email mechanism invisible to user

---

## 9. Security / Trust

### REQ-SEC-001: No CF Tunnel/Workers TCP for Control Plane
**Statement:** Cloudflare Tunnel, Workers TCP proxy, or durable data streams MUST NOT be used as the primary path into the control plane.

**Rationale:** Avoids persistent public→private tunnels; email is the control channel.

**Acceptance Criteria:**
- [ ] No cloudflared tunnel daemon for control-plane ingress
- [ ] No Workers TCP proxy configured
- [ ] CF dashboard shows no tunnels to control-plane services

### REQ-SEC-002: No Public Registration API
**Statement:** VPS MUST NOT expose public REST or gRPC endpoints for registration.

**Rationale:** Registration is email-triggered; no live API attack surface.

**Acceptance Criteria:**
- [ ] No registration API listening on public IP
- [ ] Registration only via email trigger

---

## 10. Explicit Non-Goals

### NG-001: No SNI Front-Splitting on :443
Do NOT implement nginx/haproxy/xray SNI-based routing on VPS public :443.

### NG-002: No Public Wildcard Certs for Private Names
Do NOT provision browser-trusted certs for dashboard/API names on VPS edge.

### NG-003: No CF Tunnels/Streams to Backend
Do NOT use CF Tunnel, Workers TCP, or live connections for registration or control-plane access.

### NG-004: No Gmail as Branded Sender
Do NOT configure Gmail as sender for @3tched.com or @ghostbridge.tech.

### NG-005: No Owned Names in REALITY serverNames
Do NOT add owned domains to REALITY serverNames to "fix" browser access.

### NG-006: No Public REST/gRPC Registration API
Do NOT expose public API endpoints on VPS for registration.

### NG-007: No Gmail as CF Email Routing Destination
Do NOT configure Gmail as a destination in CF Email Routing for branded domain mailboxes.

---

## Assumptions

### ASSUMPTION-001: VPS IP Stable
Public VPS IP (188.68.58.237) stable during implementation.

### ASSUMPTION-002: NetMaker API Available
NetMaker API functional for programmatic enrollment.

---

*Requirements focused on technical separation architecture: REALITY/xray optional camouflage, SNI constraints, OpenFlow, mail isolation, mesh privacy, email-triggered enrollment.*
