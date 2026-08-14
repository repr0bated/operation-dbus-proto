//! Shared tonic `Channel` connector.
//!
//! Supports two endpoint forms so the same `ZEROCLAW_GRPC` value works in dev
//! and in production:
//!   - TCP:  `http://host:port`            — local dev / gRPC-Web
//!   - UDS:  `unix:/run/ghostbridge/container.sock`
//!
//! Production is the Unix-socket form: the assistant container has no IP, so the
//! console reaches the op-grpc-bridge over `/run/ghostbridge/container.sock`
//! (native gRPC + reflection). `tonic::transport::Endpoint::from_shared` only
//! dials TCP authorities, so the `unix:` form is wired through
//! `connect_with_connector` with a `UnixStream` dialer.

use anyhow::{anyhow, Context, Result};
use hyper_util::rt::TokioIo;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::UnixStream;
use tonic::transport::{Channel, Endpoint, Uri};
use tower::service_fn;

/// `EndpointSpec::Auto` tries these in order: Unix socket first (production —
/// the assistant container has no IP), then loopback TCP (dev).
const AUTO_UNIX_SOCKET: &str = "/run/ghostbridge/grpc.sock";
const AUTO_TCP_ENDPOINT: &str = "http://127.0.0.1:8090";

/// Parse a `unix:` endpoint into its filesystem path.
///
/// Accepts `unix:/path`, `unix://authority/path` (authority ignored), and
/// `unix:///path`. Returns `None` for non-`unix:` endpoints.
fn unix_socket_path(endpoint: &str) -> Option<String> {
    let rest = endpoint.strip_prefix("unix:")?;
    let path = match rest.strip_prefix("//") {
        // `unix://authority/path` or `unix:///path` — drop the authority.
        Some(after) => match after.find('/') {
            Some(idx) => &after[idx..],
            None => after,
        },
        // `unix:/path`
        None => rest,
    };
    Some(path.to_string())
}

/// Open a tonic `Channel` to either a `unix:` socket or a TCP `http://` endpoint.
pub async fn connect_channel(endpoint: &str) -> Result<Channel> {
    if let Some(path) = unix_socket_path(endpoint) {
        // The HTTP authority below is a placeholder required by tonic; the
        // connector ignores it and dials the Unix socket instead.
        let channel =
            Endpoint::try_from("http://[::]:50051")
                .context("placeholder URI for Unix-socket endpoint")?
                .connect_with_connector(service_fn(move |_: Uri| {
                    let path = path.clone();
                    async move {
                        Ok::<_, std::io::Error>(TokioIo::new(UnixStream::connect(path).await?))
                    }
                }))
                .await
                .with_context(|| format!("connecting gRPC over Unix socket: {endpoint}"))?;
        Ok(channel)
    } else {
        let channel = Endpoint::from_shared(endpoint.to_string())
            .with_context(|| format!("invalid gRPC endpoint: {endpoint}"))?
            .connect()
            .await
            .with_context(|| format!("connecting gRPC over TCP: {endpoint}"))?;
        Ok(channel)
    }
}

// ----------------- endpoint specs -----------------

/// Where to reach a gRPC server. The tagged representation matches the form
/// used in page JSON, e.g. `{"type":"unix","path":"/run/ghostbridge/grpc.sock"}`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EndpointSpec {
    Unix {
        path: String,
    },
    Tcp {
        host: String,
        port: u16,
    },
    GrpcWeb {
        url: String,
    },
    /// Resolve at connect time: Unix socket, then TCP.
    Auto,
}

impl EndpointSpec {
    /// The `connect_channel` endpoint string for this spec. `Auto` has no
    /// single form — [`ConnectionPool::get_channel`] walks its candidates.
    pub fn endpoint_string(&self) -> Option<String> {
        match self {
            EndpointSpec::Unix { path } => Some(format!("unix:{path}")),
            EndpointSpec::Tcp { host, port } => Some(format!("http://{host}:{port}")),
            EndpointSpec::GrpcWeb { url } => Some(url.clone()),
            EndpointSpec::Auto => None,
        }
    }

    /// Ordered connection candidates. Everything but `Auto` yields exactly one.
    fn candidates(&self) -> Vec<EndpointSpec> {
        match self {
            EndpointSpec::Auto => vec![
                EndpointSpec::Unix {
                    path: AUTO_UNIX_SOCKET.to_string(),
                },
                EndpointSpec::GrpcWeb {
                    url: AUTO_TCP_ENDPOINT.to_string(),
                },
            ],
            other => vec![other.clone()],
        }
    }

    /// Stable cache/display key.
    pub fn key(&self) -> String {
        match self {
            EndpointSpec::Auto => "auto".to_string(),
            other => other.endpoint_string().unwrap_or_default(),
        }
    }
}

impl std::fmt::Display for EndpointSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.key())
    }
}

/// Health of one pooled endpoint.
// Read by the status components in task 7; only `get_channel`/`mark_dead` have
// callers today.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionStatus {
    /// Never dialled, or dropped after a failure.
    Disconnected,
    Connected,
    Error(String),
}

// ----------------- connection pool -----------------

/// Lazily-dialled, cached `Channel` per endpoint.
///
/// tonic `Channel`s are cheap to clone and multiplex internally, so one per
/// endpoint is enough. A channel that errors is dropped and re-dialled on the
/// next request rather than being health-checked in the background — the UI
/// only cares whether the *next* call can go out.
#[derive(Clone, Default)]
pub struct ConnectionPool {
    entries: Arc<Mutex<HashMap<String, PoolEntry>>>,
}

#[derive(Clone)]
struct PoolEntry {
    channel: Option<Channel>,
    status: ConnectionStatus,
}

impl ConnectionPool {
    pub fn new() -> Self {
        Self::default()
    }

    /// Cached channel for `endpoint`, dialling on first use.
    ///
    /// For `Auto`, candidates are tried in priority order and the first that
    /// connects is cached under the `Auto` key, so subsequent calls skip the
    /// walk. Errors mark the entry dead; the next call re-dials.
    pub async fn get_channel(&self, endpoint: &EndpointSpec) -> Result<Channel> {
        let key = endpoint.key();
        if let Some(channel) = self.cached(&key) {
            return Ok(channel);
        }

        let mut last_err = None;
        for candidate in endpoint.candidates() {
            let Some(target) = candidate.endpoint_string() else {
                continue;
            };
            match connect_channel(&target).await {
                Ok(channel) => {
                    self.entries.lock().insert(
                        key,
                        PoolEntry {
                            channel: Some(channel.clone()),
                            status: ConnectionStatus::Connected,
                        },
                    );
                    return Ok(channel);
                }
                Err(e) => last_err = Some(format!("{target}: {e:#}")),
            }
        }

        let message = last_err.unwrap_or_else(|| format!("no candidates for endpoint {endpoint}"));
        self.entries.lock().insert(
            key,
            PoolEntry {
                channel: None,
                status: ConnectionStatus::Error(message.clone()),
            },
        );
        Err(anyhow!("could not connect to {endpoint}: {message}"))
    }

    fn cached(&self, key: &str) -> Option<Channel> {
        let entries = self.entries.lock();
        let entry = entries.get(key)?;
        match entry.status {
            ConnectionStatus::Connected => entry.channel.clone(),
            _ => None,
        }
    }

    /// Drop a channel after a call failed on it, so the next request re-dials.
    pub fn mark_dead(&self, endpoint: &EndpointSpec, error: impl Into<String>) {
        self.entries.lock().insert(
            endpoint.key(),
            PoolEntry {
                channel: None,
                status: ConnectionStatus::Error(error.into()),
            },
        );
    }

    #[allow(dead_code)] // consumed by the task 7 status components
    pub fn status(&self, endpoint: &EndpointSpec) -> ConnectionStatus {
        self.entries
            .lock()
            .get(&endpoint.key())
            .map(|e| e.status.clone())
            .unwrap_or(ConnectionStatus::Disconnected)
    }

    /// Every endpoint dialled so far, with its current status. For status UI.
    #[allow(dead_code)] // consumed by the task 7 status components
    pub fn statuses(&self) -> Vec<(String, ConnectionStatus)> {
        let mut out: Vec<(String, ConnectionStatus)> = self
            .entries
            .lock()
            .iter()
            .map(|(k, v)| (k.clone(), v.status.clone()))
            .collect();
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_socket_path_accepts_all_three_forms() {
        assert_eq!(unix_socket_path("unix:/a.sock").as_deref(), Some("/a.sock"));
        assert_eq!(
            unix_socket_path("unix:///a.sock").as_deref(),
            Some("/a.sock")
        );
        assert_eq!(
            unix_socket_path("unix://authority/a.sock").as_deref(),
            Some("/a.sock")
        );
        assert_eq!(unix_socket_path("http://127.0.0.1:8090"), None);
    }

    #[test]
    fn endpoint_spec_round_trips_through_json() {
        let cases = vec![
            EndpointSpec::Unix {
                path: "/run/ghostbridge/grpc.sock".into(),
            },
            EndpointSpec::Tcp {
                host: "127.0.0.1".into(),
                port: 8090,
            },
            EndpointSpec::GrpcWeb {
                url: "http://127.0.0.1:8090".into(),
            },
            EndpointSpec::Auto,
        ];
        for spec in cases {
            let text = serde_json::to_string(&spec).unwrap();
            let back: EndpointSpec = serde_json::from_str(&text).unwrap();
            assert_eq!(spec, back, "round-trip failed for {text}");
        }
    }

    #[test]
    fn endpoint_spec_parses_page_json_form() {
        let spec: EndpointSpec =
            serde_json::from_str(r#"{"type":"unix","path":"/run/ghostbridge/grpc.sock"}"#).unwrap();
        assert_eq!(
            spec,
            EndpointSpec::Unix {
                path: "/run/ghostbridge/grpc.sock".into()
            }
        );

        let spec: EndpointSpec =
            serde_json::from_str(r#"{"type":"tcp","host":"127.0.0.1","port":8081}"#).unwrap();
        assert_eq!(
            spec,
            EndpointSpec::Tcp {
                host: "127.0.0.1".into(),
                port: 8081
            }
        );
    }

    #[test]
    fn endpoint_strings_match_connect_channel_forms() {
        assert_eq!(
            EndpointSpec::Unix {
                path: "/a.sock".into()
            }
            .endpoint_string()
            .as_deref(),
            Some("unix:/a.sock")
        );
        assert_eq!(
            EndpointSpec::Tcp {
                host: "h".into(),
                port: 1
            }
            .endpoint_string()
            .as_deref(),
            Some("http://h:1")
        );
        assert_eq!(EndpointSpec::Auto.endpoint_string(), None);
    }

    #[test]
    fn auto_prefers_unix_then_tcp() {
        let candidates = EndpointSpec::Auto.candidates();
        assert_eq!(
            candidates[0],
            EndpointSpec::Unix {
                path: AUTO_UNIX_SOCKET.into()
            }
        );
        assert_eq!(
            candidates[1],
            EndpointSpec::GrpcWeb {
                url: AUTO_TCP_ENDPOINT.into()
            }
        );
    }

    #[test]
    fn unknown_endpoint_is_disconnected() {
        let pool = ConnectionPool::new();
        assert_eq!(
            pool.status(&EndpointSpec::Unix {
                path: "/nope.sock".into()
            }),
            ConnectionStatus::Disconnected
        );
    }

    #[tokio::test]
    async fn connecting_to_a_missing_socket_records_an_error() {
        let pool = ConnectionPool::new();
        let endpoint = EndpointSpec::Unix {
            path: "/nonexistent/zeroclaw-test.sock".into(),
        };

        assert!(pool.get_channel(&endpoint).await.is_err());
        assert!(matches!(pool.status(&endpoint), ConnectionStatus::Error(_)));
    }

    #[tokio::test]
    async fn mark_dead_clears_a_cached_channel() {
        let pool = ConnectionPool::new();
        let endpoint = EndpointSpec::Tcp {
            host: "127.0.0.1".into(),
            port: 1,
        };
        pool.mark_dead(&endpoint, "boom");
        assert_eq!(
            pool.status(&endpoint),
            ConnectionStatus::Error("boom".into())
        );
        assert!(pool.cached(&endpoint.key()).is_none());
    }
}
