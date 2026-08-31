# Design — Subscriber Registration Flow

**Version:** 1.0
**Status:** Draft
**Implements:** requirements.md (revised: no email to end user)

---

## 1 · Design Decisions

### D-1: End user never receives email

The registration page is a normal web form. The user submits, sees a spinner,
and receives their WG config + QR code on the same page. No verification
email, no magic link, no "check your inbox." Email is exclusively the
internal machine-to-machine trigger between CF and ghostbridge — invisible
infrastructure plumbing.

### D-2: CF Email Service as internal trigger channel

Cloudflare Email Service (public beta, Workers Paid plan) provides:


- **Email Sending** (outbound from CF): Workers binding `env.EMAIL.send()`,
  REST API, or SMTP at `smtp.mx.cloudflare.net:465`. Domain must be
  "onboarded" in CF dashboard (adds SPF/DKIM/DMARC to `cf-bounce` subdomain).
  3,000 emails/mo included on Workers Paid ($5/mo), $0.35/1,000 after.
- **Email Routing** (inbound to CF): MX pointed at CF, routes to Workers
  (`email()` handler) or verified destination addresses. Free.
- **SMTP relay for ghostbridge outbound**: ghostbridge can submit outbound
  mail through `smtp.mx.cloudflare.net:465` using a CF API token. Mail
  appears from CF infrastructure (CF IPs, CF DKIM signing), NOT from the VPS
  IP directly. This is how ghostbridge sends mail without revealing itself.

For this registration flow, only one direction matters:
**CF Worker → (email) → mail CT on ghostbridge.** The Worker uses
`env.EMAIL.send()` to send a structured machine-readable email to
`provision@ghostbridge.tech`. CF Email Routing has MX for ghostbridge.tech
pointed at CF, which forwards to the mail CT destination address (VPS
`:465`). The user never sees this email — it's an internal RPC encoded as
email to satisfy the "no live CF→VPS connection" constraint.

### D-3: CF KV as the result store + polling target

CF Workers KV provides the state bridge between the VPS provisioner and the
user's browser:
