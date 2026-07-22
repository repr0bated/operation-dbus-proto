//! Static draft pages — the pre-finalisation authoring workflow.
//!
//! Before a page is minted into the gemma catalog, it lives as a plain JSON
//! file in `pages/<route>.json`, written in the json-render.dev DSL the
//! interpreter consumes. Each file carries an optional `data` payload — a
//! canned stand-in for the live gRPC stream — and an optional `source` block
//! naming a D-Bus plugin method polled via `PluginService.CallMethod`.
//!
//! Hot-reload: the loader rescans `pages/` every ~600 ms (no-op when built
//! with `--features embed-pages`).

use std::collections::HashMap;
#[cfg(not(feature = "embed-pages"))]
use std::path::PathBuf;
#[cfg(not(feature = "embed-pages"))]
use std::time::{Duration, Instant, SystemTime};

use serde_json::Value;

use crate::nav::Route;

/// Optional live feed for a page: a D-Bus plugin object reached through
/// `operation.v1.PluginService/CallMethod` on the same server.
///
/// ```json
/// "source": { "plugin": "gemma_brain", "method": "get_ui_spec",
///             "args": [], "poll_secs": 5 }
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct PageSource {
    pub plugin: String,
    pub method: String,
    /// JSON array → repeated `google.protobuf.Value` arguments.
    pub args: Value,
    pub poll_secs: u64,
}

/// One parsed page file.
#[derive(Clone)]
pub struct StaticPage {
    pub spec: Value,
    pub data: Value,
    pub source: Option<PageSource>,
    /// Parse error surfaced in the UI instead of the page.
    pub error: Option<String>,
}

/// Hot-reloaded (or embedded) draft pages keyed by route slug.
pub struct StaticPages {
    pages: HashMap<String, StaticPage>,
    #[cfg(not(feature = "embed-pages"))]
    last_scan: Instant,
    #[cfg(not(feature = "embed-pages"))]
    mtimes: HashMap<String, SystemTime>,
}

impl Default for StaticPages {
    fn default() -> Self {
        Self {
            pages: HashMap::new(),
            #[cfg(not(feature = "embed-pages"))]
            last_scan: Instant::now(),
            #[cfg(not(feature = "embed-pages"))]
            mtimes: HashMap::new(),
        }
    }
}

impl StaticPages {
    pub fn new() -> Self {
        let mut s = Self::default();
        s.reload();
        s
    }

    pub fn is_embedded(&self) -> bool {
        cfg!(feature = "embed-pages")
    }

    pub fn get(&self, route: Route) -> Option<&StaticPage> {
        self.pages.get(&route_slug(route))
    }

    /// Rescan disk when not embedded. Called once per frame from the app loop.
    pub fn tick(&mut self) {
        #[cfg(not(feature = "embed-pages"))]
        {
            if self.last_scan.elapsed() >= Duration::from_millis(600) {
                self.reload();
                self.last_scan = Instant::now();
            }
        }
    }

    fn reload(&mut self) {
        #[cfg(feature = "embed-pages")]
        {
            self.load_embedded();
        }
        #[cfg(not(feature = "embed-pages"))]
        {
            self.load_from_disk();
        }
    }

    #[cfg(feature = "embed-pages")]
    fn load_embedded(&mut self) {
        use include_dir::include_dir;
        static PAGES: include_dir::Dir = include_dir!("$CARGO_MANIFEST_DIR/pages");
        for entry in PAGES.entries() {
            if let include_dir::DirEntry::File(f) = entry {
                if f.path().extension().and_then(|s| s.to_str()) != Some("json") {
                    continue;
                }
                let slug = f.path().file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
                if slug.is_empty() {
                    continue;
                }
                let raw = std::str::from_utf8(f.contents()).unwrap_or("");
                self.pages.insert(slug.clone(), parse_page(&slug, raw));
            }
        }
    }

    #[cfg(not(feature = "embed-pages"))]
    fn load_from_disk(&mut self) {
        let dir = pages_dir();
        let Ok(read_dir) = std::fs::read_dir(&dir) else {
            self.pages.clear();
            self.mtimes.clear();
            return;
        };
        let mut seen = std::collections::HashSet::new();
        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let slug = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
            if slug.is_empty() {
                continue;
            }
            seen.insert(slug.clone());
            let mtime = entry.metadata().and_then(|m| m.modified()).unwrap_or(SystemTime::UNIX_EPOCH);
            if self.mtimes.get(&slug) == Some(&mtime) {
                continue;
            }
            match std::fs::read_to_string(&path) {
                Ok(raw) => {
                    self.pages.insert(slug.clone(), parse_page(&slug, &raw));
                    self.mtimes.insert(slug, mtime);
                }
                Err(e) => {
                    self.pages.insert(
                        slug.clone(),
                        StaticPage {
                            spec: Value::Null,
                            data: Value::Null,
                            source: None,
                            error: Some(format!("read {}: {e}", path.display())),
                        },
                    );
                }
            }
        }
        self.pages.retain(|slug, _| seen.contains(slug));
        self.mtimes.retain(|slug, _| seen.contains(slug));
    }
}

/// Route enum → filename slug, e.g. `Route::PrivacyNetwork` → `"privacynetwork"`.
pub fn route_slug(route: Route) -> String {
    format!("{route:?}").to_lowercase()
}

#[cfg(not(feature = "embed-pages"))]
pub fn pages_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("ZEROCLAW_PAGES_DIR") {
        return PathBuf::from(dir);
    }
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        let p = PathBuf::from(manifest).join("pages");
        if p.is_dir() {
            return p;
        }
    }
    PathBuf::from("pages")
}

fn parse_source(doc: &Value) -> Option<PageSource> {
    let src = doc.get("source")?;
    Some(PageSource {
        plugin: src.get("plugin")?.as_str()?.to_string(),
        method: src.get("method")?.as_str()?.to_string(),
        args: src.get("args").cloned().unwrap_or_else(|| Value::Array(vec![])),
        poll_secs: src.get("poll_secs").and_then(Value::as_u64).unwrap_or(5),
    })
}

fn parse_page(slug: &str, raw: &str) -> StaticPage {
    match serde_json::from_str::<Value>(raw) {
        Ok(doc) => match doc.get("spec").cloned() {
            Some(spec) => StaticPage {
                spec,
                data: doc.get("data").cloned().unwrap_or(Value::Null),
                source: parse_source(&doc),
                error: None,
            },
            None => StaticPage {
                spec: Value::Null,
                data: Value::Null,
                source: None,
                error: Some(format!("pages/{slug}.json: missing top-level `spec` field")),
            },
        },
        Err(e) => StaticPage {
            spec: Value::Null,
            data: Value::Null,
            source: None,
            error: Some(format!("pages/{slug}.json: {e}")),
        },
    }
}
