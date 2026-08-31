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
