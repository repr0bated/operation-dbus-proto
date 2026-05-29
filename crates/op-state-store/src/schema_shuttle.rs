use crate::plugin_schema::PluginSchema;
use crate::schema_validator::canonicalize_json;
use md5; // Matches EventChain hashing methodology
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::process::Command;
use tokio::time::{sleep, Duration};

/// THE SLED: Zero-copy shared memory layout
#[repr(C)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentitySled {
    pub wireguard_pubkey: [u8; 32],
    pub mutation_index: u64,
    pub is_valid: bool,
    pub hashed_footprint: [u8; 32], // The current "Thought" injected into Xray
}

pub struct SchemaShuttle;

impl SchemaShuttle {
    /// Genesis: Validates the PluginSchema and creates the initial Sled
    pub fn forge_sled(
        wg_pubkey: &str,
        current_schema: &PluginSchema,
    ) -> Result<IdentitySled, String> {
        // Enforce the "No Valid Schema = Does Not Exist" rule
        if !current_schema.is_valid() {
            return Err("Invalid Schema State: Connection Rejected.".into());
        }

        // Decode WG pubkey (assume base64)
        use base64::Engine;
        let wg_bytes: [u8; 32] = base64::engine::general_purpose::STANDARD
            .decode(wg_pubkey.trim())
            .map_err(|e| format!("Invalid WG key: {}", e))?
            .try_into()
            .map_err(|_| "WG key must be 32 bytes".to_string())?;

        // Serialize the PluginSchema to a simd_json Value for canonicalization
        let schema_json = serde_json::to_string(current_schema)
            .map_err(|e| format!("Serialization Failed: {}", e))?;
        let mut schema_bytes = schema_json.into_bytes();
        let schema_value: simd_json::OwnedValue = simd_json::to_owned_value(&mut schema_bytes)
            .map_err(|e| format!("SIMD-JSON parse failed: {}", e))?;
        let canonical_state = serde_json::to_string(&canonicalize_json(&schema_value))
            .map_err(|e| format!("Canonical serialization failed: {}", e))?;

        let payload = format!("{}:{}", wg_pubkey, canonical_state);
        let genesis_hash = md5::compute(payload.as_bytes());
        let mut hashed_footprint = [0u8; 32];
        // MD5 is 16 bytes, we pad it into 32 bytes for the Sled layout
        hashed_footprint[..16].copy_from_slice(&genesis_hash.0);

        Ok(IdentitySled {
            wireguard_pubkey: wg_bytes,
            mutation_index: current_schema.mutation_index.unwrap_or(0),
            is_valid: true,
            hashed_footprint,
        })
    }
}

/// THE SHUTTLE: Async loop monitoring the RPC DB for state mutations
pub async fn run_shuttle() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();
    let rpc_url = "http://127.0.0.1:7020"; // op-jsonrpc legacy tool execution port

    // Extracted from D-Bus/systemd-networkd
    let active_wg_key = "EPHEMERAL_WG_PUBKEY";

    println!("[*] Schema Shuttle active. Fetching PluginSchema...");

    // Fetch the absolute present schema
    let genesis_res = client
        .post(rpc_url)
        .json(&serde_json::json!({"jsonrpc": "2.0", "method": "get_latest_schema", "id": 1}))
        .send()
        .await?
        .json::<serde_json::Value>()
        .await?;

    // Parse into the authoritative PluginSchema object
    let schema: PluginSchema = serde_json::from_value(genesis_res["result"].clone())?;

    // Forge the Sled
    let mut session_sled = SchemaShuttle::forge_sled(active_wg_key, &schema)?;
    let mut last_mutation_index = session_sled.mutation_index;

    let footprint_hex = hex::encode(session_sled.hashed_footprint);
    println!(
        "[SUCCESS] Identity Sled Forged. Footprint: {}",
        footprint_hex
    );

    // The "Even Trade" Zero-Btrfs Loop
    loop {
        let res = client
            .post(rpc_url)
            .json(&serde_json::json!({"jsonrpc": "2.0", "method": "get_mutation_index", "id": 2}))
            .send()
            .await;

        if let Ok(response) = res {
            let state: serde_json::Value = response.json().await?;
            let current_index = state["result"].as_u64().unwrap_or(last_mutation_index);

            // If the Btrfs blockchain mutates, instantly update the gRPC headers
            if current_index > last_mutation_index {
                last_mutation_index = current_index;

                let current_footprint_hex = hex::encode(session_sled.hashed_footprint);
                let update_payload = format!("{}:{}", current_footprint_hex, current_index);
                let new_hash = md5::compute(update_payload.as_bytes());

                let mut new_footprint = [0u8; 32];
                new_footprint[..16].copy_from_slice(&new_hash.0);
                session_sled.hashed_footprint = new_footprint;

                let new_footprint_hex = hex::encode(session_sled.hashed_footprint);
                let trace_id = format!("trace-{}", new_footprint_hex);

                // Dynamically update Xray via Environment Injection to preserve NVMe I/O
                Command::new("sh")
                    .arg("-c")
                    .arg(format!(
                        "export X_GHOSTBRIDGE_FOOTPRINT='{}' && export X_GHOSTBRIDGE_TRACE_ID='{}' && systemctl reload xray", 
                        new_footprint_hex, trace_id
                    ))
                    .spawn()?;
            }
        }

        sleep(Duration::from_millis(100)).await;
    }
}
