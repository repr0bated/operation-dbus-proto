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
