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
