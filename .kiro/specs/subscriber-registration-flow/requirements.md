# Subscriber Registration Flow — Requirements

> A human visits a Cloudflare-hosted page, pays for a fixed-duration voucher,
> and redeems it to receive a WireGuard configuration with QR code — all on
> the same web page. No email, no username, no password. Payment and WG
> identity are cryptographically decoupled: the payment domain never sees a
> WG key or account, and the WG control plane never sees a transaction ID or
> card detail. The page looks like a normal product purchase.

| | |
|---|---|
| Status | Draft |
| Depends on | `netmaker-xray-identity-handoff/` (HumanPrincipal registry, assertion crypto) |
| Adjacent | `3tched-ghostbridge-control-plane/` (email channel, topology lock) |
| Frontend | Cloudflare Pages (static + Workers) |
| Payment | Stripe Checkout (hosted, PCI-compliant) / crypto / reseller voucher |
| Backend trigger | Email (CF → mail CT → control plane) — internal only |
| Delivery | Web page with WG config text + QR code |
| Privacy model | Payment domain ≠ WG domain; voucher is the unlinkable bridge |

---

## 1 · Problem Statement

The user wants to buy VPN access and connect. The system must:

1. Accept payment without requiring identity (no email, no name, no account).
2. Issue a bearer voucher decoupled from the payment transaction.
3. On voucher redemption, provision a WG identity and deliver the config.
4. Never allow the payment domain to learn the WG key, and never allow the
   WG control plane to learn the payment details.

The existing control-plane spec defines an email channel for CF→VPS
communication. That channel remains the internal trigger mechanism — but the
**end user never receives or sends email**. From their perspective, this is:

1. Visit page.
2. Pay (card / crypto / pre-purchased voucher code).
3. Receive voucher (or enter pre-purchased code).
4. See WG config + QR code.
5. Scan/copy. Connect.

No signup, no inbox, no account creation step.

---

## 2 · Hard Constraints

1. **No user-facing email.** The end user never receives or sends email.
   Email is internal infrastructure only (CF Worker → mail CT trigger).
2. **No live CF→VPS connection.** CF Workers/Pages MUST NOT maintain HTTP,
   WebSocket, gRPC, or Tunnel connections to the VPS control plane. The
   trigger into the backend is email only (REQ-CF-002 / REQ-SEC-001 from
   control-plane spec).
3. **Payment–WG separation.** The payment domain (Stripe, crypto node) MUST
   NEVER receive a WG public key, account ID, tunnel address, or Oracle
   endpoint. The WG control plane MUST NEVER receive a transaction ID, card
   details, or payment timestamp.
4. **Voucher is the bridge.** A high-entropy bearer token is the sole link
   between "payment settled" and "provision WG access." After atomic
   redemption, no persistent record joins voucher→account.
5. **Oracle decoy endpoint is the WG peer.** Generated configs point at the
   Oracle decoy, NOT the VPS host (topology lock from handoff spec).
6. **HumanPrincipal registration.** Provisioning calls
   `human_principal.register_key` on the bridge so the subscriber is
   assertion-ready from first connect.
7. **No VPS public registration API.** No HTTP/gRPC endpoint on VPS public
   IP for registration or redemption (REQ-SEC-002). Provisioning is triggered
   by ingest email arriving at the mail CT.
8. **QR contains full WG config.** The QR code encodes the complete
   `[Interface]` + `[Peer]` config text so mobile WireGuard apps can import.
9. **Fixed denominations.** Products are fixed-duration (30/90/365 days).
   No custom amounts, no fractional billing, no recurring subscriptions
   for anonymous purchasers.
10. **Client never reveals identity.** No email, name, username, password,
    phone number, or device fingerprint required at any point. An optional
    recovery key may be offered but never mandatory.
11. **Rate limiting.** CF WAF rate-limits the purchase page per IP.
    Redemption endpoint rate-limited per voucher attempt (≤ 10 failed
    attempts/hour per IP).
12. **Ghostbridge outbound mail relays through CF.** Any outbound email from
    ghostbridge (operational, not user-facing) uses CF Email Service SMTP
    (`smtp.mx.cloudflare.net:465`) so mail appears from CF infrastructure,
    never revealing VPS IP in email headers.

---

## 3 · Functional Requirements

### FR-1: Purchase Page (CF Pages + Workers)

A static site on CF Pages served from a branded subdomain. Contains:

- Product selection (30 / 90 / 365 day durations with pricing).
- Stripe Checkout button (redirects to Stripe-hosted payment page).
- "I have a voucher code" input for pre-purchased / reseller codes.
- Post-payment: voucher display + auto-redemption flow.
- WG config display area (appears after redemption).
- QR code canvas (rendered client-side from config text).
- Copy-to-clipboard and download `.conf` buttons.

**Acceptance Criteria:**
- [ ] Page loads from CF Pages custom domain (orange-proxied)
- [ ] No JS fetches to VPS IP or non-CF origins
- [ ] Stripe Checkout is hosted (card data never touches CF Worker)
- [ ] Mobile-responsive (QR scannable on separate device)
- [ ] No email/name/identity fields anywhere on the page

### FR-2: Payment → Voucher Issuance (CF Worker + Stripe)

On successful Stripe Checkout payment:

1. Stripe webhook hits CF Worker with `checkout.session.completed`.
2. Worker validates webhook signature.
3. Worker generates a voucher: 160-bit CSPRNG, Crockford Base32 encoded,
   grouped for readability (e.g. `7K4M2-9ZJQF-W6TD8-H3XNP-4RV`).
4. Worker stores `{voucher_mac, product_class, duration_days, status: "active",
   issued_day}` in CF D1 or KV. **No Stripe session ID or payment reference
   stored with the voucher.**
5. Worker stores `{stripe_session_id, voucher_mac, status, issued_day}` in a
   SEPARATE payment-log table/namespace with TTL = refund period (14 days).
   This is the ONLY temporary cross-reference; it is auto-deleted.
6. Worker returns voucher code to the user's browser (via Stripe success URL
   query param or post-redirect fetch).

For crypto: self-hosted BTCPay Server or Monero wallet on VPS confirms
payment → VPS generates voucher → writes to CF KV via REST API.

**Acceptance Criteria:**
- [ ] Voucher generated with ≥128 bits entropy
- [ ] Voucher stored with NO payment identifier after refund-period TTL
- [ ] Stripe never receives WG key, account ID, or Oracle endpoint
- [ ] Webhook signature validated (reject replays)
- [ ] Fixed denominations only (30/90/365)
- [ ] Rate limit: ≤ 20 voucher issuances per Stripe account per hour

### FR-3: Voucher Redemption (CF Worker → Email Trigger → VPS)

User enters or auto-submits voucher code on the page:

1. CF Worker normalizes code (strip whitespace/dashes, uppercase).
2. Worker computes `voucher_mac = HMAC-SHA-256(pepper, normalized_code)`.
3. Worker looks up voucher in D1/KV: must be `status: "active"`.
4. Worker marks voucher `status: "redeeming"`, stores a `redemption_token`
   (opaque, returned to browser for polling).
5. Worker sends structured email to `provision@ghostbridge.tech` containing:
   `{redemption_token, duration_days, product_class}`. **Voucher code itself
   is NOT sent to VPS.** VPS never learns the voucher.
6. Returns `{redemption_token, status: "provisioning"}` to browser.
7. Browser begins polling Worker with `redemption_token`.

**Acceptance Criteria:**
- [ ] Invalid/spent/expired voucher returns clear error immediately
- [ ] Voucher marked non-redeemable atomically (no double-spend)
- [ ] Email to VPS contains NO voucher code, NO payment data
- [ ] Redemption is single-use (second attempt fails)
- [ ] Failed redemption does NOT consume the voucher

### FR-4: Backend Provisioning (Mail CT Ingest → Control Plane)

On VPS, the mail CT ingest pipeline:

1. Receives structured email from CF (via CF Email Routing forward).
2. Parses `{redemption_token, duration_days, product_class}`.
3. Generates a WireGuard keypair (curve25519).
4. Derives `principal_id` from the pubkey via
   `op_identity::session::derive_principal_id`.
5. Registers the human pubkey via `human_principal.register_key` on
   `op-grpc-bridge` (mesh-internal gRPC call).
6. Enrolls the pubkey as a peer on the Oracle decoy (NetMaker API or
   WireGuard peer add — external boundary).
7. Allocates a mesh IP from the subscriber pool.
8. Sets entitlement expiry (now + `duration_days`).
9. Builds the complete WG client config:
   ```ini
   [Interface]
   PrivateKey = <generated_private_key>
   Address = <allocated_ip>/32
   DNS = <mesh_dns>

   [Peer]
   PublicKey = <oracle_decoy_pubkey>
   Endpoint = <oracle_decoy_endpoint>:<port>
   AllowedIPs = 10.0.0.0/8
   PersistentKeepalive = 25
   ```
10. Writes result to CF KV via REST API:
    key = `redemption_token`, value = `{wireguard_config, assigned_ip,
    endpoint, expires_at}`, `expiration_ttl = 600` (10 min).
11. Deletes the private key from VPS memory after CF KV write succeeds.

**Acceptance Criteria:**
- [ ] `human_principal.register_key` called with valid base64 pubkey
- [ ] Oracle decoy peer enrollment triggered (external boundary)
- [ ] WG config includes Oracle decoy endpoint (NOT VPS host)
- [ ] Result in CF KV within 120 s of ingest under normal conditions
- [ ] Private key NOT persisted on VPS after delivery to CF KV
- [ ] VPS never receives or stores the voucher code
- [ ] No payment/transaction data reaches the VPS at any point

### FR-5: Config Delivery (CF Worker → Page)

Page polls CF Worker with `redemption_token`:

1. Worker reads CF KV for the token.
2. Returns one of: `provisioning` / `ready` / `failed` / `expired`.
3. On `ready`: returns `{wireguard_config, assigned_ip, endpoint,
   expires_at}`.
4. Page renders config text in a `<pre>` block + copy button.
5. Page renders QR code (client-side, e.g. `qrcode.js`) encoding the full
   config text.
6. Page offers a "Download .conf" button (Blob URL).
7. Config is retrievable for 10 minutes (KV TTL), then gone.

**Acceptance Criteria:**
- [ ] Page transitions from "provisioning..." to config display without reload
- [ ] QR code scannable by WireGuard iOS/Android app
- [ ] Config text copy and `.conf` download both work
- [ ] After KV TTL expiry, page shows "config expired"
- [ ] Private key not visible in any server logs (CF or VPS)
- [ ] No second retrieval after KV expiry — key is gone forever

### FR-6: QR Code Generation

Client-side QR rendering of the full WireGuard config text:

- QR version auto-selected for content length (~300 bytes typical).
- Error correction level M (15% recovery).
- Rendered as SVG or Canvas, not a remote image.
- Minimum 200×200px display, scalable.

**Acceptance Criteria:**
- [ ] WireGuard mobile app imports config from QR scan
- [ ] QR renders without external network requests
- [ ] Config round-trips: text → QR → scan → identical text

### FR-7: Pre-Purchased / Reseller Voucher Entry

Users who purchased voucher codes from resellers or received them
out-of-band can enter them directly (skip payment step):

1. Page shows "I have a code" input.
2. User enters code → same redemption flow as FR-3 (step 1 onward).
3. No payment processing involved.

This supports: physical scratch cards, reseller-sold digital codes,
promotional codes, gifted codes.

**Acceptance Criteria:**
- [ ] Pre-purchased code redeems identically to post-payment code
- [ ] No payment session required
- [ ] Invalid code returns clear error without consuming anything

### FR-8: Refund Path (Payment Domain Only)

During the refund period (14 days), the temporary payment-log entry links
`stripe_session_id → voucher_mac`. If a refund/chargeback occurs:

1. Stripe webhook `charge.refunded` or `charge.dispute.created` hits Worker.
2. Worker looks up `voucher_mac` from payment log.
3. If voucher is still `active` (unredeemed): mark it `revoked`.
4. If voucher is already `redeemed`: no action possible (separation is
   complete; accept the loss).
5. After 14-day TTL: payment log entry auto-deletes; no revocation possible.

**Acceptance Criteria:**
- [ ] Unredeemed voucher revoked on chargeback
- [ ] Redeemed voucher: loss accepted, no account disruption
- [ ] Payment log auto-deletes after refund period
- [ ] No account/WG data touched during refund processing

---

## 4 · Non-Functional Requirements

- **NFR-1:** Page load < 2 s on 3G (CF Pages CDN, minimal JS).
- **NFR-2:** Zero VPS downtime impact on page load (static site; only
  provisioning blocked if VPS offline).
- **NFR-3:** Provisioning completes within 120 s of voucher redemption under
  normal conditions.
- **NFR-4:** CF Worker code in TypeScript. Static page in HTML/CSS/JS (no
  framework required).
- **NFR-5:** VPS ingest pipeline in Rust or shell (no new Python).
- **NFR-6:** WCAG 2.1 AA compliance on purchase page.
- **NFR-7:** Voucher codes are human-readable: Crockford Base32, grouped,
  with error-detection checksum.
- **NFR-8:** All secrets (voucher pepper, CF API token, Stripe webhook
  signing key) stored in CF Worker secrets or VPS env — never in code.

---

## 5 · Out of Scope

| Item | Reason |
|------|--------|
| Oracle assertion crypto / HumanPrincipal validation | Owned by handoff spec |
| Oracle decoy deployment / WG server config | External ops boundary |
| Dashboard / post-login mesh UI | Separate spec (op-web) |
| Recurring subscriptions | Deliberate: anonymous prepaid only |
| Email to user (verification, welcome, etc.) | Deliberate: no user email ever |
| Multi-device / key rotation | Phase 2 |
| Blind-signature voucher issuance | Phase 2 (stronger crypto unlinkability) |
| Physical scratch card distribution | Ops/reseller concern; this spec handles digital redemption |
| Crypto payment integration (BTCPay/Monero) | Phase 2; architecture supports it via same voucher path |
| Account recovery | Phase 2; optional recovery key |
| Admin panel for voucher management | Separate ops tooling |

---

## 6 · Privacy Domains (Separation Model)

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    PAYMENT DOMAIN (CF Worker)                           │
│                                                                         │
│  Knows: Stripe session, card last-4, amount, timestamp                 │
│  Issues: voucher_mac (but deletes cross-ref after 14 days)             │
│  NEVER knows: WG key, account ID, Oracle endpoint, tunnel IP           │
└─────────────────────────────────────────────────────────────────────────┘
                              │
                    voucher code (bearer token)
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                    REDEMPTION DOMAIN (CF Worker)                        │
│                                                                         │
│  Knows: voucher_mac, redemption_token, duration                        │
│  Emits: provisioning trigger (email to VPS)                            │
│  NEVER knows: payment details, card info, Stripe session               │
│  NEVER persists: voucher_code → account mapping                        │
└─────────────────────────────────────────────────────────────────────────┘
                              │
                    email trigger (redemption_token + duration)
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                    PROVISIONING DOMAIN (VPS)                            │
│                                                                         │
│  Knows: redemption_token, duration, generated WG keypair, mesh IP      │
│  Calls: human_principal.register_key, Oracle peer enroll               │
│  Writes: WG config to CF KV (then forgets private key)                 │
│  NEVER knows: voucher code, payment details, card info                 │
└─────────────────────────────────────────────────────────────────────────┘
                              │
                    WG config (via CF KV, 10 min TTL)
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                    USER'S BROWSER                                       │
│                                                                         │
│  Receives: WG config + QR code                                         │
│  Connects: WireGuard → Oracle decoy → mesh                            │
└─────────────────────────────────────────────────────────────────────────┘
```

**No identifier appears in all domains.** Specifically:
- `stripe_session_id` never enters redemption or provisioning domains.
- `voucher_code` never enters the provisioning domain or WG control plane.
- `wg_public_key` never enters the payment domain.
- `redemption_token` is ephemeral (10 min TTL) and non-correlatable.

---

## 7 · Channel Topology (Revised)

```
Browser ──HTTPS──► CF Pages/Workers (static + purchase + redeem + poll)
                        │
                        ├─► Stripe Checkout (hosted payment page)
                        │        │
                        │        └─► Webhook back to CF Worker (payment settled)
                        │
                        ├─► CF Email Service: send to provision@ghostbridge.tech
                        │        │  (internal trigger — user never sees)
                        │        ▼
                        │   CF Email Routing MX → mail CT (VPS :465)
                        │        │
                        │        ▼
                        │   Mail CT ingest → op-grpc-bridge (mesh gRPC)
                        │        │
                        │        ▼
                        │   VPS writes result to CF KV (REST API, write-only)
                        │
                        └─► CF Worker reads KV → returns config to browser
```

**VPS → CF result delivery** uses the CF KV REST API:
```
PUT /accounts/{account_id}/storage/kv/namespaces/{ns}/values/{redemption_token}
Authorization: Bearer <CF_API_TOKEN>
expiration_ttl: 600
```
Requires one CF API token on VPS with "Workers KV Storage: Write" permission
only. This is narrow, write-only, and the token cannot read KV or access
other CF resources.

---

## 8 · Cloudflare Capabilities Used

| CF Product | Role in this flow | Plan required |
|------------|-------------------|---------------|
| CF Pages | Static site hosting (purchase page) | Free |
| CF Workers | API endpoints (webhook, redeem, poll) | Paid ($5/mo) |
| CF Email Service — Sending | Internal trigger email to VPS | Paid (3,000/mo included) |
| CF Email Service — Routing | MX → forward to mail CT destination | Free |
| CF KV | Voucher store + result store | Paid (included with Workers) |
| CF D1 | Alternative to KV for voucher inventory (SQL) | Paid |
| CF WAF | Rate limiting on purchase/redeem endpoints | Free tier sufficient |
| Stripe Checkout | Hosted payment page (PCI compliant) | External |

**Key CF Email Service facts:**
- Outbound sending: Workers binding `env.EMAIL.send()`, REST API, or SMTP
  at `smtp.mx.cloudflare.net:465` with API token auth.
- Domain must be "onboarded" (CF adds SPF/DKIM/DMARC records).
- 3,000 emails/mo included; $0.35/1,000 after.
- Ghostbridge can ALSO relay outbound mail through CF SMTP endpoint
  (appears from CF infra, not VPS IP) — but this is for operational mail,
  NOT user-facing in this flow.
- Inbound routing: MX at CF, routes to Workers or verified addresses. Free.

---

## 9 · Adjacent Issues

| Issue | Disposition |
|------|-------------|
| CF KV contains private keys briefly (10 min) | Accepted: CF encrypts at rest; short TTL; write-only from VPS; read-once semantics achievable with delete-after-read in Worker. |
| VPS needs a CF API token for KV writes | Narrow scope: "Workers KV Storage: Write" only. Stored in VPS env. |
| Voucher pepper must be shared between issuance and redemption Workers | Both run in same CF account; stored as Worker secret. |
| Oracle decoy peer enrollment is external | Documented boundary; provisioner calls it but this spec doesn't implement decoy side. |
| Chargeback on redeemed voucher = accepted loss | By design: separation means we cannot revoke a redeemed account. Budget for ~1-2% chargeback rate on card volume. |
| User loses config after 10 min KV TTL | Must re-purchase. Future: optional recovery key that can re-derive config. |

---

## 10 · Relationship to Existing Protos

The existing `registration.proto` and `privacy_network.proto` define
mesh-private gRPC services on `op-grpc-bridge`. They are internal control-plane
RPCs callable only from the mesh. This spec's provisioning step calls them
indirectly via the ingest pipeline. The purchase page never calls gRPC.

| Layer | Component | Network |
|-------|-----------|---------|
| Public | CF Pages + Workers | CF edge, HTTPS |
| Payment | Stripe Checkout (hosted) | Stripe infrastructure |
| Trigger | CF Email Service → mail CT | Email (async, internal) |
| Provisioner | Ingest script → bridge gRPC | Mesh-only (10.x.x.x) |
| Result delivery | VPS → CF KV REST API | Outbound HTTPS from VPS |
| Config render | CF Worker → browser | CF edge, HTTPS |

