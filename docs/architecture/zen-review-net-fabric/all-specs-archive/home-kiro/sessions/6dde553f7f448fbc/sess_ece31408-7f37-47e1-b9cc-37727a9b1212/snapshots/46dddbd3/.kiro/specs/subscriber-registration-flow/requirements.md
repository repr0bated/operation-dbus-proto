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

1. **No live CF→VPS connection.** CF Workers/Pages MUST NOT maintain HTTP,
   WebSocket, gRPC, or Tunnel connections to the VPS control plane. The
   trigger into the backend is email only (REQ-CF-002 / REQ-SEC-001 from
   control-plane spec).
2. **Polling for completion on CF side.** Since CF cannot receive a push from
   VPS, the CF Worker polls a CF-side store (KV / D1) for provisioning
   results. The VPS pushes results INTO that store via a narrow outbound
   channel (email-encoded payload or CF API token write — see design).
3. **WG config never in email.** The verification email contains ONLY a
   one-time code or magic link. WG credentials are delivered exclusively on
   the authenticated web page.
4. **Oracle decoy endpoint is the WG peer.** The generated config points at
   the Oracle decoy, NOT the VPS host (topology lock from handoff spec).
5. **HumanPrincipal registration.** Provisioning calls
   `human_principal.register_key` on the bridge so the new subscriber is
   assertion-ready from first connect.
6. **Single-use token.** The verification code / magic link is single-use,
   short-lived (≤ 15 min), and bound to the submitted email.
7. **No VPS public registration API.** The VPS exposes NO HTTP/gRPC endpoint
   on public IP for registration (REQ-SEC-002). Provisioning is triggered by
   ingest email arriving at the mail CT.
8. **QR contains full WG config.** The QR code encodes the complete
   `[Interface]` + `[Peer]` config text so mobile WireGuard apps can import
   directly.
9. **Rate limiting.** CF WAF / Workers rate-limit form submissions per IP
   (≤ 5/hour) and per email (≤ 2 pending verifications).
10. **Branded domain.** Page served from `register.ghostbridge.tech` or
    `join.3tched.com` (CF Pages custom domain, orange-proxied).

---

## 3 · Functional Requirements

### FR-1: Registration Page (CF Pages)

A static site on CF Pages (HTML/CSS/JS, no server-side rendering) served from
a branded subdomain. Contains:

- Email input form.
- Verification code input (appears after submit).
- WG config display area (appears after verification).
- QR code canvas (rendered client-side from config text).
- Copy-to-clipboard button for config text.
- Download `.conf` file button.

**Acceptance Criteria:**
- [ ] Page loads from CF Pages custom domain
- [ ] No JS fetches to VPS IP or non-CF origins
- [ ] Lighthouse accessibility score ≥ 90
- [ ] Mobile-responsive (QR scannable on another device)

### FR-2: Email Submission (CF Worker)

On form submit, a CF Worker:

1. Validates email format (RFC 5322 basic).
2. Rate-checks (IP + email).
3. Generates a 6-digit verification code + opaque token.
4. Stores `{token, email, code, created_at, status: "pending"}` in CF KV/D1
   with 15 min TTL.
5. Sends a verification email to the user (CF Email Workers or Mailchannels)
   containing ONLY the 6-digit code and a branded "verify your email" message.
6. Emits the registration trigger to VPS: sends a structured email to
   `ingest@<domain>` via CF Email Routing containing
   `{token, email, domain}`. This is the sole CF→VPS signal.
7. Returns `{token}` (opaque, no secrets) to the page for polling.

**Acceptance Criteria:**
- [ ] Verification email arrives within 60 s
- [ ] Email body contains no WG keys, no VPS IP, no Oracle endpoint
- [ ] Ingest email arrives at mail CT
- [ ] Rate limit rejects > 5 submissions/IP/hour
- [ ] Duplicate pending email rejects (≤ 2 pending)

### FR-3: Email Verification (CF Worker)

User enters 6-digit code on the page. CF Worker:

1. Looks up token in KV/D1.
2. Validates code matches, not expired, status == "pending".
3. Marks status = "verified".
4. Returns `{status: "verified", message: "provisioning..."}`.
5. Page begins polling for WG config.

**Acceptance Criteria:**
- [ ] Correct code transitions to "verified"
- [ ] Wrong code returns error, does NOT consume the token
- [ ] Expired token returns "expired, please re-register"
- [ ] Code is single-use (second submit with same code fails)

### FR-4: Backend Provisioning (Mail CT Ingest → Control Plane)

On VPS, the mail CT ingest pipeline:

1. Receives structured email from CF (via CF Email Routing forward).
2. Parses `{token, email, domain}` from the email body (JSON or
   structured-text in a machine-readable section).
3. Generates a WireGuard keypair (ed25519/curve25519).
4. Registers the human pubkey via `human_principal.register_key` on
   `op-grpc-bridge` (mesh-internal gRPC call).
5. Enrolls the pubkey as a peer on the Oracle decoy (NetMaker API or
   direct WG peer add — external boundary, ops-provisioned).
6. Allocates a mesh IP from the subscriber pool.
7. Builds the complete WG client config:
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
8. Delivers the result back to CF-side store. Two options (design chooses):
   - **Option A:** Sends a result email to a CF Email Worker address that
     writes to KV/D1.
   - **Option B:** Calls CF KV API directly (requires a CF API token on VPS;
     narrow, write-only).
9. On failure, delivers an error status via the same channel.

**Acceptance Criteria:**
- [ ] Ingest parses structured registration email
- [ ] `human_principal.register_key` called with valid base64 pubkey
- [ ] Oracle decoy peer enrollment triggered (external boundary — documented)
- [ ] WG config includes Oracle decoy endpoint (NOT VPS host)
- [ ] Result delivered to CF KV/D1 within 120 s of ingest
- [ ] Private key ONLY in the CF-side KV entry and the rendered page — never
      in email, never persisted on VPS after delivery

### FR-5: Config Delivery (CF Worker → Page)

Page polls CF Worker with token:

1. Worker reads KV/D1 for token status.
2. Returns one of: `pending` / `verified` / `provisioning` / `ready` /
   `failed` / `expired`.
3. On `ready`: returns `{wireguard_config, assigned_ip, endpoint}`.
4. Page renders config text in a `<pre>` block + copy button.
5. Page renders QR code (client-side, e.g. `qrcode.js`) encoding the full
   config text.
6. Page offers a "Download .conf" button (Blob URL).
7. Config is available for 10 minutes after first render, then KV entry
   expires (private key no longer retrievable).

**Acceptance Criteria:**
- [ ] Page transitions from "provisioning..." to config display without reload
- [ ] QR code scannable by WireGuard iOS/Android app
- [ ] Config text copy works
- [ ] `.conf` download works
- [ ] After 10 min expiry, page shows "expired — re-register"
- [ ] Private key not visible in any server logs

### FR-6: QR Code Generation

Client-side QR rendering of the full WireGuard config text:

- QR version auto-selected for content length (typically version 10–15 for
  a full WG config ~300 bytes).
- Error correction level M (15% recovery).
- Rendered as SVG or Canvas, not a remote image.
- Minimum 200×200px display, scalable.

**Acceptance Criteria:**
- [ ] WireGuard mobile app imports config from QR scan
- [ ] QR renders without external network requests
- [ ] Config round-trips: text → QR → scan → identical text

### FR-7: Security — Private Key Lifecycle

The WG private key:

1. Generated on VPS during provisioning.
2. Transmitted to CF KV/D1 (encrypted at rest by CF).
3. Delivered to user's browser exactly once.
4. Deleted from KV/D1 after 10 min TTL or first retrieval (whichever first).
5. Never stored on VPS after delivery to CF.
6. Never appears in email (ingest or verification).
7. Never logged.

**Acceptance Criteria:**
- [ ] VPS does not persist private key after CF delivery
- [ ] KV entry TTL ≤ 10 min from ready state
- [ ] No private key in any log output (VPS or CF Worker)
- [ ] Second poll for config after expiry returns error, not key

---

## 4 · Non-Functional Requirements

- **NFR-1:** Page load < 2 s on 3G (CF Pages CDN, minimal JS).
- **NFR-2:** Zero VPS downtime impact on page load (static site; only
  provisioning blocked if VPS offline).
- **NFR-3:** Provisioning completes within 120 s of email verification under
  normal conditions.
- **NFR-4:** All CF Worker code in TypeScript; no server-side Rust/Python on
  CF.
- **NFR-5:** VPS ingest pipeline in Rust or shell (no new Python).
- **NFR-6:** Page works without JS for the form submit (progressive
  enhancement); QR and polling require JS.
- **NFR-7:** WCAG 2.1 AA compliance on registration page.

---

## 5 · Out of Scope

| Item | Reason |
|------|--------|
| Oracle assertion crypto / HumanPrincipal validation | Owned by handoff spec |
| Oracle decoy deployment / WG server config | External ops boundary |
| Dashboard / post-login mesh UI | Separate spec (op-web) |
| Admin user management | Future spec |
| Payment / billing | Future spec |
| REALITY client config delivery | Optional; separate from primary WG |
| Email change / re-enrollment | Future spec |
| Multi-device enrollment | Phase 2 |

---

## 6 · User Flow Summary

```
┌──────────────────────────────────────────────────────────────────────────┐
│                         USER'S BROWSER                                   │
│                                                                          │
│  1. Visit register.ghostbridge.tech                                      │
│  2. Enter email → Submit                                                 │
│  3. Receive verification email (6-digit code)                            │
│  4. Enter code on same page                                              │
│  5. See "provisioning..." spinner (5–60 s)                               │
│  6. See WireGuard config + QR code                                       │
│  7. Scan QR / copy config / download .conf                               │
│  8. Connect WireGuard → access mesh                                      │
└──────────────────────────────────────────────────────────────────────────┘
```

---

## 7 · Channel Topology

```
Browser ──HTTPS──► CF Pages/Workers (static + API)
                        │
                        │ CF Email Workers (verification email to user)
                        │ CF Email Routing (ingest email to VPS)
                        ▼
                   Mail CT (VPS)
                        │
                        │ Mesh-internal gRPC
                        ▼
                   op-grpc-bridge
                        │
                        │ human_principal.register_key
                        │ + WG keypair gen + Oracle peer enroll
                        ▼
                   Result → CF KV/D1 (via email-back or CF API)
                        │
                        │ (page polls Worker)
                        ▼
                   Browser renders config + QR
```

**NO live VPS connection at any point.** The only VPS↔CF data paths are:
- CF → VPS: email (CF Email Routing → mail CT)
- VPS → CF: result delivery (email-back to CF Worker, or CF KV API write)

---

## 8 · Adjacent Issues (documented, not solved here)

| Issue | Disposition |
|------|-------------|
| CF KV/D1 contains private keys briefly | Accepted: CF encrypts at rest; 10 min TTL; single-read delete. Better than email delivery. |
| VPS → CF result delivery requires either a CF API token on VPS or an email-back path | Design decision (§FR-4 option A vs B); both are narrow, write-only. |
| Oracle decoy peer enrollment is an external boundary | Documented; may be NetMaker API or manual; provisioner calls it but this spec doesn't implement the decoy side. |
| Rate limiting bypassed by distributed IPs | Standard CF WAF concern; not specific to this flow. |
| User loses config after 10 min | Re-registration flow (out of scope); could add "resend config" in future. |

---

## 9 · Relationship to Existing Protos

The existing `registration.proto` (`operation.registration.v1`) and
`privacy_network.proto` define gRPC services on the mesh-private bridge.
These are **internal control-plane RPCs** callable only from the mesh — they
are NOT the public registration surface. This spec's public flow triggers
those internal RPCs indirectly via the email ingest pipeline. The page never
calls gRPC.

| Layer | Component | Network |
|-------|-----------|---------|
| Public | CF Pages + Workers | CF edge, HTTPS |
| Trigger | CF Email Routing → mail CT | Email (async) |
| Provisioner | Ingest script → bridge gRPC | Mesh-only (10.x.x.x) |
| Delivery | VPS → CF KV/D1 | Outbound from VPS (API or email) |
| Render | CF Worker → browser | CF edge, HTTPS |
