# Design — privacy-signup-container-provisioning

## 1. Crate Placement

```
crates/
  op-web/
    src/
      handlers/
        privacy.rs          ← verify handler enqueues provisioning task (FR-1)
      users.rs              ← PrivacyUser gains provisioning_status, mcp_token,
                              netmaker_peer_id, ghostbridge_enabled; new UserStore
                              methods (FR-7)
      privacy_provisioner.rs  ← NEW: async provisioning engine (FR-2, FR-3, FR-4, FR-5, FR-6, FR-8)
      privacy_network.rs    ← unchanged: host WG network setup
      privacy_container.rs  ← refactored: delegates to privacy_provisioner
      state.rs              ← AppState gains ProvisioningQueue handle
  op-identity/
    src/
      registration.rs       ← generate_wireguard_keypair already exists; no new code
```

All new source lives under `crates/op-web/src/`. No new crates, no `src/` at workspace root.

---

## 2. Data Model Additions

### 2a. `PrivacyUser` field additions (`users.rs`)

```rust
/// Provisioning lifecycle state
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProvisioningStatus {
    #[default]
    Pending,
    Provisioning,
    Ready,
    Failed,
}

pub struct PrivacyUser {
    // ... existing fields unchanged ...

    /// Derived MCP bearer token: UUID v5(wg_public_key)
    #[serde(default)]
    pub mcp_token: Option<String>,

    /// Whether GhostBridge privacy routing is active for this user
    #[serde(default)]
    pub ghostbridge_enabled: bool,

    /// Netmaker peer ID assigned on GhostBridge registration
    #[serde(default)]
    pub netmaker_peer_id: Option<String>,

    /// Container provisioning lifecycle state
    #[serde(default)]
    pub provisioning_status: ProvisioningStatus,

    /// Last provisioning error message (set on Failed, cleared on Ready)
    #[serde(default)]
    pub provisioning_error: Option<String>,
}
```

`privacy_network_connected: bool` is retained for backward compatibility but always reflects
`provisioning_status == ProvisioningStatus::Ready`.

### 2b. New `UserStore` methods

```rust
impl UserStore {
    /// Transition user to Provisioning state.
    pub async fn mark_provisioning_started(&self, user_id: &str) -> Result<PrivacyUser>;

    /// Transition user to Ready; clears provisioning_error.
    /// Replaces the role of mark_privacy_network_connected.
    pub async fn mark_provisioning_ready(
        &self,
        user_id: &str,
        container_name: String,
        route_id: String,
        mcp_token: String,
        netmaker_peer_id: Option<String>,
    ) -> Result<PrivacyUser>;

    /// Transition user to Failed; records error string.
    pub async fn mark_provisioning_failed(
        &self,
        user_id: &str,
        error: String,
    ) -> Result<PrivacyUser>;
}
```

`mark_privacy_network_connected` is kept as a thin wrapper calling
`mark_provisioning_ready` so existing call sites in `privacy.rs` continue to compile.

### 2c. CozoDB schema columns

The following columns are added to the `privacy_users` CozoScript relation (no schema
migration file needed — CozoDB upserts are schemaless, column order is tracked in
`upsert_privacy_user` / `parse_user_row`):

| column | type | default |
|---|---|---|
| `mcp_token` | `String` | `""` |
| `ghostbridge_enabled` | `String` (`"true"`/`"false"`) | `"false"` |
| `netmaker_peer_id` | `String` | `""` |
| `provisioning_status` | `String` | `"pending"` |
| `provisioning_error` | `String` | `""` |

`parse_user_row` and `user_to_cozo_fields` are extended to include these five columns.
Row index shifts are applied to all subsequent columns; the golden row count becomes 20.

---

## 3. Provisioning Architecture

### 3a. Async background task

The verify handler must not block on provisioning. The pattern is:

```
verify_magic_link() → email_verified = true
       │
       └─ tokio::spawn(run_provisioning(state.clone(), user.id))
              │ returns immediately
              ▼
    HTTP 200 ← caller
```

`run_provisioning` is a free async function in `privacy_provisioner.rs`. It is spawned with
`tokio::spawn`; the `JoinHandle` is dropped (fire-and-forget). Errors are recorded via
`mark_provisioning_failed`.

At `AppState` construction (`state.rs`), a startup task re-queues any users in `Pending` or
`Provisioning` state (NFR-5):

```rust
// state.rs startup
for user in user_store.list_users_by_status(&[ProvisioningStatus::Pending,
                                              ProvisioningStatus::Provisioning]).await {
    tokio::spawn(run_provisioning(state.clone(), user.id));
}
```

### 3b. `privacy_provisioner.rs` — step sequence

```
run_provisioning(state, user_id)
  1. load user; if Ready → return (idempotent, FR-8)
  2. mark_provisioning_started (→ Provisioning)
  3. ensure_host_privacy_network()          [from privacy_network.rs]
  4. provision_incus_container(user)        [FR-2]
       retry(3, backoff=[5s,15s,45s])
         incus info ws-<user_id>  OR  incus launch images:debian/12 ws-<user_id>
         wait_for_init(container)
         install_base_packages(container)
  5. inject_wireguard_identity(container, user)   [FR-3]
       incus exec: write privkey + pubkey, chmod 0600
  6. derive_mcp_token(user.wg_public_key) → UUID v5   [FR-3]
  7. seed_mcp_namespaces(container, user, token)  [FR-4]
       POST http://100.90.37.254:3003/mcp  ×  9 keys
  8. if ghostbridge_enabled:
       register_netmaker_peer(user)          [FR-5]  retry(3, backoff)
     else:
       skip
  9. apply_email_privacy_rule(user)          [FR-6]
       if ghostbridge: clear email in CozoDB
  10. publish_user_privacy_route(user, container_name)
  11. publish_openflow_for_privacy_routes()
  12. mark_provisioning_ready(user_id, container, route, token, peer_id)
```

Any step returning `Err` short-circuits to `mark_provisioning_failed`.

### 3c. Incus interaction — no CLI subprocesses in Rust

Per AGENTS.md §4, CLI subprocess calls (`Command::new("incus")`) are **forbidden in plugin
and service code**. The provisioner calls `incus` commands only via an abstraction boundary
defined in `privacy_container.rs`:

```rust
pub trait IncusDriver: Send + Sync {
    async fn container_exists(&self, name: &str) -> Result<bool>;
    async fn launch(&self, image: &str, name: &str) -> Result<()>;
    async fn exec(&self, container: &str, cmd: &[&str]) -> Result<String>;
}
```

The production implementation (`IncusCliDriver`) uses `tokio::process::Command` internally
and is the only place in the codebase that calls the `incus` CLI. Tests inject a mock.
A native OVSDB/REST Incus driver may replace `IncusCliDriver` in a future iteration without
touching provisioner logic.

---

## 4. WireGuard Identity Resolution

### Decision: server-side keypair is the source of truth

| Option | Verdict | Reason |
|---|---|---|
| A: Generate keypair inside container (shell script behaviour) | Rejected | Creates a second identity; the server-side pubkey is already stored in CozoDB and registered as the user's WG identity at signup. Reconciling two keypairs would require a replace + re-registration flow. |
| B: Server-side keypair injected into container | **Chosen** | Single source of truth. The pubkey already on `PrivacyUser.wg_public_key` is used for MCP token derivation, Netmaker registration, and OpenFlow route publication. Injecting the private key into the container is a one-way write (`chmod 0600`). |

MCP token derivation is deterministic:

```rust
fn derive_mcp_token(pubkey: &str) -> String {
    // UUID v5 with OID namespace, matches uuidgen -v5 -n @oid -N "$PEER_PUBKEY"
    uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, pubkey.as_bytes()).to_string()
}
```

---

## 5. GhostBridge / Netmaker Integration

### 5a. Netmaker API surface

Environment variables consumed by the provisioner:

| Var | Purpose |
|---|---|
| `NETMAKER_API_URL` | Base URL, e.g. `https://netmaker.example.com` |
| `NETMAKER_API_TOKEN` | Bearer token for Netmaker Master API |
| `NETMAKER_NETWORK_NAME` | Network to register the peer in |

### 5b. Registration call

```
POST {NETMAKER_API_URL}/api/nodes/{NETMAKER_NETWORK_NAME}
Authorization: Bearer {NETMAKER_API_TOKEN}
Content-Type: application/json

{
  "publickey": "<user.wg_public_key>",
  "name": "ws-<user_id>",
  "address": "<user.assigned_ip>"
}
```

Response body contains `"id"` — recorded as `PrivacyUser.netmaker_peer_id`.

Idempotency: before POST, the provisioner calls
`GET {NETMAKER_API_URL}/api/nodes/{NETMAKER_NETWORK_NAME}` and checks whether a node with
`publickey == user.wg_public_key` already exists. If found, the existing node ID is used.

### 5c. Email privacy enforcement (FR-6)

```rust
if user.ghostbridge_enabled {
    // Wipe email from CozoDB row; it must not persist
    store.clear_user_email(user_id).await?;
    // Do NOT write identity/email to cognitive-mcp
} else {
    // Write email to cognitive-mcp identity namespace
    mcp_remember(&token, &ns, "email", &user.email).await?;
}
```

`clear_user_email` sets `email = ""` in CozoDB and is the only place allowed to mutate the
email field post-signup.

---

## 6. Sequence Diagram

```
Client            op-web/privacy.rs       UserStore         privacy_provisioner      Incus       Netmaker     cognitive-mcp
  │                     │                     │                     │                   │             │              │
  │ GET /verify?token=  │                     │                     │                   │             │              │
  │────────────────────>│                     │                     │                   │             │              │
  │                     │ verify_magic_link() │                     │                   │             │              │
  │                     │────────────────────>│                     │                   │             │              │
  │                     │  ← PrivacyUser (verified)                 │                   │             │              │
  │                     │                     │                     │                   │             │              │
  │                     │ tokio::spawn ────────────────────────────>│                   │             │              │
  │                     │ (fire-and-forget)    │   mark_provisioning_started()          │             │              │
  │                     │                     │<────────────────────│                   │             │              │
  │  HTTP 200 (Pending) │                     │                     │                   │             │              │
  │<────────────────────│                     │                     │                   │             │              │
  │                     │                     │                     │ incus launch / info             │              │
  │                     │                     │                     │──────────────────>│             │              │
  │                     │                     │                     │ inject WG keys    │             │              │
  │                     │                     │                     │──────────────────>│             │              │
  │                     │                     │                     │ derive mcp_token  │             │              │
  │                     │                     │                     │                   │             │              │
  │                     │                     │                     │ seed namespaces ──────────────────────────────>│
  │                     │                     │                     │ (if ghostbridge) register peer ─────────────>  │
  │                     │                     │   mark_provisioning_ready()             │             │              │
  │                     │                     │<────────────────────│                   │             │              │
```

---

## 7. Integration Points in Existing Code

| File | Change |
|---|---|
| `privacy.rs` — `verify` handler | After `verify_magic_link` succeeds, spawn `run_provisioning`; return `Pending` response immediately. Remove synchronous `provision_verified_user` call from this path. |
| `privacy.rs` — `verify_redirect` | Same spawn pattern; redirect to `/privacy/access?user_id=…` immediately (status shown as "Provisioning in Progress" until Ready). |
| `privacy.rs` — `provision_verified_user` | Retained for Google OAuth path; internally delegates to `run_provisioning` as a blocking `await` (OAuth callers already tolerate latency). |
| `users.rs` | Add five new fields to `PrivacyUser`; extend `parse_user_row` (row width 20), `user_to_cozo_fields`, `upsert_in_cozo`; add three new `UserStore` methods. |
| `privacy_container.rs` | Becomes a thin shim; real logic moves to `privacy_provisioner.rs`. |
| `state.rs` | Startup loop to re-queue `Pending`/`Provisioning` users. |

---

## 8. OSCAL Subid Assignments

All subids registered in `op-plugins` OSCAL registry per AGENTS.md §4a.

| subid | what |
|---|---|
| `src.service.privacy-user.verified@v1` | email_verified transition that fires the provisioning trigger |
| `mut.service.privacy-provisioner.start@v1` | mark_provisioning_started write |
| `mut.service.privacy-provisioner.ready@v1` | mark_provisioning_ready write |
| `mut.service.privacy-provisioner.failed@v1` | mark_provisioning_failed write |
| `mut.service.incus-container.launch@v1` | container launch step |
| `mut.service.wireguard-identity.inject@v1` | private key injection into container |
| `mut.service.netmaker-peer.register@v1` | Netmaker API peer registration |
| `mut.service.privacy-user.clear-email@v1` | GhostBridge email wipe from CozoDB |
| `evt.service.provisioning.completed@v1` | emitted on Ready transition |
| `evt.service.provisioning.failed@v1` | emitted on Failed transition |
| `obs.service.privacy-user.status@v1` | provisioning_status read (UI polling) |

---

## 9. Dependency Additions (`op-web/Cargo.toml`)

No new crates. `reqwest` is already present for OAuth. `uuid` with `v5` feature must be
confirmed enabled; if not, add `features = ["v5"]` to the existing pinned `uuid` dep.
`tokio` with `time` feature is required for retry backoff — confirm it is already enabled.
