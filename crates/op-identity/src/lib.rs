//! Identity crate – WireGuard pubkey as identity + OAuth token cache via
//! org.freedesktop.secrets. Zero passwords; the WireGuard handshake is the login.

#![deny(rustdoc::broken_intra_doc_links)]

pub mod gcloud_auth;
pub mod identity_vault;
pub mod oracle_assertion;
pub mod recovery;
pub mod registration;
pub mod schema_bridge;
pub mod session;
pub mod session_genesis;
pub mod token; // Keeping for now if needed internally
pub mod wireguard;

pub use gcloud_auth::GCloudAuth;
pub use recovery::{
    derive_keypair, generate_seed, load_mnemonic_local, mnemonic_to_seed, provision_local_identity,
    recover_keypair, seed_to_mnemonic, store_mnemonic_local,
};
pub use registration::{generate_magic_link_token, generate_wireguard_keypair, WireGuardKeyPair};
pub use schema_bridge::{
    read_schema_blob, read_sled, read_sled_at, run_schema_shuttle, socket_entries_from_env,
    verify_ghostbridge_footprint, verify_session_genesis, write_schema_blob, write_sled,
    write_sled_advance,
    FootprintVerifyError, IdentitySled,
    SocketEntry, SubidCategory, SubidTaxonomy, SHM_SLED_PATH, SHM_XRAY_CONFIG,
};
pub use session::{Session, SessionManager};
pub use token::{CachedToken, TokenManager};
pub use wireguard::{PeerInfo, WireGuardIdentity};
