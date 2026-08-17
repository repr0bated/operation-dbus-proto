// ── Dispatch ────────────────────────────────────────────────────────────────

/// Dispatch a config-surface method. Returns `None` when `method` is not
/// owned by this surface so the caller can fall through to other dispatchers.
pub fn dispatch_config_method(
    method: &str,
    json_args: &str,
    state: &TchedRouterState,
) -> Option<std::result::Result<DispatchOutcome, TchedRouterError>> {
    match method {
        "GetConfig" => Some(get_full_config()),
        "PatchConfig" => Some(patch_config(json_args)),
        other => SECTIONS
            .iter()
            .find(|(name, _, _)| *name == other)
            .map(|(_, key, source)| read_section(key, *source, state)),
    }
}

/// Path of the live 3tched Router config file.
///
/// Minimal mirror of upstream resolution for now: explicit
/// `TCHED_ROUTER_CONFIG_PATH` / `ZEROCLAW_CONFIG_PATH` override, else the
/// default `~/.zeroclaw/config.toml`. Upstream's full resolution (workspace
/// marker, env dirs) should replace this once exposed by the `zeroclaw` crate.
fn config_path() -> std::path::PathBuf {
    if let Ok(p) = std::env::var("TCHED_ROUTER_CONFIG_PATH") {
        return p.into();
    }
    if let Ok(p) = std::env::var("ZEROCLAW_CONFIG_PATH") {
        return p.into();
    }
    dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("/root"))
        .join(".zeroclaw")
        .join("config.toml")
}

/// Read the config file and parse it through the REAL upstream type — an
/// invalid file is an error here, never a silent partial read.
fn load_upstream_value() -> std::result::Result<JsonValue, TchedRouterError> {
    let path = config_path();
    let text = std::fs::read_to_string(&path).map_err(|e| TchedRouterError::ExecutionDenied {
        reason: format!("read {}: {e}", path.display()),
    })?;
    let config: zeroclaw::Config =
        toml::from_str(&text).map_err(|e| TchedRouterError::ExecutionDenied {
            reason: format!("parse {}: {e}", path.display()),
        })?;
    serde_json::to_value(&config).map_err(|e| TchedRouterError::ExecutionDenied {
        reason: format!("serialize config: {e}"),
    })
}

fn read_section(
    key: &str,
    source: SectionSource,
    state: &TchedRouterState,
) -> std::result::Result<DispatchOutcome, TchedRouterError> {
    let root = match source {
        SectionSource::Upstream => load_upstream_value()?,
        SectionSource::Local => {
            serde_json::to_value(state).map_err(|e| TchedRouterError::ExecutionDenied {
                reason: format!("serialize state: {e}"),
            })?
        }
    };
    let value = root
        .get(key)
        .cloned()
        .ok_or_else(|| TchedRouterError::ExecutionDenied {
            reason: format!("config section missing: {key}"),
        })?;
    Ok(DispatchOutcome::plain(
        serde_json::json!({ "config": value }),
    ))
}

fn get_full_config() -> std::result::Result<DispatchOutcome, TchedRouterError> {
    let mut root = load_upstream_value()?;
    mask_secrets(&mut root);
    Ok(DispatchOutcome::plain(
        serde_json::json!({ "config": root }),
    ))
}

fn patch_config(json_args: &str) -> std::result::Result<DispatchOutcome, TchedRouterError> {
    #[derive(Deserialize)]
    struct Args {
        section: String,
        value: JsonValue,
    }
    let args: Args =
        serde_json::from_str(json_args).map_err(|e| TchedRouterError::ExecutionDenied {
            reason: format!("PatchConfig args: {e}"),
        })?;
    if !SECTIONS
        .iter()
        .any(|(_, key, source)| *key == args.section && *source == SectionSource::Upstream)
    {
        return Err(TchedRouterError::ExecutionDenied {
            reason: format!(
                "PatchConfig: unknown or non-upstream section `{}` \
                 (local sections are mutated via their own Set* methods)",
                args.section
            ),
        });
    }
    if contains_masked_placeholder(&args.value) {
        return Err(TchedRouterError::ExecutionDenied {
            reason: "PatchConfig: value contains \"***MASKED***\" — masked \
                     placeholders cannot be written"
                .to_string(),
        });
    }
    let path = config_path();
    let text = std::fs::read_to_string(&path).map_err(|e| TchedRouterError::ExecutionDenied {
        reason: format!("read {}: {e}", path.display()),
    })?;
    let mut doc: toml::Value =
        toml::from_str(&text).map_err(|e| TchedRouterError::ExecutionDenied {
            reason: format!("parse {}: {e}", path.display()),
        })?;
    let table = doc
        .as_table_mut()
        .ok_or_else(|| TchedRouterError::ExecutionDenied {
            reason: "config root is not a table".to_string(),
        })?;
    let new_toml: toml::Value =
        serde_json::from_value(args.value.clone()).map_err(|e| {
            TchedRouterError::ExecutionDenied {
                reason: format!("section value is not TOML-representable: {e}"),
            }
        })?;
    table.insert(args.section.clone(), new_toml);
    let serialized = toml::to_string(&doc).map_err(|e| TchedRouterError::ExecutionDenied {
        reason: format!("serialize config: {e}"),
    })?;
    // Validate against the REAL upstream type before writing.
    let _: zeroclaw::Config =
        toml::from_str(&serialized).map_err(|e| TchedRouterError::ExecutionDenied {
            reason: format!("patched config fails upstream validation: {e}"),
        })?;
    std::fs::write(&path, serialized).map_err(|e| TchedRouterError::ExecutionDenied {
        reason: format!("write {}: {e}", path.display()),
    })?;
    Ok(DispatchOutcome::plain(serde_json::json!({
        "section": args.section,
        "value": args.value,
    })))
}

/// Recursively mask secret-bearing keys in a config value.
fn mask_secrets(value: &mut JsonValue) {
    const SECRET_KEYS: &[&str] = &[
        "api_key",
        "apikey",
        "token",
        "secret",
        "password",
        "private_key",
    ];
    match value {
        JsonValue::Object(map) => {
            for (k, v) in map.iter_mut() {
                let lk = k.to_ascii_lowercase();
                if SECRET_KEYS.iter().any(|s| lk.contains(s)) && v.is_string() {
                    *v = JsonValue::String("***MASKED***".to_string());
                } else {
                    mask_secrets(v);
                }
            }
        }
        JsonValue::Array(items) => items.iter_mut().for_each(mask_secrets),
        _ => {}
    }
}

fn contains_masked_placeholder(value: &JsonValue) -> bool {
    match value {
        JsonValue::String(s) => s == "***MASKED***",
        JsonValue::Array(items) => items.iter().any(contains_masked_placeholder),
        JsonValue::Object(map) => map.values().any(contains_masked_placeholder),
        _ => false,
    }
}
