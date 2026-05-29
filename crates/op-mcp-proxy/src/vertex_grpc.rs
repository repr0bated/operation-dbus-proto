//! Vertex AI gRPC client with cached OAuth token and real server-side streaming.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde_json::Value as JsonValue;
use tokio::sync::Mutex;
use tonic::{
    metadata::MetadataValue,
    transport::{Channel, ClientTlsConfig},
    Request,
};
use tracing::{debug, info, warn};

use crate::gcloud_auth::GCloudAuth;

pub mod proto {
    tonic::include_proto!("google.cloud.aiplatform.v1");
}

use proto::{
    prediction_service_client::PredictionServiceClient, Content, GenerateContentRequest,
    GenerationConfig, Part,
};

struct CachedToken {
    token: String,
    expiry: DateTime<Utc>,
}

pub struct VertexGrpcClient {
    project: String,
    region: String,
    inner: PredictionServiceClient<Channel>,
    cached_token: Mutex<Option<CachedToken>>,
    gcloud_auth: GCloudAuth,
}

impl VertexGrpcClient {
    pub async fn new(project: String, region: String) -> anyhow::Result<Arc<Self>> {
        let endpoint = if region == "global" {
            "https://aiplatform.googleapis.com".to_string()
        } else {
            format!("https://{region}-aiplatform.googleapis.com")
        };
        let tls = ClientTlsConfig::new().with_webpki_roots();
        let channel = Channel::from_shared(endpoint)?
            .tls_config(tls)?
            .connect_lazy();

        let inner =
            PredictionServiceClient::new(channel).max_decoding_message_size(64 * 1024 * 1024); // 64 MiB for large responses

        let client = Arc::new(Self {
            project,
            region,
            inner,
            cached_token: Mutex::new(None),
            gcloud_auth: GCloudAuth::new(),
        });

        // Pre-warm token in background so first request is fast.
        let c = Arc::clone(&client);
        tokio::spawn(async move {
            match c.refresh_token().await {
                Ok(_) => info!("Vertex AI: initial token cached"),
                Err(e) => warn!("Vertex AI: initial token prefetch failed: {}", e),
            }
        });

        Ok(client)
    }

    async fn get_token(&self) -> anyhow::Result<String> {
        let guard = self.cached_token.lock().await;
        if let Some(ref ct) = *guard {
            if ct.expiry > Utc::now() + chrono::Duration::minutes(5) {
                debug!("Vertex AI: using cached token");
                return Ok(ct.token.clone());
            }
        }
        drop(guard);
        self.refresh_token().await
    }

    async fn refresh_token(&self) -> anyhow::Result<String> {
        let (token, expiry) = self.gcloud_auth.get_token().await?;
        info!(
            "Vertex AI: token refreshed, valid until {}",
            expiry.format("%H:%M:%S UTC")
        );
        let mut guard = self.cached_token.lock().await;
        *guard = Some(CachedToken {
            token: token.clone(),
            expiry,
        });
        Ok(token)
    }

    fn model_resource(&self, model: &str) -> String {
        format!(
            "projects/{}/locations/{}/publishers/google/models/{}",
            self.project, self.region, model
        )
    }

    fn extract_text(content: &JsonValue) -> String {
        // Handle both plain string and OpenAI-style array: [{"type":"text","text":"..."}]
        if let Some(s) = content.as_str() {
            return s.to_string();
        }
        if let Some(arr) = content.as_array() {
            return arr
                .iter()
                .filter(|part| part["type"].as_str() == Some("text"))
                .filter_map(|part| part["text"].as_str())
                .collect::<Vec<_>>()
                .join("");
        }
        content.to_string()
    }

    fn build_contents(messages: &[JsonValue]) -> (Vec<Content>, Option<Content>) {
        let mut system_parts: Vec<String> = Vec::new();
        let mut contents: Vec<Content> = Vec::new();

        for m in messages {
            let role = m["role"].as_str().unwrap_or("user");
            let text = Self::extract_text(&m["content"]);

            if role == "system" {
                system_parts.push(text);
                continue;
            }

            let vertex_role = if role == "assistant" { "model" } else { "user" }.to_string();

            // Merge consecutive messages with the same role (Vertex requires alternating).
            if let Some(last) = contents.last_mut() {
                if last.role == vertex_role {
                    if let Some(part) = last.parts.first_mut() {
                        if let Some(proto::part::Data::Text(ref mut t)) = part.data {
                            t.push('\n');
                            t.push_str(&text);
                            continue;
                        }
                    }
                }
            }

            contents.push(Content {
                role: vertex_role,
                parts: vec![Part {
                    data: Some(proto::part::Data::Text(text)),
                }],
            });
        }

        let system_instruction = if system_parts.is_empty() {
            None
        } else {
            Some(Content {
                role: String::new(), // system_instruction must have no role
                parts: vec![Part {
                    data: Some(proto::part::Data::Text(system_parts.join("\n"))),
                }],
            })
        };

        info!(
            n_contents = contents.len(),
            roles = %contents.iter().map(|c| c.role.as_str()).collect::<Vec<_>>().join(","),
            has_system = system_instruction.is_some(),
            "built vertex request"
        );

        (contents, system_instruction)
    }

    fn make_request(
        &self,
        model: &str,
        messages: &[JsonValue],
        max_tokens: Option<u32>,
    ) -> GenerateContentRequest {
        let (contents, system_instruction) = Self::build_contents(messages);
        GenerateContentRequest {
            model: self.model_resource(model),
            contents,
            system_instruction,
            generation_config: max_tokens.map(|t| GenerationConfig {
                max_output_tokens: t as i32,
                temperature: 0.0, // 0.0 = proto3 default, not sent on wire
            }),
        }
    }

    fn inject_headers(
        req: &mut Request<impl Sized>,
        token: &str,
        model_resource: &str,
    ) -> anyhow::Result<()> {
        req.metadata_mut().insert(
            "authorization",
            format!("Bearer {}", token).parse::<MetadataValue<_>>()?,
        );
        // Required by Google Cloud gRPC routing infrastructure — tells the
        // load balancer which model/project/location to route to.
        let routing = format!("model={}", model_resource.replace('/', "%2F"));
        req.metadata_mut().insert(
            "x-goog-request-params",
            routing.parse::<MetadataValue<_>>()?,
        );
        Ok(())
    }

    /// Unary call — returns full text once complete.
    pub async fn generate(
        &self,
        model: &str,
        messages: &[JsonValue],
        max_tokens: Option<u32>,
    ) -> anyhow::Result<String> {
        let token = self.get_token().await?;
        let model_resource = self.model_resource(model);
        let grpc_req = self.make_request(model, messages, max_tokens);
        let mut req = Request::new(grpc_req);
        Self::inject_headers(&mut req, &token, &model_resource)?;

        let resp = self.inner.clone().generate_content(req).await?;
        let text = resp
            .into_inner()
            .candidates
            .into_iter()
            .next()
            .and_then(|c| c.content)
            .and_then(|c| c.parts.into_iter().next())
            .and_then(|p| {
                if let Some(proto::part::Data::Text(t)) = p.data {
                    Some(t)
                } else {
                    None
                }
            })
            .unwrap_or_default();

        Ok(text)
    }

    /// Streaming call — returns a tokio stream of text chunks as SSE data lines.
    pub async fn stream_generate(
        &self,
        model: &str,
        messages: &[JsonValue],
        max_tokens: Option<u32>,
        id: String,
        created: i64,
    ) -> anyhow::Result<
        impl tokio_stream::Stream<Item = Result<axum::response::sse::Event, std::convert::Infallible>>,
    > {
        use tokio_stream::StreamExt as _;

        let token = self.get_token().await?;
        let model_resource = self.model_resource(model);
        let grpc_req = self.make_request(model, messages, max_tokens);
        let mut req = Request::new(grpc_req);
        Self::inject_headers(&mut req, &token, &model_resource)?;

        let model_str = model.to_string();
        let stream = self
            .inner
            .clone()
            .stream_generate_content(req)
            .await?
            .into_inner();

        let sse_stream = stream
            .map(move |result| {
                let id = id.clone();
                let model_str = model_str.clone();
                match result {
                    Ok(resp) => {
                        let text = resp
                            .candidates
                            .into_iter()
                            .next()
                            .and_then(|c| c.content)
                            .and_then(|c| c.parts.into_iter().next())
                            .and_then(|p| {
                                if let Some(proto::part::Data::Text(t)) = p.data {
                                    Some(t)
                                } else {
                                    None
                                }
                            })
                            .unwrap_or_default();

                        let chunk = serde_json::json!({
                            "id": id,
                            "object": "chat.completion.chunk",
                            "created": created,
                            "model": model_str,
                            "choices": [{
                                "index": 0,
                                "delta": { "content": text },
                                "finish_reason": null
                            }]
                        });
                        Ok(axum::response::sse::Event::default().data(chunk.to_string()))
                    }
                    Err(e) => {
                        warn!("Vertex AI stream error: {}", e);
                        let chunk = serde_json::json!({
                            "error": { "message": e.to_string() }
                        });
                        Ok(axum::response::sse::Event::default().data(chunk.to_string()))
                    }
                }
            })
            .chain(tokio_stream::once(Ok(axum::response::sse::Event::default(
            )
            .data("[DONE]"))));

        Ok(sse_stream)
    }
}
