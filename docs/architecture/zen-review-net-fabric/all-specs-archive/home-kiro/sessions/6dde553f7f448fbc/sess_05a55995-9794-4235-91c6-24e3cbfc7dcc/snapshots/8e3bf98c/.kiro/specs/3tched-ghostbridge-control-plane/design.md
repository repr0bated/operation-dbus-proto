# 3tched Control Plane + ghostbridge Mesh Identity — Design

**Version:** 1.2  
**Status:** Draft  
**Implements:** requirements.md

---

## Revision 1.2

- **FIXED:** Trust diagram updated — human WG terminates at Oracle decoy, not VPS host wg-lan
- **ADDED:** Adjacent Specs section with identity topology lock (cross-ref handoff spec)
- **ADDED:** E2E verification note — HumanPrincipal/assertion path verified under handoff mission, not this task list
- **CLARIFIED:** NetMaker = decoy↔host transport layer, not human identity plane

## Revision 1.1

- **FIXED:** REALITY is optional camouflage, not mandatory subscriber path
- **FIXED:** Trust diagram shows WireGuard as primary, REALITY as optional
- **ADDED:** Subscribe sequence mermaid diagram
- **ADDED:** Failure modes for mail delay/loss, enrollment token expiry, ingest parse failure, join-mail delivery
- **CLARIFIED:** mail.* ACME certs on :465/:587/:993 are required; forbidden is web domain certs on :443
- **KEPT:** OVS safety rules (cookied flows, no bulk delete, fail-standalone)

---

## 1. Architecture: The Separation

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           PUBLIC INTERNET                                   │
│                                                                             │
│   Users see:                                                                │
│   • Cloudflare-served websites (3tched.com, ghostbridge.tech)              │
│   • "Thanks, check your email for join steps"                              │
│   • No knowledge of VPS or email-triggered backend                         │
└─────────────────────────────────────────────────────────────────────────────┘
                │                                    │
                │ HTTPS (orange-proxied)             │ Probe/Attack
                ▼                                    ▼
┌───────────────────────────┐          ┌─────────────────────────────────────┐
│      CLOUDFLARE           │          │         VPS 188.68.58.237           │
│                           │          │                                     │
│  • DNS (orange-proxied)   │          │  :443 ─► xray/REALITY (OPTIONAL)    │
│  • Marketing site         │          │          • Single decoy SNI         │
│  • Registration UI        │          │          • NO owned domain names    │
│  • Email Routing (MX)     │          │          • Camouflage only          │
│  • WAF/DDoS               │          │          • For hostile networks     │
│                           │          │                                     │
│  NO tunnels to VPS ──X    │          │  :465/587/993 ─► Mail CT            │
│  NO live API to VPS ─X    │          │          • ACME certs (mail.*)      │
│                           │          │          • Inbound via CF forward   │
│                           │          │          • Outbound direct          │
└───────────────────────────┘          │          • Join mail to subscriber  │
         │                             │                                     │
         │ Email forward               │  ALL OTHER PORTS ─► CLOSED          │
         │ (ingest trigger)            └─────────────────────────────────────┘
         ▼                                                │
┌─────────────────────────────────────────────────────────┼─────────────────┐
│                              VPS INTERNAL               │                 │
│                                                         │                 │
│  ┌─────────────┐      ┌─────────────────────────────────┼───────────────┐ │
│  │  Mail CT    │      │            MESH (10.0.0.0/8)    │               │ │
│  │             │      │                                 │               │ │
│  │ • Postfix   │      │  WireGuard ─────────────────────┘ (PRIMARY)     │ │
│  │ • Dovecot   │      │  REALITY tunnel ────────────────  (OPTIONAL)    │ │
│  │ • Ingest    │      │                                                 │ │
│  │   pipeline  │──────│► Control Plane (provisioning)                   │ │
│  │             │      │                                                 │ │
│  │ Join mail ◄─│──────│─ sends to subscriber                            │ │
│  │ outbound    │      │                                                 │ │
│  └─────────────┘      │  ┌──────────┐ ┌──────────┐ ┌──────────────────┐ │ │
│                       │  │Dashboard │ │ API      │ │ op-grpc-bridge   │ │ │
│                       │  │10.0.0.x  │ │10.0.0.x  │ │ 10.0.0.2:8090    │ │ │
│                       │  └──────────┘ └──────────┘ └──────────────────┘ │ │
│                       │       │                            ▲            │ │
│                       │       └── gRPC-web UI link ────────┘            │ │
│                       │                                                 │ │
│                       │  ┌──────────┐ ┌──────────┐ ┌──────────────────┐ │ │
│                       │  │ Broker   │ │ Qdrant   │ │ Assistant        │ │ │
│                       │  │10.0.0.x  │ │10.0.0.x  │ │ 10.0.0.x         │ │ │
│                       │  └──────────┘ └──────────┘ └──────────────────┘ │ │
│                       │                                                 │ │
│                       │         ┌─────────────────────────┐             │ │
│                       │         │   Open vSwitch (OVS)    │             │ │
│                       │         │   • Cookied OpenFlow    │             │ │
│                       │         │   • IP:port routing     │             │ │
│                       │         │   • No SNI, no L7       │             │ │
│                       │         └─────────────────────────┘             │ │
│                       │                                                 │ │
│                       │  ┌──────────────────────────────────┐           │ │
│                       │  │         NetMaker                 │           │ │
│                       │  │   • WireGuard identity mgmt      │           │ │
│                       │  │   • Subscriber enrollment        │           │ │
│                       │  │   • PRIMARY access method        │           │ │
│                       │  └──────────────────────────────────┘           │ │
│                       └─────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Trust Boundaries

```mermaid
flowchart TB
    subgraph UNTRUSTED["UNTRUSTED ZONE"]
        BROWSER[Browser/User]
        ATTACKER[Attacker/Probe]
    end

    subgraph CF_ZONE["CLOUDFLARE ZONE (Public Face)"]
        CF_WEB[Marketing + Registration UI]
        CF_EMAIL[Email Routing MX]
    end

    subgraph VPS_PUBLIC["VPS PUBLIC PORTS"]
        REALITY[:443 xray/REALITY<br/>OPTIONAL camouflage<br/>Single decoy SNI]
        MAIL_PORTS[:465/587/993<br/>Mail CT TLS<br/>ACME certs for mail.*]
    end

    subgraph ORACLE_DECOY["ORACLE DECOY (Human WG Termination)"]
        WG_DECOY[WireGuard Endpoint<br/>Human subscribers<br/>connect HERE]
    end

    subgraph VPS_PRIVATE["VPS PRIVATE (Mesh Only)"]
        MAIL_CT[Mail CT<br/>Ingest + Outbound]
        CP[Control Plane<br/>Provisioning]
        MESH_SERVICES[Dashboard, API, gRPC<br/>Broker, Qdrant, Assistant<br/>Bind: 10.0.0.x only]
        OVS[OpenFlow<br/>IP:port routing]
        NETMAKER[NetMaker<br/>Decoy↔Host Transport<br/>NOT human identity plane]
    end

    BROWSER -->|HTTPS| CF_WEB
    CF_WEB -.->|"Form submit →<br/>email trigger"| CF_EMAIL
    CF_EMAIL -->|Forward| MAIL_PORTS
    MAIL_PORTS --> MAIL_CT
    MAIL_CT -->|Ingest trigger| CP
    CP -->|Provision decoy enrollment| NETMAKER
    CP -->|Join mail| MAIL_CT
    MAIL_CT -.->|"Outbound to<br/>subscriber email"| BROWSER

    ATTACKER -->|Probe :443| REALITY
    REALITY -->|Decoy response| ATTACKER

    subgraph AUTHENTICATED["AUTHENTICATED SUBSCRIBER"]
        WG_USER[WireGuard client<br/>PRIMARY<br/>Connects to Oracle decoy]
        REALITY_USER[xray client<br/>OPTIONAL]
    end

    WG_USER -->|WireGuard tunnel| WG_DECOY
    WG_DECOY -->|NetMaker transport| NETMAKER
    NETMAKER --> OVS
    REALITY_USER -.->|REALITY tunnel<br/>hostile networks| REALITY
    REALITY -.-> OVS
    OVS -->|IP:port match| MESH_SERVICES

    ATTACKER -.->|BLOCKED| MESH_SERVICES
    BROWSER -.->|BLOCKED| MESH_SERVICES
```

### Trust Boundary Definitions

| Boundary | What Crosses | What's Blocked |
|----------|--------------|----------------|
| Public Internet → CF | HTTPS to marketing/registration | Everything else |
| Public Internet → VPS :443 | TLS probe (gets decoy) | Owned domain access |
| Public Internet → VPS mail | Authenticated SMTP/IMAP, CF forward | Unauthenticated |
| CF → VPS backend | Email only (async, one-way trigger) | Tunnels, APIs, streams, return paths |
| VPS → Subscriber | Outbound join email from mail CT | Live connections |
| WireGuard → Oracle Decoy | Human subscriber traffic (PRIMARY) | Unauthenticated |
| Oracle Decoy → Host | NetMaker transport (mesh addressing) | Direct human WG to host |
| REALITY → Mesh | Authenticated subscriber traffic (OPTIONAL) | Unauthenticated |
| Mesh → Services | OpenFlow-routed by IP:port | Non-mesh source IPs |

---

## Adjacent Specs / Identity Topology

**Cross-reference:** `.kiro/specs/netmaker-xray-identity-handoff/`

### Topology Lock (Authoritative)

```
Human Subscriber
      │
      │ WireGuard (PRIMARY)
      ▼
┌─────────────────┐
│  Oracle Decoy   │  ◄── Human WG terminates HERE
│  (identity      │
│   endpoint)     │
└────────┬────────┘
         │
         │ NetMaker transport
         │ (mesh addressing)
         ▼
┌─────────────────┐
│  VPS Host       │  ◄── NO direct human WG here
│  (services,     │
│   control plane)│
└─────────────────┘
```

### Responsibility Split

| This Spec (Control Plane) | Handoff Spec (Identity) |
|---------------------------|-------------------------|
| Email enrollment flow | Oracle assertion crypto |
| Mail CT operations | HumanPrincipal verification |
| Public CF surface | op-grpc-bridge auth middleware |
| REALITY camouflage ops | Identity authority logic |
| NetMaker transport provisioning | Application-level auth |

### Non-Goal for This Spec

Oracle assertion cryptography and HumanPrincipal token verification are **explicitly out of scope** — owned by `.kiro/specs/netmaker-xray-identity-handoff/`.

---

## 3. Subscribe Sequence

```mermaid
sequenceDiagram
    participant U as User Browser
    participant CF as Cloudflare<br/>(Site + Email Routing)
    participant MCT as Mail CT<br/>(VPS)
    participant CP as Control Plane
    participant NM as NetMaker
    participant UE as User Email

    Note over U,UE: REGISTRATION FLOW

    U->>CF: 1. Visit marketing site
    CF-->>U: 2. Serve registration form
    
    U->>CF: 3. Submit form (email, etc.)
    CF->>CF: 4. Form handler emits<br/>structured email to ingest@
    CF-->>U: 5. "Thanks, check your<br/>email for join steps"
    
    Note over CF,MCT: CF→CP EMAIL HOP (opaque to user)
    
    CF->>MCT: 6. CF Email Routing<br/>forwards to mail CT
    MCT->>MCT: 7. Deliver to ingest@ mailbox
    MCT->>CP: 8. Ingest pipeline triggers<br/>provisioning
    
    Note over CP,NM: PROVISIONING
    
    CP->>CP: 9. Parse payload, validate
    CP->>NM: 10. Create NetMaker enrollment
    NM-->>CP: 11. Return WG config
    
    Note over CP,UE: JOIN INSTRUCTIONS (outbound mail)
    
    CP->>MCT: 12. Compose join mail
    MCT->>UE: 13. Send as @3tched.com<br/>or @ghostbridge.tech
    
    Note over UE,NM: ENROLLMENT COMPLETION
    
    UE-->>U: 14. User receives join email
    U->>NM: 15. Import WG config, connect
    NM-->>U: 16. WireGuard tunnel up
    U->>CP: 17. Access mesh services
```

### Key Points
- **No CF return path:** Completion is via outbound mail to subscriber, not CF polling/webhook
- **User sees "check your email":** This is allowed and preferred
- **Opaque:** The CF→CP email hop, not the user-facing messaging
- **Async latency acceptable:** Seconds to minutes for email delivery

---

## 4. xray/REALITY Configuration (Optional)

### What REALITY Does
- Listens on VPS :443 (optional)
- Presents TLS that mimics a decoy site (e.g., www.microsoft.com)
- Unauthorized probes see decoy response
- Authorized clients (with REALITY credentials) establish tunnel
- Tunnel traffic exits onto mesh interface
- **For hostile network environments where WireGuard is blocked**

### Critical Configuration Rules

```
┌─────────────────────────────────────────────────────────────┐
│                    xray_config.json                         │
├─────────────────────────────────────────────────────────────┤
│  serverNames: ["www.microsoft.com"]     ◄── SINGLE DECOY   │
│                                                             │
│  NOT: ["www.microsoft.com", "3tched.com"]    ◄── WRONG     │
│  NOT: ["dashboard.3tched.com"]               ◄── WRONG     │
│  NOT: ["ghostbridge.tech"]                   ◄── WRONG     │
│                                                             │
│  dest: "www.microsoft.com:443"          ◄── DECOY ONLY     │
└─────────────────────────────────────────────────────────────┘
```

### REALITY is Optional
- Primary access: WireGuard via NetMaker
- REALITY: For subscribers in censored/hostile networks
- Not all subscribers need xray client
- WG-only subscribers are the common case

---

## 5. SNI Isolation

### The Problem We're Avoiding

```
WRONG APPROACH (SNI demux on public :443):
┌─────────────────────────────────────────────────────────┐
│  Browser sends SNI: dashboard.3tched.com                │
│                         │                               │
│                         ▼                               │
│  nginx/haproxy on :443 sees SNI                         │
│        │                     │                          │
│        ▼                     ▼                          │
│  SNI=dashboard.*        SNI=other                       │
│  → route to dashboard   → route to REALITY              │
│                                                         │
│  PROBLEM: Exposes that VPS serves dashboard.3tched.com │
└─────────────────────────────────────────────────────────┘
```

### The Correct Approach

```
CORRECT APPROACH (no SNI demux on public :443):
┌─────────────────────────────────────────────────────────┐
│  ANY connection to VPS :443                             │
│                         │                               │
│                         ▼                               │
│  xray/REALITY (only thing on :443, if enabled)          │
│        │                     │                          │
│        ▼                     ▼                          │
│  REALITY auth OK        REALITY auth FAIL               │
│  → tunnel to mesh       → decoy response                │
│                                                         │
│  SNI value IGNORED for routing                          │
│  No owned domains exposed                               │
└─────────────────────────────────────────────────────────┘
```

### TLS Certificate Rules
| Port | Certificate | Status |
|------|-------------|--------|
| :443 | Decoy (www.microsoft.com) | REALITY only, no owned certs |
| :465/:587/:993 | ACME for mail.3tched.com, mail.ghostbridge.tech | REQUIRED |
| Mesh services | Internal/self-signed or mesh CA | Mesh-only |

---

## 6. OpenFlow Mesh Routing

### Why IP:Port, Not SNI

| Approach | Layer | Visibility | Works with encrypted? |
|----------|-------|------------|----------------------|
| SNI demux | L7 | Exposes domain names | Only at TLS handshake |
| IP:port demux | L3/L4 | IPs only (mesh-internal) | Yes, post-tunnel |

### Flow Structure

```
┌─────────────────────────────────────────────────────────────┐
│                 OVS Bridge (br-mesh)                        │
│                                                             │
│  WG/REALITY ───►  OpenFlow Table                            │
│  egress           ┌─────────────────────────────────┐       │
│                   │ cookie=0x3tched, priority=100   │       │
│                   │ ip,nw_dst=10.0.0.2,tp_dst=8090  │       │
│                   │ actions=output:grpc_port        │       │
│                   ├─────────────────────────────────┤       │
│                   │ cookie=0x3tched, priority=100   │       │
│                   │ ip,nw_dst=10.0.0.3,tp_dst=443   │       │
│                   │ actions=output:dashboard_port   │       │
│                   ├─────────────────────────────────┤       │
│                   │ priority=0                      │       │
│                   │ actions=NORMAL                  │       │
│                   └─────────────────────────────────┘       │
└─────────────────────────────────────────────────────────────┘
```

### Safety Rules (MUST KEEP)

| Rule | Command Example | Why |
|------|-----------------|-----|
| Always use cookie | `cookie=0x3tched` | Identify managed flows |
| Never bulk delete | ~~`ovs-ofctl del-flows br-mesh`~~ | Destroys all flows |
| Filter deletions | `del-flows cookie=0x3tched/-1,...` | Safe removal |
| Fail-mode standalone | `ovs-vsctl set-fail-mode br-mesh standalone` | Survives controller disconnect |

---

## 7. Mail Architecture (Half-and-Half)

### Inbound Flow
```
External sender → MX (CF Email Routing) → Forward → Mail CT → Mailbox
                                                           └→ Ingest pipeline (if register@/ingest@)
```

### Outbound Flow
```
User/System → Mail CT :587 → DKIM sign → Direct SMTP → Recipient
                                    ↑
                        NOT Gmail, NOT CF
```

### Join Mail Flow
```
Control Plane → Mail CT → Outbound as @3tched.com/@ghostbridge.tech → Subscriber email
```

### DNS Records

| Record | Type | Value | Purpose |
|--------|------|-------|---------|
| 3tched.com | MX | CF Email Routing | Inbound via CF |
| ghostbridge.tech | MX | CF Email Routing | Inbound via CF |
| mail.3tched.com | A | 188.68.58.237 (grey) | Direct mail access |
| mail.ghostbridge.tech | A | 188.68.58.237 (grey) | Direct mail access |
| 3tched.com | TXT | `v=spf1 ip4:188.68.58.237 -all` | Authorize VPS only |
| ghostbridge.tech | TXT | `v=spf1 ip4:188.68.58.237 -all` | Authorize VPS only |
| *._domainkey.* | TXT | DKIM public key | Signature verification |
| _dmarc.* | TXT | `v=DMARC1; p=reject; ...` | Policy |

### What's NOT Allowed
- No `include:_spf.google.com` in SPF
- No Gmail servers in SPF
- No Gmail as CF Email Routing destination
- No Gmail "Send mail as"

---

## 8. What Is Public vs Mesh vs Camouflage

| Category | Components | Access Method | Who Can Reach |
|----------|------------|---------------|---------------|
| **Public (CF)** | Marketing site, registration UI | Browser → CF edge | Anyone |
| **Public (VPS mail)** | IMAP, SMTP submission | Direct to VPS :465/587/993 | Authenticated users |
| **Camouflage (optional)** | REALITY :443 | Probe → decoy; xray client → tunnel | Anyone (decoy) / Subscribers (tunnel) |
| **Mesh-private** | Dashboard, API, gRPC, broker, qdrant, assistant | WG (primary) or REALITY (optional) → mesh → OpenFlow | Enrolled subscribers only |

---

## 9. Failure Modes

### Registration/Enrollment Failures

| Failure | Detection | Impact | Mitigation |
|---------|-----------|--------|------------|
| CF Email Routing delay | Ingest mail late | Registration delayed | User told "check email"; async OK |
| CF Email Routing failure | No mail arrives | Registration blocked | Monitor CF; alert on queue |
| Ingest parse failure | Malformed payload | Single registration fails | Log error; reject gracefully |
| NetMaker API failure | Provisioning fails | No WG config | Retry logic; alert |
| Enrollment token expiry | User delays too long | Must re-register | Clear expiry in email (24-72h) |
| Join mail delivery failure | Bounce/spam | User never gets instructions | SPF/DKIM/DMARC alignment; monitor bounces |

### REALITY Failures

| Failure | Detection | Impact | Fix |
|---------|-----------|--------|-----|
| Owned name in serverNames | SNI probe reveals | Camouflage defeated | Remove from config, restart xray |
| Multiple serverNames | Config audit | Fingerprint anomaly | Reduce to single decoy |
| Decoy site down | REALITY handshake fails | Optional path broken | Choose reliable decoy; WG still works |

### OpenFlow Failures

| Failure | Detection | Impact | Fix |
|---------|-----------|--------|-----|
| Bulk del-flows | Flows disappear | Network down | Restore from backup, re-add flows |
| Missing cookie | dump-flows shows no cookie | Can't safely manage | Re-add with cookie |
| Controller disconnect | OVS logs | No new flows | fail-standalone preserves existing |

### Mail Failures

| Failure | Detection | Impact | Fix |
|---------|-----------|--------|-----|
| CF forward fails | No mail arrives | Inbound broken | Check CF routing rules |
| SPF mismatch | Bounces, DMARC reports | Outbound rejected | Fix SPF record |
| Gmail in SPF or routing | DMARC reports Gmail sends | Policy violation | Remove Gmail everywhere |

---

## 10. Verification Checklist

### REALITY/SNI Isolation (if REALITY enabled)
- [ ] `grep -i "3tched\|ghostbridge" /etc/xray/xray_config.json` returns nothing
- [ ] SNI probe to VPS:443 with owned domain returns decoy cert
- [ ] Only xray listens on :443 (`ss -tlnp | grep :443`)
- [ ] No nginx/haproxy on :443

### Public Port Surface
- [ ] External nmap shows only :443 (optional) and mail ports
- [ ] HTTP to VPS IP returns nothing useful
- [ ] Owned domains resolve to CF, not VPS (except mail.*)

### Mesh Isolation
- [ ] Services bind to 10.x.x.x (`ss -tlnp` shows mesh IPs)
- [ ] Connection from public IP to mesh services fails
- [ ] Connection through WireGuard succeeds (PRIMARY)
- [ ] Connection through REALITY succeeds (OPTIONAL, if enabled)

### Mail
- [ ] MX records point to CF
- [ ] SPF contains only VPS IP, no Gmail
- [ ] DKIM test passes
- [ ] Outbound mail delivers with aligned SPF/DKIM/DMARC
- [ ] ACME certs valid on :465/:587/:993
- [ ] CF Email Routing destinations do not include Gmail

### OpenFlow
- [ ] Flows have consistent cookie
- [ ] No scripts with unfiltered del-flows
- [ ] fail-mode is standalone

### E2E Registration
- [ ] Form submit → ingest email arrives at mail CT
- [ ] Provisioning creates NetMaker enrollment
- [ ] Join email sent to subscriber
- [ ] Subscriber receives email, imports WG config
- [ ] WG connects, mesh services accessible
- [ ] User never directly contacted VPS or saw CP URLs
- [ ] REALITY optional check (separate from main flow)

---

*Design focused on technical separation: WireGuard primary access, REALITY optional camouflage, SNI isolation, OpenFlow routing, mail architecture, email-triggered enrollment.*
