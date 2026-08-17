//! gRPC serve side: accept Tunnel RPCs, spawn `waypipe server`, bridge bytes.

use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use anyhow::{Context, Result};
use bytes::Bytes;
use futures::{Stream, StreamExt};
use tokio::net::UnixListener;
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status, Streaming};
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::auth::{authorize_on_connection, IdentityInterceptor, TunnelIdentity};
use crate::bridge::bridge_unix_to_channels;
use crate::config::TunnelConfig;
use crate::proto::waypipe_tunnel_server::{WaypipeTunnel, WaypipeTunnelServer};
use crate::proto::{client_msg, server_msg, ClientMsg, ServerMsg};

pub struct ServeOpts {
    pub config: TunnelConfig,
}

/// Build the WaypipeTunnel service for mounting on an existing tonic server
/// (cognitive-mcp on :50052). Identity is read from the SHM sled inside each
/// `Tunnel` RPC (see [`authorize_on_connection`]).
pub fn build_tunnel_service(
    config: TunnelConfig,
) -> Result<WaypipeTunnelServer<WaypipeTunnelService>> {
    std::fs::create_dir_all(&config.socket_dir)
        .with_context(|| format!("create socket_dir {}", config.socket_dir.display()))?;
    Ok(WaypipeTunnelServer::new(WaypipeTunnelService {
        cfg: Arc::new(config),
    }))
}

pub async fn serve(opts: ServeOpts) -> Result<()> {
    let addr: std::net::SocketAddr = opts.config.listen.parse().context("parse listen address")?;
    std::fs::create_dir_all(&opts.config.socket_dir)?;
    let svc = WaypipeTunnelService {
        cfg: Arc::new(opts.config),
    };

    let (mut health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter
        .set_serving::<WaypipeTunnelServer<WaypipeTunnelService>>()
        .await;

    let reflection = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(crate::proto::FILE_DESCRIPTOR_SET)
        .build_v1()?;

    info!(%addr, "op-waypipe-grpc listening (identity = SHM sled)");

    tonic::transport::Server::builder()
        .add_service(health_service)
        .add_service(reflection)
        .add_service(WaypipeTunnelServer::with_interceptor(
            svc,
            IdentityInterceptor,
        ))
        .serve(addr)
        .await
        .context("gRPC serve")?;
    Ok(())
}

pub struct WaypipeTunnelService {
    cfg: Arc<TunnelConfig>,
}

#[tonic::async_trait]
impl WaypipeTunnel for WaypipeTunnelService {
    type TunnelStream = Pin<Box<dyn Stream<Item = Result<ServerMsg, Status>> + Send + 'static>>;

    async fn tunnel(
        &self,
        request: Request<Streaming<ClientMsg>>,
    ) -> Result<Response<Self::TunnelStream>, Status> {
        // Prefer interceptor-injected identity; otherwise read SHM sled now.
        let identity = request
            .extensions()
            .get::<TunnelIdentity>()
            .cloned()
            .map(Ok)
            .unwrap_or_else(|| authorize_on_connection(request.metadata()))?;

        let mut inbound = request.into_inner();
        // The first ClientMsg must be TunnelOpen. This reads exactly one message: the
        // previous `loop` never reached a second iteration because every arm either
        // broke or returned, which `clippy::never_loop` (deny-by-default) rejected.
        let open = match inbound.next().await {
            Some(Ok(msg)) => match msg.msg {
                Some(client_msg::Msg::Open(o)) => o,
                Some(client_msg::Msg::Chunk(_)) => {
                    return Err(Status::invalid_argument(
                        "first ClientMsg must be TunnelOpen",
                    ));
                }
                None => {
                    return Err(Status::invalid_argument("empty ClientMsg"));
                }
            },
            Some(Err(e)) => return Err(e),
            None => return Err(Status::cancelled("client closed before TunnelOpen")),
        };

        if open.command.is_empty() {
            return Err(Status::invalid_argument("TunnelOpen.command is empty"));
        }
        self.cfg
            .command_allowed(&open.command)
            .map_err(|e| Status::permission_denied(e.to_string()))?;

        let session = if open.session_id.is_empty() {
            Uuid::new_v4().to_string()
        } else {
            open.session_id.clone()
        };
        let compress = if open.compress.is_empty() {
            self.cfg.default_compress.clone()
        } else {
            open.compress.clone()
        };

        info!(
            session = %session,
            identity = %identity.session_id,
            pubkey = %identity.pubkey_hex,
            command = ?open.command,
            "tunnel open"
        );

        let sock_path = self.cfg.socket_dir.join(format!("srv-{session}.sock"));
        let _ = std::fs::remove_file(&sock_path);

        let listener = UnixListener::bind(&sock_path)
            .map_err(|e| Status::internal(format!("bind {}: {e}", sock_path.display())))?;

        let mut child =
            spawn_waypipe_server(&self.cfg.waypipe_bin, &compress, &sock_path, &open.command)
                .map_err(|e| Status::internal(e.to_string()))?;

        let unix = match tokio::time::timeout(std::time::Duration::from_secs(15), listener.accept())
            .await
        {
            Ok(Ok((stream, _))) => stream,
            Ok(Err(e)) => {
                let _ = child.kill().await;
                return Err(Status::internal(format!("accept waypipe server: {e}")));
            }
            Err(_) => {
                let _ = child.kill().await;
                return Err(Status::deadline_exceeded(
                    "timed out waiting for waypipe server to connect",
                ));
            }
        };

        let (to_client_tx, to_client_rx) = mpsc::channel::<Result<ServerMsg, Status>>(64);
        let (from_client_tx, from_client_rx) = mpsc::channel::<Bytes>(64);

        let _ = to_client_tx
            .send(Ok(ServerMsg {
                msg: Some(server_msg::Msg::Ready(true)),
            }))
            .await;

        // inbound gRPC → unix
        let upload_tx = from_client_tx.clone();
        tokio::spawn(async move {
            while let Some(item) = inbound.next().await {
                match item {
                    Ok(msg) => {
                        if let Some(client_msg::Msg::Chunk(c)) = msg.msg {
                            if upload_tx.send(Bytes::from(c.data)).await.is_err() {
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        warn!(error = %e, "client stream error");
                        break;
                    }
                }
            }
        });

        // unix ↔ channels; outbound gRPC from to_client_tx
        let out_tx = to_client_tx.clone();
        tokio::spawn(async move {
            let (byte_tx, mut byte_rx) = mpsc::channel::<Bytes>(64);
            let bridge = tokio::spawn(async move {
                bridge_unix_to_channels(unix, from_client_rx, byte_tx).await
            });

            while let Some(chunk) = byte_rx.recv().await {
                if out_tx
                    .send(Ok(ServerMsg {
                        msg: Some(server_msg::Msg::Chunk(crate::proto::Chunk {
                            data: chunk.to_vec(),
                        })),
                    }))
                    .await
                    .is_err()
                {
                    break;
                }
            }

            if let Err(e) = bridge.await {
                error!(error = %e, "bridge task join error");
            }
            let _ = child.kill().await;
            let _ = std::fs::remove_file(&sock_path);
        });

        let stream = ReceiverStream::new(to_client_rx);
        Ok(Response::new(Box::pin(stream)))
    }
}

fn spawn_waypipe_server(
    waypipe_bin: &str,
    compress: &str,
    sock: &PathBuf,
    command: &[String],
) -> Result<tokio::process::Child> {
    let mut cmd = Command::new(waypipe_bin);
    cmd.arg("--compress")
        .arg(compress)
        .arg("--socket")
        .arg(sock)
        .arg("server")
        .arg("--");
    for c in command {
        cmd.arg(c);
    }
    cmd.kill_on_drop(true);
    cmd.spawn()
        .with_context(|| format!("spawn {waypipe_bin} server"))
}
