# Design: unified-authenticated-mcp-cognitive-control-plane

This design resolves every core architectural decision. Where `requirements.md`
offered an either/or, this document picks one and states the rejected alternative.
All requirement IDs (FR/SEC/DR/TR/DEP/CR/NFR) refer to `requirements.md` in this
directory.

---

## 1. Component ownership

| Concern | Owner | Notes |
|---|---|---|
| `:8090` TLS ingress (only network MCP door) | `op-grpc-bridge` | binds the canonical set **directly** (`127.0.0.1:8090`, svc0 `10.0.0.3:8090`, configured Netmaker `${NETMAKER_MESH_IP}:8090`, default `100.69.0.1`, when owned by its interface); no local relay (FR-1, §2a) |
| D-Bus session-bus name `org.opdbus.v1.plugins` | `op-grpc-bridge` | sole well-known name |
| Host UDS `/run/opdbus/grpc.sock`, container UDS `/run/ghostbridge/container.sock` | `op-grpc-bridge` | alternate transports, same stack (TR-2) |
| Canonical `ToolRegistry` (registry tools) | `op-grpc-bridge` (constructs `CognitiveMcpServer` in-process, holds `Arc<ToolRegistry>`) | FR-3, FR-6 |
| Shared authenticated dispatch catalog | `op-grpc-bridge` | one metadata/auth/dispatch catalog projected into Tonic and Axum frontends; it is not a `tonic::Routes` value (§2) |
| Sealed-plugin route surface (generated methods) | `op-grpc-bridge` Tonic projection → `PluginService.CallMethod` → `MutationEngine` → sole D-Bus name | routes, not registry tools; `PluginSchema` is projection metadata, never an execution authority (FR-3a) |
| MCP protocol adapter (Streamable HTTP/JSON-RPC, optional SSE, `resources/*`) | `op-grpc-bridge` native Axum routes composed by the top-level TLS mux | FR-2, FR-2a (stateless 2026-07-28), FR-17 |
| Authoritative capability/subid registry | `op-grpc-bridge` resolves against `oscal_subid_registry.rs` + capability taxonomy | FR-4a; unknown cap/subid → registration rejected |
| ExecutionContext construction (immutable, per call) | `op-grpc-bridge` interceptor + dispatch | FR-3d; threaded to `Tool::execute` and `CallMethod` |
| Output-schema validation | `op-grpc-bridge` dispatch, before return/persist/vectorize/prompt/event | FR-3e |
| Signed single-use approval verification | `op-grpc-bridge` (SEC-10 verifier) | binds suggestion+diff-hash+revision+actor+session+expiry+nonce |
| Public onboarding / session genesis | `op-grpc-bridge` narrow pre-auth surface | FR-18; rate-limited (SEC-13); grants no discovery/exec |
| Durable OIA1/OIG1/OPA1 replay ledger (cross-transport, restart-durable) | `op-grpc-bridge` dedicated Cozo DB `/var/lib/op-dbus/auth-replay.db`, independent of cognitive Cozo/Qdrant | SEC-13; replaces in-process `Mutex<HashMap>` and remains independent of memory degradation |
| Authoritative durable Cozo writer | `op-grpc-bridge` (via `CognitiveMemoryStore`) | one writer (DR-1); no silent in-mem fallback (DR-1a) |
| Context-awareness engine | `op-grpc-bridge` (holds `CognitiveMcpServer::context_engine()`) | on `:8090`, event-driven (FR-10) |
| Sealed catalog reads (`blob_*`, `blob://`) | `op-grpc-bridge` in-process registry tools + MCP resources | one canonical `blob_catalog` (FR-8) |
| Reflection (static + dynamic) | `op-grpc-bridge` `dynamic_reflection.rs` + build-time descriptors | parity (FR-9), see-all/execute-gated (FR-9a) |
| Event chain | `op-grpc-bridge` `MutationEngine.event_chain` | every dispatched call incl. failures (DR-5, SEC-9) |
| Sealed blob writes | `op-blob` only | consumers read-only (DR-2) |
| Waypipe | `op-grpc-bridge` shared route surface on `:8090` | retained (FR-11) |
| op-chat, op-mcp-server, op-web | **clients** of the bridge | no independent registry/DB writer / MCP listener |

`op-grpc-bridge` and `op-web` share the `opdbus` binary; op-web reaches the bridge
over gRPC (`https://…:8090`) and holds no cognitive `ToolRegistry` and no cognitive
Cozo handle.

---

## 2. Unified ingress and protocol multiplexing (FR-1, FR-2, FR-2a, SEC-1)

**Decision.** One bridge-owned TLS ingress is bound **directly** on the exact
canonical address set: `127.0.0.1:8090` (loopback), `10.0.0.3:8090` (svc0), and
`${NETMAKER_MESH_IP}:8090` (Netmaker, deployment default `100.69.0.1`, applicable
when the configured interface owns it). There is no local TCP relay in front of these
listeners. After TLS termination, one top-level Axum/Hyper service classifies the
request and composes two protocol frontends: native Axum MCP HTTP routes, and a Tonic
service for native gRPC plus gRPC-Web. A connection/request is handled as one protocol;
MCP HTTP is never inserted into `tonic::service::Routes`.

The protocol frontends are projections of one immutable
`AuthenticatedDispatchCatalog`. The catalog owns service/tool descriptors, target
capability/subid/side-effect/approval metadata, input/output validators, and dispatch
handles. `build_tonic_routes(catalog)` returns only Tonic services;
`build_mcp_routes(catalog)` returns only Axum MCP routes; `build_ingress(catalog)`
combines them under TLS and applies the same authentication/context middleware before
either can dispatch. The existing `build_operation_routes() -> tonic::service::Routes`
may remain as the Tonic builder, but it MUST NOT claim to mount raw HTTP. This is the
meaning of "one shared route stack": shared authority and dispatch metadata, not one
framework-specific router value (FR-17).

```
                    TLS :8090  bound DIRECTLY by op-grpc-bridge on
 { 127.0.0.1:8090, 10.0.0.3:8090, ${NETMAKER_MESH_IP}:8090 when present } (no local relay)
                                │  TLS handshake (mandatory, SEC-1)
                                ▼
                  top-level Axum/Hyper TLS ingress + protocol classifier
                     ┌──────────────────────┴──────────────────────┐
                     ▼                                             ▼
          Tonic service projection                       native Axum projection
       native gRPC + reflection + gRPC-Web           MCP HTTP/JSON-RPC + optional SSE
                     └──────────────────────┬──────────────────────┘
                                            ▼
                           AuthenticatedDispatchCatalog
                     auth → ExecutionContext → validation → dispatch
                                            ▼
                     sole D-Bus/MutationEngine admission
                         └─ bridge-owned or generated implementation
```

### 2a. Corrected listener topology and relay retirement (FR-1)

**Verified live state (the defect).** `sudo ss -lntp` shows the bridge directly binds
only `127.0.0.1:8090`; `10.0.0.3:8090` is a **`python3` `socket-relay`** (`fwd-8090`,
PID 1496) execing `socket-relay tcp-listen 10.0.0.3:8090 tcp-connect 127.0.0.1:8090`
(`deploy/runit/fwd-8090/run`). That relay is an unmanaged L4 forwarding hop (it does
not terminate the bridge's end-to-end TLS), owns the canonical port in a second
process, and violates the no-Python policy (NFR-6). `10.200.0.2:8090` is
**not bound at all** — it was an aspirational address in the prior design and is
retired here as an artifact.

The host also has a Netmaker forwarder, `fwd-nm-mesh-8090`, which binds
`100.69.0.1:8090` and forwards to loopback. It is a second legacy relay and is part
of the same cutover; deleting it without replacing its listener would strand remote
mesh consumers.

**Decision — canonical applicable bind set = `{127.0.0.1:8090, 10.0.0.3:8090,
${NETMAKER_MESH_IP}:8090}`, bridge-direct; Netmaker defaults to `100.69.0.1` and is
required whenever its configured interface owns that address.**

- The bridge's listener configuration binds **all three** addresses itself (one TLS
  acceptor configuration and the same top-level ingress stack on each). `10.0.0.3`
  is the svc0 service address; `100.69.0.1` is the Netmaker peer address. Readiness is
  false until every configured listener is bound. Address-not-present is a startup
  error surfaced by runit, not permission to silently fall back to loopback-only.
- The effective runit configuration is authoritative. `deploy/runit/op-grpc-bridge/run`
  MUST export the exact applicable address set through one documented variable; the current
  `FABRIC_BIND="127.0.0.1:8090"`, `ZEROCLAW_BIND_ADDR`, and `GRPC_BIND` loopback
  overrides are removed or made consistent. Tests inspect `/proc/<pid>/environ` and
  `ss`, because changing a Rust default while leaving the runit override would not
  change production.
- `10.200.0.2:8090` is **not** canonical and is **not** bound. Rejected alternative:
  keep `10.200.0.2:8090` — it has no live binder and no consumer; binding it would
  advertise an address nothing reaches.
- **Both `fwd-8090` and `fwd-nm-mesh-8090` are retired**, but their service directories
  are retained disabled through the rollback/soak window. No `socket-relay`/`tcpfwd`/
  `python3` process may front final-state `:8090` (NFR-7 gate). The safe live switch
  is: validate the release in an isolated canary namespace first; stop both relays;
  restart the new bridge so it acquires all three addresses; require TLS health,
  reflection, modern MCP, D-Bus execution, and browser gRPC-Web checks within the
  bounded readiness window. If any bind or check fails, stop the new bridge, keep
  both retired relays down, and keep the mesh firewall default-deny while the prior
  golden snapshot is restored and revalidated through the canonical authenticated
  bridge path. The rollback MUST NOT resurrect a deprecated ingress.
  Service-directory deletion happens only after the soak gate, so rollback does not
  depend on booting a Btrfs snapshot.

**Xray boundary.** Existing Xray/Netmaker identity routing may carry an opaque,
end-to-end TLS byte stream to one of the three canonical bridge listeners. It is
transport passthrough only: it MUST NOT terminate bridge TLS, manufacture or rewrite
OIA/MCP metadata, expose a second MCP port, or become the process shown by `ss` as the
owner of a canonical address. `mcp.internal` and other client aliases resolve/route to
the canonical listeners; they are not independent application endpoints.

**Firewall policy (FR-1).** `127.0.0.1:8090` is loopback-only. `10.0.0.3:8090` is
accepted only on svc0 from its enumerated service CIDR; `100.69.0.1:8090` is accepted
only on the Netmaker interface from the enumerated Netmaker CIDR. The nftables input
chain default-denies `tcp dport 8090` from every other interface, including WAN.
Rules are staged before the listeners are exposed and verified with one allowed and
one denied source per network.

**TLS certificate SANs (FR-1, SEC-1).** The bridge's server certificate MUST carry
SANs covering **every** bound address so a TLS dial to any of them validates:
`IP:127.0.0.1`, `DNS:localhost`, `IP:10.0.0.3`, and the configured
`IP:${NETMAKER_MESH_IP}` (default `IP:100.69.0.1`), plus each issued canonical DNS
alias. A dial to either network address with a missing SAN is
a misconfiguration and fails; the acceptance test dials each bound address over TLS
and asserts SAN coverage. Rejected alternative: a single-SAN loopback cert fronted by
the relay — that is exactly the retired topology and cannot present a valid cert for
the mesh address.

**Acceptance mapping (FR-1).** `sudo ss -lntp` shows only the **bridge PID** on
loopback, svc0, and the applicable configured Netmaker address (no
`python3`/`socket-relay`); both forwarders are down;
plaintext dial fails; TLS reflection succeeds against all addresses with SAN-valid certs;
no `:3003`/`:50051`/`:50052`/`:11438`/`:11437` listener.

**SSE decision.** Retain SSE **only** if a live client still requires it; the
default target is removal. The bridge's own reactive path is the gRPC
`subscribe`/event stream and MCP notifications, so SSE is not needed internally.
The Phase-0 client inventory (CR-1) determines whether any external client still
requires SSE; if none, the MCP adapter mounts no SSE route and the acceptance test
asserts its absence (FR-2 second bullet). Rejected alternative: keep SSE
unconditionally — it perpetuates the retired `:3003` context-SSE shape and adds a
transport with no consumer.

### 2b. MCP protocol conformance and canonical version — 2026-07-28 stateless (FR-2a)

**Decision — canonical version `2026-07-28`, stateless lifecycle.** The MCP adapter
advertises exactly one `protocolVersion` (`2026-07-28`,
https://blog.modelcontextprotocol.io/posts/2026-07-28/) and implements its
**stateless** lifecycle: there is no server-held session state that authorizes later
requests. Every request is self-contained and independently authenticated
(§2c below). Unsupported versions are rejected during negotiation.

**Stateless model (canonical).**

```
each MCP request (POST /mcp, JSON-RPC)
  ├─ carries a FRESH one-use OIA1 assertion (transport header, SEC-2 step 2)
  ├─ MCP-Protocol-Version: 2026-07-28
  ├─ Mcp-Method: exact JSON-RPC method; Mcp-Name: exact target name when applicable
  ├─ optional Origin: if present it MUST be allowed; browser requests MUST send it
  └─ authenticated + authorized PER REQUEST — no session lookup grants access
```

There is no `Mcp-Session-Id` requirement in the canonical path; a client may send a
correlation id, but the server never treats it as authorization state. `tools/list`
pagination, JSON-RPC error objects, request cancellation, notifications, body-size
limits, and per-request timeouts all apply per request.

**Canonical header and discovery contract.** `MCP-Protocol-Version` is mandatory and
must equal `2026-07-28`. `Mcp-Method` is mandatory and must byte-for-byte identify the
JSON-RPC method in the parsed body. `Mcp-Name` is mandatory for name-bearing methods
such as `tools/call` and `resources/read`, and must agree with the canonical name/URI
in the body. Missing, duplicated, malformed, unsupported, or body-disagreeing headers
are rejected before dispatch. `server/discover` returns the supported protocol,
capabilities, method/header requirements, and schema dialects; it never returns
identity-scoped data. Canonical list results are sorted deterministically and include
the protocol's `ttlMs` and `cacheScope`; a catalog generation/hash change invalidates
the prior cache view.

Origin is conditional, not a blanket CLI requirement: every present Origin must be
on the exact allowlist; a browser/gRPC-Web request without Origin is rejected; an
authenticated non-browser MCP client may omit Origin. The allowlist never accepts
`*` with credentials.

**Bounded legacy stateful compat shim (FR-2a).** For a legacy client that still
speaks the older `initialize` + `Mcp-Session-Id` stateful lifecycle, the adapter
offers a **bounded, explicitly-legacy** shim. It is off the canonical path, documented
as legacy-only, and governed by the invariant below.

**CRITICAL invariant (FR-2a, SEC-2, SEC-13): the MCP session id is a correlation
handle only; it is NEVER authentication.** Authentication binds to a **one-use OIA1
assertion per request**. A stolen or replayed session id is never sufficient — every
request, canonical or legacy, still passes the full SEC-2 pipeline including a fresh,
unreplayed assertion.

```
LEGACY STATEFUL SHIM  (bounded; each arrow still requires a FRESH one-use OIA1)

client                              bridge (MCP adapter)                 durable stores
  │  initialize                        │
  │  + OIA1 assertion #1 (one-use) ───▶│ SEC-2 pipeline on assertion #1
  │                                    │ (sig→expiry→replay(consume #1)→bind→resolve→cap)
  │                                    │ mint Mcp-Session-Id S = random 256-bit,
  │                                    │ bind S → { actor, identity_sled ref, scope,
  │                                    │            transport-binding fingerprint };
  │                                    │ S is a CORRELATION record, carries NO standing
  │                                    │ authorization and NO cached assertion
  │◀── initialize result, Mcp-Session-Id: S
  │
  │  tools/call  (Mcp-Session-Id: S)   │
  │  + OIA1 assertion #2 (one-use) ───▶│ 1. look up S → correlation record (NOT auth)
  │                                    │ 2. SEC-2 pipeline on assertion #2
  │                                    │    (fresh sig/expiry/replay(consume #2)/binding)
  │                                    │ 3. assertion #2 principal MUST equal S.actor
  │                                    │    AND transport-binding fingerprint MUST match
  │                                    │    S's  (else DENY — session/assertion mismatch)
  │                                    │ 4. target capability + nested input validation
  │                                    │ 5. dispatch with ExecutionContext (FR-3d)
  │◀── result (+ event_id/event_hash)
```

So a legacy session id `S` merely lets the server *correlate* turns and re-use the
resolved-scope record; it never *replaces* per-request assertion validation. "Re-present
/ bind fresh assertion material" means: each later request carries a new one-use OIA1
assertion that (a) passes replay-check as unseen, and (b) is bound to the same actor
and transport fingerprint recorded for `S`. A request presenting `S` but no valid fresh
assertion, or an assertion whose principal/binding disagrees with `S`, is rejected
(FR-2a acceptance: "session id bearing but no fresh OIA1 → rejected"; "replayed session
id does not grant access"). The shim is bounded: session records expire, are capped in
count, and are dropped on restart (they carry no durable authority, so losing them only
forces a fresh `initialize`).

Rejected alternative: cache the `initialize` assertion and treat subsequent
same-session requests as pre-authorized — this is exactly the session-id-as-auth
vulnerability SEC-2/FR-2a forbid.

**Normative conformance matrices (FR-2a).** Modern and legacy are tested separately;
no modern test performs `initialize`, and no legacy result is used as evidence of
2026 conformance.

| Matrix | Required lifecycle | Required positive cases | Required negative cases |
|---|---|---|---|
| Modern `2026-07-28` | Stateless; optional `server/discover`, then any independently authenticated method; no `initialize`, `initialized`, session issuance, or `Mcp-Session-Id` | `server/discover`; direct `tools/list`, `tools/call`, `resources/list`, `resources/read`; deterministic/cacheable lists; request `_meta`; notifications and cancellation | missing/wrong version, missing/mismatched `Mcp-Method`/`Mcp-Name`, reused OIA1, malformed JSON-RPC, invalid present Origin, browser-missing Origin, invalid schema dialect |
| Bounded legacy | `initialize` before legacy use; server-issued expiring `Mcp-Session-Id`; fresh OIA1 on **every** request | initialize then list/call with matching actor and transport binding | call before initialize, unknown/expired session, session without fresh OIA1, assertion/session principal mismatch, assertion replay across TCP/UDS/restart |

Both matrices cover JSON-RPC error objects, body/depth limits, deadlines, cancellation,
and SSE resumption only if SSE survives the client inventory.

**Schema dialect contract.** MCP-facing input and output schemas use full JSON Schema
2020-12 and carry an explicit `$schema` URI. The validator is dialect-aware; it does
not hard-code draft-07. Legacy `PluginSchema` draft-07 documents are parsed with a
draft-07 validator and projected to 2020-12 only through a deterministic compatibility
translator. A keyword whose semantics cannot be preserved rejects activation with a
diagnostic; it is never silently dropped or weakened. Descriptor, `tools/list`, input
validation, output validation, and blob projections all use the same normalized
schema artifact and its hash.

### 2c. Per-request one-use assertion binding (FR-2a, SEC-2, SEC-13)

Authentication is bound to the OIA1 assertion, never to any MCP construct. The
one-use property is enforced by the durable, cross-transport replay store (§4b,
SEC-13): the assertion's nonce is consumed on first acceptance and rejected on any
re-presentation, whether over TCP or UDS, before or after a restart. This is what
makes "one-use per request" meaningful and makes a captured session id or a captured
assertion useless on replay.

Ordinary MCP HTTP carries OIA1 in exactly one
`X-Oracle-Identity-Assertion` header encoded as canonical base64url without padding;
the decoded envelope is capped at 16 KiB. Native gRPC/UDS uses raw bytes in
`x-oracle-identity-assertion-bin`; gRPC-Web uses its standard binary-metadata wire
encoding for that same raw envelope. Duplicate headers, padding/non-canonical
alphabet, oversized decode, trailing bytes, or simultaneous/conflicting HTTP and
`-bin` representations fail before nonce lookup. No proxy may translate a footprint,
email, or other self-asserted identity header into OIA1. Cross-transport fixtures use
the same canonical OIA1 bytes and must resolve identically.

---

## 3. TCP and UDS transport flow (TR-1, TR-2, SEC-4)

All three transports use the **same `AuthenticatedDispatchCatalog` and interceptor**;
each socket exposes only its protocol-specific frontend. Only the transport binding
evidence differs.

```
TCP client ───TLS───▶ :8090 ─┐
host UDS  ──────────▶ grpc.sock ─┤
container UDS ──────▶ container.sock ─┤
                                      ▼
                        Ghostbridge/OIA1 interceptor (SEC-2)
                        ├─ TCP:  ConnectInfo<SocketAddr> → network/source binding
                        └─ UDS:  SO_PEERCRED (uid/gid/pid) + socket ownership
                                 + session/container binding
                                      ▼
                        identical authorization decision (SEC-4)
                                      ▼
             Tonic/Axum projection → shared catalog → MutationEngine → event chain
```

**Decision (TR-2).** The interceptor obtains per-transport binding evidence: for TCP,
the tonic `ConnectInfo<SocketAddr>`/`TcpConnectInfo`+`TlsConnectInfo` peer address
(the source binding OIA1 already validates); for UDS, `SO_PEERCRED` peer credentials
plus socket ownership and the session/container the socket belongs to. The OIA1
assertion is required on both. The **authorization semantics** (which identity, which
capability) are identical; the **binding step** is transport-appropriate. A UDS peer
with no valid assertion is rejected exactly like an unauthenticated TCP peer — there
is no "trusted local" bypass (SEC-3). Rejected alternative: treat local UDS as
implicitly trusted — forbidden by the target architecture and the source-of-truth
security model.

### 3a. UDS security model (TR-5)

**Verified defect.** Host UDS (`/run/opdbus/grpc.sock`) and container UDS are served
**plaintext** — `serve_with_incoming` is wired with no `.tls_config` — and the shared
socket is chmod `0o666` (world read/write) in `shared_socket.rs`. That is a
world-writable, unencrypted control-plane door.

**Decision — TLS-over-UDS, not a plaintext exception.** The UDS transports MUST run
the **same TLS server config** as the TCP listener (mutual identity via OIA1 on top;
TLS provides the encrypted, integrity-protected channel and a consistent code path).
Rejected alternative: retain plaintext UDS as an "explicit policy exception with
compensating controls." Rejected because (a) SEC-1/target-architecture already mandate
"never wire a new gRPC service as plaintext, even over UDS" (grpc-expert skill), and
(b) a plaintext world-order socket plus peer-cred trust is precisely the "trusted
local" posture SEC-3 forbids; using the identical TLS path removes a whole class of
divergence between transports (SEC-4). The self-signed loopback/mesh cert already
present is reused; UDS peers pin the same trust root. UDS clients use the logical TLS
server name `op-grpc-bridge.internal` as SNI/authority, and that DNS SAN is mandatory
on the certificate; a filesystem socket path is never treated as a certificate name.
The stdio shim, host clients, and container connector all set this same server name
explicitly and reject hostname verification bypasses.

**Socket ownership/mode (TR-5).** The world-writable `0o666` is tightened to the
minimal owner/group:
- host UDS `/run/opdbus/grpc.sock`: owner `opdbus:opdbus`, mode `0o660` (owner+group
  rw, world none); only the bridge (owner) and members of the `opdbus` group may
  connect.
- container UDS `/run/ghostbridge/container.sock`: owner is the bridge; the mount is
  exposed into the container namespace with group ownership matching the container's
  mapped principal group, mode `0o660`. `stat` in the acceptance test asserts the mode
  is not world-writable.

**Accepted wire protocols per UDS (TR-5).** Each UDS enumerates and enforces what it
accepts:
- host UDS: native gRPC (h2) **and** MCP HTTP/JSON-RPC (the local agent/`op-mcp-server`
  shim path). gRPC-Web is **not** accepted on UDS (it exists for browsers over TCP).
- container UDS: native gRPC only (the container gateway dials gRPC). MCP-HTTP and
  gRPC-Web are rejected on the container socket.
The demux (§2) applies per-socket allowlists; a protocol not on a socket's list is
refused at the framing layer.

**Peer-credential extraction and namespace mapping (TR-5).** The interceptor reads
`SO_PEERCRED` (uid/gid/pid). For a container peer, the raw namespaced uid/gid is
mapped to the host principal through the container's user-namespace mapping
(`/proc/<pid>/uid_map` / `gid_map`), so a container's in-namespace `uid 0` maps to its
actual unprivileged host principal, not host root. A peer whose credentials cannot be
mapped to a known principal is **denied** (fail closed). The mapped principal is then
required to match the OIA1 assertion's resolved principal (below).

**OIA source binding adapted for UDS (TR-5, TR-2).** OIA1's TCP "source-IP ==
netmaker_inner_ip" binding is replaced, on UDS, by "assertion principal == mapped
peer-cred principal AND socket-session/container == assertion's session/container."
The binding evidence differs; the requirement that the assertion be *bound to the
caller* is identical (SEC-4). A forged or absent assertion over UDS is rejected like
an unauthenticated TCP peer (TR-5 acceptance).

---

## 4. Authentication sequence (SEC-2, SEC-3, SEC-9; OIA1 owned by netmaker-xray-identity-handoff)

```
request (any transport) ──▶ op-grpc-bridge interceptor
  0. pre-auth rate limit   per-source + global pre-auth bucket (SEC-13)         — exceeded → 429, pre-auth telemetry
  1. transport auth        TLS (TCP) / TLS-over-UDS + SO_PEERCRED + ownership   — reject → close
  2. assertion present?    transport-canonical OIA1, ONE-USE                  — absent → only exact public allowlist; otherwise reject (legacy included)
  3. parse OIA1            malformed envelope                                   — reject (Malformed)
  4. trusted decoy key     decoy_key_id ∈ trust store                          — unknown → reject
  5. signature             Ed25519 over canonical bytes                        — bad → reject
  6. expiry/activity       now ≤ expires_at (+leeway); issued_at not future    — expired/future → reject
  7. replay lookup         nonce unseen in dedicated durable store (NO consume yet) — replayed → reject
  8. binding               TCP source-IP==inner_ip / UDS mapped peer-cred      — mismatch → reject (TR-5)
  9. HumanPrincipal        resolve_key(human_pubkey); revoked? unknown?        — unknown/revoked → reject
 10. identity_sled         resolve per-session projection                      — unresolved → reject
 10b. per-principal limit  per-resolved-principal bucket (SEC-13)              — exceeded → throttle (event)
 10c. BUILD base ExecutionContext (FR-3d): immutable {actor, resolved identity, DERIVED scope (FR-3f),
                           granted+selected cap, trace/event/parent ids, delegation depth,
                           deadline+cancel, approval provenance, transport binding} — caller cannot forge
 10d. replay consume       atomic OIA1 insert-if-absent after binding/principal — race/replay → reject
 11. TARGET capability     required_capability vs AUTHORITATIVE registry (FR-4a); header NOT
                           authoritative; no degraded is_some()&&match allow  — not granted → DENY (event, SEC-9)
 12. input validation      nested target-tool schema (FR-5a); caller scope may NARROW not
                           REPLACE the derived scope (FR-3f)                    — invalid/forged → reject (event)
 12b. approval validation  for approval_required: verify OPA1 fields/signer/policy/diff;
                           non-mutating replay lookup; finalize immutable VerifiedApproval
 13. audit/idempotency     atomically commit redacted execution_intent + operation binding
 13b. approval consume     atomically consume OPA1 immediately before mutation admission
 14. canonical admission   PluginService.CallMethod → D-Bus → MutationEngine EXACTLY ONCE
                           → selected in-process implementation
 15. OUTPUT validation     result vs normalized output_schema (FR-3e)           — invalid → FAIL CLOSED
 16. terminal+outbox       redact/bound; atomically append terminal audit + response-outbox
 17. response/sinks        only the committed validated outbox payload may leave the bridge
```

Steps 0–13 run **before** any tool executes or memory is read/written (SEC-2
acceptance). Ordering test: signature (5) → expiry (6) → replay lookup (7) → binding
(8) → principal/context (9–10c) → atomic consume (10d) → capability (11) → input
validation/approval checks (12–12b) → audit/idempotency intent (13) → approval
consume when required (13b) → D-Bus admission (14).
Output validation (15), redaction, and the terminal/outbox transaction (16) gate every
sink per FR-3e/SEC-11; rejected pre-dispatch calls still commit a redacted denial event.

**No wildcard, no self-asserted authority (SEC-3, FR-4a).** Step 11 resolves the
target's `required_capability` against the **authoritative** capability/subid registry
(`oscal_subid_registry.rs` + the capability taxonomy) and the caller's resolved
grants. Three defects are removed:
- the `"*"` wildcard grant for MCP/cognitive/agent capabilities is removed from
  `capability-grants.json` **and** prohibited inside sealed
  `PluginSchema.capability_grants` (the `tched_router.rs` `"*"` insert and the
  `grants.get(footprint).or_else(|| grants.get("*"))` fallback at
  `plugin_schema.rs:321`) — a CI gate (NFR-7) fails on either. In fact, any non-empty
  sealed `PluginSchema.capability_grants` is an activation/migration error: schemas
  declare capability vocabulary/requirements, never identity assignments;
- the caller-supplied `x-opdbus-capability` header (`DECLARED_CAPABILITY_HEADER`) is
  **not** authoritative — it may express intent, but authorization derives from
  resolved grants;
- the degraded allow-path "`identity.is_some() && capability_matches` when a method
  declares no grants" (`enforce_bridge_capability_with_schema`, `grpc_server.rs:114`)
  is **removed**: a method/tool with no resolvable grant is **denied**, not allowed.

There is no wildcard exception for liveness/onboarding. Those two public paths exist
only in the exact route allowlist (§12a). All identity grants come from the active
per-session `identity_sled`; a sealed per-footprint grant is ignored for authorization
and forces reseal rather than being materialized.

### 4a. Authoritative capability/subid registry; hand-written services carry metadata (FR-4a)

**Decision.** Every tool/method `subid` resolves against `oscal_subid_registry.rs` and
its `required_capability` resolves to the identical canonical `CapabilityDecl` in the
manifest-selected sealed schema; these define vocabulary, not grants. Registering a
tool/method/plugin whose capability or subid is unknown to the registry is **rejected**
at registration (fail closed). The hand-written services projected from the shared
dispatch catalog — **context, Waypipe, registration, health** — are not exempt: each declares
`required_capability` + `side_effect` + `subid` + `approval_required` and is authorized
by step 11 like any generated method. (Health's liveness sub-endpoint is the sole
public exception, FR-18.) Rejected alternative: let hand-written services skip metadata
"because they're internal" — that reintroduces an unauthorized surface.

### 4b. Dedicated durable OIA1/OIG1/OPA1 replay ledger (SEC-13)

**Verified defect.** `AssertionReplayCache` (`oracle_assertion.rs:205`) is an
in-process `Mutex<HashMap>` — lost on restart, not shared across TCP/UDS or processes.

**Decision — a dedicated durable security ledger shared by all transports.** Replay
protection uses the bridge-owned Cozo database `/var/lib/op-dbus/auth-replay.db`, not
the cognitive-memory Cozo database, Qdrant, SHM, or an in-process map. Records are
keyed by `(envelope_kind, trusted_issuer_key_id, nonce)` with domain-separated
`OIA1`, `OIG1`, and `OPA1` kinds, subject hash, issued/expiry time, and first execution
id. Validation first performs a non-mutating existence lookup; signature, time,
transport binding, and principal/session/target checks then run; only a request that
has passed those checks atomically consumes the nonce immediately before protected
authorization/dispatch. Thus a wrong-source or wrong-principal request cannot burn a
valid envelope. Every TCP/UDS listener uses this handle, so cross-transport and
post-restart replay fails. File ownership is `opdbus:opdbus`, mode `0600` with a
`0700` parent; lock/corruption/durability failure leaves only liveness available and
returns `Unavailable` for authentication, onboarding, and approval—never an in-memory
fallback. Expired records purge only after expiry, maximum skew, and audit retention.
Rollback reopens or forward-restores the newest compatible ledger and never restores
an earlier nonce state.

### 4c. ExecutionContext — immutable, bridge-built, threaded to execution (FR-3d)

**Verified defect.** `Tool::execute(&self, input: Value)`
(`crates/op-mcp/src/tool_registry.rs:41,:89`) receives **only caller-controlled JSON**
— no identity, scope, deadline, approval, or transport binding.

**Decision — an immutable `ExecutionContext` built by the bridge, never by the caller.**
The `Tool` trait signature changes (contract for the implementation phase; this spec
writes no code):

```
struct ExecutionContext {              // immutable; base/target child constructed only by bridge (steps 10c/12b)
    actor: ActorId,                    // resolved principal (identity_sled)
    identity: ResolvedIdentity,
    scope: DerivedScope,               // container/workspace/session/namespace/collection/
                                       //   path-root — DERIVED (FR-3f), not caller-supplied
    granted_capability: CapabilityId,  // what the caller holds for this target
    selected_capability: CapabilityId, // the specific cap exercised
    trace_id: TraceId, event_corr: EventCorrelationId,
    parent_invocation_id: Option<InvocationId>, delegation_depth: u8,
    deadline: Instant, cancel: CancellationToken,      // NFR-4
    approval: Option<VerifiedApproval>,                // Some iff target.approval_required (SEC-10)
    transport: TransportBinding,       // TcpSource(ip) | UdsPeer(uid,gid,pid,mapped_principal)
}

trait Tool { …; async fn execute(&self, ctx: &ExecutionContext, input: Value) -> Result<Value>; }
```

- The base is built at step 10c from validated identity, derived scope, capability,
  and transport binding. Target resolution creates the final immutable/attenuated
  context at step 12b and inserts `VerifiedApproval` only after all OPA1 checks. No
  field comes from caller `arguments`.
- Threads through the single `PluginService.CallMethod`/MutationEngine admission.
  MutationEngine carries it unchanged to `schema_router` or, only after admission,
  to the selected registry implementation; no frontend calls `ToolRegistry`.
- **Meta-tool propagation and attenuation.** Read-only `search_tools`/
  `get_tool_schema` preserve the original context. Dispatching `execute_tool` uses a
  bridge-only constructor to create an attenuated child: actor, transport, approval
  provenance, parent trace, deadline, cancellation, and scope are preserved or
  narrowed; selected capability is replaced with the server-resolved target
  capability; parent invocation is set and delegation depth increments. Both outer
  and target capabilities are required. Caller-supplied child context is impossible,
  and recursive meta-dispatch beyond the configured depth fails before audit intent.
- **Forgery test (FR-3d).** A caller placing `actor`/`container_id`/`workspace`/
  `approval` in `arguments` cannot override any `ExecutionContext` field.

### 4d. Bridge-derived scope; arguments narrow, never replace (FR-3f)

**Verified defect.** Tools accept caller-provided `container_id`/`identity_id`/
`namespace` (`cognitive_tools.rs`) and `session_id`/`collections_from`
(`code_tools.rs`) — self-assertable scope.

**Decision.** All scope (container, identity, namespace, workspace, collection,
session id, path-root) is derived by the bridge into `ExecutionContext.scope` (FR-3d).
Caller `arguments` may **narrow** within the authorized scope (e.g. select one of the
caller's own collections) but never **replace/widen** it. A caller supplying another
identity's `container_id`/`namespace`/`workspace`/`collection`/`session_id` is scoped to
its OWN authorized scope, or denied — never the forged value (FR-3f acceptance).

**Coding-tool scope hardening (FR-3f).** Coding tools additionally enforce, using
`ExecutionContext.scope.path_root` as the workspace root: path **canonicalization**;
rejection of path traversal (`../`) and absolute paths outside the root; rejection of
symlink escape (canonical target must remain under the root); **collection
authorization** (the target collection must be in the caller's authorized set); and
**archive/input size limits** (an oversized archive is rejected). A traversal path, an
out-of-root absolute path, or a symlink escaping the root is rejected (FR-3f
acceptance).

### 4e. Pre-auth telemetry and rate limits, separated from the actor chain (SEC-13)

Rejections before an actor is resolved (steps 0–9) go to a **pre-auth telemetry** sink
(source/transport, reason, timestamp; **no actor, no event-chain hash**). The bridge
never forges an actor for an unauthenticated rejection (SEC-13). Only from step 10+ do
outcomes join the actor-attributed event chain (DR-5, SEC-9). Two rate-limit tiers:
a **pre-auth** bucket (per-source + global, step 0) throttles unauthenticated floods; a
**per-principal** bucket (step 10b) throttles an authenticated principal. Both are
token-bucket timers (permitted by NFR-2).

---

## 5. Tool discovery and execution (FR-3b, FR-4, FR-5, FR-5a, FR-6)

**Decision — sealed declaration is authoritative; runtime descriptors are projections
(FR-3b).** Every registry implementation maps to exactly one method in the
manifest-selected sealed `PluginSchema`. The runtime `Tool`/`ToolDefinition` fields
below are read-only projections of that `MethodDecl`, not independently authored
security metadata:

```
trait Tool {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> Value;
    fn output_schema(&self) -> Value;          // NEW
    fn required_capability(&self) -> &str;      // NEW — target-tool capability
    fn subid(&self) -> &str;                    // NEW — OSCAL subid
    fn side_effect(&self) -> SideEffect;        // NEW — Read | Mutation
    fn idempotent(&self) -> bool;               // NEW
    fn approval_required(&self) -> bool;        // NEW — gates apply/mutating tools
    fn category(&self) -> &str; fn namespace(&self) -> &str; fn tags(&self) -> Vec<String>;
    async fn execute(&self, ctx: &ExecutionContext, input: Value) -> Result<Value>;  // ctx added (FR-3d)
}
```

`ToolDefinition` (in `op_core`) exposes the same projected fields so `list_tools` /
`get_tool_schema` do not create a second contract. Registration compares the
implementation projection to the exact manifest-selected declaration and schema
hash; missing or semantically different fields reject the tool before activation.
The `PluginSchema` Rust/protobuf model remains signed/sealed declarative input for
validation, public projection, reflection, and route generation. The legacy
`plugin_schema.dat` file/component name is removed from runtime, source, deployment,
and documentation; it may appear only in migration/removal notes and negative CI
fixtures. Neither the schema model nor its sealed blob loads code, owns a listener,
selects an executor, or becomes a system component.

**Registration (FR-3c, FR-4a).** `ToolRegistry::register` rejects duplicate names
(returns `Err`, keeps the first) — no silent overwrite for any tool. It **also**
rejects any tool whose `required_capability` or `subid` is unknown to the authoritative
registry (§4a, FR-4a), and any tool/plugin declaring a `"*"` capability grant. A tool
missing `required_capability` or `subid` fails registration (FR-3b/FR-4a acceptance).

**Discovery flow (capability-filtered view, FR-4).** `list_tools`/`search_tools`
project the registry through the caller's granted capabilities: a tool is *listed*
only if the caller holds its `required_capability`. This is a convenience filter, not
a confidentiality boundary (FR-9a) — the authoritative control is at execution.

**Execution flow (FR-3b, FR-5a, FR-6).**

```
execute_tool{tool_name, arguments}      (or invoke_tool)   — carries the caller's ExecutionContext (FR-3d)
  │  outer-envelope validation ({tool_name:string, arguments:object})
  ▼
resolve target tool in ToolRegistry     (FR-3c: unique name)  ── not found → error envelope
  │
  ├─ TARGET capability check: caller grants tool.required_capability? (FR-4a) ── no → DENY (SEC-2 step 11, event: denied)
  ├─ if tool.approval_required: require apply cap AND verified SEC-10 approval  ── missing/mismatch → DENY (FR-12)
  ├─ NESTED validation: validate `arguments` against tool.input_schema (FR-5a)
  │        missing/invalid tool schema → FAIL CLOSED (reject)
  ├─ scope: arguments may NARROW ctx.scope, never replace it (FR-3f)
  ▼
PluginService.CallMethod → MutationEngine admission/audit-intent  [EXACTLY ONCE]
  ▼
tokio::spawn (bounded, deadline, cancellable — NFR-4) → selected in-process implementation
  ▼
OUTPUT validation vs tool.output_schema (FR-3e)  ── invalid → FAIL CLOSED (no return/persist/vectorize/prompt/event)
  ▼
redact secrets + bound size (SEC-11)  ▼
event-chain append (success/failure, DR-5, SEC-9) → response envelope {success, result, error, event_id, event_hash}
```

**Meta-tool `ExecutionContext` propagation (FR-3d).** `execute_tool` passes its own
`ExecutionContext` to the resolved target unchanged (§4c) — it does not rebuild scope
or capability from `arguments`. A target reached via `execute_tool` is authorized and
scoped identically to a direct call.

**D-Bus/MutationEngine is the only execution admission point; implementation remains
in-process (FR-6).** The bridge constructs one `CognitiveMcpServer`/`ToolRegistry` and
stores it behind `MutationEngine`. Every network/UDS/MCP/gRPC/meta-tool call first
enters `PluginService.CallMethod` on the sole well-known D-Bus contract
`org.opdbus.v1.plugins`, crosses MutationEngine admission exactly once, and only then
may MutationEngine select `ToolRegistry::execute`. "In-process" describes where the
implementation object lives; it does not authorize an adapter to invoke it directly.
The HTTP loopback (`10.200.0.2:3003`) and every adapter-to-registry shortcut are
deleted. A route-spy test asserts exactly one admission crossing per logical call,
including `execute_tool`; an idempotency key prevents retry from becoming a second
execution. Rejected alternatives: HTTP loopback, a new D-Bus rendezvous name, or a
direct Axum/Tonic-to-`ToolRegistry` path.

`get_config`/`get_health` stay in-process projection reads; `set_config`/
`restart_service` stay apply-state acknowledgements.

### 5a. MCP read/execute adapter for generated plugin methods (FR-3a)

Generated sealed-plugin methods are **routes**, not registry tools. The MCP adapter
presents a deterministic synthesized projection (`tools/list` + `tools/call`) but
dispatches through `PluginService.CallMethod` → sole D-Bus contract → `MutationEngine`
→ `schema_router`, with the same `ExecutionContext` and SEC-2 checks. It never copies
the method into `ToolRegistry` and never directly executes an implementation.

**Synthesized MCP naming and collisions.** A generated method's canonical tool name
is `plugin.<plugin_id>.<method_name>`, where both components use the sealed canonical
lowercase identifier grammar `[a-z0-9_]+`; display names never participate. Names are
sorted by `(plugin_id, method_name, schema_hash)`. Before publishing a catalog
generation, activation builds one namespace containing registry names, compact
meta-tools, and synthesized names. Any duplicate canonical name, normalization
collision, or collision with a captured legacy alias fails the entire generation;
there is no last-writer-wins or suffixing. A legacy alias may be retained only as an
explicit one-to-one alias in the sealed migration map and cannot collide. Input,
output, capability, subid, side-effect, idempotency, and approval fields are projected
from the same normalized sealed `MethodDecl` used by gRPC/reflection.

### 5b. Output-schema validation before any use (FR-3e)

**Verified gap.** No code validates a tool's **output** today; `output_schema`/
`returns` are used only for proto/reflection generation.

**Decision.** After execution (step 13), and **before** the result is returned to the
caller, persisted to Cozo, vectorized into Qdrant, inserted into a prompt, or written
to the event chain, the dispatcher validates it against the tool's `output_schema`
(using the normalized dialect-aware schema from §2b). Validation failure **fails
closed**: the candidate result is never returned, persisted, vectorized, prompted, or
published. This runs for registry tools and generated methods alike.

**Audit/response atomicity.** "No event artifact" applies to the invalid candidate
payload, not to accountability: every call has a durable redacted outcome. Before a
potential side effect, MutationEngine commits an `execution_intent` containing the
idempotency key, actor, target, schema hash, approval id, and redacted input hash.
The operation id is bridge-issued or validated and bound to actor, derived scope,
target subid, canonical input digest, and policy/schema versions. For an idempotent
method, an identical retry returns the committed response-outbox result; a changed
binding is denied. For a non-idempotent method, any repeated operation id is denied.
This idempotency/audit state is durable across restart.
After execution and output validation it commits, in one event-store transaction,
the terminal outcome plus a response-outbox record. Only that committed outbox record
may be released to the caller or forwarded to any persistence/prompt/vector/event
sink. Invalid output commits `invalid_output` with hashes/diagnostics but no candidate
body. If the terminal commit fails, the bridge returns `Unavailable` and releases no
candidate result. Mutation adapters use `prepare → execute candidate → validate typed
output → commit` whenever the implementation supports transactions, so malformed
output cannot commit prepared state. If an external system cannot participate, the
intent records that limitation before dispatch; invalid output or cancellation after
an external effect records `partial_side_effect` and enqueues an explicit idempotent
reconciliation action. Restart closes or reconciles every open intent before retry.
The design does not falsely claim that a non-transactional D-Bus side effect and the
local event-store transaction are atomic.

---

## 6. Correct code-tool routing (FR-7)

**Decision.** `map_schema_method_to_tool` maps `code_search → code_search`,
`code_context → code_context`, `code_index → code_index` (identity mapping to the
real registered tools in `code_tools.rs`: `CodeSearchTool`/`CodeContextTool`/
`CodeIndexTool`). The prior mis-map to `search_blob_vectors`/`refresh_blob_vectors`
is removed. `search_blob_vectors`/`refresh_blob_vectors` remain their own distinct
tools for the blob-vector flow (FR-8a) — they are not code tools. Rejected
alternative: keep code_* aliased to blob-vector search — it silently returns schema
similarity instead of code results.

---

## 7. Blob schema/catalog reads (FR-8, FR-8a, DR-2)

**Decision — one canonical implementation in `op-mcp` `blob_schema.rs`.** The
`op-mcp` `BlobSchemaExecutor` already implements the full five-tool contract
(`blob_catalog`, `blob_schema`, `blob_manifest`, `blob_methods`, `blob_search`) plus
`blob://<plugin_id>` resources (`resources.rs`). The bridge's in-process registry
registers this one implementation. The duplicate `blob_catalog` in
`op-cognitive-mcp/blob_catalog_tool.rs` is removed (its semantics folded into the
canonical one). Rejected alternative: keep both and reconcile at call time — leaves
two divergeable contracts and trips FR-3c's collision rule.

Canonical `blob_catalog` contract:
- `mode`: `list` (ids only) | `summary` (id + name + version + method count) |
  `full` (every active sealed schema);
- optional `plugin_ids: [String]` and `category: String` filters;
- `full` is **cursor-paginated**, never streamed and never returned as one unbounded
  result. The first request accepts `page_size` (default 50, maximum 100) and no
  cursor; it captures an immutable active-manifest snapshot identified by
  `{catalog_hash, generation}`. Results are ordered lexicographically by
  `(plugin_id, schema_hash)`. The response is
  `{items, next_cursor, catalog_hash, generation}`. `next_cursor` is an opaque,
  integrity-protected token bound to that snapshot, filters, last key, and a five
  minute expiry. Later pages read the captured snapshot even if a new catalog seals
  concurrently. An expired/unknown cursor returns `CURSOR_EXPIRED`; a filter or
  actor change returns `INVALID_CURSOR`. Clients restart at page one to observe a new
  generation. This same snapshot/cursor contract backs MCP `resources/list`.

**Sanitized projection.** Blob tools/resources expose only the public declarative
projection: canonical plugin id/name/version/category, pinned schema hash, public
descriptions, normalized input/output schemas, method names, capability/subid,
side-effect/idempotency/approval metadata, and catalog generation. They remove SHM
and host paths, file offsets/layout, writer/process data, implementation symbols,
internal D-Bus addresses, identity/session/scope projections, secret/config values,
private signature material, and any schema extension not on the explicit public
allowlist. Sanitization occurs before pagination, search indexing, vectorization,
caching, logging, hashing, or response generation. Raw sealed
bytes are never returned through `blob_catalog`, `blob_schema`, search, vectors,
reflection, or `blob://`. If operations proves raw access necessary, it exists only
as a separately named `admin_blob_schema_raw` read method with
`schema.raw.admin` capability, mandatory reason, exact manifest-pinned hash, strict
size bound, and a redacted audit event; it is absent from ordinary MCP resources and
capability-filtered listings.

Source of truth `/dev/shm/opdbus/plugin-blobs/`; `op-blob` sole writer; all these
tools read-only via `read_manifest_plugin_ids_shm()` / `read_plugin_schema_shm()`
(DR-2). `blob_vectors` semantics preserved exactly (FR-8a): user-triggered
`refresh_blob_vectors` only; UUIDv5 point IDs from `plugin_id`; one point per active
plugin; wholesale replace-on-refresh handles removed plugins; the refresh reads the
same manifest/catalog-hash the sealed catalog uses (consistency); `search_blob_vectors`
fails closed when Qdrant is absent (DR-4).

`resources/list` is generated from the current active manifest at request time (or
an arrival-triggered generation cache), not from a registry-construction snapshot.
Seal/unseal atomically swaps the resource generation so removed plugins disappear
and new active plugins appear without a bridge restart. `resources/read` resolves the
manifest-pinned exact hash and returns the same sanitized projection as `blob_schema`.

### 7a. Exact schema-hash blob resolution; atomic vector refresh (DR-8)

**Verified defect.** `read_plugin_schema_shm` resolves `<plugin_id>.` by
**first-prefix match** (`op-blob/src/catalog.rs:352`), so when two blobs for one plugin
id coexist (`<plugin_id>.<hashA>.blob` and `<plugin_id>.<hashB>.blob`) it may return
the wrong generation.

**Decision — resolve by the manifest-pinned exact `<schema_hash16>`.** The reader takes
the plugin id **and** the manifest's pinned `schema_hash16` and opens exactly
`<plugin_id>.<schema_hash16>.blob`; a first-prefix match is a defect and is removed.
With two blobs for one id present, the reader returns the manifest-pinned hash, not an
arbitrary first entry (DR-8 acceptance). A CI grep gate (NFR-7) fails on first-prefix
resolution.

**Atomic blob-vector refresh (DR-8, FR-8a).** `refresh_blob_vectors` builds the new
generation into a staging collection (or a new named generation) and **atomically
swaps** the active alias only on full success. A partial/failed/interrupted refresh
leaves the **previous** generation active and queryable — callers never observe a
half-built collection (DR-8/FR-8a acceptance). Rejected alternative: in-place
mutation of the live collection — an interruption leaves a partially-rebuilt, servable
collection.

---

## 8. Reflection / callability parity and hot-seal bounds (FR-9, FR-9a, DR-3)

**Decision.** Two descriptor layers, both mounted, kept in parity (owned by
`schemars-to-reflection-plugin-pipeline`): (a) build-time `operation_descriptor.bin`
(Struct-typed routes from PluginSchema methods, generated by `build.rs:234`
`generate_plugin_method_routes`) for Rust dispatch; (b) runtime
`ActiveReflectionCatalog` hydrated from the sealed SHM catalog
(`hydrate_reflection_from_shm`, arrival-triggered, then frozen via
`freeze_plugin_method_reflection`) advertising `operation.method.*` only for sealed
plugins, with retired plugins filtered out. Runtime dispatch also flows through
`schema_router`, which reads sealed schemas at call time.

**Parity rule — BOTH directions (FR-9).** Reflection advertises a method ⟺ a mounted
**active callable** route exists: `reflected ⇒ active-callable` (no
advertise-then-UNIMPLEMENTED) **and** `active-callable ⇒ reflected` (no hidden active
route). A statically registered Tonic handler whose activation gate is closed is not
"mounted" for parity purposes. This fixes the current defect where
reflection advertised cognitive methods that fail at runtime (loopback to a dead
`:3003`): once dispatch is in-process (FR-6), the advertised cognitive/generated
methods are callable.

**Hot-seal bounds — the seal-vs-compiled gap (FR-9).** Per-method typed gRPC routes are
generated **statically at build time** (`build.rs:234`), so a newly sealed method
*shape* that was not compiled in has no typed generated route even though dynamic
reflection could otherwise advertise it. The design bounds "sealed ⇒ callable"
precisely:

```
sealed blob arrives (SHM)
   ▼
compatibility + schema-hash validation  ── incompatible / hash-mismatch → REJECT (fail closed), NOT activated
   ▼
does a compiled typed route exist for this method shape?
   ├─ YES → atomically add (plugin_id, method, schema_hash) to ActiveRouteCatalog;
   │        then dynamic reflection advertises it (CALLABLE)
   └─ NO  → do NOT advertise as callable; flag "requires rebuild/redeploy"
            (never advertise-then-UNIMPLEMENTED)
```

- a newly sealed method shape not compiled in requires a **rebuild/redeploy** to
  become callable on the typed generated service; until then it is **not advertised as
  callable** (it may appear flagged as "requires redeploy", never as a live route);
- **compatibility / schema-hash validation runs before activation**; a mismatched or
  incompatible blob fails closed and is not activated;
- **compiled-but-unsealed** methods stay **uncallable** — compilation alone does not
  activate a route; sealing (plus the check above) is required;
- dynamic reflection MUST NOT advertise a method with no mounted callable route.

**Mandatory call-time activation gate.** Every generated typed handler, including a
direct gRPC call that bypasses discovery, checks the immutable active catalog for the
exact `(plugin_id, method_name, schema_hash)` before capability resolution or D-Bus
dispatch. Missing, retired, hash-mismatched, or incompatible entries fail closed with
`FAILED_PRECONDITION`/typed inactive-route error and no implementation call. There is
no "schema missing means gate open" fallback. Seal/unseal builds reflection, MCP
projection, and the active gate set off to the side and swaps the generation
atomically, so callers never observe mixed generations. Fixed hand-written services
(health's authenticated portion, registration, context, Waypipe, memory, reflection)
also appear in the authoritative catalog and in the both-direction parity test.

For every RPC returned by reflection at `:8090`, an authenticated call reaches a
mounted route (normal / typed-error / capability-error — never "unimplemented"), and
every mounted method appears in reflection (FR-9 acceptance, both directions). Rejected
alternative: let dynamic reflection advertise any sealed shape regardless of compiled
routes — reintroduces advertise-then-UNIMPLEMENTED.

**Visibility (FR-9a).** Authenticated callers see the complete **sanitized public
projection** of the schema/reflection catalog (§7); raw sealed JSON, grants,
footprints, tenant metadata, and secret examples/defaults are excluded. Execution is
capability-gated per target (FR-3b). Public schemas describe
services/capability requirements, not identities, so the catalog is not identity-partitioned.
The capability-filtered *listing* (FR-4) is a convenience view, not a confidentiality
boundary. Rejected alternative: dynamically filter reflection per identity — it would
imply schemas are secrets (they are not) and would make `grpcurl describe` results
depend on the caller, breaking tooling.

---

## 9. Code-context retrieval and coding-assistance workflow (FR-12)

```
gather context (session + workspace)
   │  code_context tool (real, FR-7) → session signals + relevant code
   ▼
retrieve evidence: code_search + blob_schema/blob_catalog + live projection + prior outcomes (memory)
   │
   ▼
produce SUGGESTION (read-only): { rationale, evidence[], proposed_diff }   ── suggestion contract, no listener/registry
   │      required_capability = coding.suggest ; side_effect = Read ; approval_required = false
   ▼
(separate step) APPLY: requires coding.apply capability AND a signed single-use approval (SEC-10)
       tool.approval_required = true ; side_effect = Mutation ; stronger capability (FR-3b)
   ▼
verify (INDEPENDENTLY) → record OUTCOME ∈ {accepted, rejected, corrected, failed, successful}  (DR-5, SEC-9)
   ▼
feed INDEPENDENTLY-VERIFIED outcomes into retrieval/ranking (FR-16) — not chatbot self-labels
```

**Decision.** Suggestion generation and application are distinct tools with distinct
capabilities; the apply tool carries `approval_required = true`. The **suggestion
contract is mandatory** for any change-applying tool (typed input/output; the
workspace-root and path rules of FR-3f apply) — no listener, no second registry.
Applying requires **both** `coding.apply` **and** a valid SEC-10 approval token; a
suggestion-only identity (holds `coding.suggest`, not `coding.apply`) is denied at step
11, and even an identity holding `coding.apply` is denied without a valid approval
(FR-12 acceptance). Outcomes are memory records (§11) feeding ranking, and promotion
uses **independently verified** outcomes (§9a), never the chatbot's own success label.

### 9a. Signed single-use approval token bound to the exact change (SEC-10)

**Decision.** Approval is **not** a boolean on the request. For a tool marked
`approval_required`, the bridge requires a signed, single-use approval token whose
signed payload binds:

```
OPA1 ApprovalToken (signed) {
  version, purpose, approver_principal_id, approver_key_id,
  suggestion_id,          // the specific suggestion being applied
  diff_hash,              // hash of the EXACT proposed diff
  workspace, base_revision,   // the workspace + base revision the diff applies to
  actor, session,         // the applying principal + its session
  target_tool, target_subid, policy_version, schema_version,
  issued_at, expiry, nonce
}
```

Verification (at step 10c → `ExecutionContext.approval = Some(VerifiedApproval)`; else
`None` and the apply is denied) checks, **fail-closed** on any mismatch:
- canonical domain-separated OPA1 version/purpose match; the signature validates
  against the authoritative approver registry; approver principal/key is active and
  holds the target approval capability (for example `coding.approve`); the approver
  is distinct from both proposer and applying actor for code changes and system
  curation;
- `diff_hash` equals the hash of the request's actual proposed diff (a modified diff
  fails);
- `base_revision` matches the current workspace base (a moved base fails);
- `actor` and `session` match the requesting `ExecutionContext`;
- target tool/subid and policy/schema versions equal the activated target contract;
- `issued_at` is not future and `expiry` is within the allowed approval lifetime;
- `expiry` not passed;
- `nonce` unused — consumed atomically in the `approval` namespace of the dedicated
  security ledger (§4b), so a reused approval fails across restart and transports and
  cannot collide with an OIA1 nonce namespace.

A mismatched diff hash, wrong base revision, wrong actor/session, expired token, or
reused nonce fails closed (SEC-10 acceptance). Rejected alternative: an
`approved: true` request flag or a session-scoped "approved" bit — either lets a stolen
session or a swapped diff apply an unreviewed change.

Approval issuance is a separate authenticated human/automation action: the reviewer
receives the immutable suggestion, normalized diff, base revision, and calculated
hash; the approval service signs exactly that tuple after an explicit decision. The
apply call carries the resulting envelope. Approve, deny, expiry, verification
failure, consumption, execution, and terminal result are linked by `suggestion_id`,
`approval_id`, and `execution_id` in the audit chain. Invalid signature, input, scope,
target, policy, or base checks occur before nonce consumption, so they cannot burn a
valid OPA1. Revocation is honored until the nonce is consumed; consumption occurs
immediately before mutation admission in the same serialized admission critical
section, so concurrent apply requests cannot both pass.

**Independently-verified promotion (SEC-10, FR-12, FR-15/FR-16).** A lesson/outcome is
promoted (into ranking or the shared-semantic domain) only from an **independently
verified** signal — build/test success, applied-diff acceptance recorded against the
approval, or an explicit human accept/reject — never from the chatbot self-reporting
"this worked."

---

## 10. Chatbot memory injection sequence (FR-13, SEC-6, DR-6)

```
turn start (identity I, container C, workspace W)
   ▼
recall: durable Cozo query scoped to (I,C,W) + domain (DR-6)         ── isolation SEC-6
   ▼
DURABLE-TRUTH check (DR-7): Cozo is source of truth; a deleted/tombstoned memory is
   never surfaced (no tombstone leakage to callers)
   ▼
semantic rank vs current prompt (Qdrant; degrade to recency if Qdrant down, DR-4)
   ▼
select top-k (bounded by token budget)
   ▼
redact secrets + bound size (SEC-11) on every item before it enters the prompt
   ▼
inject each with provenance { source, identity/container, workspace, timestamp, confidence, event_id/hash }
   │   curated SYSTEM memory → instruction/system role (only via authorized curation path)
   │   everything else (user/container, workspace, unverified) → DELIMITED UNTRUSTED DATA (SEC-12)
   ▼
model turn
```

Only relevant top-ranked memories are injected; each carries all six provenance
fields (FR-13). Retrieval is identity/container/workspace-scoped (SEC-6) and
domain-aware (DR-6). **Injection roles (SEC-12).** Only curated system memory may
occupy an instruction/system role, and only via the authorized curation path. All
non-curated memory (user/container, workspace, any unverified domain) is injected as
clearly **delimited untrusted data**, never as system instructions — a memory whose
text mimics a system instruction does not change role assignment or tool selection.
Injected memory content is data, never control-plane instruction (SEC-7): the
injection layer never lets memory text alter capability/auth/routing/tool selection.
**Secret redaction + size bounding (SEC-11)** are applied before any memory enters a
prompt (and, symmetrically, before args/results enter Cozo/Qdrant/log/event — §4 step
13c).

---

## 11. Post-turn persistence, lifecycle, consolidation, evolution (FR-14, FR-15, FR-16, DR-6, SEC-8)

**Post-turn (FR-14).** Replace the `memory_loop.rs` regex with structured extraction:
persist user facts, decisions, corrections, tool results (tool name + arguments +
result), and outcomes. Suggestion feedback (accepted/rejected/corrected) is an
explicit typed record, not a substring match.

**Domains (DR-6).** Every memory is tagged with a domain: system-curated,
chatbot-soul, user/container, workspace/project, or shared-semantic. Provenance +
trust classification are complete (SEC-8) so low-trust content is neither promoted nor
outranks verified content.

**Authoritative data model and schema evolution.** The bridge Cozo store has explicit,
versioned relations rather than opaque prompt strings:

```
memory(memory_id, revision, domain, actor_id, container_id?, workspace_id?, session_id?,
       kind, content_redacted, content_hash, trust, confidence, source_event_id,
       source_memory_ids[], created_at, updated_at, expires_at?, tombstoned_at?)
memory_outbox(outbox_id, memory_id, revision, op, payload_ref, state, attempts, next_attempt_at)
memory_feedback(feedback_id, suggestion_id?, memory_id?, actor_id, outcome,
                verifier, evidence_hash, event_id, created_at)
memory_migration(version, checksum, started_at, committed_at, source_high_watermark,
                 row_count, content_merkle_root)
```

Scope columns are mandatory/nullable according to domain and are included in every
primary/access index; absence never means global access. Content is redacted and
bounded before insertion. Corrections create a monotonic revision; deletion writes a
tombstone. Derived records list their source ids so invalidation is deterministic.
Domain, identity/container/workspace scope, stable key, trust/verification,
provenance, revision, and tombstone ownership are server-derived; caller/model text
cannot author them. Curated-system writes additionally require the promotion/curation
capability and a valid OPA1 approval.
Migrations are ordered, checksummed, forward-only transformations with an explicit
reverse/export procedure; the bridge refuses to open an unknown newer schema.

**Legacy-memory migration and data rollback.** Before changing the writer, quiesce
legacy writes, capture an immutable backup of `/var/lib/op-dbus/chat-memory.db`, and
record its high-water mark, count, and content Merkle root. Import into staging with
deterministic UUIDv5 ids, normalize/redact/validate each row, quarantine invalid rows
without losing them, and verify per-domain counts and hashes. Atomically commit the
schema version and switch the writer only after verification; keep the legacy DB
read-only through the soak window. No dual writer is allowed. Every new-store write
also enters a versioned migration export journal, so binary rollback first quiesces
the bridge, exports post-cutover deltas in the legacy-compatible interchange format,
restores the pre-migration backup, applies the verified delta or explicitly reports
non-representable records, and only then starts the old writer. A Btrfs binary rollback
alone is insufficient and MUST NOT discard post-cutover memory. Destructive legacy
DB/Qdrant cleanup occurs only after the retention and rollback gates pass.

**Cross-store lifecycle via a Cozo-transactional outbox (FR-15, DR-7).** Cozo
(durable) is the source of truth; Qdrant is a derived index. Best-effort dual-write is
insufficient (a crash between the two stores diverges them), so reconciliation is
**durable and idempotent**:

```
memory write / correction / deletion  (one Cozo transaction)
   ├─ upsert/tombstone the memory row        (stable point_id, monotonic revision r)
   └─ enqueue an OUTBOX entry {point_id, op, revision r, payload-ref}   ← same txn (atomic)
                         ▼
reconciler drains OUTBOX → Qdrant
   ├─ idempotent replay keyed by stable point_id                        (re-run ⇒ no duplicate)
   ├─ apply only if entry.revision ≥ Qdrant's stored revision           (stale-revision SUPPRESSED)
   ├─ delete/tombstone → remove the point from Qdrant
   └─ on success: mark OUTBOX entry drained
                         ▼
restart recovery: undrained OUTBOX entries are replayed on startup      (converges Cozo↔Qdrant)
                         ▼
read path: DURABLE-TRUTH check — a semantic hit is confirmed against Cozo before return;
   a deleted/tombstoned memory is NEVER returned to a caller (no tombstone leakage)
```

- **Stable scoped point IDs** are UUIDv5 over canonical
  `(domain, identity, container, workspace, stable_record_key)` and Qdrant payload
  filters carry the same complete server-derived scope. They survive re-embeds while
  identical keys/content in different scopes cannot collide; replay is idempotent.
- **Monotonic revisions** let the reconciler **suppress stale-revision** Qdrant writes.
- **Restart recovery:** killing the bridge mid-reconcile and restarting drains the
  outbox and converges the stores (no lost/duplicated points) — DR-7 acceptance.
- **No tombstone leakage:** a deleted memory is not returned by a semantic query; the
  durable-truth check gates every vector result — DR-7 acceptance.
- Derived/semantic memory is invalidated when its source changes; dedup prevents
  duplicate records; expiry and confidence decay run on **permitted schedule timers**
  (NFR-2 allows expiry/decay timers; this is not state-polling). A transient Qdrant
  failure leaves the outbox entry undrained and is reconciled later — fail toward
  durable truth (DR-4).

Rejected alternative: best-effort write-both-then-hope — it silently diverges on any
partial failure and can leak tombstones or lose points.

**Consolidation / evolution (FR-16).** Repeatedly successful episodic lessons are
promoted to the shared-semantic domain, which requires the promotion capability (DR-6)
and strips per-user private content. The loop is:

```
observe (context/activity) → retrieve (memory+context) → suggest/act →
verify (result) → capture (feedback + tool outcome) → consolidate (lessons) →
improve (retrieval/ranking)
```

Bounded: no change to model weights, policy, capabilities, or auth. Model fine-tuning
requires a separate reviewed dataset/approval; the loop invokes **no** training/
fine-tuning/update API (FR-16 acceptance).

### 11a. Context-awareness relocation, event-driven (FR-10, NFR-2)

The `ContextAwarenessEngine` is owned by the bridge (via `context_engine()`), served
under `:8090` through the shared dispatch catalog/Tonic projection; the independent HTTP/SSE
`context_server` listener is removed. Proactive evaluation is triggered by
`record_activity` events (mpsc) rather than the 5 s `EVALUATION_INTERVAL_MS` poll:
each activity event evaluates its own session's push conditions inline. All signal
types are preserved (file opened, edit applied, build error, test failure, diff
viewed, symbol navigation, tool call, query, context switch, stuck-session, error
assistance, topic changes, idle recovery). Snapshots are session/identity-scoped and
bounded by a prompt/token budget. Legitimate timers (idle detection deadline, SSE
heartbeat if SSE retained) remain (NFR-2). Rejected alternative: keep the 5 s poll —
violates reactive-not-polled and wakes idle sessions needlessly.

**Durable context journal.** Accepted activity is appended before acknowledgement to
a versioned bridge-owned Cozo journal logically separate from user memory, keyed by
`(identity, container, workspace, session, sequence)`, with gap-free monotonic
sequence allocation and append in one transaction per derived scope. Each immutable
entry contains event id/type, sanitized schema-validated bounded payload or hash,
source event, and timestamp; correction is a linked superseding event. A durable
per-scope checkpoint records
the last evaluated cursor, idle-episode generation, and summary state. On restart the
engine loads the checkpoint and replays only later journal entries, making signal
evaluation and recovery pushes idempotent by `(session, cursor, rule_id)`. Journal
failure makes context mutation/subscription unavailable rather than silently
falling back to an in-memory queue. Retention compacts acknowledged entries into a
sanitized summary/checkpoint before pruning under explicit TTL/size limits.

### 11b. Context idle recovery and stream resumption (FR-19, NFR-2)

**Idle recovery — per-session one-shot deadline.** When a session goes idle, the
engine arms a **single one-shot deadline** for that session; when it fires, exactly
**one** recovery push is produced for that idle episode. It is not a repeating timer:
a returning (active) session re-arms the one-shot for the next idle episode. This
fires exactly one recovery per idle episode (FR-19 acceptance) and stays within NFR-2
(a deadline, not state-polling).

**Stream restart/resume cursors.** Each context stream carries the journal's
monotonic sequence in an opaque integrity-protected, expiry-bound cursor bound to the
exact server-derived `(identity,container,workspace,session)` scope; the cursor is
never authorization. Delivery acknowledgements are persisted. On reconnect, the
client presents its last cursor and resumes from the next durable entry across a
bridge restart. A forged, expired, or cross-scope cursor is rejected. If retention
removed the sequence, the bridge returns `CursorExpired` with the retained checkpoint
cursor and bounded sanitized summary; the client acknowledges that checkpoint before
live delivery resumes. It never silently drops a gap or replays full history. An
in-memory ring may cache recent entries but is not resume truth.

### 11c. One authoritative Cozo writer (DR-1, DR-1a)

The bridge is the sole durable writer of cognitive memory. `op-chat` no longer opens
`/var/lib/op-dbus/chat-memory.db`; it reaches memory through the authenticated bridge
(`:8090` memory tools). If the persistent Cozo path is locked/unavailable, durable
memory tools return an `Unavailable`-class error (DR-1a) — the bridge does **not**
silently open an in-memory Cozo and accept production writes. Rejected alternative:
the current silent in-memory fallback in `CognitiveMcpServer::new` — it forks/loses
memory and violates single-writer truth. (op-web's distinct users DB is untouched.)

---

## 12. Error / degradation handling (NFR-3, NFR-4, DR-4, DR-1a, CR-3, SEC-9)

- **Optional-dependency loss (NFR-3):** if Qdrant/Voyage is down, the bridge starts
  and serves all plugins; semantic retrieval degrades to durable/recency ranking
  (DR-4). If durable Cozo is unavailable, memory tools return `Unavailable` (DR-1a);
  the bridge still serves non-memory plugins.
- **Execution safety (NFR-4):** tools run on `tokio::spawn` with bounded concurrency
  (a semaphore), per-call deadlines/timeouts, cancellation propagation wired to MCP
  `cancel` (FR-2a), backpressure on the concurrency bound, and `spawn_blocking` for
  blocking work. A timed-out/cancelled tool emits a timeout/cancelled event (SEC-9).
  Cancellation before commit stops admission/prepared work; after an external commit
  it records `committed` or `reconciliation_required` and never claims the side effect
  was rolled back. Retrying follows the durable operation-id rules of §5b.
- **Audit of failures (SEC-9):** denied, invalid-args, cancelled, timed-out, and
  failed/invalid-output/partial-side-effect calls each close or link to a durable
  intent with the outcome; sensitive arguments/results are redacted/hashed. Killing
  the bridge at intent, prepared, external-effect, validation, terminal-commit, and
  response-release boundaries leaves either a committed result or an open intent
  that restart reconciliation classifies—never an untracked claimed rollback.

### 12a. Public liveness and onboarding / session genesis (FR-18, SEC-13)

**Decision — exactly two public surfaces, both incapable of tool discovery/exec.**

1. **Liveness/health.** Exactly `GET /healthz` is unauthenticated and returns only
   process liveness (no schema, dependency details, tool list, or reflection).
   Everything else — including rich/gRPC health, `tools/list`, reflection, context
   streams, memory, `execute_tool` — requires authentication and is rejected **before
   dispatch** (SEC-2) when unauthenticated (FR-18 acceptance).

2. **Onboarding / session genesis** (distinct from the SEC-2 auth pipeline). The only
   public onboarding route is `POST /genesis/complete`; there is no public start,
   discovery, email lookup, or caller-selected identity endpoint. It accepts exactly
   one canonical, versioned OIG1 Oracle Identity Genesis envelope produced after the
   trusted Oracle decoy verifies WireGuard peer ownership and human-key ownership.

```
Oracle/decoy                           POST /genesis/complete                 bridge
 verifies peer + human-key   ──▶  OIG1 {version, purpose, human_public_key,
 signs canonical envelope          netmaker_inner_ip, derivation_inputs,
                                   decoy_key_id, iat, exp<=15m, nonce}
                                      │ exact parse/body/Origin/CSRF/rate limits
                                      │ trusted-key signature + purpose/time check
                                      │ TCP source/Xray binding == signed inner IP
                                      │ atomic nonce consume in security ledger
                                      ▼
                              atomically derive and anchor principal/session
                              generic success/failure response; no tools/caps/schema
```

The bridge rejects unsigned envelopes, untrusted decoy keys, raw email/user aliases,
caller-selected principal/session ids, wrong purpose/domain, expired/future envelopes,
source-binding mismatches, and nonce replay. Principal/session ids are derived from
signed inputs under a versioned deterministic derivation; duplicate completion is
anti-enumerating and cannot reveal whether an identity exists. Browser calls require
exact Origin and CSRF; non-browser calls do not manufacture an Origin but must retain
the signed transport binding. Body/depth limits and per-source/global rate limits run
before signature work. Success returns only `{status, onboarding_version}`; the first
OIA1 is obtained separately from the Oracle issuance path. All observations are
pre-auth telemetry (§4e), never actor-attributed chain entries. `/genesis/complete`
cannot route into the shared dispatch catalog, list tools/reflection, issue grants,
or execute. Rejected alternatives are a bridge-issued unsigned genesis challenge,
self-asserted identity fields, or bootstrap discovery.

### 12b. Closing the op-web alternate gRPC ingress (TR-4)

**Verified defect.** `op-web :8080` proxies **all** `application/grpc*` to
`https://127.0.0.1:8090` via `crates/op-web/src/grpc_proxy.rs` (`dispatch`
middleware), plus `/jsonrpc`, `/rpc`, `/.well-known/mcp.json` (`mcp_discovery.rs`),
`mcp_smart_router.rs`, and `mcp_discovery.rs`. That is a **full alternate gRPC/MCP
ingress**; removing only `/mcp*` does not close it.

**Decision — delete the proxy; point the dashboard at `:8090` gRPC-Web first.** The
bridge already serves authenticated gRPC-Web directly on `:8090` (`crate::grpc_web::enable`
wrapping `tonic_web::GrpcWebLayer` + CORS, with `.accept_http1(true)`), so the browser
dashboard does not need the `:8080` proxy for framing. Ordered so the dashboard is
never stranded:

**Browser OIA broker.** A browser cannot reuse a static footprint header or hold the
Oracle signing key. The retained op-web dashboard origin exposes a narrow,
non-MCP `POST /api/oia/v1/assertion` broker protected by its authenticated HttpOnly
session, exact Origin, CSRF token, and per-user/source rate limits. The request names
the bridge authority, RPC/MCP method, target name, and a browser-generated request
nonce. The broker asks the Oracle/decoy service for one OIA1 envelope bound to the
resolved human, browser session, trusted Netmaker/Xray source binding, exact target,
short expiry, and nonce; op-web cannot self-sign or alter identity. The browser
attaches that envelope to exactly one gRPC-Web request and discards it. It never
caches/reuses an assertion; after an expiry/replay rejection it may obtain one fresh
assertion and retry once using the same application idempotency key. Broker responses
are `Cache-Control: no-store`, reveal no capabilities, and are unavailable to foreign
Origins. Bridge preflight permits only enumerated methods/OIA metadata and returns
`Vary: Origin`; wildcard, suffix/regex/reflected, `null`, missing, duplicate, or
malformed browser Origins fail before assertion lookup. Cross-origin redirects never
carry OIA/approval metadata. The broker is issuance only, never a gRPC/MCP proxy.

```
1. Bridge :8090 exposes gRPC-Web with exact CORS/Origin and OIA binary-metadata allowlists
2. Deploy and verify the one-use browser OIA broker; static identity headers are rejected
3. Repoint the dashboard client to https://<bridge>:8090 (gRPC-Web)
4. Browser E2E PASSES: fresh OIA per call, replay denied, CORS/Origin/auth/expiry covered ← GATE
5. ONLY THEN delete crates/op-web/src/grpc_proxy.rs (the application/grpc* forwarder)
6. Remove /jsonrpc, /rpc, /.well-known/mcp.json, mcp_discovery, mcp_smart_router  → 404/410
```

`op-web` cannot be pointed off the proxy until step 3 passes (TR-4). Sending
`application/grpc` to `:8080` then no longer reaches `:8090` (proxy deleted); the MCP
aliases return 404/410 (TR-4 acceptance). If, contrary to the default, some non-MCP
gRPC on `:8080` proves necessary, it is replaced by an explicit **non-MCP allowlist**
that cannot reach cognitive/generated/memory/context/tool RPCs — but the default and
preferred outcome is deletion. Rejected alternative: keep the blanket forwarder and
strip only `/mcp*` — it leaves the cognitive/generated/memory RPCs reachable
unauthenticated-relative-to-`:8090`'s own checks via a second door.

---

## 13. Migration / cutover (CR-1, CR-4, CR-6)

Ordered so no consumer is stranded; each step reversible until the irreversible
deletions.

```
Phase 0  capture listeners/consumers for loopback, svc0, Netmaker, Xray and both relays;
         capture tool names, sealed schema hashes, memory counts/high-water marks
Phase 1  AuthenticatedDispatchCatalog + top-level Axum/TLS mux + Tonic/Axum projections;
         dedicated security replay ledger; capability/subid registry; remove wildcard
Phase 2  ExecutionContext and bridge-derived scope; require exactly one
         PluginService.CallMethod/MutationEngine admission before any implementation;
         remove HTTP loopback and every adapter→ToolRegistry shortcut
Phase 3  normalized PluginSchema projections + JSON Schema 2020-12/draft-07 compatibility;
         output validation and audit-intent/terminal-response-outbox atomicity
Phase 4  modern 2026-07-28 stateless MCP matrix + separate bounded legacy matrix;
         deterministic synthesized names; OIG1 /genesis/complete; optional SSE decision
Phase 5  sanitized blob projection + exact snapshot cursor pagination + exact hash reads;
         dynamic resources and atomic vector refresh
Phase 6  event-driven context relocation + durable journal/checkpoints/resume; remove :3003
Phase 7  quiesced/verified memory data migration; one Cozo writer; outbox/tombstones;
         untrusted-delimited injection; rehearse data rollback including post-cutover delta
Phase 8  suggestion/feedback + independent verifier + signed single-use approval ledger
Phase 9  TLS-over-UDS with SNI op-grpc-bridge.internal, 0660 modes and userns mapping;
         migrate and test every UDS client before enforcement
Phase 10 build golden release; run it in an isolated network namespace canary using :8090
         and disposable copies of security/memory data; run native gRPC, gRPC-Web, modern
         MCP, legacy negative cases, D-Bus route spy, blob, context and restart tests
Phase 11 deploy OIA browser broker; point dashboard canary to direct :8090 and pass E2E;
         keep :8080 grpc_proxy until the new release and broker are proven
Phase 12 live atomic listener cutover: stage firewall/cert/runit exact 3-bind config;
         stop fwd-8090 + fwd-nm-mesh-8090; start new bridge; bounded readiness gate;
         on failure keep relays down/firewall closed and restore the prior golden
         snapshot through the canonical authenticated bridge path
Phase 13 after soak, delete op-web grpc_proxy/MCP aliases and standalone listeners;
         reseal/activate exact routes; both-direction active-route/reflection parity
Phase 14 restart/durability/data-rollback acceptance; disable then remove relay dirs only
         after rollback window; zero-trace CI gates and final Btrfs golden snapshot
```

Client inventory migrated **before** Phase 10 deletion: `.mcp.json`,
`~/.factory/mcp.json`, `.kiro/settings/mcp.json`, `deploy/config/*mcp*.json`
(`cognitive-mcp-clients.json`, `mcp-servers.json`, `factory-mcp.json`), container
gateways, and Xray routes (any `mcp.internal`/`:50052`/`:3003` targets repointed to
`:8090`). Duplicate `kiro/specs/` tree edits mirror `.kiro/specs/` (CR-6).

Surviving shims (CR-4): an `op-mcp-server` reduced to a client-only stdio/UDS shim may
keep its name **iff** it opens no listener, owns no registry/DB writer, and always
calls the bridge (`MergedToolExecutor` → bridge `PluginV1.Call invoke_tool`).

---

## 14. Rollback through btrfs deployment snapshots (DEP-2, DEP-3)

Deployment is a btrfs golden-image send/receive via
`CXXFLAGS="-include cstdint" cargo build --workspace --release` then
`sudo deploy/runit/build-golden.sh` (`--dry-run` reviewed first). Rollback = boot the
prior golden subvolume snapshot; no hand-copying of binaries. Network-critical
services (OVS, uplink, DHCP, session bus) are never auto-restarted — the script
reports them for deliberate console action. After restart, sealed routes, identity
projections, and memory access are restored (DEP-3): the bridge re-hydrates
reflection from the sealed catalog and re-opens the single Cozo writer.

Listener cutover failure is fail-closed: stop the failed bridge, keep both relays down,
keep the mesh firewall closed, and restore the prior golden snapshot. Relay service
directories remain present-but-disabled only to support explicit forensic comparison;
they are never restarted as a rollback path. Persistent state is never blindly rolled
back with the binary: the security replay ledger stays at its newest compatible state;
memory follows the backup-plus-delta procedure in §11; context journal and event/audit
stores are backed up and migrated with checksummed schema versions.
Before migration or deletion, one coordinated checkpoint records verified checksums
for cognitive Cozo memory/outbox, context journal/checkpoints, OIA/OIG/OPA replay
nonces, audit/idempotency intents/outcomes, sealed manifest/catalog generation,
active Qdrant aliases/generations, and the sanitized identity-grant version/hash
(never raw live grants). Post-cutover writes are backward-readable or exported and
replayed explicitly; restore logs state the disposition of every delta.
Restore policy reapplies the current sealed identity/grant and listener invariants;
it never resurrects wildcard/schema grants, sentinel identities, retired listeners,
or a consumed nonce merely because an older snapshot contained them.
Rollback refuses to start an older binary that cannot read those versions until the
documented reverse/export migration has completed. This prevents restored binaries
from resurrecting used nonces, losing post-cutover memory, or resetting resume cursors.

---

## 15. Threat model

| Threat | Vector | Mitigation | Reqs |
|---|---|---|---|
| Identity spoofing | self-asserted headers, forged footprint | OIA1 signature + trusted decoy key + binding + HumanPrincipal resolution; no self-asserted/wildcard/sentinel/trusted-local | SEC-2, SEC-3, TR-2 |
| Broad-capability bypass | one `invoke_tool` grant unlocks all tools (incl. shell/python exec) | per-tool `required_capability` enforced at execution; nested arg validation; approval-required for apply | FR-3b, FR-5a, FR-12 |
| Reflection/schema leakage | raw sealed data leaks grants, footprints, defaults, or tenant metadata | all ordinary reflection/blob/vector/search surfaces use the deterministic sanitized public projection; separately authorized raw admin read is audited; execution remains capability-gated | FR-8b, FR-9a, SEC-5 |
| Stored prompt injection | poisoned memory tells model to escalate | memory is data not instruction; injection cannot change caps/auth/routing/tool selection | SEC-7 |
| Memory poisoning | low-trust content ranked/promoted as authoritative | complete provenance + trust classification; no silent promotion; verified outranks unverified | SEC-8, DR-6 |
| Replay | resend a captured identity/genesis/approval envelope | dedicated domain-separated OIA1/OIG1/OPA1 Cozo ledger; lookup before binding and atomic consume only after validation; survives restart and transport changes | SEC-2, SEC-10, SEC-13 |
| Cross-container/identity leakage | read another tenant's memory/context | (I,C,W)-scoped memory + identity-scoped context; shared domain strips private content and needs promotion cap | SEC-5, SEC-6, DR-6 |
| Untrusted local socket | assume local UDS = trusted | TLS-over-UDS + SO_PEERCRED + restricted ownership + namespace mapping + assertion; identical authorization; no local bypass | TR-2, TR-5, SEC-3, SEC-4 |
| Silent memory fork/loss | Cozo lock → ephemeral fallback accepts writes | durable-write path returns `Unavailable`, never silent in-mem substitution | DR-1a |
| Reflection/route drift | advertise methods that fail at runtime | static+dynamic parity; every reflected RPC callable | FR-9, DR-3 |
| Failure-hiding | denial, invalid output, cancellation, or external partial effect is not audited | durable pre-dispatch intent plus linked redacted terminal/partial outcome and restart reconciliation; response releases only after terminal/outbox commit | SEC-9, DR-5 |
| **Session-id theft** | steal/replay an MCP session id to act as the victim | session id is a correlation handle only; every request needs a fresh one-use OIA1; principal+binding of the fresh assertion must match the session record | FR-2a, SEC-2, §2b |
| **Forged scope** | caller supplies another tenant's `container_id`/`namespace`/`workspace`/`collection`/`session_id` | scope is bridge-derived into the immutable ExecutionContext; args may narrow, never replace; coding paths canonicalized + traversal/symlink rejected | FR-3d, FR-3f, §4c–4d |
| **Schema-assigned identity grant** | sealed `PluginSchema.capability_grants` carries wildcard or per-footprint authority | any non-empty field fails activation/reseal; schema declares vocabulary only; all grants derive from active `identity_sled`; no wildcard anywhere | FR-4a, SEC-3, NFR-7 |
| **Caller-supplied capability header** | pass `x-opdbus-capability` to self-authorize | header is non-authoritative; authorization derives from resolved grants; degraded `is_some()&&match` allow-path removed | FR-4a, §4 step 11 |
| **Plaintext / world-writable UDS** | connect to `0o666` plaintext socket, sniff/inject, assume local trust | TLS-over-UDS; socket `0o660` owner/group only; peer-cred + namespace mapping; assertion still required | TR-5, SEC-3, §3a |
| **Alternate :8080 gRPC ingress** | send `application/grpc*` (or `/jsonrpc`,`/rpc`,`/.well-known/mcp.json`) to op-web to reach `:8090` RPCs via a second door | delete `grpc_proxy` + MCP aliases (404/410) after dashboard moves to `:8090` gRPC-Web with a browser-E2E gate | TR-4, §12b |
| **Memory-store divergence / tombstone leakage** | crash between Cozo and Qdrant diverges stores; deleted memory resurfaces | Cozo-transactional outbox; idempotent replay by stable point id; monotonic-revision suppression; restart drain; durable-truth check before returning vector hits | DR-7, §11 |
| **Unsigned / replayed/self approval** | boolean flag, swapped diff, unauthorized signer, or proposer approves itself | domain-separated OPA1 binds approver, target/subid, policy/schema, exact diff/base/scope; active approval capability and separation-of-duty checked before atomic nonce consumption | SEC-10, §9a |
| **Secret leakage into stores/prompts** | credential in tool arg/result flows into Cozo/Qdrant/log/event/prompt | secret-redaction + size-bounding before the terminal/outbox transaction (§4 steps 15–16, §10); invalid bodies never leave the bridge | SEC-11, FR-3e |
| **Seal-vs-compiled route gap** | dynamic reflection advertises a newly sealed method with no compiled typed route → UNIMPLEMENTED, or an incompatible blob activates | both-direction parity; compatibility+schema-hash validation before activation (fail closed); uncompiled shape not advertised as callable ("requires redeploy"); compiled-but-unsealed uncallable | FR-9, §8 |
| **OIA1 replay across restart/transport** | reuse a captured assertion after restart, or reuse a TCP nonce on UDS | dedicated ledger lookup at step 7 and atomic consume at step 10d after binding/principal validation | SEC-13, §4b |
| **Unauthenticated onboarding bypass** | self-assert or use genesis to enumerate/execute | only signed OIG1 at `/genesis/complete`; exact purpose/source/nonce checks, anti-enumerating response, pre-auth limits, no dispatch-catalog edge | FR-18, SEC-13, §12a |
| **Frontend execution bypass** | Axum/Tonic adapter invokes a registry/tool directly | every executable frontend crosses `PluginService.CallMethod`/D-Bus/MutationEngine exactly once; route-spy and operation-id gates | FR-3a, FR-6, §5 |

---

## 16. Conflicts resolved

1. **`CLAUDE.md` "MCP gateways (settled — do not redesign)"** (op-cognitive-mcp as
   universal `:50052` gateway; compact-mcp loopback) is **superseded** by this spec's
   `:8090`-only architecture and by the live host (no `:50052`/`:3003`; op-cognitive-mcp
   down; reduced to a UDS shim). This spec is the authority for MCP architecture; that
   paragraph is stale.
2. **Source lags live/intent.** `mutation_engine.rs` still HTTP-loopbacks to
   `:3003` (`cognitive_mcp_endpoint`, `reqwest`) even though the bridge run script
   and live host are already "in-process, no loopback." FR-6/§5 preserve locality but
   require every frontend to cross the sole D-Bus/MutationEngine admission exactly once.
3. **Waypipe either/or** (requirements FR-11 vs old phase-2) → **retained on `:8090`**
   (§ FR-11), not retired to `:50052`.
4. **SSE either/or** → default removal; retained only if Phase-0 inventory finds a
   live consumer (§2).
5. **Reflection visibility** → see the complete sanitized public projection,
   execute-only-authorized; raw sealed data is administrative (FR-8b/FR-9a, §7–8);
   the "identity A can't read identity B's schemas" framing is retired.
6. **op-web MCP execution** (`/mcp`, `/mcp/compact`, `/mcp/agents*`) conflicts with
   FR-1 → removed (Phase 9); op-web keeps dashboard/REST only.
7. **Duplicate `blob_catalog`** → one canonical impl in `op-mcp/blob_schema.rs` (§7).
8. **Duplicate spec tree** (`kiro/specs/` vs `.kiro/specs/`) → canonical is
   `.kiro/specs/`; both de-conflicted (CR-6).
9. **Corrected listener topology (python relay).** Earlier designs assumed the bridge
   directly bound the mesh `:8090` and referenced `10.200.0.2:8090`. Live truth
   (`sudo ss -lntp`): the bridge binds only `127.0.0.1:8090`; `10.0.0.3:8090` is a
   **`python3` `socket-relay`** (`fwd-8090`), and `10.200.0.2:8090` is not bound.
   `fwd-nm-mesh-8090` also binds Netmaker `100.69.0.1:8090`. Resolved: canonical
   direct-bind set `{127.0.0.1:8090, 10.0.0.3:8090, 100.69.0.1:8090}`; both relays
   retire after a canary and reversible cutover; `10.200.0.2:8090` is dropped; TLS
   SANs, firewall, and runit effective configuration cover every bound address (§2a,
   FR-1).
10. **`:8080` grpc_proxy alternate ingress.** Beyond `/mcp*`, op-web forwards **all**
    `application/grpc*` to `:8090` plus `/jsonrpc`,`/rpc`,`/.well-known/mcp.json`,
    `mcp_smart_router`, `mcp_discovery` — a full second gRPC/MCP door. Resolved: delete
    the proxy and aliases (TR-4, §12b), not merely strip `/mcp*`.
11. **MCP version selection.** Canonical MCP version is **2026-07-28 (stateless
    lifecycle)**; the older `initialize` + `Mcp-Session-Id` stateful model is a bounded,
    explicitly-legacy shim only, and a session id is never authentication (§2b, FR-2a).
12. **op-web cannot leave the proxy prematurely.** The dashboard MUST be repointed to
    the bridge's `:8090` gRPC-Web with a defined CORS/`Origin` policy and pass a
    **browser E2E** *before* the `:8080` proxy is removed (§12b, TR-4). Ordering is a
    hard gate, not a preference.
13. **`tonic::Routes` cannot mount raw MCP HTTP.** "Shared route builder" resolves to
    one authenticated dispatch catalog projected into separate Tonic and Axum route
    values, composed by the top-level TLS/Axum ingress (§2); raw HTTP is never
    represented as a Tonic route.
14. **`plugin_schema.dat` component status.** The old file/component is removed.
    `PluginSchema` survives only as the sealed declarative model that produces public
    descriptors/reflection and admission metadata; execution remains D-Bus-only (§5).
15. **Replay ownership.** OIA1, OIG1, and OPA1 use the dedicated Cozo database
    `/var/lib/op-dbus/auth-replay.db`, not cognitive memory and not an in-process map;
    validation failures before binding do not burn a nonce (§4b).
16. **Binary rollback is not data rollback.** Btrfs rollback is coordinated with
    versioned checkpoints/export for replay, audit/idempotency, memory/outbox, context,
    sealed manifest, Qdrant aliases, and sanitized grant version (§11, §14).
