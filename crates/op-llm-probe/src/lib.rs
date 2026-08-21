//! Generic probe for OpenAI-compatible LLM endpoints.
//!
//! Any provider that speaks `GET {base_url}/models` and
//! `POST {base_url}/chat/completions` with `Authorization: Bearer <key>`
//! is probeable here. This exists as a leaf crate so both `op-llm`
//! (which depends on `op-plugins`) and `op-plugins` itself can call the
//! same probe without `op-plugins` taking a reverse dependency on `op-llm`.
//!
//! The functions are best-effort: they return empty/false on any failure
//! rather than propagating errors, because they feed schema construction
//! and present-state projection where a missing probe must never block
//! the control plane.

use std::time::Duration;

use serde::Deserialize;

const LIST_TIMEOUT: Duration = Duration::from_secs(10);
const REACH_TIMEOUT: Duration = Duration::from_secs(20);

/// Model entry in an OpenAI-compatible `/models` response.
#[derive(Debug, Deserialize)]
struct ModelEntry {
    id: String,
}

#[derive(Debug, Deserialize)]
struct ModelsResponse {
    #[serde(default)]
    data: Vec<ModelEntry>,
}

/// List model IDs from an OpenAI-compatible endpoint.
///
/// Returns `Vec<String>` — empty on any failure (missing key, network
/// error, parse error). The caller treats empty as "unavailable" rather
/// than blocking schema construction.
pub async fn list_models(base_url: &str, api_key: &str) -> Vec<String> {
    let url = format!("{}/models", base_url.trim_end_matches('/'));
    let Ok(client) = reqwest::Client::builder().timeout(LIST_TIMEOUT).build() else {
        return Vec::new();
    };
    let response = match client.get(&url).bearer_auth(api_key).send().await {
        Ok(r) if r.status().is_success() => r,
        Ok(r) => {
            tracing::warn!("model list probe failed ({}) at {url}", r.status());
            return Vec::new();
        }
        Err(e) => {
            tracing::warn!("model list probe unreachable at {url}: {e}");
            return Vec::new();
        }
    };
    let Ok(body) = response.text().await else {
        return Vec::new();
    };
    let Ok(parsed) = serde_json::from_str::<ModelsResponse>(&body) else {
        return Vec::new();
    };
    parsed.data.into_iter().map(|entry| entry.id).collect()
}

/// Check whether a specific model actually answers a minimal request.
///
/// `/v1/models` reports every model the provider has ever declared; it
/// does not guarantee a warm replica is serving. This sends a 4-token
/// ping to `/chat/completions` and returns `true` only on a 2xx response.
pub async fn is_model_reachable(base_url: &str, api_key: &str, model: &str) -> bool {
    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    let Ok(client) = reqwest::Client::builder().timeout(REACH_TIMEOUT).build() else {
        return false;
    };
    let body = serde_json::json!({
        "model": model,
        "messages": [{"role": "user", "content": "ping"}],
        "max_tokens": 4,
        "stream": false
    });
    match client
        .post(&url)
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await
    {
        Ok(r) if r.status().is_success() => true,
        Ok(r) => {
            tracing::warn!(
                "reachability probe for {model} failed ({}) at {url}",
                r.status()
            );
            false
        }
        Err(e) => {
            tracing::warn!("reachability probe for {model} timed out/unreachable at {url}: {e}");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_models_url_trims_trailing_slash() {
        // The URL is built inside the async fn; this just confirms the
        // trim logic by checking the format string directly.
        let url = format!(
            "{}/models",
            "https://ai.salad.cloud/v1/".trim_end_matches('/')
        );
        assert_eq!(url, "https://ai.salad.cloud/v1/models");
    }

    #[test]
    fn models_response_parses_openai_shape() {
        let json = r#"{"data":[{"id":"qwen3.6-35b-a3b"},{"id":"qwen3.6-27b"}]}"#;
        let parsed: ModelsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.data.len(), 2);
        assert_eq!(parsed.data[0].id, "qwen3.6-35b-a3b");
        assert_eq!(parsed.data[1].id, "qwen3.6-27b");
    }

    #[test]
    fn empty_data_array_is_valid() {
        let json = r#"{"data":[]}"#;
        let parsed: ModelsResponse = serde_json::from_str(json).unwrap();
        assert!(parsed.data.is_empty());
    }

    #[test]
    fn missing_data_field_defaults_to_empty() {
        let json = r#"{}"#;
        let parsed: ModelsResponse = serde_json::from_str(json).unwrap();
        assert!(parsed.data.is_empty());
    }
}
