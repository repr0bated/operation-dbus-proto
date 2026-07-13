//! Identity crate – WireGuard pubkey as identity + OAuth token cache via
//! org.freedesktop.secrets. Zero passwords; the WireGuard handshake is the login.

pub mod anna_scribe;
pub mod gcloud_auth;
pub mod host_identity;
pub mod identity_vault;
pub mod recovery;
pub mod registration;
pub mod schema_bridge;
pub mod session;
pub mod token; // Keeping for now if needed internally
pub mod wireguard;

pub use anna_scribe::{AnnaScribe, SessionLedger};
pub use gcloud_auth::GCloudAuth;
pub use host_identity::{
    host_identity, HostIdentity, DECOY_HUB_PUBKEY, FIXTURE_PSK_B64, HOST_PEER_IP, HOST_PEER_PUBKEY,
};
pub use recovery::{
    derive_keypair, generate_seed, load_mnemonic_local, mnemonic_to_seed, provision_local_identity,
    recover_keypair, seed_to_mnemonic, store_mnemonic_local,
};
pub use registration::{
    generate_magic_link_token, generate_wireguard_keypair, generate_wireguard_psk, WireGuardKeyPair,
};
pub use schema_bridge::{
    read_schema_blob, read_sled, read_sled_at, run_schema_shuttle, socket_entries_from_env,
    watch_wireguard_handshakes, write_schema_blob, write_sled, write_sled_from_wg, write_sled_full,
    IdentitySled, SocketEntry, SubidCategory, SubidTaxonomy, SHM_SLED_PATH, SHM_XRAY_CONFIG,
};
pub use session::{
    derive_session_id, derive_session_id_from_psk, session_proof, verify_session_proof, Session,
    SessionManager,
};
pub use token::{CachedToken, TokenManager};
pub use wireguard::{PeerInfo, WireGuardIdentity};
