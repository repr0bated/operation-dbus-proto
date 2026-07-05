//! Live data feed for static draft pages.
//!
//! A page's optional `source` block names a D-Bus plugin object; data is fetched
//! via `operation.v1.PluginService/CallMethod` — no per-service protos. The
//! unwrapped `result` payload becomes the bind root for the page, replacing the
//! static `data` sample once the plugin returns something non-empty.

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;
use serde_json::{json, Value};
use tokio::task::AbortHandle;
use tokio::time::{sleep, Duration};

use crate::catalog::static_pages::PageSource;
use crate::grpc::{self, ReflectionRegistry};

const PLUGIN_SERVICE: &str = "operation.v1.PluginService";
const CALL_METHOD: &str = "CallMethod";

#[derive(Clone, Debug, Default)]
pub struct PageLive {
    pub value: Option<Value>,
    pub status: String,
}

struct Entry {
    fingerprint: String,
    live: PageLive,
    abort: AbortHandle,
}

#[derive(Default)]
pub struct PageDataHub {
    inner: Arc<Mutex<HashMap<String, Entry>>>,
}

impl PageDataHub {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn live(&self, slug: &str) -> Option<PageLive> {
        self.inner.lock().get(slug).map(|e| e.live.clone())
    }

    /// Ensure a poll task is running for `slug` + `source`. Restarts when the
    /// source fingerprint changes (hot reload of pages/*.json).
    pub fn ensure(
        &self,
        slug: &str,
        source: &PageSource,
        registry: ReflectionRegistry,
        ctx: egui::Context,
    ) {
        let fingerprint = format!("{}:{}:{}", source.plugin, source.method, source.args);
        {
            let map = self.inner.lock();
            if let Some(e) = map.get(slug) {
                if e.fingerprint == fingerprint {
                    return;
                }
            }
        }

        // Abort stale task for this slug.
        {
            let mut map = self.inner.lock();
            if let Some(old) = map.remove(slug) {
                old.abort.abort();
            }
        }

        let slug_owned = slug.to_string();
        let source = source.clone();
        let inner = self.inner.clone();

        let handle = tokio::spawn(async move {
            let poll = Duration::from_secs(source.poll_secs.max(1));
            loop {
                set_status(&inner, &slug_owned, &format!("polling {}.{}…", source.plugin, source.method));
                match fetch_plugin(&registry, &source).await {
                    Ok(Some(val)) if !val.is_null() && val != json!({}) => {
                        set_value(
                            &inner,
                            &slug_owned,
                            Some(val),
                            &format!("LIVE — {}.{}", source.plugin, source.method),
                        );
                    }
                    Ok(_) => {
                        set_value(
                            &inner,
                            &slug_owned,
                            None,
                            &format!("static sample — {}.{} returned empty", source.plugin, source.method),
                        );
                    }
                    Err(e) => {
                        set_value(
                            &inner,
                            &slug_owned,
                            None,
                            &format!("error — {}.{}: {e:#}", source.plugin, source.method),
                        );
                    }
                }
                ctx.request_repaint();
                sleep(poll).await;
            }
        });

        let mut map = self.inner.lock();
        map.insert(
            slug.to_string(),
            Entry {
                fingerprint,
                live: PageLive {
                    value: None,
                    status: "connecting…".into(),
                },
                abort: handle.abort_handle(),
            },
        );
    }
}

async fn fetch_plugin(reg: &ReflectionRegistry, source: &PageSource) -> anyhow::Result<Option<Value>> {
    let args = match &source.args {
        Value::Array(arr) => arr.clone(),
        other if other.is_null() => vec![],
        other => vec![other.clone()],
    };

    let body = json!({
        "plugin_id": source.plugin,
        "object_path": format!("/org/opdbus/v1/plugins/{}", source.plugin),
        "interface_name": format!("org.opdbus.{}.v1.Plugin", source.plugin),
        "method_name": source.method,
        "arguments": args,
        "actor_id": "",
        "capability_id": "",
    });

    let resp = grpc::invoke_unary(reg, PLUGIN_SERVICE, CALL_METHOD, &body).await?;
    if resp.get("success").and_then(Value::as_bool) == Some(false) {
        let msg = resp
            .pointer("/error/message")
            .and_then(Value::as_str)
            .unwrap_or("call failed");
        anyhow::bail!("{msg}");
    }
    Ok(resp.get("result").cloned())
}

fn set_status(inner: &Arc<Mutex<HashMap<String, Entry>>>, slug: &str, status: &str) {
    if let Some(e) = inner.lock().get_mut(slug) {
        e.live.status = status.to_string();
    }
}

fn set_value(inner: &Arc<Mutex<HashMap<String, Entry>>>, slug: &str, value: Option<Value>, status: &str) {
    if let Some(e) = inner.lock().get_mut(slug) {
        e.live.value = value;
        e.live.status = status.to_string();
    }
}
