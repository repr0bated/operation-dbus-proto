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
