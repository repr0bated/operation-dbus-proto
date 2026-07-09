//! Unified magic-link verify → identity sled → memory → container provision pipeline.
//!
//! Driven by zeroclaw `configurable_options` from the sealed blob (Absolute Base).

use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde_json::json;
use std::sync::Arc;
use tracing::{info, warn};
use uuid::Uuid;

use crate::state::AppState;
use crate::users::{PrivacyUser, VerifiedSignup};
use crate::zeroclaw_configurable::{
    load_configurable_options, resolve_provision_features,
    substitute_template, withhold_email, ZeroclawConfigurableOptions,
};

const PROVISION_ACTOR: &str = "mut.service.registration-provision.apply@v1";
const PROVISION_CAPABILITY: &str = "mut.service.identity-sled.write@v1";

/// Full post-verify provisioning for a consumer account.
pub async fn provision_verified_signup(
    state: &Arc<AppState>,
    verified: VerifiedSignup,
) -> Result<PrivacyUser> {
    let options = load_configurable_options();
    let features = resolve_provision_features(&options);
    let psk = verified.psk;
    let mut user = verified.user;

    info!(
        session_id = %user.id,
        ghostbridge = features.ghostbridge,
        semantic_search = features.semantic_search,
        container_image = %options.user_container.image,
        "provisioning from zeroclaw configurable_options"
    );

    crate::privacy_network::ensure_host_privacy_network().await?;

    if !psk.is_empty() {
        write_identity_sled(state, &user, &psk).await?;
        provision_memory_namespaces(state, &user, &options, &features, &psk).await?;
        record_provisioned_event(state, &user).await?;
    }

    apply_agent_model_defaults(state, &mut user, &options).await?;

    let container_name =
        crate::privacy_container::ensure_user_container_from_options(&user, &options).await?;
    let route_id =
        crate::privacy_routes::publish_user_privacy_route(&user, Some(container_name.as_str()))
            .await?;
    crate::privacy_openflow::publish_openflow_for_privacy_routes().await?;

    let already_connected = user.privacy_network_connected
        && user.privacy_container_name.as_deref() == Some(container_name.as_str())
        && user.privacy_route_id.as_deref() == Some(route_id.as_str());
    if already_connected {
        return Ok(user);
    }

    if withhold_email(&options, &features) {
        info!(session_id = %user.id, "email withheld per ghostbridge privacy policy");
    }

    state
        .user_store
        .mark_privacy_network_connected(&user.id, container_name, route_id)
        .await
}

async fn write_identity_sled(state: &Arc<AppState>, user: &PrivacyUser, psk_b64: &str) -> Result<()> {
    let blob_ref = op_blob::catalog::read_manifest_plugin_ids_shm()
        .and_then(|ids| ids.into_iter().find(|id| id == "identity_sled"))
        .map(|id| format!("{id}.blob"));

    let args = vec![simd_json::json!({
        "wireguard_pubkey": user.wg_public_key,
        "psk": psk_b64,
        "peer_ip": user.assigned_ip.trim_end_matches("/32"),
        "interface": "netmaker",
        "blob_ref": blob_ref,
    })];

    state
        .grpc_client
        .call_method(
            "identity_sled",
            "/org/opdbus/v1/plugins/identity_sled",
            "org.opdbus.v1.plugins.PluginService",
            "write_identity",
            args,
            PROVISION_ACTOR,
            PROVISION_CAPABILITY,
        )
        .await
        .map_err(|e| anyhow::anyhow!("identity_sled.write_identity failed: {e}"))?;

    info!(session_id = %user.id, "identity_sled.write_identity complete");
    Ok(())
}

async fn provision_memory_namespaces(
    state: &Arc<AppState>,
    user: &PrivacyUser,
    options: &ZeroclawConfigurableOptions,
    features: &crate::zeroclaw_configurable::ProvisionFeatures,
    psk_b64: &str,
) -> Result<()> {
    let mcp_url = std::env::var("COGNITIVE_MCP_URL")
        .unwrap_or_else(|_| options.user_container.cognitive_mcp_endpoint.clone());
    let container_id = &user.id;
    let mcp_token = derive_mcp_token(&user.wg_public_key, &options.identity_chain.mcp_token_derivation);

    // Soul profile (schema: container:{id}:soul)
    cognitive_mcp_call(
        &mcp_url,
        json!({
            "operation": "ensure_soul",
            "owner": "user_container",
            "container_id": container_id,
            "identity_id": user.wg_public_key,
            "wireguard_pubkey": user.wg_public_key,
            "namespace": substitute_template("container:{container_id}:soul", container_id),
        }),
    )
    .await?;

    let withhold = withhold_email(options, features);
    let now = chrono::Utc::now().to_rfc3339();

    for ns in &options.memory_namespaces {
        let namespace = substitute_template(&ns.namespace_template, container_id);
        for key in &ns.keys {
            let value = match key.as_str() {
                "wireguard_pubkey" => json!(user.wg_public_key),
                "mcp_token" => json!(mcp_token),
                "psk" => json!(psk_b64),
                "mac_address" => json!(null),
                "email" if withhold => continue,
                "email" => json!(user.email),
                "profile" => json!({
                    "kind": "soul",
                    "container_id": container_id,
                    "created_at": now,
                }),
                "MEMORY_INDEX" if namespace.ends_with(":index") => json!({
                    "kind": "index",
                    "container_id": container_id,
                    "namespaces": [
                        "identity", "soul", "domain:work", "domain:personal", "domain:home"
                    ],
                }),
                "MEMORY_INDEX" if namespace.contains(":domain:") => {
                    let domain = namespace.rsplit(':').next().unwrap_or("work");
                    json!({
                        "kind": "domain",
                        "domain": domain,
                        "container_id": container_id,
                        "entries": [],
                    })
                }
                "ghostbridge" => json!({ "enabled": features.ghostbridge }),
                "semantic_search" => json!({ "enabled": features.semantic_search }),
                other => {
                    warn!(namespace = %namespace, key = %other, "skipping unmapped memory namespace key");
                    continue;
                }
            };

            cognitive_mcp_call(
                &mcp_url,
                json!({
                    "operation": "store",
                    "namespace": namespace,
                    "container_id": container_id,
                    "wireguard_pubkey": user.wg_public_key,
                    "key": key,
                    "value": value,
                }),
            )
            .await?;
        }
    }

    info!(session_id = %user.id, namespaces = options.memory_namespaces.len(), "memory namespaces provisioned");
    let _ = state;
    Ok(())
}

fn derive_mcp_token(wg_pubkey: &str, rule: &str) -> String {
    if rule.contains("uuid_v5") {
        Uuid::new_v5(&Uuid::NAMESPACE_OID, wg_pubkey.as_bytes()).to_string()
    } else {
        Uuid::new_v4().to_string()
    }
}

async fn cognitive_mcp_call(mcp_url: &str, arguments: serde_json::Value) -> Result<()> {
    let body = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "cognitive_memory",
            "arguments": arguments,
        }
    });

    let client = reqwest::Client::new();
    match client
        .post(mcp_url)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => Ok(()),
        Ok(resp) => {
            warn!(status = %resp.status(), url = %mcp_url, "cognitive-mcp call returned non-success");
            Ok(())
        }
        Err(e) => {
            warn!(error = %e, url = %mcp_url, "cognitive-mcp unreachable during provision");
            Ok(())
        }
    }
}

async fn apply_agent_model_defaults(
    state: &Arc<AppState>,
    user: &mut PrivacyUser,
    options: &ZeroclawConfigurableOptions,
) -> Result<()> {
    let (provider, model) = match crate::zeroclaw_configurable::default_agent_model_from_shm() {
        Some(pair) => pair,
        None => return Ok(()),
    };

    let preferred = format!("{provider}/{model}");
    if user.api_credentials.is_none() {
        user.api_credentials = Some(crate::users::UserApiCredentials {
            token: String::new(),
            gemini_api_key: None,
            anthropic_api_key: None,
            openai_api_key: None,
            preferred_provider: Some(preferred.clone()),
        });
        info!(
            session_id = %user.id,
            provider = %provider,
            model = %model,
            "applied independent agent model from zeroclaw blob"
        );
    }

    let _ = options;
    let _ = state;
    Ok(())
}

async fn record_provisioned_event(state: &Arc<AppState>, user: &PrivacyUser) -> Result<()> {
    let args = vec![simd_json::json!({
        "session_id": user.id,
        "kind": "provisioned",
        "subid": "evt.service.registration-provision.complete@v1",
        "content": "magic-link verify pipeline (zeroclaw configurable_options)",
    })];

    state
        .grpc_client
        .call_method(
            "identity_sled",
            "/org/opdbus/v1/plugins/identity_sled",
            "org.opdbus.v1.plugins.PluginService",
            "record_session_event",
            args,
            PROVISION_ACTOR,
            PROVISION_CAPABILITY,
        )
        .await
        .map_err(|e| anyhow::anyhow!("identity_sled.record_session_event failed: {e}"))?;
    Ok(())
}

/// Decode a base64 PSK for Argon2 session derivation.
pub fn decode_psk(psk_b64: &str) -> Result<Vec<u8>> {
    let bytes = BASE64.decode(psk_b64.trim()).context("decode psk")?;
    if bytes.len() != 32 {
        anyhow::bail!("psk must be 32 bytes, got {}", bytes.len());
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zeroclaw_configurable::{resolve_container_name, ZeroclawConfigurableOptions};

    #[test]
    fn mcp_token_is_uuid_v5_of_pubkey() {
        let pubkey = "test-pubkey";
        let token = derive_mcp_token(pubkey, "uuid_v5(wireguard_pubkey)");
        let expected = Uuid::new_v5(&Uuid::NAMESPACE_OID, pubkey.as_bytes()).to_string();
        assert_eq!(token, expected);
    }

    #[test]
    fn container_name_follows_schema_template() {
        let options = ZeroclawConfigurableOptions::default();
        let name = resolve_container_name(&options.user_container.container_id_template, "sess-9");
        assert_eq!(name, "sess-9");
    }
}
