# 3tched Control Plane + ghostbridge Mesh Identity — Requirements

**Version:** 1.0  
**Status:** Draft  
**Domains:** 3tched.com, ghostbridge.tech (and related)

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

---

## 2. VPS Public Port Exposure

### REQ-VPS-001: Minimal Public Port Surface
**Statement:** VPS public IP SHALL expose ONLY: port 443 (REALITY) and mail ports (465/587/993).

**Rationale:** Minimal attack surface; everything else is mesh-private.

**Acceptance Criteria:**
- [ ] External port scan shows only :443 and mail ports
- [ ] No other services bound to public interface
- [ ] Firewall drops all other inbound on public IP

### REQ-VPS-002: No Web Services on Public IP
**Statement:** VPS MUST NOT serve any owned web content (dashboard, API, marketing) on public IP.

**Rationale:** Web content is CF-only; VPS :443 is REALITY camouflage.

**Acceptance Criteria:**
- [ ] HTTP/HTTPS to VPS IP returns REALITY decoy or connection refused
- [ ] No owned domain content accessible via VPS public IP
- [ ] Browser to VPS:443 never sees owned service

---

## 3. REALITY/xray Configuration

### REQ-REALITY-001: Camouflage Only
**Statement:** xray REALITY on VPS :443 SHALL function ONLY as hostile-underlay camouflage tunnel, NOT as a browser HTTPS endpoint for owned services.

**Rationale:** REALITY masquerades as another site; owned services are mesh-private.

**Acceptance Criteria:**
- [ ] Browser connecting to VPS:443 gets decoy TLS response
- [ ] No owned service TLS certificates on REALITY listener
- [ ] REALITY serves camouflage, not content

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

### REQ-REALITY-004: Authorized Tunnel Path
**Statement:** Authorized subscribers SHALL access mesh via: xray client → REALITY tunnel → mesh interface → OpenFlow → services.

**Rationale:** All legitimate access traverses authenticated REALITY tunnel to mesh.

**Acceptance Criteria:**
- [ ] Subscribers connect using xray/REALITY client credentials
- [ ] Tunnel traffic emerges on mesh interface (not public)
- [ ] Mesh services reachable only through tunnel

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
- [ ] Only xray/REALITY handles public :443

### REQ-SNI-003: No Public TLS Certs for Owned Domains on VPS
**Statement:** Browser-trusted TLS certificates for owned domains (3tched.com, ghostbridge.tech, etc.) MUST NOT be deployed on VPS public edge.

**Rationale:** Would enable direct browser access; owned domains are CF-only or mesh-private.

**Acceptance Criteria:**
- [ ] No Let's Encrypt / ACME certs for owned web domains on VPS :443
- [ ] VPS :443 presents decoy cert only
- [ ] Owned domain certs exist only on CF edge or mesh-internal services

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

**Rationale:** Defense in depth; only authenticated tunnel users reach mesh.

**Acceptance Criteria:**
- [ ] Firewall blocks non-mesh source IPs from service ports
- [ ] Services reject connections from outside mesh range
- [ ] Public internet cannot reach mesh services

### REQ-MESH-003: WireGuard/NetMaker for Identity
**Statement:** Mesh identity SHALL be managed by NetMaker; all subscribers receive WireGuard credentials upon enrollment.

**Rationale:** Existing infrastructure; WG provides authenticated mesh access.

**Acceptance Criteria:**
- [ ] Enrollment creates NetMaker node
- [ ] Subscriber receives WG config
- [ ] WG connection grants mesh access

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

## 8. Backend Communication

### REQ-BACKEND-001: Email as Transport
**Statement:** CF-to-backend communication (e.g., registration triggers) SHALL use email transport, not live API connections.

**Rationale:** No persistent tunnels; async is acceptable for low-volume operations.

**Acceptance Criteria:**
- [ ] Registration triggers email to ingest address
- [ ] No REST/gRPC API exposed publicly for registration
- [ ] Backend processes email triggers

### REQ-BACKEND-002: Invisible to End User
**Statement:** The email transport between CF and backend SHALL be invisible infrastructure; end users see normal web experience.

**Rationale:** Implementation detail; user perceives normal website.

**Acceptance Criteria:**
- [ ] User sees standard web UI, not "check your email"
- [ ] No internal addresses exposed to users
- [ ] Latency appears as normal page load (seconds to minutes acceptable)

---

## 9. Explicit Non-Goals

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

---

## Assumptions

### ASSUMPTION-001: CF Return Path Mechanism
CF has a mechanism (Email Routing inbound, Workers polling, or native tool) to receive completion signals from backend and update user page. Specific implementation TBD.

### ASSUMPTION-002: VPS IP Stable
Public VPS IP (188.68.58.237) stable during implementation.

### ASSUMPTION-003: NetMaker API Available
NetMaker API functional for programmatic enrollment.

---

*Requirements focused on technical separation architecture: REALITY/xray, SNI constraints, OpenFlow, mail isolation, mesh privacy.*
