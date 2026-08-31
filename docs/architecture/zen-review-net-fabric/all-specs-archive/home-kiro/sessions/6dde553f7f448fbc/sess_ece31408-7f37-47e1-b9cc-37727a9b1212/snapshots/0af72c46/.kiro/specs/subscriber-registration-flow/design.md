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
