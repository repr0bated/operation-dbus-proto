//! Live data feed for static draft pages.
//!
//! A page's optional `source` block names a D-Bus plugin object; data is fetched
//! via `operation.v1.PluginService/CallMethod` — no per-service protos. The
//! unwrapped `result` payload becomes the bind root for the page, replacing the
//! static `data` sample once the plugin returns something non-empty.

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;
use serde_json::{json, Map, Value};
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
                    Ok(Some(val)) if is_meaningful_result(&val) => {
                        set_value(
                            &inner,
                            &slug_owned,
                            Some(unwrap_protobuf_value(val)),
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

/// True when a CallMethod `result` carries bindable data (not null/empty).
fn is_meaningful_result(v: &Value) -> bool {
    match unwrap_protobuf_value(v.clone()) {
        Value::Null => false,
        Value::Object(m) if m.is_empty() => false,
        Value::Array(a) if a.is_empty() => false,
        _ => true,
    }
}

/// Reflection decodes `google.protobuf.Value` with kind wrappers; flatten to plain JSON.
fn unwrap_protobuf_value(v: Value) -> Value {
    let Value::Object(map) = v else {
        return v;
    };
    if map.len() != 1 {
        return Value::Object(map);
    }

    if map.contains_key("nullValue") || map.contains_key("null_value") {
        return Value::Null;
    }
    if let Some(Value::Bool(b)) = map.get("boolValue").or_else(|| map.get("bool_value")) {
        return Value::Bool(*b);
    }
    if let Some(Value::String(s)) = map.get("stringValue").or_else(|| map.get("string_value")) {
        return Value::String(s.clone());
    }
    if let Some(Value::Number(n)) = map.get("numberValue").or_else(|| map.get("number_value")) {
        return Value::Number(n.clone());
    }
    if let Some(sv) = map.get("structValue").or_else(|| map.get("struct_value")) {
        return unwrap_protobuf_struct(sv);
    }
    if let Some(lv) = map.get("listValue").or_else(|| map.get("list_value")) {
        return unwrap_protobuf_list(lv);
    }
    Value::Object(map)
}

fn unwrap_protobuf_struct(v: &Value) -> Value {
    let fm = v
        .get("fields")
        .and_then(Value::as_object)
        .or_else(|| v.as_object());
    if let Some(fm) = fm {
        let out: Map<String, Value> = fm
            .iter()
            .map(|(k, v)| (k.clone(), unwrap_protobuf_value(v.clone())))
            .collect();
        return Value::Object(out);
    }
    v.clone()
}

fn unwrap_protobuf_list(v: &Value) -> Value {
    let values = v
        .get("values")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    Value::Array(values.into_iter().map(unwrap_protobuf_value).collect())
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
