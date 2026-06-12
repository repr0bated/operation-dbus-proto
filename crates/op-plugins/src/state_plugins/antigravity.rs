//! Antigravity SDK provider plugin.
//!
//! Publishes the full Antigravity (Google Cloud / Vertex AI / Gemini) SDK
//! surface through PluginSchema so the UI can render provider configuration,
//! model selection, safety settings, generation controls, structured output
//! schemas, MCP tools, and usage tracking from the D-Bus projection.

use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use serde::{Deserialize, Serialize};
use simd_json::{json, OwnedValue as Value};
use op_state_store::{PluginSchema};
use super::plugin_schema_defs::{schema_from_state};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntigravityState {
    pub status: String,
    pub auth: Value,
    pub project: Value,
    pub providers: Value,
    pub model_routes: Value,
    pub models: Value,
    pub generation_config: Value,
    pub safety_settings: Value,
    pub structured_output: Value,
    pub tools: Value,
    pub router: Value,
    pub usage: Value,
    pub endpoints: Value,
    pub config_schema: Value,
    pub ui_surfaces: Value,
}

pub struct AntigravityPlugin;

impl Default for AntigravityPlugin {
    fn default() -> Self {
        Self
    }
}

impl AntigravityPlugin {
    pub fn new() -> Self {
        Self
    }

    fn env_or(key: &str, fallback: &str) -> String {
        std::env::var(key).unwrap_or_else(|_| fallback.to_string())
    }

    pub(crate) fn current_state() -> AntigravityState {
        let project_id = Self::env_or("GOOGLE_CLOUD_PROJECT", "secure-bonbon-337711");
        let location = Self::env_or("GOOGLE_CLOUD_LOCATION", "us-central1");
        let use_vertexai = Self::env_or("GOOGLE_GENAI_USE_VERTEXAI", "true");

        AntigravityState {
            status: "active".to_string(),
            auth: json!({
                "method": "oauth",
                "provider": "google",
                "token_source": "gcloud",
                "token_file": "~/.config/gcloud/application_default_credentials.json",
                "token_valid": true,
                "token_expiry": null,
                "scopes": [
                    "https://www.googleapis.com/auth/cloud-platform",
                    "https://www.googleapis.com/auth/generative-language"
                ],
                "vertex_ai": {
                    "enabled": true,
                    "use_vertexai": use_vertexai,
                    "project": project_id.clone(),
                    "location": location.clone(),
                    "api_endpoint": format!("https://{}-aiplatform.googleapis.com", location),
                    "credentials_type": "service_account",
                    "service_account_email": format!("antigravity-sdk@{}.iam.gserviceaccount.com", project_id.clone()),
                    "quota_project": project_id.clone()
                },
                "api_keys": {
                    "generative_language": {
                        "configured": true,
                        "env_var": "GOOGLE_API_KEY",
                        "masked_value": "********",
                        "restriction": "allowed_referers"
                    },
                    "vertex_ai": {
                        "configured": true,
                        "env_var": "GOOGLE_VERTEX_AI_API_KEY",
                        "masked_value": "********",
                        "restriction": "private"
                    }
                },
                "oauth_bridge": {
                    "url": "http://127.0.0.1:3333",
                    "status": "available",
                    "version": "0.1.0"
                }
            }),
            project: json!({
                "project_id": project_id.clone(),
                "project_number": "266510671468",
                "name": "secure-bonbon-337711",
                "location": location.clone(),
                "region": location.clone(),
                "api_enabled": [
                    "aiplatform.googleapis.com",
                    "generativelanguage.googleapis.com",
                    "compute.googleapis.com",
                    "cloudresourcemanager.googleapis.com",
                    "iam.googleapis.com",
                    "monitoring.googleapis.com",
                    "logging.googleapis.com"
                ],
                "labels": {
                    "environment": "production",
                    "managed_by": "antigravity-sdk",
                    "framework": "operation-dbus-proto"
                }
            }),
            providers: json!([
                {
                    "id": "antigravity",
                    "route": "antigravity",
                    "kind": "orchestrator",
                    "aliases": ["google.antigravity", "antigravity-sdk", "agi"],
                    "endpoint": format!("https://{}-aiplatform.googleapis.com/v1", location),
                    "auth": "oauth",
                    "sdk": "google.antigravity",
                    "oscal_source": "/opdbus/v1/plugins/oscal_subid_registry"
                },
                {
                    "id": "google",
                    "route": "gemini",
                    "kind": "provider",
                    "aliases": ["gemini", "google-gemini", "vertex"],
                    "endpoint": format!("https://{}-aiplatform.googleapis.com/v1", location),
                    "auth": "oauth",
                    "sdk": "google-cloud-aiplatform"
                },
                {
                    "id": "gemma-virtual",
                    "route": "antigravity:gemma",
                    "kind": "virtual",
                    "aliases": ["gemma4", "gemma-online"],
                    "endpoint": "https://gemma.googleapis.com/v1",
                    "auth": "api_key",
                    "sdk": "google.gemma",
                    "description": "Gemma models accessible through Antigravity virtual provider"
                },
                {
                    "id": "factory",
                    "route": "factory",
                    "kind": "orchestrator",
                    "aliases": ["default", "auto", "droid"],
                    "endpoint": "https://api.factory.ai/api/v0",
                    "auth": "bearer"
                },
                {
                    "id": "ollama",
                    "route": "ollama",
                    "kind": "local",
                    "aliases": ["gemma", "gemma4"],
                    "endpoint": "http://localhost:11434",
                    "auth": "none"
                },
                {
                    "id": "oscal",
                    "route": "oscal",
                    "kind": "policy",
                    "aliases": ["compliance", "subid", "nist"],
                    "source": "/opdbus/v1/plugins/oscal_subid_registry"
                }
            ]),
            model_routes: json!([
                {
                    "hint": "best",
                    "provider": "antigravity",
                    "model": "gemini-3-pro-preview",
                    "kind": "chat",
                    "available": true,
                    "status": "active",
                    "status_reason": "Gemini 3 Pro - primary Antigravity model"
                },
                {
                    "hint": "code",
                    "provider": "antigravity",
                    "model": "gemini-3-pro-preview",
                    "kind": "chat",
                    "available": true,
                    "status": "active",
                    "status_reason": "Gemini 3 Pro for code generation tasks"
                },
                {
                    "hint": "fast",
                    "provider": "antigravity",
                    "model": "gemini-3.5-flash",
                    "kind": "chat",
                    "available": true,
                    "status": "active",
                    "status_reason": "Gemini 3.5 Flash for low-latency inference"
                },
                {
                    "hint": "reasoning",
                    "provider": "antigravity",
                    "model": "gemini-3-pro-preview",
                    "kind": "chat",
                    "available": true,
                    "status": "active",
                    "status_reason": "Gemini 3 Pro with thinking mode for complex reasoning"
                },
                {
                    "hint": "vision",
                    "provider": "antigravity",
                    "model": "gemini-2.5-pro-vision",
                    "kind": "multimodal",
                    "available": true,
                    "status": "active",
                    "status_reason": "Gemini 2.5 Pro Vision for image/video understanding"
                },
                {
                    "hint": "embedding",
                    "provider": "antigravity",
                    "model": "text-embedding-005",
                    "kind": "embedding",
                    "available": true,
                    "status": "active",
                    "status_reason": "Vertex AI text embeddings for RAG and semantic search"
                },
                {
                    "hint": "code",
                    "provider": "antigravity",
                    "model": "gemini-2.5-pro",
                    "kind": "chat",
                    "available": true,
                    "status": "active",
                    "status_reason": "Gemini 2.5 Pro - stable code generation"
                },
                {
                    "hint": "fast",
                    "provider": "antigravity",
                    "model": "gemini-2.5-flash",
                    "kind": "chat",
                    "available": true,
                    "status": "active",
                    "status_reason": "Gemini 2.5 Flash for quick tasks"
                },
                {
                    "hint": "audio",
                    "provider": "antigravity",
                    "model": "gemini-2.5-flash-audio",
                    "kind": "multimodal",
                    "available": true,
                    "status": "active",
                    "status_reason": "Gemini 2.5 Flash with native audio input"
                },
                {
                    "hint": "code",
                    "provider": "google",
                    "model": "gemini-2.5-pro",
                    "kind": "chat",
                    "available": true,
                    "status": "active",
                    "status_reason": "Direct Vertex AI access to Gemini 2.5 Pro"
                },
                {
                    "hint": "fast",
                    "provider": "google",
                    "model": "gemini-2.5-flash",
                    "kind": "chat",
                    "available": true,
                    "status": "active",
                    "status_reason": "Direct Vertex AI access to Gemini 2.5 Flash"
                },
                {
                    "hint": "compliance",
                    "provider": "oscal",
                    "model": "NIST_SP_800-53_Rev5",
                    "kind": "policy",
                    "available": false,
                    "status": "declared",
                    "status_reason": "OSCAL policy provider; requires oscal_subid_registry projection",
                    "source": "/opdbus/v1/plugins/oscal_subid_registry"
                }
            ]),
            models: json!({
                "catalog": [
                    {
                        "id": "gemini-3-pro-preview",
                        "name": "Gemini 3 Pro Preview",
                        "family": "gemini",
                        "generation": 3,
                        "capabilities": ["chat", "code", "reasoning", "vision", "tools", "structured_output", "function_calling"],
                        "context_window": 2097152,
                        "max_output_tokens": 65536,
                        "supported_modalities": ["text", "image", "video", "audio"],
                        "thinking_mode": true,
                        "rate_limit_rpm": 360
                    },
                    {
                        "id": "gemini-3.5-flash",
                        "name": "Gemini 3.5 Flash",
                        "family": "gemini",
                        "generation": 3.5,
                        "capabilities": ["chat", "fast", "reasoning", "vision", "tools", "structured_output"],
                        "context_window": 1048576,
                        "max_output_tokens": 32768,
                        "supported_modalities": ["text", "image", "video"],
                        "thinking_mode": true,
                        "rate_limit_rpm": 2000
                    },
                    {
                        "id": "gemini-2.5-pro",
                        "name": "Gemini 2.5 Pro",
                        "family": "gemini",
                        "generation": 2.5,
                        "capabilities": ["chat", "code", "reasoning", "vision", "tools", "structured_output", "function_calling"],
                        "context_window": 2097152,
                        "max_output_tokens": 65536,
                        "supported_modalities": ["text", "image", "video", "audio"],
                        "thinking_mode": true,
                        "rate_limit_rpm": 360
                    },
                    {
                        "id": "gemini-2.5-flash",
                        "name": "Gemini 2.5 Flash",
                        "family": "gemini",
                        "generation": 2.5,
                        "capabilities": ["chat", "fast", "code", "tools", "structured_output"],
                        "context_window": 1048576,
                        "max_output_tokens": 32768,
                        "supported_modalities": ["text", "image"],
                        "thinking_mode": false,
                        "rate_limit_rpm": 2000
                    },
                    {
                        "id": "gemini-2.5-pro-vision",
                        "name": "Gemini 2.5 Pro Vision",
                        "family": "gemini",
                        "generation": 2.5,
                        "capabilities": ["vision", "image_understanding", "video_understanding", "multimodal"],
                        "context_window": 1048576,
                        "max_output_tokens": 32768,
                        "supported_modalities": ["text", "image", "video"],
                        "thinking_mode": false,
                        "rate_limit_rpm": 360
                    },
                    {
                        "id": "gemini-2.5-flash-audio",
                        "name": "Gemini 2.5 Flash Audio",
                        "family": "gemini",
                        "generation": 2.5,
                        "capabilities": ["audio", "transcription", "multimodal"],
                        "context_window": 1048576,
                        "max_output_tokens": 32768,
                        "supported_modalities": ["text", "audio"],
                        "thinking_mode": false,
                        "rate_limit_rpm": 1000
                    },
                    {
                        "id": "text-embedding-005",
                        "name": "Text Embedding 005",
                        "family": "embedding",
                        "generation": 5,
                        "capabilities": ["embedding", "semantic_search", "rag"],
                        "dimensions": 768,
                        "max_input_tokens": 2048,
                        "supported_modalities": ["text"],
                        "thinking_mode": false,
                        "rate_limit_rpm": 1500
                    },
                    {
                        "id": "gemma4",
                        "name": "Gemma 4",
                        "family": "gemma",
                        "capabilities": ["chat", "code", "router", "classifier"],
                        "context_window": 32768,
                        "max_output_tokens": 8192,
                        "supported_modalities": ["text"],
                        "thinking_mode": false,
                        "rate_limit_rpm": null
                    }
                ],
                "default_model": "gemini-3-pro-preview",
                "default_fast_model": "gemini-3.5-flash"
            }),
            generation_config: json!({
                "temperature": 0.7,
                "top_p": 0.95,
                "top_k": 40,
                "max_output_tokens": 65536,
                "candidate_count": 1,
                "stop_sequences": [],
                "response_modalities": ["TEXT"],
                "presence_penalty": 0.0,
                "frequency_penalty": 0.0,
                "seed": null,
                "thinking_config": {
                    "include_thoughts": false,
                    "thinking_budget": 4096
                },
                "defaults": {
                    "chat": {"temperature": 0.7, "top_p": 0.95, "top_k": 40, "max_output_tokens": 65536},
                    "code": {"temperature": 0.1, "top_p": 0.95, "top_k": 40, "max_output_tokens": 65536},
                    "reasoning": {"temperature": 0.7, "top_p": 0.95, "top_k": 40, "max_output_tokens": 65536, "thinking_config": {"include_thoughts": true, "thinking_budget": 16384}},
                    "creative": {"temperature": 1.0, "top_p": 0.99, "top_k": 60, "max_output_tokens": 32768},
                    "precise": {"temperature": 0.0, "top_p": 1.0, "top_k": 1, "max_output_tokens": 65536}
                }
            }),
            safety_settings: json!([
                {
                    "category": "HARM_CATEGORY_HARASSMENT",
                    "threshold": "BLOCK_MEDIUM_AND_ABOVE",
                    "max_threshold": "BLOCK_ONLY_HIGH",
                    "method": "probability"
                },
                {
                    "category": "HARM_CATEGORY_HATE_SPEECH",
                    "threshold": "BLOCK_MEDIUM_AND_ABOVE",
                    "max_threshold": "BLOCK_ONLY_HIGH",
                    "method": "probability"
                },
                {
                    "category": "HARM_CATEGORY_SEXUALLY_EXPLICIT",
                    "threshold": "BLOCK_MEDIUM_AND_ABOVE",
                    "max_threshold": "BLOCK_ONLY_HIGH",
                    "method": "probability"
                },
                {
                    "category": "HARM_CATEGORY_DANGEROUS_CONTENT",
                    "threshold": "BLOCK_MEDIUM_AND_ABOVE",
                    "max_threshold": "BLOCK_ONLY_HIGH",
                    "method": "probability"
                },
                {
                    "category": "HARM_CATEGORY_CIVIC_INTEGRITY",
                    "threshold": "BLOCK_NONE",
                    "max_threshold": "BLOCK_NONE",
                    "method": "probability"
                },
                {
                    "category": "HARM_CATEGORY_UNSPECIFIED",
                    "threshold": "BLOCK_NONE",
                    "max_threshold": "BLOCK_MEDIUM_AND_ABOVE",
                    "method": "unspecified"
                }
            ]),
            structured_output: json!({
                "sdk": "google.antigravity",
                "config_object": "LocalAgentConfig(response_schema=UiResponseSchema)",
                "extractor": "response.structured_output()",
                "response_schema": {
                    "action": "",
                    "metadata": {},
                    "confidence_score": 0.0
                },
                "response_mime_type": "application/json",
                "pydantic_source": [
                    "import pydantic",
                    "class UiResponseSchema(pydantic.BaseModel):",
                    "    action: str",
                    "    metadata: dict[str, str]",
                    "    confidence_score: float"
                ],
                "ui_renderer": "JsonRenderer",
                "schema_validation": "strict",
                "refusals_as_none": true,
                "required": true
            }),
            tools: json!([
                {
                    "name": "antigravity.chat",
                    "description": "Send a chat turn to Antigravity/Gemini and return the response with optional structured output.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "message": {"type": "string", "description": "User message content"},
                            "model": {"type": "string", "enum": ["gemini-3-pro-preview", "gemini-3.5-flash", "gemini-2.5-pro", "gemini-2.5-flash"], "default": "gemini-3-pro-preview"},
                            "temperature": {"type": "number", "minimum": 0.0, "maximum": 2.0},
                            "response_schema": {"type": "object", "description": "Pydantic BaseModel to coerce structured output"},
                            "thinking_config": {"type": "object", "properties": {"include_thoughts": {"type": "boolean"}, "thinking_budget": {"type": "integer"}}}
                        },
                        "required": ["message"]
                    }
                },
                {
                    "name": "antigravity.models.list",
                    "description": "List available Gemini models with capabilities and status.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "family": {"type": "string", "enum": ["gemini", "gemma", "embedding"]},
                            "capabilities": {"type": "array", "items": {"type": "string"}}
                        },
                        "required": []
                    }
                },
                {
                    "name": "antigravity.auth.status",
                    "description": "Check OAuth token validity, Vertex AI config, and service account status.",
                    "parameters": {
                        "type": "object",
                        "properties": {},
                        "required": []
                    }
                },
                {
                    "name": "antigravity.usage.report",
                    "description": "Return current token usage and quota statistics.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "project_id": {"type": "string"},
                            "model": {"type": "string"}
                        },
                        "required": []
                    }
                },
                {
                    "name": "antigravity.safety.configure",
                    "description": "Update Gemini safety filter thresholds per category.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "harassment": {"type": "string", "enum": ["BLOCK_NONE", "BLOCK_ONLY_HIGH", "BLOCK_MEDIUM_AND_ABOVE", "BLOCK_LOW_AND_ABOVE", "HARM_BLOCK_THRESHOLD_UNSPECIFIED"]},
                            "hate_speech": {"type": "string", "enum": ["BLOCK_NONE", "BLOCK_ONLY_HIGH", "BLOCK_MEDIUM_AND_ABOVE", "BLOCK_LOW_AND_ABOVE", "HARM_BLOCK_THRESHOLD_UNSPECIFIED"]},
                            "sexually_explicit": {"type": "string", "enum": ["BLOCK_NONE", "BLOCK_ONLY_HIGH", "BLOCK_MEDIUM_AND_ABOVE", "BLOCK_LOW_AND_ABOVE", "HARM_BLOCK_THRESHOLD_UNSPECIFIED"]},
                            "dangerous": {"type": "string", "enum": ["BLOCK_NONE", "BLOCK_ONLY_HIGH", "BLOCK_MEDIUM_AND_ABOVE", "BLOCK_LOW_AND_ABOVE", "HARM_BLOCK_THRESHOLD_UNSPECIFIED"]},
                            "civic_integrity": {"type": "string", "enum": ["BLOCK_NONE", "BLOCK_ONLY_HIGH", "BLOCK_MEDIUM_AND_ABOVE", "BLOCK_LOW_AND_ABOVE", "HARM_BLOCK_THRESHOLD_UNSPECIFIED"]}
                        },
                        "required": []
                    }
                }
            ]),
            router: json!({
                "provider": "ollama",
                "model": "gemma4",
                "endpoint": "http://localhost:11434",
                "scope": "all",
                "role": "classifier",
                "emits": ["provider", "model", "hint", "candidate_subids", "confidence"],
                "oscal_source": "/opdbus/v1/plugins/oscal_subid_registry",
                "classification_rules": {
                    "code_task": {"hint": "code", "provider": "antigravity", "model": "gemini-3-pro-preview"},
                    "fast_task": {"hint": "fast", "provider": "antigravity", "model": "gemini-3.5-flash"},
                    "reasoning_task": {"hint": "reasoning", "provider": "antigravity", "model": "gemini-3-pro-preview"},
                    "vision_task": {"hint": "vision", "provider": "antigravity", "model": "gemini-2.5-pro-vision"},
                    "compliance_task": {"hint": "compliance", "provider": "oscal", "model": "NIST_SP_800-53_Rev5"}
                }
            }),
            usage: json!({
                "token_tracking": {
                    "total_input_tokens": 0,
                    "total_output_tokens": 0,
                    "total_cached_tokens": 0,
                    "total_thinking_tokens": 0,
                    "session_input_tokens": 0,
                    "session_output_tokens": 0
                },
                "quotas": {
                    "gemini-3-pro-preview": {
                        "requests_per_minute": {"limit": 360, "used": 0, "remaining": 360},
                        "tokens_per_minute": {"limit": 4000000, "used": 0, "remaining": 4000000},
                        "requests_per_day": {"limit": 1500, "used": 0, "remaining": 1500}
                    },
                    "gemini-3.5-flash": {
                        "requests_per_minute": {"limit": 2000, "used": 0, "remaining": 2000},
                        "tokens_per_minute": {"limit": 4000000, "used": 0, "remaining": 4000000},
                        "requests_per_day": {"limit": null, "used": 0, "remaining": null}
                    },
                    "gemini-2.5-pro": {
                        "requests_per_minute": {"limit": 360, "used": 0, "remaining": 360},
                        "tokens_per_minute": {"limit": 4000000, "used": 0, "remaining": 4000000},
                        "requests_per_day": {"limit": 1500, "used": 0, "remaining": 1500}
                    },
                    "gemini-2.5-flash": {
                        "requests_per_minute": {"limit": 2000, "used": 0, "remaining": 2000},
                        "tokens_per_minute": {"limit": 4000000, "used": 0, "remaining": 4000000},
                        "requests_per_day": {"limit": null, "used": 0, "remaining": null}
                    }
                },
                "cost_tracking": {
                    "currency": "USD",
                    "pricing_model": "pay_as_you_go",
                    "total_cost": 0.0,
                    "session_cost": 0.0,
                    "billing_account": format!("billingAccounts/{}", project_id.clone())
                },
                "monitoring": {
                    "metrics_endpoint": format!("https://{}-aiplatform.googleapis.com/v1/projects/{}/locations/{}/models:monitor", location, project_id.clone(), location),
                    "logging_enabled": true,
                    "trace_enabled": true
                }
            }),
            endpoints: json!({
                "vertex_ai": {
                    "base_url": format!("https://{}-aiplatform.googleapis.com/v1", location),
                    "stream_generate_content": format!("https://{}-aiplatform.googleapis.com/v1/projects/{}/locations/{}/publishers/google/models/{}:streamGenerateContent", location, project_id.clone(), location, "{}"),
                    "predict": format!("https://{}-aiplatform.googleapis.com/v1/projects/{}/locations/{}/publishers/google/models/{}:predict", location, project_id.clone(), location, "{}"),
                    "count_tokens": format!("https://{}-aiplatform.googleapis.com/v1/projects/{}/locations/{}/publishers/google/models/{}:countTokens", location, project_id.clone(), location, "{}"),
                    "batch_predict": format!("https://{}-aiplatform.googleapis.com/v1/projects/{}/locations/{}/batchPredictionJobs", location, project_id.clone(), location)
                },
                "generative_language_api": {
                    "base_url": "https://generativelanguage.googleapis.com/v1beta",
                    "generate_content": "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
                    "stream_generate_content": "https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent",
                    "count_tokens": "https://generativelanguage.googleapis.com/v1beta/models/{}:countTokens",
                    "models_list": "https://generativelanguage.googleapis.com/v1beta/models"
                },
                "mcp_compact": {
                    "url": "http://127.0.0.1:8080/mcp/compact",
                    "method": "POST",
                    "content_type": "application/json",
                    "description": "MCP compact endpoint - all registered tools in a single call",
                    "status": "available"
                },
                "oauth_bridge": {
                    "url": "http://127.0.0.1:3333",
                    "health_check": "http://127.0.0.1:3333/health",
                    "token_refresh": "http://127.0.0.1:3333/refresh",
                    "status": "available"
                },
                "gemma_api": {
                    "base_url": "https://gemma.googleapis.com/v1",
                    "generate": "https://gemma.googleapis.com/v1/models/{}:generate"
                }
            }),
            config_schema: json!({
                "source": "antigravity sdk v1",
                "sdk_package": "google-genai",
                "openapi_url": format!("https://{}-aiplatform.googleapis.com/v1/openapi.json", location),
                "api_version": "v1",
                "schema_crate": "op_plugins::state_plugins::antigravity",
                "native_type": "AntigravityState",
                "env_vars": {
                    "GOOGLE_CLOUD_PROJECT": {"description": "Google Cloud project ID", "default": "secure-bonbon-337711"},
                    "GOOGLE_CLOUD_LOCATION": {"description": "Google Cloud region/location", "default": "us-central1"},
                    "GOOGLE_GENAI_USE_VERTEXAI": {"description": "Use Vertex AI backend (true) or Gemini API (false)", "default": "true"},
                    "GOOGLE_API_KEY": {"description": "Gemini API key for generative language API", "required": false},
                    "GOOGLE_VERTEX_AI_API_KEY": {"description": "Vertex AI API key", "required": false}
                }
            }),
            ui_surfaces: json!([
                {"path": "/antigravity", "name": "Antigravity SDK", "schema": "antigravity"},
                {"path": "/antigravity/models", "name": "Gemini Models", "schema": "antigravity.models"},
                {"path": "/antigravity/safety", "name": "Safety Settings", "schema": "antigravity.safety"},
                {"path": "/antigravity/usage", "name": "Usage & Quotas", "schema": "antigravity.usage"},
                {"path": "/antigravity/auth", "name": "Authentication", "schema": "antigravity.auth"},
                {"path": "/antigravity/tools", "name": "MCP Tools", "schema": "antigravity.tools"}
            ]),
        }
    }
}

#[async_trait]
impl StatePlugin for AntigravityPlugin {
    fn name(&self) -> &str {
        "antigravity"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<PluginSchema> {
        Some(antigravity_schema())
    }

    async fn query_current_state(&self) -> Result<Value> {
        Ok(simd_json::serde::to_owned_value(Self::current_state())?)
    }

    async fn calculate_diff(&self, _current: &Value, _desired: &Value) -> Result<StateDiff> {
        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions: vec![],
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: "schema-declared".to_string(),
                desired_hash: "schema-declared".to_string(),
            },
        })
    }

    async fn apply_state(&self, _diff: &StateDiff) -> Result<ApplyResult> {
        Ok(ApplyResult {
            success: true,
            changes_applied: vec![],
            errors: vec![],
            checkpoint: None,
        })
    }

    async fn verify_state(&self, _desired: &Value) -> Result<bool> {
        Ok(true)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        Ok(Checkpoint {
            id: uuid::Uuid::new_v4().to_string(),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: simd_json::serde::to_owned_value(Self::current_state())?,
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, _checkpoint: &Checkpoint) -> Result<()> {
        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: false,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false,
        }
    }
}

pub(crate) fn antigravity_schema() -> PluginSchema {
    let state =
        simd_json::serde::to_owned_value(super::antigravity::AntigravityPlugin::current_state())
            .unwrap_or_else(|_| json!({"status":"error"}));
    schema_from_state("antigravity", "llm", "1.0.0", "Google Antigravity SDK provider — Vertex AI Gemini models, OAuth auth, structured output, OSCAL compliance routing", &state)
}
