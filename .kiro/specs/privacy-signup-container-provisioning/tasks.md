# Tasks — privacy-signup-container-provisioning

Tasks are ordered by dependency. Each task lists the requirement IDs it satisfies and its
acceptance criteria. Implement and verify each task independently before moving to the next.

---

## Phase 1 — Data model

### T-01 — Extend `PrivacyUser` and `UserStore`

**Crate:** `crates/op-web/src/users.rs`
**Satisfies:** FR-7, AC-02, AC-04, AC-05, AC-08

#### Work
1. Add `ProvisioningStatus` enum (`Pending`, `Provisioning`, `Ready`, `Failed`) with
   `#[derive(Default)]` defaulting to `Pending`.
2. Add five fields to `PrivacyUser`:
   - `mcp_token: Option<String>`
   - `ghostbridge_enabled: bool`
   - `netmaker_peer_id: Option<String>`
   - `provisioning_status: ProvisioningStatus`
   - `provisioning_error: Option<String>`
3. Update `parse_user_row` to read these five columns (row width: 20 total).
4. Update `user_to_cozo_fields` to include the five new columns.
5. Update `upsert_in_cozo` accordingly.
6. Add `UserStore::mark_provisioning_started`, `mark_provisioning_ready`,
   `mark_provisioning_failed` methods per the design.
7. Add `UserStore::clear_user_email` that sets `email = ""` and writes to CozoDB.
8. Add `UserStore::list_users_by_status` that filters by `provisioning_status`.
9. Keep `mark_privacy_network_connected` as a wrapper calling `mark_provisioning_ready`.

#### Acceptance
- `cargo check -p op-web` passes.
- Unit test `should_default_provisioning_status_to_pending` passes.
- Unit test `should_transition_through_provisioning_states` passes.
- Unit test `should_clear_email_on_mark_clear_user_email` passes.

---

### T-02 — OSCAL subid registry update

**Crate:** `crates/op-plugins`
**Satisfies:** C-07, AGENTS.md §4a

#### Work
Register all eleven subids from design §8 in the canonical OSCAL registry in `op-plugins`.
Locate the existing registry file before creating a new one.

#### Acceptance
- CI subid uniqueness check passes.
- All eleven subids present with `uuid`, `name`, `ns`, `value` fields.

---

## Phase 2 — Provisioning engine

### T-03 — `IncusDriver` trait and `IncusCliDriver` impl

**Crate:** `crates/op-web/src/privacy_container.rs`
**Satisfies:** FR-2, C-01, NFR-8

#### Work
1. Define the `IncusDriver` trait:
   ```rust
   #[async_trait::async_trait]
   pub trait IncusDriver: Send + Sync {
       async fn container_exists(&self, name: &str) -> anyhow::Result<bool>;
       async fn launch(&self, image: &str, name: &str) -> anyhow::Result<()>;
       async fn exec(&self, container: &str, cmd: &[&str]) -> anyhow::Result<String>;
   }
   ```
2. Implement `IncusCliDriver` using `tokio::process::Command`. This is the only place in the
   codebase that calls the `incus` binary.
3. Implement `MockIncusDriver` (cfg(test)) that records calls and returns preset responses.

#### Acceptance
- `cargo check -p op-web` passes.
- Unit test `should_skip_launch_when_container_exists` passes against `MockIncusDriver`.

---

### T-04 — `privacy_provisioner.rs` — core provisioning steps

**New file:** `crates/op-web/src/privacy_provisioner.rs`
**Satisfies:** FR-1, FR-2, FR-3, FR-4, FR-6, FR-7, FR-8, NFR-1, NFR-2

#### Work
1. Create `run_provisioning(state: Arc<AppState>, user_id: String)` as the top-level entry
   point, following the 12-step sequence in design §3b.
2. Implement `inject_wireguard_identity(driver, container, user)` — writes privkey and pubkey
   files, sets `chmod 0600` on privkey. Private key must not appear in any log line.
3. Implement `derive_mcp_token(pubkey: &str) -> String` using
   `uuid::Uuid::new_v5(&Uuid::NAMESPACE_OID, pubkey.as_bytes())`.
4. Implement `seed_mcp_namespaces(container, user, token)` — nine `cognitive_memory` store
   calls; skip `identity/email` when `ghostbridge_enabled`.
5. Implement retry helper: `retry_async(attempts: u8, backoff: &[Duration], f)`.
6. Emit `tracing::error!` (with `subid = "evt.service.provisioning.failed@v1"`) on any step
   failure. Never include the WG private key in any log field.

#### Acceptance
- `cargo check -p op-web` passes.
- Unit test `should_derive_mcp_token_from_pubkey` passes (known input/output pair).
- Unit test `should_provision_full_flow_with_mocks` passes (mock Incus + mock HTTP).
- Unit test `should_skip_email_namespace_when_ghostbridge_enabled` passes.
- Unit test `should_not_log_private_key` passes (capture tracing output, assert absent).
- Unit test `should_be_noop_when_status_is_ready` passes (idempotency, FR-8, AC-03).

---

### T-05 — Netmaker GhostBridge peer registration

**File:** `crates/op-web/src/privacy_provisioner.rs`
**Satisfies:** FR-5, FR-6, AC-05, AC-06

#### Work
1. Implement `register_netmaker_peer(user, http_client)`:
   - Read `NETMAKER_API_URL`, `NETMAKER_API_TOKEN`, `NETMAKER_NETWORK_NAME` from env.
   - `GET …/api/nodes/{network}` — if a node with matching `publickey` exists, return its `id`.
   - Otherwise `POST …/api/nodes/{network}` with `publickey`, `name`, `address`.
   - Return the node `id` string.
2. Wrap in `retry_async(3, &[5s, 15s, 45s], …)`.
3. On non-2xx: return `Err(…)` so the caller transitions to `Failed`.

#### Acceptance
- Unit test `should_register_new_netmaker_peer` passes (mock HTTP 200).
- Unit test `should_reuse_existing_netmaker_peer` passes (mock GET returns existing node).
- Unit test `should_fail_provisioning_on_netmaker_error` passes (mock HTTP 500, AC-06).

---

### T-06 — Email privacy enforcement

**Files:** `crates/op-web/src/privacy_provisioner.rs`, `crates/op-web/src/users.rs`
**Satisfies:** FR-6, C-03, NFR-3, AC-04

#### Work
1. In `run_provisioning`, after the Netmaker step, call `store.clear_user_email(user_id)` when
   `ghostbridge_enabled = true`.
2. In `seed_mcp_namespaces`, gate the `identity/email` write on `!ghostbridge_enabled`.
3. Add `UserStore::clear_user_email` if not already done in T-01.

#### Acceptance
- Unit test `should_wipe_email_from_cozo_when_ghostbridge_on` passes.
- Unit test `should_not_write_email_to_mcp_when_ghostbridge_on` passes.
- Inspection: no `email` field appears in any log line when `ghostbridge_enabled = true`.

---

## Phase 3 — Wiring the verify path

### T-07 — Fire-and-forget spawn in `privacy.rs`

**File:** `crates/op-web/src/handlers/privacy.rs`
**Satisfies:** FR-1, NFR-1, AC-01

#### Work
1. In the `verify` handler, replace the synchronous `provision_verified_user(…).await` call
   with:
   ```rust
   tokio::spawn(run_provisioning(state.clone(), user.id.clone()));
   ```
2. Return `HTTP 200` immediately with `provisioning_status: "pending"` in the response body.
3. In `verify_redirect`, apply the same pattern; redirect immediately (status page shows
   "Provisioning in Progress" until Ready).
4. In `provision_verified_user` (used by Google OAuth), retain the synchronous `await` of
   `run_provisioning` so OAuth callers get a synchronous result.

#### Acceptance
- `cargo check -p op-web` passes.
- Integration test `should_return_200_immediately_on_verify` passes (mock provisioner, AC-01).
- Integration test `should_not_block_verify_on_slow_provisioner` passes (injected 2 s delay,
  HTTP response arrives in < 500 ms).

---

### T-08 — Startup re-queue of incomplete provisioning

**File:** `crates/op-web/src/state.rs`
**Satisfies:** NFR-5, AC-09

#### Work
1. After `UserStore` is initialised, call `list_users_by_status(&[Pending, Provisioning])`.
2. For each returned user, `tokio::spawn(run_provisioning(state.clone(), user.id))`.
3. Log the count at `tracing::info!` level with `subid = "src.service.privacy-user.verified@v1"`.

#### Acceptance
- Unit test `should_requeue_pending_users_on_startup` passes (inject two `Pending` users,
  assert provisioner called twice).

---

## Phase 4 — Tests and CI

### T-09 — Integration test: full provisioning flow

**New test file:** `crates/op-web/tests/privacy_provisioning.rs`
**Satisfies:** AC-02, AC-07, NFR-8

#### Work
1. Spin up an in-memory `UserStore` with a pre-created verified user.
2. Inject `MockIncusDriver` and a mock HTTP server (for cognitive-mcp and Netmaker).
3. Call `run_provisioning` and `.await` the result.
4. Assert:
   - `provisioning_status == Ready`
   - `privacy_container_name == "ws-<user_id>"`
   - `mcp_token == derive_mcp_token(pubkey)`
   - `MockIncusDriver.exec` was called with the `wg` key-injection commands (AC-07)

#### Acceptance
- Test passes with `cargo test -p op-web`.

---

### T-10 — CI lint and test gate

**Satisfies:** NFR-6, NFR-7, AC-10, AC-11

#### Work
1. Confirm `cargo clippy --workspace --all-targets --all-features -- -D warnings` passes.
2. Confirm `cargo fmt --all -- --check` passes.
3. Confirm `cargo test --workspace --all-targets --all-features` passes.
4. Measure coverage on `privacy_provisioner.rs`; ensure ≥ 80 %.

#### Acceptance
- All three commands exit 0 on the feature branch with no new warnings.
- Release build `cargo build --workspace --release` passes.
