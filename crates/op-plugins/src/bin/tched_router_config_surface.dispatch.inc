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
/// marker, env dirs) should be mirrored here if the binary ever exposes it.
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

/// The official binary's full config schema, compiled once.
///
/// This is the same document `zeroclaw config schema` prints, captured at
/// `schemas/zeroclaw/config.schema.json`. It is the authority for what a valid
/// config looks like — there is no linked upstream type to defer to.
fn official_config_validator() -> &'static jsonschema::Validator {
    static VALIDATOR: std::sync::OnceLock<jsonschema::Validator> = std::sync::OnceLock::new();
    VALIDATOR.get_or_init(|| {
        let document: JsonValue = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/zeroclaw/sections/_full.json"
        )))
        .expect("embedded zeroclaw config schema is valid JSON");
        jsonschema::validator_for(&document).expect("embedded zeroclaw config schema compiles")
    })
}

/// Validate a whole config document against the official binary's schema.
fn validate_against_official_schema(
    value: &JsonValue,
    what: &str,
) -> std::result::Result<(), TchedRouterError> {
    official_config_validator()
        .validate(value)
        .map_err(|e| TchedRouterError::ExecutionDenied {
            reason: format!("{what} fails official zeroclaw schema validation: {e}"),
        })
}

/// Read the config file and validate it against the OFFICIAL binary's schema —
/// an invalid file is an error here, never a silent partial read.
fn load_upstream_value() -> std::result::Result<JsonValue, TchedRouterError> {
    let path = config_path();
    let text = std::fs::read_to_string(&path).map_err(|e| TchedRouterError::ExecutionDenied {
        reason: format!("read {}: {e}", path.display()),
    })?;
    let parsed: toml::Value =
        toml::from_str(&text).map_err(|e| TchedRouterError::ExecutionDenied {
            reason: format!("parse {}: {e}", path.display()),
        })?;
    let value = serde_json::to_value(&parsed).map_err(|e| TchedRouterError::ExecutionDenied {
        reason: format!("serialize config: {e}"),
    })?;
    validate_against_official_schema(&value, &path.display().to_string())?;
    Ok(value)
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
    let mut incoming = args.value;
    if contains_masked_placeholder(&incoming) {
        let existing_root = load_upstream_value()?;
        if let Some(existing_section) = existing_root.get(&args.section) {
            restore_masked_leaves(&mut incoming, existing_section);
        }
    }
    if contains_masked_placeholder(&incoming) {
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
        serde_json::from_value(incoming.clone()).map_err(|e| {
            TchedRouterError::ExecutionDenied {
                reason: format!("section value is not TOML-representable: {e}"),
            }
        })?;
    table.insert(args.section.clone(), new_toml);
    let serialized = toml::to_string(&doc).map_err(|e| TchedRouterError::ExecutionDenied {
        reason: format!("serialize config: {e}"),
    })?;
    // Validate against the OFFICIAL binary's schema before writing.
    let candidate: toml::Value =
        toml::from_str(&serialized).map_err(|e| TchedRouterError::ExecutionDenied {
            reason: format!("patched config is not valid TOML: {e}"),
        })?;
    let candidate_json =
        serde_json::to_value(&candidate).map_err(|e| TchedRouterError::ExecutionDenied {
            reason: format!("serialize patched config: {e}"),
        })?;
    validate_against_official_schema(&candidate_json, "patched config")?;
    write_config_atomically(&path, &serialized)?;
    Ok(DispatchOutcome::plain(serde_json::json!({
        "section": args.section,
        "value": incoming,
    })))
}

/// Durably replace the live config without exposing a truncated intermediate file.
fn write_config_atomically(
    path: &std::path::Path,
    serialized: &str,
) -> std::result::Result<(), TchedRouterError> {
    use std::io::Write;
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::MetadataExt;

    // Follow an operator-supplied symlink exactly as the previous in-place
    // write did, while still staging on the target's filesystem.
    let resolved = std::fs::canonicalize(path).map_err(|e| TchedRouterError::ExecutionDenied {
        reason: format!("resolve config path {}: {e}", path.display()),
    })?;
    let metadata = std::fs::metadata(&resolved).map_err(|e| TchedRouterError::ExecutionDenied {
        reason: format!("read config metadata {}: {e}", resolved.display()),
    })?;
    let parent = resolved
        .parent()
        .ok_or_else(|| TchedRouterError::ExecutionDenied {
            reason: format!("config path has no parent: {}", resolved.display()),
        })?;
    let mut staged =
        tempfile::NamedTempFile::new_in(parent).map_err(|e| TchedRouterError::ExecutionDenied {
            reason: format!("stage config beside {}: {e}", resolved.display()),
        })?;
    staged
        .write_all(serialized.as_bytes())
        .map_err(|e| TchedRouterError::ExecutionDenied {
            reason: format!("write staged config for {}: {e}", resolved.display()),
        })?;
    // Writing can clear set-id mode bits and security attributes. Restore all
    // inherited metadata only after the staged contents are final.
    nix::unistd::fchown(
        staged.as_file().as_raw_fd(),
        Some(nix::unistd::Uid::from_raw(metadata.uid())),
        Some(nix::unistd::Gid::from_raw(metadata.gid())),
    )
    .map_err(|e| TchedRouterError::ExecutionDenied {
        reason: format!("preserve config ownership for {}: {e}", resolved.display()),
    })?;
    staged
        .as_file()
        .set_permissions(metadata.permissions())
        .map_err(|e| TchedRouterError::ExecutionDenied {
            reason: format!(
                "preserve config permissions for {}: {e}",
                resolved.display()
            ),
        })?;
    for name in xattr::list(&resolved).map_err(|e| TchedRouterError::ExecutionDenied {
        reason: format!("list config attributes for {}: {e}", resolved.display()),
    })? {
        let value = xattr::get(&resolved, &name)
            .map_err(|e| TchedRouterError::ExecutionDenied {
                reason: format!(
                    "read config attribute {:?} for {}: {e}",
                    name,
                    resolved.display()
                ),
            })?
            .ok_or_else(|| TchedRouterError::ExecutionDenied {
                reason: format!(
                    "config attribute {:?} disappeared from {}",
                    name,
                    resolved.display()
                ),
            })?;
        xattr::set(staged.path(), &name, &value).map_err(|e| {
            TchedRouterError::ExecutionDenied {
                reason: format!(
                    "preserve config attribute {:?} for {}: {e}",
                    name,
                    resolved.display()
                ),
            }
        })?;
    }
    staged
        .as_file_mut()
        .sync_all()
        .map_err(|e| TchedRouterError::ExecutionDenied {
            reason: format!("sync staged config for {}: {e}", resolved.display()),
        })?;
    staged
        .persist(&resolved)
        .map_err(|e| TchedRouterError::ExecutionDenied {
            reason: format!(
                "replace {} with staged config: {}",
                resolved.display(),
                e.error
            ),
        })?;
    std::fs::File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|e| TchedRouterError::ExecutionDenied {
            reason: format!("sync config directory {}: {e}", parent.display()),
        })
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

fn restore_masked_leaves(incoming: &mut JsonValue, existing: &JsonValue) {
    match incoming {
        JsonValue::String(value) if value == "***MASKED***" => {
            *incoming = existing.clone();
        }
        JsonValue::Object(map) => {
            if let JsonValue::Object(prior) = existing {
                for (key, value) in map.iter_mut() {
                    if let Some(previous) = prior.get(key) {
                        restore_masked_leaves(value, previous);
                    }
                }
            }
        }
        JsonValue::Array(items) => {
            if let JsonValue::Array(prior) = existing {
                for (index, value) in items.iter_mut().enumerate() {
                    if let Some(previous) = prior.get(index) {
                        restore_masked_leaves(value, previous);
                    }
                }
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{symlink, MetadataExt, PermissionsExt};

    #[test]
    fn patch_config_preserves_live_file_when_atomic_staging_fails() {
        let dir = tempfile::tempdir().expect("temp config directory");
        let path = dir.path().join("config.toml");
        symlink("/proc/version", &path).expect("link to deterministic read-only target");
        let original = std::fs::read_to_string(&path).expect("read original target");

        assert!(
            write_config_atomically(&path, "destroyed").is_err_and(|error| error
                .to_string()
                .contains("stage config beside /proc/version")),
            "patch must fail specifically while staging"
        );
        assert!(
            std::fs::symlink_metadata(&path)
                .expect("link metadata")
                .file_type()
                .is_symlink(),
            "failed patch must preserve the configured symlink"
        );
        assert_eq!(
            std::fs::read_to_string(&path).expect("read target after failed patch"),
            original,
            "failed patch must preserve live config"
        );
    }

    #[test]
    fn atomic_config_write_follows_symlink_and_preserves_metadata() {
        let dir = tempfile::tempdir().expect("temp config directory");
        let target = dir.path().join("config-target.toml");
        let path = dir.path().join("config.toml");
        std::fs::write(&target, "old").expect("write original config");
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o4640))
            .expect("set original permissions");
        xattr::set(&target, "user.opdbus-test", b"preserve").expect("set original xattr");
        let original_metadata = std::fs::metadata(&target).expect("original metadata");
        symlink(&target, &path).expect("create config symlink");

        write_config_atomically(&path, "new").expect("replace config atomically");

        assert!(
            std::fs::symlink_metadata(&path)
                .expect("link metadata")
                .file_type()
                .is_symlink(),
            "atomic replacement must not replace the configured symlink"
        );
        assert_eq!(
            std::fs::read_to_string(&target).expect("read replaced target"),
            "new"
        );
        let replaced_metadata = std::fs::metadata(&target).expect("replaced metadata");
        assert_eq!(replaced_metadata.uid(), original_metadata.uid());
        assert_eq!(replaced_metadata.gid(), original_metadata.gid());
        assert_eq!(replaced_metadata.mode() & 0o7777, 0o4640);
        assert_eq!(
            xattr::get(&target, "user.opdbus-test").expect("read replaced xattr"),
            Some(b"preserve".to_vec())
        );
    }
}
