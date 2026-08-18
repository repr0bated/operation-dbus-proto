//! Bridge a Tokio UnixStream to a pair of mpsc channels (gRPC chunk edges).

use anyhow::Result;
use bytes::Bytes;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::sync::mpsc;
use tracing::{debug, warn};

const READ_BUF: usize = 64 * 1024;

/// Copy unix → `to_grpc` and `from_grpc` → unix until either side closes.
pub async fn bridge_unix_to_channels(
    mut stream: UnixStream,
    mut from_grpc: mpsc::Receiver<Bytes>,
    to_grpc: mpsc::Sender<Bytes>,
) -> Result<()> {
    let (mut reader, mut writer) = stream.split();

    let upload = async {
        let mut buf = vec![0u8; READ_BUF];
        loop {
            let n = reader.read(&mut buf).await?;
            if n == 0 {
                debug!("unix peer closed (EOF)");
                break;
            }
            if to_grpc
                .send(Bytes::copy_from_slice(&buf[..n]))
                .await
                .is_err()
            {
                debug!("gRPC upload channel closed");
                break;
            }
        }
        anyhow::Ok(())
    };

    let download = async {
        while let Some(chunk) = from_grpc.recv().await {
            if chunk.is_empty() {
                continue;
            }
            writer.write_all(&chunk).await?;
            writer.flush().await?;
        }
        debug!("gRPC download channel closed");
        anyhow::Ok(())
    };

    tokio::select! {
        r = upload => {
            if let Err(e) = r {
                warn!(error = %e, "unix→grpc bridge error");
            }
        }
        r = download => {
            if let Err(e) = r {
                warn!(error = %e, "grpc→unix bridge error");
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::{UnixListener, UnixStream};

    #[tokio::test]
    async fn bridges_bytes_both_ways() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.sock");
        let listener = UnixListener::bind(&path).unwrap();

        let server = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 4];
            sock.read_exact(&mut buf).await.unwrap();
            assert_eq!(&buf, b"ping");
            sock.write_all(b"pong").await.unwrap();
        });

        let stream = UnixStream::connect(&path).await.unwrap();
        let (tx_up, mut rx_up) = mpsc::channel::<Bytes>(8);
        let (tx_down, rx_down) = mpsc::channel::<Bytes>(8);

        let bridge = tokio::spawn(async move {
            bridge_unix_to_channels(stream, rx_down, tx_up)
                .await
                .unwrap();
        });

        tx_down.send(Bytes::from_static(b"ping")).await.unwrap();
        let got = rx_up.recv().await.expect("pong");
        assert_eq!(&got[..], b"pong");

        drop(tx_down);
        let _ = bridge.await;
        server.await.unwrap();
    }
}
