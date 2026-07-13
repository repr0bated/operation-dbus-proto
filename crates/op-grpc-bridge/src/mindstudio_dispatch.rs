//! MindStudio method dispatch.
//!
//! Bridges the `mindstudio` D-Bus/gRPC plugin surface (what `zcall mindstudio
//! run` hits) to the MindStudio `apps/run` REST API. Unlike `op-mindstudio-shim`
//! (loopback OpenAI-compat, no identity), calls here go through
//! `PluginService.CallMethod` so they carry Ghostbridge identity headers and
//! are notarized in the event chain like every other command.
//!
//! `MIND_STUDIO_API_KEY` is read from the process environment (operator
//! secrets file, sourced by the s6 service supervisor at launch — never
//! passed as a call argument).

use anyhow::{anyhow, Result};
use serde_json::Value;
use std::time::Duration;

const DEFAULT_RUN_URL: &str = "https://api.mindstudio.ai/developer/v2/apps/run";
const DEFAULT_APP_ID: &str = "83dc1fff-d16e-4a7f-80b9-fd68f6117346";

fn run_url() -> String {
    std::env::var("MINDSTUDIO_RUN_URL").unwrap_or_else(|_| DEFAULT_RUN_URL.to_string())
}

fn app_id() -> String {
    std::env::var("MINDSTUDIO_APP_ID").unwrap_or_else(|_| DEFAULT_APP_ID.to_string())
}

async fn run(args: &Value) -> Result<Value> {
    let api_key = std::env::var("MIND_STUDIO_API_KEY")
        .map_err(|_| anyhow!("MIND_STUDIO_API_KEY not set in environment"))?;
    let prompt = args
        .get("prompt")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing required 'prompt' argument"))?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()?;

    let resp = client
        .post(run_url())
        .bearer_auth(api_key)
        .json(&serde_json::json!({
            "appId": app_id(),
            "variables": { "prompt": prompt },
        }))
        .send()
        .await
        .map_err(|e| anyhow!("MindStudio request failed: {e}"))?;

    let status = resp.status();
    let body: Value = resp
        .json()
        .await
        .map_err(|e| anyhow!("MindStudio response not JSON: {e}"))?;

    if !status.is_success() || body.get("success").and_then(Value::as_bool) == Some(false) {
        return Err(anyhow!("MindStudio run failed ({status}): {body}"));
    }

    let thread_id = body
        .get("threadId")
        .and_then(Value::as_str)
        .map(String::from);

    Ok(serde_json::json!({
        "success": true,
        "thread_id": thread_id,
        "result": body,
    }))
}

fn get_config() -> Result<Value> {
    Ok(serde_json::json!({
        "config": {
            "app_id": app_id(),
            "endpoint": run_url(),
            "api_key_set": std::env::var("MIND_STUDIO_API_KEY").is_ok(),
        }
    }))
}

/// Dispatch a `mindstudio` method to the MindStudio REST API.
pub async fn dispatch_mindstudio_method(method: &str, args: &Value) -> Result<Value> {
    match method {
        "run" => run(args).await,
        "get_config" => get_config(),
        other => Err(anyhow!("unknown mindstudio method '{other}'")),
    }
}
