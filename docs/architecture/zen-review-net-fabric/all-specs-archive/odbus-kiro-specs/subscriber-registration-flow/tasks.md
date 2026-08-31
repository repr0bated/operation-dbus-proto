# Tasks — Subscriber Registration Flow

---

## Phase 1 — CF Frontend + Voucher System

### TASK-001: Static Purchase Page (CF Pages)

**Linked REQs:** FR-1, NFR-1, NFR-6

- [ ] Create CF Pages project with HTML/CSS/JS (no framework).
- [ ] Product selection UI: 30d / 90d / 365d with prices.
- [ ] "I have a code" voucher input field.
- [ ] Post-redemption: config display area, QR canvas, copy button,
      download .conf button.
- [ ] Mobile-responsive, WCAG 2.1 AA.
- [ ] QR library included (`qrcode.js` or similar, bundled, no CDN).
- [ ] Deploy to CF Pages on branded subdomain.

### TASK-002: Stripe Webhook Worker (Voucher Issuance)

**Linked REQs:** FR-2, NFR-8

- [ ] CF Worker function: `POST /webhook`.
- [ ] Validate Stripe webhook signature (`STRIPE_WEBHOOK_SECRET`).
- [ ] On `checkout.session.completed`:
      - Generate 160-bit voucher (CSPRNG + Crockford Base32).
      - Compute `voucher_mac = HMAC-SHA-256(VOUCHER_PEPPER, code)`.
      - Write to VOUCHERS KV: `{product_class, duration_days, status:"active", issued_day}`.
      - Write to PAYMENT_LOG KV: `{voucher_mac, amount, currency}` with
        `expiration_ttl=1209600`.
- [ ] Return voucher code in Stripe success_url redirect.
- [ ] Handle duplicate webhooks idempotently (check if voucher already exists
      for this session via payment_log lookup).

### TASK-003: Redemption Worker (Voucher → Provisioning Trigger)

**Linked REQs:** FR-3, FR-4

- [ ] CF Worker function: `POST /redeem`.
- [ ] Accept `{code}` in request body.
- [ ] Normalize code, compute MAC, look up in VOUCHERS KV.
- [ ] Reject if not found / not "active" / expired.
- [ ] Atomically mark `status: "redeeming"`, generate `redemption_token`.
- [ ] Send trigger email via `env.EMAIL.send()`:
      - To: `provision@ghostbridge.tech`
      - From: `system@ghostbridge.tech`
      - Subject: `PROVISION`
      - Body: JSON `{redemption_token, duration_days, product_class}`
- [ ] Return `{redemption_token, status: "provisioning"}`.
- [ ] On any failure: do NOT change voucher status (leave "active").
- [ ] Rate limit: ≤ 10 failed attempts per IP per hour.

### TASK-004: Polling Worker (Config Delivery)

**Linked REQs:** FR-5

- [ ] CF Worker function: `GET /poll/:token`.
- [ ] Read RESULTS KV by `redemption_token`.
- [ ] If not found: return `{status: "provisioning"}`.
- [ ] If found with `status: "ready"`: return full payload (config, IP, etc.).
- [ ] If found with `status: "failed"`: return error message.
- [ ] Optional: delete-on-first-successful-read (stronger: private key
      available exactly once).
- [ ] CORS headers for CF Pages origin.

---

## Phase 2 — VPS Provisioning Pipeline

### TASK-005: Ingest Script (Mail CT → Provisioner)

**Linked REQs:** FR-4, NFR-5

- [ ] Mail CT delivery rule: `provision@ghostbridge.tech` pipes to
      `/usr/local/bin/op-provision-ingest`.
- [ ] Script reads email body from stdin, extracts JSON payload.
- [ ] Validates: `redemption_token` present, `duration_days` is 30/90/365.
- [ ] Generates WG keypair (`wg genkey | tee /dev/fd/3 | wg pubkey`
      or Rust curve25519).
- [ ] Calls `op-grpc-bridge` (mesh gRPC at 10.0.0.2:8090):
      - `human_principal.register_key({human_pubkey: <base64_pubkey>})`
- [ ] Allocates IP from subscriber pool
      (`/var/lib/opdbus/subscriber-pool.json` or D-Bus call).
- [ ] Assembles WG config string.
- [ ] Calls Oracle decoy peer enrollment (external boundary — may be
      NetMaker API, `wg set`, or ops-specific script).
- [ ] Sets entitlement expiry (future: entitlement ledger; phase 1: metadata
      in HumanPrincipal or separate Cozo record).
- [ ] Writes result to CF KV via REST API:
      ```
      PUT /accounts/{CF_ACCOUNT_ID}/storage/kv/namespaces/{RESULTS_NS}/values/{redemption_token}
      Authorization: Bearer {CF_KV_WRITE_TOKEN}
      Content-Type: application/json
      ?expiration_ttl=600
      ```
- [ ] On success: zero private key from memory, exit 0.
- [ ] On failure: write `{status:"failed", error:"..."}` to CF KV, exit 1.
- [ ] Never log the private key.

### TASK-006: Subscriber IP Pool

**Linked REQs:** FR-4

- [ ] Define subscriber IP range (e.g. `10.100.0.0/16`).
- [ ] Allocation mechanism (simple: file-based counter; robust: Cozo table).
- [ ] Prevent double-allocation (atomic increment or lock).
- [ ] Record: `{ip, principal_id, allocated_day}`.

---

## Phase 3 — Stripe + CF Email Configuration

### TASK-007: Stripe Setup

**Linked REQs:** FR-2

- [ ] Create 3 Stripe Products (30d, 90d, 365d) with fixed prices.
- [ ] Configure Stripe Checkout with success_url and cancel_url.
- [ ] Register webhook endpoint (`https://purchase.ghostbridge.tech/webhook`).
- [ ] Enable 3-D Secure (default on Checkout).
- [ ] Set statement descriptor.
- [ ] Store `STRIPE_WEBHOOK_SECRET` in CF Worker secrets.

### TASK-008: CF Email Service Setup

**Linked REQs:** FR-3, constraint 12

- [ ] Onboard `ghostbridge.tech` for Email Sending in CF dashboard.
- [ ] Verify DNS records (SPF/DKIM/DMARC on `cf-bounce` subdomain).
- [ ] Create Email Routing rule: `provision@ghostbridge.tech` → VPS mail CT
      destination address.
- [ ] Verify destination address in CF dashboard.
- [ ] Test: Worker sends email → arrives at mail CT.
- [ ] Configure SMTP relay credentials for ghostbridge outbound
      (`smtp.mx.cloudflare.net:465`, API token with Email Sending: Edit).

### TASK-009: CF KV Namespaces

**Linked REQs:** FR-2, FR-3, FR-5

- [ ] Create KV namespace `VOUCHERS`.
- [ ] Create KV namespace `RESULTS`.
- [ ] Create KV namespace `PAYMENT_LOG`.
- [ ] Bind all three to the Worker in wrangler config.
- [ ] Create CF API token for VPS: "Workers KV Storage: Write" on RESULTS
      namespace only.
- [ ] Store token on VPS in env (`CF_KV_WRITE_TOKEN`).

---

## Phase 4 — Security + Integration Testing

### TASK-010: Rate Limiting

**Linked REQs:** constraint 11

- [ ] CF WAF rule: ≤ 20 requests/min to `/webhook` per IP.
- [ ] Worker-level: ≤ 10 failed `/redeem` attempts per IP per hour
      (track in KV with short TTL).
- [ ] Worker-level: reject if > 100 active vouchers issued in last hour
      (velocity control on issuance).

### TASK-011: End-to-End Integration Test

- [ ] Test card payment → voucher → redemption → config on page.
- [ ] Test pre-purchased code entry → config on page.
- [ ] Test invalid/expired/spent voucher rejection.
- [ ] Test config QR scans successfully in WireGuard mobile app.
- [ ] Test KV TTL expiry (config unavailable after 10 min).
- [ ] Test chargeback on unredeemed voucher → revocation.
- [ ] Test chargeback on redeemed voucher → accepted loss (no disruption).
- [ ] Verify: no private key in any log (CF Worker logs, VPS syslog).
- [ ] Verify: no voucher code in VPS logs or VPS storage.
- [ ] Verify: no Stripe session ID in RESULTS KV or VPS storage.

### TASK-012: Documentation

- [ ] User-facing: minimal (page is self-explanatory; no docs needed).
- [ ] Ops: how to rotate voucher pepper, CF API token, Stripe keys.
- [ ] Ops: how to manually issue voucher codes for resellers/promos.
- [ ] Ops: monitoring (failed provisions, stuck redemptions, KV usage).

---

## Definition of Done

- [ ] User can purchase and receive WG config in < 2 min (normal conditions).
- [ ] User can enter pre-purchased voucher and receive config.
- [ ] QR code scans in WireGuard iOS and Android apps.
- [ ] No email sent to user at any point.
- [ ] No identity (email/name/password) required at any point.
- [ ] Payment domain cannot see WG keys or account IDs.
- [ ] VPS cannot see voucher codes or payment data.
- [ ] Private key not persisted anywhere after 10-min delivery window.
- [ ] Payment→voucher cross-reference auto-deleted after 14 days.
- [ ] CF WAF and Worker rate limiting active.
- [ ] Stripe 3-D Secure active on all card payments.
