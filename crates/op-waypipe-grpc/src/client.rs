//! Laptop-side launch: local `waypipe client` + gRPC Tunnel authenticated from SHM sled.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use bytes::Bytes;
use futures::StreamExt;
use tokio::net::UnixStream;
use tokio::process::Command;
use tokio::sync::mpsc;
use tonic::Request;
use tracing::{info, warn};
use uuid::Uuid;

use crate::bridge::bridge_unix_to_channels;
use crate::config::TunnelConfig;
use crate::proto::waypipe_tunnel_client::WaypipeTunnelClient;
use crate::proto::{client_msg, server_msg, ClientMsg, TunnelOpen};

pub struct LaunchOpts {
    pub config: TunnelConfig,
    pub grpc_endpoint: String,
    pub command: Vec<String>,
    pub compress: Option<String>,
    pub client_socket: Option<PathBuf>,
}

pub async fn launch(opts: LaunchOpts) -> Result<()> {
    if opts.command.is_empty() {
        bail!("command argv is empty");
    }
    opts.config.command_allowed(&opts.command)?;

    let session = Uuid::new_v4().to_string();
    let sock = opts.client_socket.unwrap_or_else(|| {
        opts.config
            .socket_dir
            .join(format!("cli-{session}.sock"))
    });
    std::fs::create_dir_all(&opts.config.socket_dir)?;
    let _ = std::fs::remove_file(&sock);

    let compress = opts
        .compress
        .unwrap_or_else(|| opts.config.default_compress.clone());

    // Identity is resolved on the server from the SHM sled at connection time.
    // Laptop clients do not need a local sled or Ghostbridge headers.

    let mut child = Command::new(&opts.config.waypipe_bin)
        .arg("--compress")
        .arg(&compress)
        .arg("--socket")
        .arg(&sock)
        .arg("client")
        .kill_on_drop(true)
        .spawn()
        .with_context(|| {
            format!(
                "spawn waypipe client (`{}` not found — install waypipe on this machine)",
                opts.config.waypipe_bin
            )
        })?;

    // Wait until the client socket is accepting.
    let unix = wait_connect_unix(&sock, Duration::from_secs(10))
        .await
        .context("connect to local waypipe client socket")?;

    let endpoint = if opts.grpc_endpoint.starts_with("http://")
        || opts.grpc_endpoint.starts_with("https://")
    {
        opts.grpc_endpoint.clone()
    } else {
        format!("http://{}", opts.grpc_endpoint)
    };

    let channel = tonic::transport::Endpoint::from_shared(endpoint.clone())?
        .connect_timeout(Duration::from_secs(10))
        .connect()
        .await
        .with_context(|| format!("connect gRPC {endpoint}"))?;

    let mut client = WaypipeTunnelClient::new(channel);

    let (grpc_out_tx, grpc_out_rx) = mpsc::channel::<ClientMsg>(64);
    let (to_unix_tx, to_unix_rx) = mpsc::channel::<Bytes>(64);
    let (from_unix_tx, mut from_unix_rx) = mpsc::channel::<Bytes>(64);

    grpc_out_tx
        .send(ClientMsg {
            msg: Some(client_msg::Msg::Open(TunnelOpen {
                session_id: session.clone(),
                command: opts.command.clone(),
                compress: compress.clone(),
            })),
        })
        .await?;

    let outbound = tokio_stream::wrappers::ReceiverStream::new(grpc_out_rx);
    let request = Request::new(outbound);

    info!(
        session = %session,
        endpoint = %endpoint,
        command = ?opts.command,
        "launching waypipe tunnel (server reads identity sled on connection)"
    );

    let response = client.tunnel(request).await.context("Tunnel RPC")?;
    let mut inbound = response.into_inner();

    // Wait for ready
    let ready = inbound.next().await;
    match ready {
        Some(Ok(msg)) => match msg.msg {
            Some(server_msg::Msg::Ready(true)) => info!("remote waypipe server ready"),
            Some(server_msg::Msg::Error(e)) => bail!("server error: {e}"),
            other => bail!("expected Ready, got {other:?}"),
        },
        Some(Err(e)) => return Err(e.into()),
        None => bail!("server closed before Ready"),
    }

    let bridge = tokio::spawn(async move {
        bridge_unix_to_channels(unix, to_unix_rx, from_unix_tx).await
    });

    let up = {
        let grpc_out_tx = grpc_out_tx.clone();
        tokio::spawn(async move {
            while let Some(chunk) = from_unix_rx.recv().await {
                if grpc_out_tx
                    .send(ClientMsg {
                        msg: Some(client_msg::Msg::Chunk(crate::proto::Chunk {
                            data: chunk.to_vec(),
                        })),
                    })
                    .await
                    .is_err()
                {
                    break;
                }
            }
        })
    };

    let down = tokio::spawn(async move {
        while let Some(item) = inbound.next().await {
            match item {
                Ok(msg) => match msg.msg {
                    Some(server_msg::Msg::Chunk(c)) => {
                        if to_unix_tx.send(Bytes::from(c.data)).await.is_err() {
                            break;
                        }
                    }
                    Some(server_msg::Msg::Error(e)) => {
                        warn!(error = %e, "server error frame");
                        break;
                    }
                    Some(server_msg::Msg::Ready(_)) => {}
                    None => {}
                },
                Err(e) => {
                    warn!(error = %e, "inbound gRPC error");
                    break;
                }
            }
        }
    });

    tokio::select! {
        r = bridge => { if let Err(e) = r { warn!(error = %e, "bridge join"); } }
        r = up => { if let Err(e) = r { warn!(error = %e, "upload join"); } }
        r = down => { if let Err(e) = r { warn!(error = %e, "download join"); } }
        status = child.wait() => {
            warn!(?status, "waypipe client exited");
        }
    }

    let _ = child.kill().await;
    let _ = std::fs::remove_file(&sock);
    Ok(())
}

async fn wait_connect_unix(path: &PathBuf, timeout: Duration) -> Result<UnixStream> {
    let start = std::time::Instant::now();
    loop {
        match UnixStream::connect(path).await {
            Ok(s) => return Ok(s),
            Err(e) => {
                if start.elapsed() > timeout {
                    bail!("unix connect {}: {e}", path.display());
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
    }
}
