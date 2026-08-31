# Design — Subscriber Registration Flow

**Version:** 1.0
**Status:** Draft
**Implements:** requirements.md

---

## 1 · Design Decisions

### D-1: End user never receives email

The purchase page is a normal product page. User pays, receives a voucher
(or enters a pre-purchased one), and gets their WG config — all on the same
page. No email verification, no magic link, no "check your inbox." Email is
exclusively the internal machine-to-machine trigger from CF to ghostbridge.

### D-2: Voucher as the unlinkable bridge

Inspired by Mullvad's architecture but with stronger separation:


- Payment settles → voucher issued (CF Worker, same process).
- Voucher code is the ONLY artifact the user carries between payment and
  provisioning. It is a 160-bit random bearer token.
- After redemption, no persistent record joins voucher → account.
- The payment-log cross-reference (stripe_session → voucher_mac) auto-deletes
  after the 14-day refund window.
- VPS never learns the voucher code at all — only a `redemption_token`.

This is stronger than Mullvad's documented model (which retains
`voucher_id → account_number` in its activation-code table). Our redemption
domain NEVER sends the voucher code to VPS; it sends only an opaque
one-time `redemption_token`. The provisioning domain cannot correlate back
to the payment even if compromised.

### D-3: CF Email Service as internal trigger

CF Email Service (public beta, Workers Paid) enables:
- **Outbound from Workers**: `env.EMAIL.send({to, from, subject, text})`.
  Domain must be onboarded. Sends via CF infrastructure with CF DKIM.
- **Inbound routing**: MX → CF, routes to Workers or destination addresses.
- **SMTP relay for ghostbridge**: `smtp.mx.cloudflare.net:465`, API token
  auth. Ghostbridge outbound mail appears from CF infra.

The trigger email from CF Worker to `provision@ghostbridge.tech` uses
`env.EMAIL.send()`. CF Email Routing forwards it to the mail CT destination
address (VPS mail port). This is an async fire-and-forget RPC encoded as
email.

### D-4: CF KV REST API for result delivery

VPS writes provisioning results to CF KV via:
```
PUT https://api.cloudflare.com/client/v4/accounts/{id}/storage/kv/namespaces/{ns}/values/{redemption_token}
Authorization: Bearer <CF_KV_WRITE_TOKEN>
```
With `expiration_ttl=600` (10 min). Requires only "Workers KV Storage: Write"
permission — cannot read KV, cannot access other CF resources. This is the
narrowest possible outbound channel from VPS to CF.

### D-5: Server-side key generation (phase 1)

In phase 1, the VPS generates the WireGuard keypair. The private key is:
1. Generated in memory.
2. Written to CF KV (encrypted at rest by CF).
3. Deleted from VPS memory after successful KV write.
4. Available to the user's browser for 10 min via CF Worker.
5. Gone forever after KV TTL expires.

Phase 2 improvement: client-side key generation where the user generates
their keypair in-browser (WebCrypto/libsodium-wasm) and submits only the
public key. This eliminates server-side private key handling entirely.

### D-6: Fixed denominations, no identity

Products: 30 days, 90 days, 365 days. No custom amounts. No identity.
No recurring billing for anonymous purchasers (recurring requires knowing
who to bill, which requires identity). Users buy prepaid time.

---

## 2 · Sequence Diagrams


### 2.1 Card Purchase → Config Delivery (happy path)

```mermaid
sequenceDiagram
    participant U as User Browser
    participant CF as CF Pages/Workers
    participant S as Stripe (hosted)
    participant KV as CF KV
    participant E as CF Email Service
    participant M as Mail CT (VPS)
    participant B as op-grpc-bridge
    participant O as Oracle Decoy

    U->>CF: Visit purchase page
    U->>S: Click "Buy 30 days" → Stripe Checkout
    S-->>S: User pays (card details stay at Stripe)
    S->>CF: Webhook: checkout.session.completed
    CF->>CF: Validate signature, generate voucher (160-bit)
    CF->>KV: Store voucher {mac, duration, status:active}
    CF-->>U: Redirect with voucher code displayed

    U->>CF: Auto-submit voucher for redemption
    CF->>KV: Lock voucher, mark "redeeming"
    CF->>CF: Generate redemption_token
    CF->>E: Send email to provision@ghostbridge.tech
    Note over E: {redemption_token, duration_days} only
    E->>M: CF Email Routing → mail CT
    CF-->>U: {redemption_token, status: "provisioning"}

    U->>CF: Poll with redemption_token (every 3s)

    M->>M: Parse trigger, generate WG keypair
    M->>B: human_principal.register_key(pubkey)
    M->>O: Enroll peer (external boundary)
    M->>M: Build WG config, allocate IP
    M->>KV: PUT result (config + metadata, TTL=600)
    M->>M: Delete private key from memory

    U->>CF: Poll → Worker reads KV → "ready"
    CF-->>U: {wireguard_config, assigned_ip, expires_at}
    U->>U: Render config text + QR code
    U->>U: Scan QR / copy / download .conf
    U->>O: WireGuard connect
```

### 2.2 Pre-Purchased Voucher Redemption

```mermaid
sequenceDiagram
    participant U as User Browser
    participant CF as CF Pages/Workers
    participant KV as CF KV
    participant E as CF Email Service
    participant M as Mail CT (VPS)

    U->>CF: Enter voucher code in "I have a code" field
    CF->>KV: Look up voucher_mac → found, active
    CF->>KV: Mark "redeeming", generate redemption_token
    CF->>E: Send trigger to provision@ghostbridge.tech
    CF-->>U: {redemption_token, status: "provisioning"}
    U->>CF: Poll...
    M->>M: Provision (same as 2.1)
    M->>KV: PUT result
    U->>CF: Poll → "ready"
    CF-->>U: WG config + QR
```

### 2.3 Chargeback on Unredeemed Voucher

```mermaid
sequenceDiagram
    participant S as Stripe
    participant CF as CF Worker
    participant KV as CF KV

    S->>CF: Webhook: charge.dispute.created
    CF->>CF: Look up payment_log (stripe_session → voucher_mac)
    CF->>KV: Read voucher status
    alt status = "active" (unredeemed)
        CF->>KV: Mark voucher "revoked"
        CF-->>S: Dispute evidence (optional)
    else status = "redeemed" or "redeeming"
        CF->>CF: Accept loss (cannot revoke provisioned account)
    end
```

---

## 3 · Component Architecture


### 3.1 CF Pages (Static Site)

```
/
├── index.html          (product selection + pricing)
├── checkout.html       (post-payment voucher display + redemption)
├── assets/
│   ├── qrcode.min.js   (client-side QR generation)
│   ├── app.js          (polling logic, QR render, copy/download)
│   └── style.css
└── _worker/            (CF Pages Functions — the API layer)
    └── functions/
        ├── webhook.ts          POST /webhook (Stripe webhook)
        ├── redeem.ts           POST /redeem  (voucher redemption)
        └── poll/[token].ts     GET  /poll/:token (config polling)
```

### 3.2 CF Worker Data Stores

**CF KV Namespace: `VOUCHERS`**
```
Key: voucher_mac (hex)
Value: JSON {
  product_class: "30d" | "90d" | "365d",
  duration_days: number,
  status: "active" | "redeeming" | "redeemed" | "revoked" | "expired",
  issued_day: "2026-08-08",
  redemption_token: string | null
}
Metadata: { expiry: issued_day + 365 days }
```

**CF KV Namespace: `RESULTS`**
```
Key: redemption_token
Value: JSON {
  status: "provisioning" | "ready" | "failed",
  wireguard_config?: string,    (full [Interface]+[Peer] text)
  assigned_ip?: string,
  endpoint?: string,
  expires_at?: string,          (ISO 8601, entitlement expiry)
  error?: string
}
expiration_ttl: 600 (10 min from VPS write)
```

**CF KV Namespace: `PAYMENT_LOG`** (separate, TTL-scoped)
```
Key: stripe_session_id
Value: JSON {
  voucher_mac: string,
  amount_minor: number,
  currency: "usd" | "eur",
  issued_day: "2026-08-08"
}
expiration_ttl: 1209600 (14 days — refund period)
```

After 14 days the payment→voucher cross-reference auto-deletes. No manual
cleanup needed. This is the ONLY record that could theoretically link payment
to voucher, and it's gone after the refund window.

### 3.3 Voucher Generation

```typescript
// In CF Worker (webhook.ts)
import { webcrypto } from 'node:crypto'; // CF Workers runtime

const VOUCHER_PEPPER = env.VOUCHER_PEPPER; // 32-byte secret

function generateVoucher(): string {
  const bytes = new Uint8Array(20); // 160 bits
  crypto.getRandomValues(bytes);
  return crockfordBase32Encode(bytes); // grouped: XXXXX-XXXXX-XXXXX-XXXXX
}

function voucherMac(code: string): string {
  const normalized = code.replace(/[-\s]/g, '').toUpperCase();
  // HMAC-SHA-256(pepper, normalized_code) → hex
  return hmacSha256Hex(VOUCHER_PEPPER, normalized);
}
```

The MAC is the database key. The plaintext code is never stored server-side
after issuance. The user receives the plaintext; the system stores only the
keyed hash.

### 3.4 VPS Ingest Pipeline

```
/usr/local/bin/op-provision-ingest
  ├── Reads email from stdin (piped by mail CT delivery)
  ├── Parses JSON payload: {redemption_token, duration_days}
  ├── Generates WG keypair (wg genkey + wg pubkey)
  ├── Calls op-grpc-bridge:
  │   ├── human_principal.register_key(pubkey)
  │   └── (future) entitlement.set_expiry(principal_id, duration)
  ├── Calls Oracle decoy peer enrollment (external, ops-defined)
  ├── Allocates IP from pool (/var/lib/opdbus/subscriber-pool.json)
  ├── Assembles WG config text
  ├── PUTs to CF KV: /accounts/{id}/storage/kv/namespaces/{ns}/values/{token}
  │   Body: {status:"ready", wireguard_config:..., ...}
  │   expiration_ttl: 600
  ├── Zeroes private key in memory
  └── Exits 0
```

Language: Rust binary (`crates/op-provision-ingest/`) or shell script calling
`grpcurl` + `curl` + `wg`. Design choice made during implementation.

### 3.5 Stripe Integration

- **Stripe Checkout** (hosted): user redirected to Stripe's page. Card data
  never touches CF Worker or VPS.
- **Products**: 3 fixed-price products in Stripe (30d, 90d, 365d).
- **Webhook**: `checkout.session.completed` → CF Worker at `/webhook`.
- **No Stripe Customer object**: one-time payments, no profiles.
- **3-D Secure**: enabled by default on Stripe Checkout.
- **Statement descriptor**: "GHOSTBRIDGE VPN" or similar (clear, no confusion).
- **Metadata**: random `purchase_handle` only — no account ID, no WG key.
- **Success URL**: `https://purchase.ghostbridge.tech/checkout?code={VOUCHER}`
  (Worker generates voucher and encodes in redirect URL query param).

---

## 4 · Security Analysis
