//! Redis stream for real-time job notifications
//!
//! Provides pub/sub capabilities for job status updates,
//! enabling real-time monitoring and distributed coordination.

use crate::error::{Result, StateStoreError};
use crate::execution_job::ExecutionJob;
use redis::aio::MultiplexedConnection;
use redis::{AsyncCommands, Client};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

/// Stream name for job events
const JOB_STREAM: &str = "op:jobs";
/// Stream name for plugin state events
const PLUGIN_STREAM: &str = "op:plugins";
/// Consumer group name
const CONSUMER_GROUP: &str = "op-dbus";
/// Max stream length (for automatic trimming)
const MAX_STREAM_LENGTH: i64 = 10000;

/// Redis stream client for real-time notifications
pub struct RedisStream {
    conn: MultiplexedConnection,
    consumer_name: String,
}

/// Job event published to Redis stream
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobEvent {
    pub job_id: String,
    pub tool_name: String,
    pub status: String,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Plugin state event published to Redis stream
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginEvent {
    pub plugin_name: String,
    pub operation: String,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_hash: Option<String>,
}

impl RedisStream {
    /// Create a new Redis stream client
    ///
    /// URL format: `redis://localhost:6379` or `redis://:password@localhost:6379`
    pub async fn new(url: &str) -> Result<Self> {
        info!("Connecting to Redis at {}", url);

        let client = Client::open(url).map_err(StateStoreError::Redis)?;
        let conn = client
            .get_multiplexed_async_connection()
            .await
            .map_err(StateStoreError::Redis)?;

        // Generate unique consumer name
        let consumer_name = format!(
            "op-dbus-{}",
            uuid::Uuid::new_v4().to_string().split('-').next().unwrap()
        );

        let stream = Self {
            conn,
            consumer_name,
        };

        // Initialize consumer groups
        stream.initialize_streams().await?;

        info!(
            "Redis stream connected as consumer: {}",
            stream.consumer_name
        );
        Ok(stream)
    }

    /// Initialize streams and consumer groups
    async fn initialize_streams(&self) -> Result<()> {
        let mut conn = self.conn.clone();

        // Create consumer groups if they don't exist
        // We ignore errors if the group already exists
        let _: std::result::Result<(), redis::RedisError> = redis::cmd("XGROUP")
            .arg("CREATE")
            .arg(JOB_STREAM)
            .arg(CONSUMER_GROUP)
            .arg("$")
            .arg("MKSTREAM")
            .query_async(&mut conn)
            .await;

        let _: std::result::Result<(), redis::RedisError> = redis::cmd("XGROUP")
            .arg("CREATE")
            .arg(PLUGIN_STREAM)
            .arg(CONSUMER_GROUP)
            .arg("$")
            .arg("MKSTREAM")
            .query_async(&mut conn)
            .await;

        debug!("Redis streams initialized");
        Ok(())
    }

    /// Publish a job event
    pub async fn publish_job(&self, job: &ExecutionJob) -> Result<()> {
        let mut conn = self.conn.clone();

        let event = JobEvent {
            job_id: job.id.to_string(),
            tool_name: job.tool_name.clone(),
            status: format!("{:?}", job.status),
            timestamp: job.updated_at.to_rfc3339(),
            error: job.result.as_ref().and_then(|r| r.error.clone()),
        };

        let event_json = simd_json::to_string(&event)?;

        // Add to stream with automatic trimming
        let _: String = redis::cmd("XADD")
            .arg(JOB_STREAM)
            .arg("MAXLEN")
            .arg("~")
            .arg(MAX_STREAM_LENGTH)
            .arg("*")
            .arg("event")
            .arg(&event_json)
            .query_async(&mut conn)
            .await
            .map_err(StateStoreError::Redis)?;

        debug!("Published job event: {} - {:?}", job.id, job.status);
        Ok(())
    }

    /// Publish a plugin state event
    pub async fn publish_plugin_event(
        &self,
        plugin_name: &str,
        operation: &str,
        state_hash: Option<&str>,
    ) -> Result<()> {
        let mut conn = self.conn.clone();

        let event = PluginEvent {
            plugin_name: plugin_name.to_string(),
            operation: operation.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            state_hash: state_hash.map(String::from),
        };

        let event_json = simd_json::to_string(&event)?;

        let _: String = redis::cmd("XADD")
            .arg(PLUGIN_STREAM)
            .arg("MAXLEN")
            .arg("~")
            .arg(MAX_STREAM_LENGTH)
            .arg("*")
            .arg("event")
            .arg(&event_json)
            .query_async(&mut conn)
            .await
            .map_err(StateStoreError::Redis)?;

        debug!("Published plugin event: {} - {}", plugin_name, operation);
        Ok(())
    }

    /// Read pending job events (for catching up)
    pub async fn read_job_events(&self, count: usize) -> Result<Vec<JobEvent>> {
        let mut conn = self.conn.clone();

        let results: Vec<redis::Value> = redis::cmd("XREADGROUP")
            .arg("GROUP")
            .arg(CONSUMER_GROUP)
            .arg(&self.consumer_name)
            .arg("COUNT")
            .arg(count)
            .arg("STREAMS")
            .arg(JOB_STREAM)
            .arg(">")
            .query_async(&mut conn)
            .await
            .map_err(StateStoreError::Redis)?;

        parse_job_events(results)
    }

    /// Read pending plugin events
    pub async fn read_plugin_events(&self, count: usize) -> Result<Vec<PluginEvent>> {
        let mut conn = self.conn.clone();

        let results: Vec<redis::Value> = redis::cmd("XREADGROUP")
            .arg("GROUP")
            .arg(CONSUMER_GROUP)
            .arg(&self.consumer_name)
            .arg("COUNT")
            .arg(count)
            .arg("STREAMS")
            .arg(PLUGIN_STREAM)
            .arg(">")
            .query_async(&mut conn)
            .await
            .map_err(StateStoreError::Redis)?;

        parse_plugin_events(results)
    }

    /// Acknowledge processed events
    pub async fn ack_job_event(&self, event_id: &str) -> Result<()> {
        let mut conn = self.conn.clone();

        let _: i64 = redis::cmd("XACK")
            .arg(JOB_STREAM)
            .arg(CONSUMER_GROUP)
            .arg(event_id)
            .query_async(&mut conn)
            .await
            .map_err(StateStoreError::Redis)?;

        Ok(())
    }

    /// Get stream info
    pub async fn get_stream_info(&self) -> Result<StreamInfo> {
        let mut conn = self.conn.clone();

        let job_len: i64 = redis::cmd("XLEN")
            .arg(JOB_STREAM)
            .query_async(&mut conn)
            .await
            .unwrap_or(0);

        let plugin_len: i64 = redis::cmd("XLEN")
            .arg(PLUGIN_STREAM)
            .query_async(&mut conn)
            .await
            .unwrap_or(0);

        Ok(StreamInfo {
            job_stream_length: job_len as u64,
            plugin_stream_length: plugin_len as u64,
            consumer_name: self.consumer_name.clone(),
        })
    }

    /// Publish a simple key-value update (for caching)
    pub async fn set_cached_state(
        &self,
        key: &str,
        value: &simd_json::OwnedValue,
        ttl_secs: u64,
    ) -> Result<()> {
        let mut conn = self.conn.clone();
        let value_json = simd_json::to_string(value)?;

        let _: () = conn
            .set_ex(key, value_json, ttl_secs)
            .await
            .map_err(StateStoreError::Redis)?;

        Ok(())
    }

    /// Get cached state
    pub async fn get_cached_state(&self, key: &str) -> Result<Option<simd_json::OwnedValue>> {
        let mut conn = self.conn.clone();

        let value: Option<String> = conn.get(key).await.map_err(StateStoreError::Redis)?;

        match value {
            Some(json) => {
                let mut json_mut = json;
                Ok(Some(unsafe { simd_json::from_str(&mut json_mut)? }))
            }
            None => Ok(None),
        }
    }

    /// Check if Redis is connected
    pub async fn ping(&self) -> Result<bool> {
        let mut conn = self.conn.clone();
        let result: std::result::Result<String, _> =
            redis::cmd("PING").query_async(&mut conn).await;
        Ok(result.map(|s| s == "PONG").unwrap_or(false))
    }
}

/// Stream statistics
#[derive(Debug, Clone)]
pub struct StreamInfo {
    pub job_stream_length: u64,
    pub plugin_stream_length: u64,
    pub consumer_name: String,
}

/// Parse job events from Redis response - simplified to avoid version-specific enum variants
fn parse_job_events(results: Vec<redis::Value>) -> Result<Vec<JobEvent>> {
    use redis::FromRedisValue;

    let mut events = Vec::new();

    // Convert using redis's built-in parsing where possible
    // For stream responses, we try to extract strings from the nested structure
    for result in &results {
        if let Ok(entries) = Vec::<(String, Vec<(String, String)>)>::from_redis_value(result) {
            for (_entry_id, fields) in entries {
                for (key, mut value) in fields {
                    if key == "event" {
                        if let Ok(event) = unsafe { simd_json::from_str::<JobEvent>(&mut value) } {
                            events.push(event);
                        }
                    }
                }
            }
        }
    }

    Ok(events)
}

/// Parse plugin events from Redis response
fn parse_plugin_events(results: Vec<redis::Value>) -> Result<Vec<PluginEvent>> {
    use redis::FromRedisValue;

    let mut events = Vec::new();

    for result in &results {
        if let Ok(entries) = Vec::<(String, Vec<(String, String)>)>::from_redis_value(result) {
            for (_entry_id, fields) in entries {
                for (key, mut value) in fields {
                    if key == "event" {
                        if let Ok(event) = unsafe { simd_json::from_str::<PluginEvent>(&mut value) }
                        {
                            events.push(event);
                        }
                    }
                }
            }
        }
    }

    Ok(events)
}

/// Try to connect to Redis (optional, returns None if unavailable)
pub async fn try_connect(url: &str) -> Option<RedisStream> {
    match RedisStream::new(url).await {
        Ok(stream) => Some(stream),
        Err(e) => {
            warn!("Redis not available ({}): {}", url, e);
            None
        }
    }
}
