//! Identity crate – WireGuard pubkey as identity + OAuth token cache via
//! org.freedesktop.secrets. Zero passwords; the WireGuard handshake is the login.

pub mod anna_scribe;
pub mod gcloud_auth;
pub mod registration;
pub mod schema_bridge;
pub mod session;
pub mod token; // Keeping for now if needed internally
pub mod wireguard;

pub use anna_scribe::{AnnaScribe, PluginSchema, SessionLedger};
pub use gcloud_auth::GCloudAuth;
pub use registration::{generate_magic_link_token, generate_wireguard_keypair, WireGuardKeyPair};
pub use schema_bridge::{run_schema_shuttle, IdentitySled};
pub use session::{Session, SessionManager};
pub use token::{CachedToken, TokenManager};
pub use wireguard::{PeerInfo, WireGuardIdentity};
