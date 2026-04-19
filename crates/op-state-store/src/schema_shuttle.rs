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
    pub wireguard_pubkey: String,
    pub mutation_index: u64,
    pub hashed_footprint: String, // The current "Thought" injected into Xray
    pub trace_id: String,         // Links to Qdrant Semantic Memory
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

        // Serialize the PluginSchema to a simd_json Value for canonicalization
        let schema_json = serde_json::to_string(current_schema)
            .map_err(|e| format!("Serialization Failed: {}", e))?;
        let mut schema_bytes = schema_json.into_bytes();
        let schema_value: simd_json::OwnedValue = simd_json::to_owned_value(&mut schema_bytes)
            .map_err(|e| format!("SIMD-JSON parse failed: {}", e))?;
        let canonical_state = serde_json::to_string(&canonicalize_json(&schema_value))
            .map_err(|e| format!("Canonical serialization failed: {}", e))?;

        let payload = format!("{}:{}", wg_pubkey, canonical_state);
        let genesis_hash = format!("{:x}", md5::compute(payload.as_bytes()));

        Ok(IdentitySled {
            wireguard_pubkey: wg_pubkey.to_string(),
            mutation_index: current_schema.mutation_index.unwrap_or(0),
            hashed_footprint: genesis_hash.clone(),
            trace_id: format!("trace-{}", genesis_hash),
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

    println!(
        "[SUCCESS] Identity Sled Forged. Footprint: {}",
        session_sled.hashed_footprint
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

                let update_payload = format!("{}:{}", session_sled.hashed_footprint, current_index);
                session_sled.hashed_footprint =
                    format!("{:x}", md5::compute(update_payload.as_bytes()));
                session_sled.trace_id = format!("trace-{}", session_sled.hashed_footprint);

                // Dynamically update Xray via Environment Injection to preserve NVMe I/O
                Command::new("sh")
                    .arg("-c")
                    .arg(format!(
                        "export X_GHOSTBRIDGE_FOOTPRINT='{}' && export X_GHOSTBRIDGE_TRACE_ID='{}' && systemctl reload xray", 
                        session_sled.hashed_footprint, session_sled.trace_id
                    ))
                    .spawn()?;
            }
        }

        sleep(Duration::from_millis(100)).await;
    }
}
