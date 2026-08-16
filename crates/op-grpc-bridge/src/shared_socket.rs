//! Shared Unix Domain Socket — Full Tonic Data Plane
//!
//! One host-side UDS carries the whole gRPC stack for every container. There is
//! deliberately **one** socket, not one per container, so the peer has to be
//! identified from kernel state rather than from the path it arrived on.
//!
//! ## Two lifetimes, do not conflate them
//!
//! - **Account** — lifelong. `Argon2(PSK, salt = wireguard_pubkey)`, and the
//!   container's name IS that value. One per human. Never re-minted. This is
//!   the thing `ContainerIdentitySled.session_id` holds. (The field is still
//!   named `session_id` on the wire, in the plugin schema and in Cozo; renaming
//!   it changes the schema hash and therefore the published catalog hash, so it
//!   is a deliberate one-shot change, not something to do in passing.)
//! - **Connection** — transient. Many per account, one per accept on this
//!   socket. The genesis anchor belongs to *this* lifetime: it binds the chain
//!   head and the arrival timestamp, which are properties of an arrival, not of
//!   an account.
//!
//! ## What happens on accept
//!
//! ```text
//! container connects → UDS accept → UdsConnectInfo.peer_cred (kernel)
//!   → peer pid → /proc/<pid>/cgroup → lxc.payload.<name> → account id
//!   → that account's record; no anchor yet? mint one now (this arrival)
//!   → inject x-ghostbridge-genesis + x-ghostbridge-trace-id
//!   → the normal GhostbridgeInterceptor gate applies
//! ```
//!
//! Minting on accept is what makes first contact possible. Minting on "first
//! authenticated mutation" cannot bootstrap: the gate refuses the request that
//! would have created the anchor it is demanding.
//!
//! Socket path: `GHOSTBRIDGE_SOCKET_PATH`, default
//! `/run/ghostbridge/container.sock`.

use std::path::PathBuf;

use op_plugins::state_plugins::identity_sled::ContainerIdentitySled;
use tokio::net::UnixListener;
use tokio_stream::wrappers::UnixListenerStream;
use tonic::transport::server::UdsConnectInfo;
use tracing::info;

/// Default path for the shared container socket.
pub const DEFAULT_SOCKET_PATH: &str = "/run/ghostbridge/container.sock";

// A `uid=session_id` map used to live here, fed by GHOSTBRIDGE_UID_SESSION_MAP.
// It was removed rather than configured: it presumed per-container subuid
// ranges, and this host gives every container the same idmap, so the map could
// never select among accounts. See `account_id_for_cgroup`.

/// The account a connecting peer belongs to, resolved from kernel state.
///
/// The peer's uid cannot answer this. Every container on this host shares one
/// idmap (`Hostid 1000000, Nsid 0, Maprange 1000000000`), so root in *every*
/// container is uid 1000000 here — one uid for all accounts. The peer's **pid**
/// does answer it: Incus places each container's processes in
/// `0::/lxc.payload.<container-name>/…`, and the container's name IS the
/// account id (Argon2 of the WireGuard pubkey). Both the pid and the cgroup are
/// kernel-supplied, so nothing the caller sends can change the answer.
///
/// A peer with no `lxc.payload.` segment is a host process, which maps to the
/// host's own account ("container zero").
fn account_id_for_cgroup(pid: i32) -> Option<String> {
    const PAYLOAD_PREFIX: &str = "lxc.payload.";
    let cgroup = std::fs::read_to_string(format!("/proc/{pid}/cgroup")).ok()?;
    cgroup.lines().find_map(|line| {
        // cgroup v2: "0::/lxc.payload.<name>/init.scope"; v1 lines carry a
        // controller list first, but the payload segment reads the same.
        let path = line.rsplit_once(':')?.1;
        let after = path.split('/').find_map(|seg| seg.strip_prefix(PAYLOAD_PREFIX))?;
        (!after.is_empty()).then(|| after.to_owned())
    })
}

fn account_id_for_peer(cred: &tokio::net::unix::UCred, bridge_uid: u32) -> Option<String> {
    if let Some(account) = cred.pid().and_then(account_id_for_cgroup) {
        return Some(account);
    }
    let uid = cred.uid();
    (uid == 0 || uid == bridge_uid)
        .then(crate::identity_sled_dispatch::host_session_id)
        .flatten()
}

#[cfg(unix)]
fn bridge_uid() -> std::io::Result<u32> {
    use std::os::unix::fs::MetadataExt;
    Ok(std::fs::metadata("/proc/self")?.uid())
}

/// What gets stamped onto a peer's requests once the kernel has told us who it
/// is.
///
/// The cgroup names the *account*; the values below come from that account's
/// record, which is the same record the GhostbridgeInterceptor checks against.
/// Nothing here is derived from the cgroup name itself or from caller-supplied
/// metadata, and nothing reads the retired 152-byte global sled.
#[derive(Debug, Clone)]
pub struct CanonicalPeerIdentity {
    /// Hex genesis anchoring this account's arrival.
    pub genesis_hex: String,
    /// Hex trace_id from the account's record.
    pub trace_id_hex: String,
    /// False when the account has no record, no trace, or an arrival that was
    /// never anchored — see [`ContainerIdentitySled::is_anchored`].
    pub is_valid: bool,
}

impl CanonicalPeerIdentity {
    fn invalid() -> Self {
        Self {
            genesis_hex: String::new(),
            trace_id_hex: String::new(),
            is_valid: false,
        }
    }

    /// What this peer will present downstream, taken from its account record.
    ///
    /// The record is the same one the GhostbridgeInterceptor validates
    /// against, so the two cannot disagree. An account whose arrival was never
    /// anchored yields an invalid identity rather than a blank one that would
    /// pass presence checks.
    pub fn from_record(record: ContainerIdentitySled) -> Self {
        let is_valid = record.is_anchored() && !record.trace_id.is_empty();
        Self {
            genesis_hex: record.genesis.unwrap_or_default(),
            trace_id_hex: record.trace_id,
            is_valid,
        }
    }

    /// Read an account's identity without anchoring it. Callers on the accept
    /// path want [`anchor_this_arrival`] instead, which mints when the account
    /// has arrived for the first time.
    pub fn from_session(session_id: &str) -> Self {
        let engine = match crate::interceptor::engine_handle() {
            Some(engine) => engine,
            None => return Self::invalid(),
        };
        let record = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(
                crate::identity_sled_dispatch::session_record_for_actor(
                    engine.as_ref(),
                    session_id,
                ),
            )
        });
        record.map(Self::from_record).unwrap_or_else(Self::invalid)
    }
}

/// Bind the shared container socket and return a stream suitable for
/// `tonic::transport::Server::serve_with_incoming()`.
///
/// The returned `UnixListenerStream` yields `tokio::net::UnixStream` items
/// that tonic will automatically extract `UdsConnectInfo` (including peer_cred)
/// from via its `Connected` trait implementation.
pub async fn bind_shared_socket() -> std::io::Result<UnixListenerStream> {
    let path = PathBuf::from(
        std::env::var("GHOSTBRIDGE_SOCKET_PATH")
            .unwrap_or_else(|_| DEFAULT_SOCKET_PATH.to_string()),
    );

    // Ensure parent directory exists.
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
        // `create_dir_all` is umask-limited, so it lands 0755 root:root and
        // silently re-breaks the parent every time this process restarts.
        // Unprivileged containers map root to a subuid: at 0755 they can
        // CONNECT to this socket but cannot BIND their own alongside it, which
        // is the only inbound channel a NIC-less container has. That is the
        // regression f9188f96 fixed in opdbus-rundirs-up; keep both in sync.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            tokio::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o777)).await?;
        }
    }

    if let Err(e) = tokio::fs::remove_file(&path).await {
        if e.kind() != std::io::ErrorKind::NotFound {
            return Err(e);
        }
    }

    let listener = UnixListener::bind(&path)?;

    // Set permissions so containers can connect (world-writable).
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o666);
        std::fs::set_permissions(&path, perms)?;
    }

    info!(socket_path = %path.display(), "Shared container socket bound");

    Ok(UnixListenerStream::new(listener))
}

/// Extract peer credentials from a tonic request's extensions.
///
/// This is available on any request served over the UDS path because tonic
/// automatically populates `UdsConnectInfo` for `UnixStream` connections.
pub fn extract_peer_cred<T>(request: &tonic::Request<T>) -> Option<tokio::net::unix::UCred> {
    request
        .extensions()
        .get::<UdsConnectInfo>()
        .and_then(|info| info.peer_cred)
}

/// Mint this arrival's anchor for an account that has none yet.
///
/// Provisioning creates the account record (name, pubkey, trace) but cannot
/// mint the anchor, because an anchor binds a chain head and an arrival
/// timestamp — facts that only exist once someone actually arrives. This is
/// that moment.
///
/// Returns the account's record as it stands afterwards.
fn anchor_this_arrival(account_id: &str) -> Option<ContainerIdentitySled> {
    let engine = crate::interceptor::engine_handle()?;
    tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            let record =
                crate::identity_sled_dispatch::session_record_for_actor(engine.as_ref(), account_id)
                    .await?;
            if record.is_anchored() {
                return Some(record);
            }
            if let Err(error) = engine
                .mint_and_store_genesis(account_id, &record.wireguard_pubkey)
                .await
            {
                tracing::warn!(%error, account_id, "could not anchor this arrival");
                return Some(record);
            }
            crate::identity_sled_dispatch::session_record_for_actor(engine.as_ref(), account_id)
                .await
        })
    })
}

/// UDS identity interceptor — the fabric's equivalent of xray stamping the
/// identity header, for peers that arrive over the shared socket instead of
/// over TLS.
///
/// Nothing here trusts the caller: the account comes from the peer's cgroup and
/// the anchor comes from that account's record, both read after the kernel has
/// told us who connected. Whatever the caller put in these headers is
/// overwritten.
#[allow(clippy::result_large_err)]
pub fn uds_identity_interceptor(
    req: tonic::Request<()>,
) -> Result<tonic::Request<()>, tonic::Status> {
    let cred = extract_peer_cred(&req).ok_or_else(|| {
        tonic::Status::unauthenticated("UDS peer credentials unavailable — connection rejected")
    })?;
    let bridge_uid =
        bridge_uid().map_err(|_| tonic::Status::internal("Unable to resolve bridge process uid"))?;

    let account_id = account_id_for_peer(&cred, bridge_uid).ok_or_else(|| {
        tonic::Status::unauthenticated(
            "UDS peer belongs to no known account — not in an lxc.payload cgroup, and not the host",
        )
    })?;

    let record = anchor_this_arrival(&account_id).ok_or_else(|| {
        tonic::Status::unauthenticated(format!(
            "no identity record for account '{account_id}' — provision it before connecting"
        ))
    })?;

    inject_canonical_identity(req, CanonicalPeerIdentity::from_record(record))
}

#[allow(clippy::result_large_err)]
fn inject_canonical_identity(
    mut req: tonic::Request<()>,
    identity: CanonicalPeerIdentity,
) -> Result<tonic::Request<()>, tonic::Status> {
    if !identity.is_valid {
        return Err(tonic::Status::failed_precondition(
            "Identity sled unavailable or invalid — UDS connection rejected",
        ));
    }

    // Inject the canonical session genesis so the GhostbridgeInterceptor passes.
    let genesis = identity
        .genesis_hex
        .parse::<tonic::metadata::MetadataValue<tonic::metadata::Ascii>>()
        .map_err(|_| tonic::Status::internal("Failed to encode genesis header"))?;
    let tr = identity
        .trace_id_hex
        .parse::<tonic::metadata::MetadataValue<tonic::metadata::Ascii>>()
        .map_err(|_| tonic::Status::internal("Failed to encode trace_id header"))?;

    req.metadata_mut().insert("x-ghostbridge-genesis", genesis);
    req.metadata_mut().insert("x-ghostbridge-trace-id", tr);

    Ok(req)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tonic::Code;

    fn valid_identity() -> CanonicalPeerIdentity {
        CanonicalPeerIdentity {
            genesis_hex: "11".repeat(32),
            trace_id_hex: "22".repeat(16),
            is_valid: true,
        }
    }

    #[test]
    fn canonical_identity_overwrites_caller_metadata() {
        let mut request = tonic::Request::new(());
        request.metadata_mut().insert(
            "x-ghostbridge-genesis",
            "spoofed".parse().expect("test metadata"),
        );
        request.metadata_mut().insert(
            "x-ghostbridge-trace-id",
            "spoofed".parse().expect("test metadata"),
        );

        let request = inject_canonical_identity(request, valid_identity())
            .expect("valid canonical identity should be injected");

        assert_eq!(
            request
                .metadata()
                .get("x-ghostbridge-genesis")
                .expect("genesis")
                .to_str()
                .expect("ASCII footprint"),
            "11".repeat(32)
        );
        assert_eq!(
            request
                .metadata()
                .get("x-ghostbridge-trace-id")
                .expect("trace id")
                .to_str()
                .expect("ASCII trace id"),
            "22".repeat(16)
        );
    }

    #[test]
    fn invalid_canonical_identity_fails_closed() {
        let error = inject_canonical_identity(
            tonic::Request::new(()),
            CanonicalPeerIdentity {
                genesis_hex: String::new(),
                trace_id_hex: String::new(),
                is_valid: false,
            },
        )
        .expect_err("invalid sled identity must be rejected");

        assert_eq!(error.code(), Code::FailedPrecondition);
    }

    #[test]
    fn interceptor_rejects_request_without_uds_connect_info() {
        let error = uds_identity_interceptor(tonic::Request::new(()))
            .expect_err("non-UDS request must not enter the UDS identity path");

        assert_eq!(error.code(), Code::Unauthenticated);
    }

}
