//! Throwaway tool to invoke identity_sled's write_identity via the real
//! PluginService.CallMethod gRPC path, so provisioning goes through the
//! authoritative mutation/notarization pipeline instead of manual patches.
use op_grpc_bridge::grpc_client::{GrpcClientPool, RemoteOperationClient};
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let pubkey = std::env::args()
        .nth(1)
        .expect("usage: provision_identity <wireguard_pubkey> [psk_b64] [ttl_seconds]");
    let psk = std::env::args().nth(2).filter(|s| !s.is_empty());
    let ttl_seconds = std::env::args().nth(3).and_then(|s| s.parse::<i64>().ok());

    let pool = Arc::new(GrpcClientPool::new());
    let client =
        RemoteOperationClient::new(pool, "http://127.0.0.1:8090", "provision-identity-tool");

    let args = simd_json::json!({
        "wireguard_pubkey": pubkey,
        "psk": psk.unwrap_or_default(),
        "ttl_seconds": ttl_seconds,
    });

    let result = client
        .call_method(
            "identity_sled",
            "",
            "",
            "write_identity",
            vec![args],
            "provision-identity-tool",
            "identity_sled.write",
        )
        .await?;

    println!("{}", simd_json::to_string_pretty(&result)?);
    Ok(())
}
