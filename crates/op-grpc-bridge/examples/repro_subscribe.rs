//! Reproduce op-web's StateSync.Subscribe channel against a live bridge.
//!
//! Mirrors op-web's exact client path (`GrpcClientPool` + `RemoteOperationClient`)
//! so transport-level failures (TLS, ALPN, h2 frames) can be observed in
//! isolation with `RUST_LOG=h2=debug,hyper=debug`.
//!
//! Env (same knobs as op-web's run script):
//!   OP_DBUS_GRPC_ADDR            target, default https://localhost:8090
//!   OP_DBUS_GRPC_CA_FILE         CA cert for rustls verification
//!   IDENTITY_SLED_HOST_SESSION_ID host session id for metadata attachment

use std::sync::Arc;

use op_grpc_bridge::grpc_client::{GrpcClientPool, RemoteOperationClient};
use tokio_stream::StreamExt;

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    let addr =
        std::env::var("OP_DBUS_GRPC_ADDR").unwrap_or_else(|_| "https://localhost:8090".to_string());
    eprintln!("dialing {addr}");

    // Raw tonic probe mirroring GrpcClientPool::configure_endpoint, so connect
    // failures surface with their full source chain instead of "transport error".
    {
        use tonic::transport::{Certificate, ClientTlsConfig, Endpoint};
        let endpoint = Endpoint::from_shared(addr.clone()).expect("valid uri");
        let endpoint = if addr.starts_with("https://") {
            let mut tls = ClientTlsConfig::new().with_native_roots();
            if let Ok(path) = std::env::var("OP_DBUS_GRPC_CA_FILE") {
                if !path.trim().is_empty() {
                    let pem = std::fs::read(&path).expect("read CA file");
                    tls = tls.ca_certificate(Certificate::from_pem(pem));
                }
            }
            if let Ok(domain) = std::env::var("OP_DBUS_GRPC_TLS_DOMAIN") {
                if !domain.trim().is_empty() {
                    tls = tls.domain_name(domain);
                }
            }
            endpoint.tls_config(tls).expect("tls config")
        } else {
            endpoint
        };
        match endpoint.connect().await {
            Ok(_) => eprintln!("raw probe: connect OK"),
            Err(e) => {
                eprintln!("raw probe: connect FAILED: {e:?}");
                let mut src: Option<&(dyn std::error::Error + 'static)> =
                    std::error::Error::source(&e);
                while let Some(s) = src {
                    eprintln!("  caused by: {s}");
                    src = s.source();
                }
            }
        }
    }

    let pool = Arc::new(GrpcClientPool::new());
    let client = RemoteOperationClient::new(pool, &addr, "repro-subscribe");

    let mut stream = match client.subscribe(vec![], vec![], vec![]).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("SUBSCRIBE FAILED: {e}");
            std::process::exit(2);
        }
    };
    eprintln!("subscribed OK, reading frames...");

    let mut n = 0usize;
    while let Some(item) = stream.next().await {
        match item {
            Ok(msg) => {
                n += 1;
                if n <= 5 || n % 50 == 0 {
                    eprintln!(
                        "frame {n}: plugin={} path={} property={:?}",
                        msg.plugin_id, msg.object_path, msg.property_name
                    );
                }
                if n >= 250 {
                    break;
                }
            }
            Err(e) => {
                eprintln!("STREAM ERROR after {n} frames: {e}");
                std::process::exit(3);
            }
        }
    }
    eprintln!("done: {n} frames received");
}
