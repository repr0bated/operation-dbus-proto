//! Builds a DescriptorPool from a server's gRPC reflection (v1) service.

use std::collections::HashMap;

use prost::Message;
use prost_reflect::DescriptorPool;
use prost_types::FileDescriptorProto;
use tonic::transport::Channel;
use tonic_reflection::pb::v1::{
    server_reflection_client::ServerReflectionClient, server_reflection_request::MessageRequest,
    server_reflection_response::MessageResponse, ServerReflectionRequest,
};

pub async fn build_pool(channel: Channel) -> anyhow::Result<DescriptorPool> {
    let mut client = ServerReflectionClient::new(channel);

    let services = match call(&mut client, MessageRequest::ListServices(String::new())).await? {
        MessageResponse::ListServicesResponse(list) => {
            list.service.into_iter().map(|s| s.name).collect::<Vec<_>>()
        }
        other => anyhow::bail!("unexpected reflection response: {other:?}"),
    };

    let mut files: HashMap<String, FileDescriptorProto> = HashMap::new();
    for service in &services {
        if service.starts_with("grpc.reflection") {
            continue;
        }
        let resp = call(&mut client, MessageRequest::FileContainingSymbol(service.clone())).await?;
        if let MessageResponse::FileDescriptorResponse(fdr) = resp {
            for bytes in fdr.file_descriptor_proto {
                let fd = FileDescriptorProto::decode(&bytes[..])?;
                files.entry(fd.name().to_string()).or_insert(fd);
            }
        }
    }

    // Reflection returns files in arbitrary order; add until dependencies resolve.
    let mut pool = DescriptorPool::new();
    let mut remaining: Vec<FileDescriptorProto> = files.into_values().collect();
    while !remaining.is_empty() {
        let before = remaining.len();
        let mut next = Vec::new();
        for fd in remaining {
            if pool.add_file_descriptor_proto(fd.clone()).is_err() {
                next.push(fd);
            }
        }
        remaining = next;
        if remaining.len() == before {
            anyhow::bail!(
                "unresolvable proto dependencies: {:?}",
                remaining.iter().map(|f| f.name().to_string()).collect::<Vec<_>>()
            );
        }
    }
    Ok(pool)
}

async fn call(
    client: &mut ServerReflectionClient<Channel>,
    req: MessageRequest,
) -> anyhow::Result<MessageResponse> {
    let request = ServerReflectionRequest {
        host: String::new(),
        message_request: Some(req),
    };
    let mut stream = client
        .server_reflection_info(tokio_stream::once(request))
        .await?
        .into_inner();
    let resp = stream
        .message()
        .await?
        .ok_or_else(|| anyhow::anyhow!("empty reflection response"))?;
    match resp.message_response {
        Some(MessageResponse::ErrorResponse(e)) => {
            anyhow::bail!("reflection error {}: {}", e.error_code, e.error_message)
        }
        Some(m) => Ok(m),
        None => anyhow::bail!("missing reflection payload"),
    }
}
