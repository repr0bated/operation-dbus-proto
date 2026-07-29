//! Live reflection probe: assert frozen per-method descriptors are *activated*.
//!
//! Sealing a plugin's descriptors into the SHM blob and hydrating the reflection
//! catalog only makes them **discoverable**. Mounting them as callable typed gRPC
//! services requires `freeze_plugin_method_reflection()` to run before routes are
//! built, because tonic-reflection is immutable once mounted.
//!
//! Regression this guards: `run_zeroclaw_server` (the path the `op-grpc-bridge`
//! binary actually takes) called `hydrate_reflection_from_shm()` but not
//! `freeze_plugin_method_reflection()`, so the bridge advertised every sealed
//! plugin while serving none of their per-method services. Separately,
//! `cognitive_mcp`'s availability check looked for an s6 service definition on a
//! runit host, marking the plugin unavailable.
//!
//! Ignored by default: requires a running bridge.
//!   cargo test -p op-grpc-bridge --test live_reflection_probe -- --nocapture --ignored

#[tokio::test]
#[ignore = "requires a running bridge on 127.0.0.1:50051"]
async fn cognitive_mcp_per_method_services_are_mounted() {
    use tonic_reflection::pb::v1::{
        server_reflection_client::ServerReflectionClient,
        server_reflection_request::MessageRequest,
        server_reflection_response::MessageResponse, ServerReflectionRequest,
    };

    let channel = tonic::transport::Channel::from_static("http://127.0.0.1:50051")
        .connect()
        .await
        .expect("connect to live bridge :50051");

    let mut client = ServerReflectionClient::new(channel);
    let (tx, rx) = tokio::sync::mpsc::channel(16);
    let request_stream = tokio_stream::wrappers::ReceiverStream::new(rx);
    let mut stream = client
        .server_reflection_info(tonic::Request::new(request_stream))
        .await
        .expect("open reflection stream")
        .into_inner();

    tx.send(ServerReflectionRequest {
        host: String::new(),
        message_request: Some(MessageRequest::ListServices(String::new())),
    })
    .await
    .expect("send ListServices");

    let names: Vec<String> = match stream.message().await.expect("reflection response") {
        Some(resp) => match resp.message_response {
            Some(MessageResponse::ListServicesResponse(r)) => {
                r.service.into_iter().map(|s| s.name).collect()
            }
            other => panic!("unexpected reflection response: {other:?}"),
        },
        None => panic!("no reflection response"),
    };

    let cognitive: Vec<&String> = names
        .iter()
        .filter(|n| n.contains("cognitive_mcp"))
        .collect();

    println!("total services: {}", names.len());
    println!("cognitive_mcp services: {}", cognitive.len());
    for n in &cognitive {
        println!("  {n}");
    }

    assert!(
        !cognitive.is_empty(),
        "no cognitive_mcp per-method services mounted — descriptors were sealed but \
         never activated (is freeze_plugin_method_reflection running before routes \
         are built, and is the plugin reported available?)"
    );

    assert!(
        names
            .iter()
            .any(|n| n == "operation.method.cognitive_mcp.invoke_tool.InvokeToolService"),
        "invoke_tool's typed service is not mounted; found: {cognitive:?}"
    );
}
