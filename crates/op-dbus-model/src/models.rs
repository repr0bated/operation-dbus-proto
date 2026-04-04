use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plugin {
    pub name: String,
    pub service_name: String,
    pub base_object: simd_json::OwnedValue,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schema {
    pub id: String,
    pub plugin_name: String,
    pub definition: simd_json::OwnedValue,
    pub discovered_from: Option<String>,
    pub discovered_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}
