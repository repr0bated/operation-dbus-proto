//! Device pairing for dashboard / egui clients that lack Ghostbridge identity.
//!
//! - `POST /admin/paircode/new` — mint a short-lived pairing code (mesh/localhost only)
//! - `POST /pair` with `X-Pairing-Code` — exchange code for bearer token + sled identity
//!
//! Response shape matches zeroclaw-gui `AuthState::pair`:
//! `{ "token": "...", "hashed_footprint": "...", "trace_id": "..." }`

use axum::{
    body::Body,
    extract::{Extension, Request},
    http::{header, HeaderMap, StatusCode},
    response::Response,
    Json,
};
use op_core::security::AccessZone;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::{info, warn};
use uuid::Uuid;

use crate::state::AppState;

const CODE_TTL: Duration = Duration::from_secs(5 * 60);

#[derive(Clone)]
struct PendingCode {
    expires_at: Instant,
}

struct PairState {
    pending: HashMap<String, PendingCode>,
    /// token → device metadata (session ledger for paired browsers)
    tokens: HashMap<String, PairedSession>,
}

#[derive(Clone)]
struct PairedSession {
    device_name: String,
    device_type: String,
    footprint: String,
    trace_id: String,
    created_at: Instant,
}

impl Default for PairState {
    fn default() -> Self {
        Self {
            pending: HashMap::new(),
            tokens: HashMap::new(),
        }
    }
}

static PAIR_STATE: std::sync::OnceLock<Mutex<PairState>> = std::sync::OnceLock::new();

fn pair_state() -> &'static Mutex<PairState> {
    PAIR_STATE.get_or_init(|| Mutex::new(PairState::default()))
}

fn lock_pair() -> std::sync::MutexGuard<'static, PairState> {
    pair_state()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn generate_code() -> String {
    // 8-char Crockford-ish base32 without ambiguous chars
    const ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let bytes = Uuid::new_v4().as_bytes().to_vec();
    bytes
        .iter()
        .take(8)
        .map(|b| ALPHABET[(*b as usize) % ALPHABET.len()] as char)
        .collect()
}

fn generate_token() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

fn purge_expired(state: &mut PairState) {
    let now = Instant::now();
    state.pending.retain(|_, p| p.expires_at > now);
}

fn json_response(status: StatusCode, body: serde_json::Value) -> Response {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::to_vec(&body).expect("json body serialization"),
        ))
        .expect("response builder")
}

/// The identity a paired browser will present: `(genesis_hex, trace_id_hex)`.
///
/// Reads the host's own record from the identity_sled state in SHM. This used to
/// mmap the global 152-byte sled at `/dev/shm/plugin_schema.dat`, which no
/// longer exists — the genesis work retired every writer of it and moved
/// identity to per-session records. Pairing was the one call site left behind,
/// so it failed with "No such file or directory", which meant a browser could
/// never obtain a footprint, which meant every gRPC-Web call was correctly
/// rejected by the interceptor as "Missing Ghostbridge Identity Sled".
///
/// Which record is "the host" comes from `IDENTITY_SLED_HOST_SESSION_ID`, the
/// same variable the gRPC client path uses, so both agree by construction.
fn read_live_identity() -> Result<(String, String), String> {
    const STATE: &str = "/dev/shm/opdbus/state/identity_sled.json";
    let path = std::env::var("IDENTITY_SLED_STATE").unwrap_or_else(|_| STATE.to_string());
    let session_id = std::env::var("IDENTITY_SLED_HOST_SESSION_ID")
        .map_err(|_| "IDENTITY_SLED_HOST_SESSION_ID is not set".to_string())?;
    let bytes = std::fs::read(&path)
        .map_err(|e| format!("identity state unavailable at {path}: {e}"))?;
    let state: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|e| format!("identity state is not valid json: {e}"))?;

    let record = state
        .get("sleds")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .find(|s| s.get("session_id").and_then(|v| v.as_str()) == Some(session_id.trim()))
        .ok_or_else(|| format!("no identity record for host session '{session_id}'"))?;

    let genesis = record
        .get("genesis")
        .and_then(|v| v.as_str())
        .filter(|g| !g.is_empty())
        .ok_or("host identity has no genesis; its arrival was never anchored")?;
    let trace_id = record
        .get("trace_id")
        .and_then(|v| v.as_str())
        .filter(|t| !t.is_empty())
        .ok_or("host identity has no trace_id")?;

    Ok((genesis.to_string(), trace_id.to_string()))
}

/// `POST /admin/paircode/new` — generate a one-time pairing code.
/// Restricted to Localhost / TrustedMesh so public callers cannot mint codes.
pub async fn paircode_new_handler(
    Extension(_state): Extension<Arc<AppState>>,
    request: Request,
) -> Response {
    let zone = request
        .extensions()
        .get::<AccessZone>()
        .copied()
        .unwrap_or(AccessZone::Public);

    match zone {
        AccessZone::Localhost | AccessZone::TrustedMesh => {}
        _ => {
            warn!(?zone, "paircode mint rejected: zone not trusted");
            return json_response(
                StatusCode::FORBIDDEN,
                json!({
                    "error": "pairing code generation requires mesh or localhost access",
                }),
            );
        }
    }

    let code = generate_code();
    {
        let mut st = lock_pair();
        purge_expired(&mut st);
        st.pending.insert(
            code.clone(),
            PendingCode {
                expires_at: Instant::now() + CODE_TTL,
            },
        );
    }

    info!(%code, "minted dashboard pairing code");
    json_response(
        StatusCode::OK,
        json!({
            "pairing_code": code,
            "expires_in_secs": CODE_TTL.as_secs(),
        }),
    )
}

#[derive(Debug, Deserialize)]
pub struct PairRequest {
    #[serde(default = "default_device_name")]
    pub device_name: String,
    #[serde(default = "default_device_type")]
    pub device_type: String,
}

fn default_device_name() -> String {
    "dashboard".into()
}

fn default_device_type() -> String {
    "web".into()
}

#[derive(Debug, Serialize)]
struct PairResponse {
    token: String,
    hashed_footprint: String,
    trace_id: String,
    device_name: String,
}

/// `POST /pair` — exchange `X-Pairing-Code` for bearer token + sled identity headers.
pub async fn pair_handler(
    Extension(_state): Extension<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<PairRequest>,
) -> Response {
    let code = headers
        .get("x-pairing-code")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_uppercase())
        .filter(|s| !s.is_empty());

    let Some(code) = code else {
        return json_response(
            StatusCode::BAD_REQUEST,
            json!({ "error": "missing X-Pairing-Code header" }),
        );
    };

    {
        let mut st = lock_pair();
        purge_expired(&mut st);
        match st.pending.remove(&code) {
            Some(pending) if pending.expires_at > Instant::now() => {}
            Some(_) => {
                return json_response(
                    StatusCode::UNAUTHORIZED,
                    json!({ "error": "pairing code expired" }),
                );
            }
            None => {
                return json_response(
                    StatusCode::UNAUTHORIZED,
                    json!({ "error": "invalid pairing code" }),
                );
            }
        }
    }

    let (footprint, trace_id) = match read_live_identity() {
        Ok(v) => v,
        Err(e) => {
            warn!(error = %e, "pair failed: no live identity");
            return json_response(StatusCode::SERVICE_UNAVAILABLE, json!({ "error": e }));
        }
    };

    let token = generate_token();
    {
        let mut st = lock_pair();
        st.tokens.insert(
            token.clone(),
            PairedSession {
                device_name: body.device_name.clone(),
                device_type: body.device_type.clone(),
                footprint: footprint.clone(),
                trace_id: trace_id.clone(),
                created_at: Instant::now(),
            },
        );
    }

    info!(
        device = %body.device_name,
        device_type = %body.device_type,
        "dashboard device paired"
    );

    json_response(
        StatusCode::OK,
        serde_json::to_value(PairResponse {
            token,
            hashed_footprint: footprint,
            trace_id,
            device_name: body.device_name,
        })
        .expect("pair response"),
    )
}

/// Look up a paired session by bearer token (for future request middleware).
#[allow(dead_code)]
pub fn lookup_paired_token(token: &str) -> Option<(String, String)> {
    let st = lock_pair();
    st.tokens
        .get(token)
        .map(|s| (s.footprint.clone(), s.trace_id.clone()))
}
