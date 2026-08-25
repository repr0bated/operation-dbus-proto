//! opblob — drive the schema→blob→catalog→btrfs pipeline from the shell.
//!
//! ```text
//! opblob demo-seal <dir>              blobify demo plugins into a catalog dir
//! opblob seal-shm                     blobify real plugins into /dev/shm/opdbus/plugin-blobs
//! opblob stage-shm                    stage the persisted catalog into SHM (boot path)
//! opblob persist                      copy the SHM catalog to the persisted store
//! opblob seal-plugins <dir>           blobify real plugins into a catalog dir
//! opblob inspect <file.blob>          print identity, services, methods
//! opblob catalog <dir>                list active services in a catalog dir
//! opblob btrfs-seal <image> <dir>     seal a catalog dir into a btrfs image
//! opblob keygen <keyfile>             generate the account WG identity keypair
//! ```

use op_blob::{
    blob, blobify_plugin_schema, btrfs,
    catalog::{ActiveReflectionCatalog, DEFAULT_SHM_DIR},
    demo, descriptor,
    identity::WgKeypair,
    BlobRef,
};
use op_plugins::DefaultPluginRegistry;
use op_state_store::MemoryStore;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.as_slice() {
        [cmd, dir] if cmd == "demo-seal" => demo_seal(Path::new(dir)),
        [cmd] if cmd == "seal-shm" => seal_plugins(Path::new(DEFAULT_SHM_DIR)),
        [cmd] if cmd == "stage-shm" => stage_shm(),
        [cmd] if cmd == "persist" => {
            persist_catalog(Path::new(DEFAULT_SHM_DIR), Path::new(PERSISTED_CATALOG_DIR))
        }
        [cmd, dir] if cmd == "seal-plugins" => seal_plugins(Path::new(dir)),
        [cmd, file] if cmd == "inspect" => inspect(Path::new(file)),
        [cmd, dir] if cmd == "catalog" => catalog(Path::new(dir)),
        [cmd, image, dir] if cmd == "btrfs-seal" => btrfs_seal(Path::new(image), Path::new(dir)),
        [cmd, keyfile] if cmd == "keygen" => keygen(Path::new(keyfile)),
        _ => {
            eprintln!(
                "usage: opblob demo-seal <dir> | seal-shm | stage-shm | persist | seal-plugins <dir> | inspect <file.blob> | catalog <dir> | btrfs-seal <image> <dir> | keygen <keyfile>"
            );
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("opblob: {e}");
            ExitCode::FAILURE
        }
    }
}

fn account_identity(dir: &Path) -> Result<WgKeypair, String> {
    // The account keypair is the identity; the session it defines persists
    // for the life of the account, so reuse the key if present.
    let keyfile = dir.join("identity.key");
    if keyfile.exists() {
        WgKeypair::load(&keyfile).map_err(|e| e.to_string())
    } else {
        let kp = WgKeypair::generate();
        kp.save(&keyfile).map_err(|e| e.to_string())?;
        Ok(kp)
    }
}

fn demo_seal(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let kp = account_identity(dir)?;
    let id = kp.public_identity();
    println!(
        "identity  wg-pub {}  key-id {}",
        id.wg_public_key, id.key_id
    );
    println!("session   {} (account-persistent)", id.session.session_id);

    let mut cat = ActiveReflectionCatalog::open(dir).map_err(|e| e.to_string())?;
    for schema in [demo::unix_socket_schema(), demo::wireguard_schema()] {
        let b = blob::blobify_with_identity(&schema, Some(id.clone()));
        let path = cat.upsert_blob(&b).map_err(|e| e.to_string())?;
        println!(
            "sealed    {}  ({} methods, schema {})",
            path.display(),
            b.manifest.methods.len(),
            &b.manifest.schema_hash[..16]
        );
    }
    println!("services:");
    for s in cat.list_services() {
        println!("  {s}");
    }
    Ok(())
}

/// Where the sealed catalog survives a reboot. `/dev/shm` is tmpfs, so without
/// a persisted copy the only way to refill it is to re-seal — which means
/// loading every plugin and probing live backends on the boot path.
const PERSISTED_CATALOG_DIR: &str = "/var/lib/opdbus/plugin-blobs";

/// Catalog manifest filename — written LAST, as the atomic commit point that
/// readers watch for. Mirrors `op_blob::catalog`'s own contract.
const MANIFEST_NAME: &str = ".manifest.json";

/// Boot path: stage the persisted catalog into SHM without regenerating it.
///
/// Sealing is a build-time act — it loads every plugin and calls
/// `schema_live()`, which probes real backends. Doing that at boot makes
/// startup depend on service ordering and costs minutes. Staging is a copy.
///
/// Falls back to a one-time seal when no persisted catalog exists yet (first
/// boot after install), then persists it so later boots only ever stage.
fn stage_shm() -> Result<(), String> {
    let shm = Path::new(DEFAULT_SHM_DIR);
    let persisted = Path::new(PERSISTED_CATALOG_DIR);

    if !persisted.join(MANIFEST_NAME).is_file() {
        eprintln!(
            "opblob: no persisted catalog at {} — sealing once, then persisting",
            persisted.display()
        );
        seal_plugins(shm)?;
        return persist_catalog(shm, persisted);
    }

    copy_catalog(persisted, shm)?;
    println!("staged    {} -> {}", persisted.display(), shm.display());
    Ok(())
}

/// Capture the current SHM catalog as the artifact future boots will stage.
/// Run this after a deliberate `seal-shm`, not on the boot path.
fn persist_catalog(shm: &Path, persisted: &Path) -> Result<(), String> {
    if !shm.join(MANIFEST_NAME).is_file() {
        return Err(format!(
            "no catalog to persist: {} has no {MANIFEST_NAME}",
            shm.display()
        ));
    }
    copy_catalog(shm, persisted)?;
    println!("persisted {} -> {}", shm.display(), persisted.display());
    Ok(())
}

/// Copy a sealed catalog, committing the manifest last so a reader never sees
/// blobs that the manifest does not yet describe (or vice versa).
fn copy_catalog(src: &Path, dst: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dst).map_err(|e| format!("create {}: {e}", dst.display()))?;

    // Drop blobs that are no longer in the source: a blob in the catalog IS
    // the plugin, so a stale leftover would resurrect a deregistered one.
    for entry in std::fs::read_dir(dst).map_err(|e| format!("read {}: {e}", dst.display()))? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.extension().and_then(|x| x.to_str()) == Some("blob") {
            std::fs::remove_file(&path).map_err(|e| format!("remove {}: {e}", path.display()))?;
        }
    }

    let mut staged = 0usize;
    for entry in std::fs::read_dir(src).map_err(|e| format!("read {}: {e}", src.display()))? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.extension().and_then(|x| x.to_str()) != Some("blob") {
            continue;
        }
        let name = path
            .file_name()
            .ok_or_else(|| format!("blob path has no name: {}", path.display()))?;
        std::fs::copy(&path, dst.join(name))
            .map_err(|e| format!("copy {}: {e}", path.display()))?;
        staged += 1;
    }

    // Manifest last — the commit point.
    std::fs::copy(src.join(MANIFEST_NAME), dst.join(MANIFEST_NAME))
        .map_err(|e| format!("copy {MANIFEST_NAME}: {e}"))?;

    eprintln!("opblob: {staged} blobs + manifest");
    Ok(())
}

fn seal_plugins(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let mut cat = ActiveReflectionCatalog::open(dir).map_err(|e| e.to_string())?;
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    let registry = DefaultPluginRegistry::new(Arc::new(MemoryStore::new()));
    let plugins = rt
        .block_on(registry.load_all_plugins())
        .map_err(|e| e.to_string())?;

    let mut sealed = 0usize;
    let mut sealed_ids = std::collections::HashSet::new();
    let mut missing_schema = Vec::new();
    for plugin in plugins {
        let plugin_id = plugin.name().to_string();
        // Prefer a live-probed schema (real backend reachability folded in)
        // over the static declaration; most plugins have nothing to probe
        // and schema_live() defaults to None, in which case schema() is used.
        let live = rt.block_on(plugin.schema_live());
        let schema = match live.or_else(|| plugin.schema()) {
            Some(schema) => schema,
            None => {
                missing_schema.push(plugin_id);
                continue;
            }
        };
        let blob = blobify_plugin_schema(&plugin_id, schema);
        sealed_ids.insert(blob.manifest.plugin_id.clone());
        let path = cat.upsert_blob(&blob).map_err(|e| e.to_string())?;
        println!(
            "sealed    {}  ({} methods, schema {})",
            path.display(),
            blob.manifest.methods.len(),
            &blob.manifest.schema_hash[..16]
        );
        sealed += 1;
    }

    // A blob in the catalog IS the plugin: sweep blobs whose plugin no
    // longer exists in the build (deregistration by disappearance).
    let mut swept = cat.retain_plugins(&sealed_ids).map_err(|e| e.to_string())?;
    swept.sort();
    for plugin_id in &swept {
        println!("swept     {plugin_id} (plugin no longer exists)");
    }

    println!("catalog   {}", dir.display());
    println!("hash      {}", cat.catalog_hash());
    println!("sealed    {sealed} plugin blobs");
    if !missing_schema.is_empty() {
        missing_schema.sort();
        println!("skipped   {} plugins without schema", missing_schema.len());
        for plugin_id in missing_schema {
            println!("  {plugin_id}");
        }
    }
    println!("services:");
    for s in cat.list_services() {
        println!("  {s}");
    }
    Ok(())
}

fn inspect(file: &Path) -> Result<(), String> {
    let bytes = std::fs::read(file).map_err(|e| e.to_string())?;
    let blob = BlobRef::new(&bytes)?;
    let m = blob.manifest()?;
    println!("plugin     {} v{}", m.plugin_id, m.schema_version);
    println!("schema     sha256 {}", m.schema_hash);
    println!(
        "dbus       {} @ {}",
        m.dbus.interface_name, m.dbus.object_path
    );
    if let Some(id) = &m.identity {
        println!(
            "identity   wg-pub {}  key-id {}",
            id.wg_public_key, id.key_id
        );
        println!(
            "session    {} ({:?})",
            id.session.session_id, id.session.lifespan
        );
    }
    for mm in &m.methods {
        println!(
            "method     {}  {}  cap={}  subid={}",
            mm.schema_name,
            mm.grpc_path,
            mm.required_capability.as_deref().unwrap_or("-"),
            mm.subid
        );
    }
    for f in descriptor::decode_file_summaries(blob.descriptor_set())? {
        println!(
            "descriptor {} pkg={} messages={:?} services={:?}",
            f.name, f.package, f.messages, f.services
        );
    }
    Ok(())
}

fn catalog(dir: &Path) -> Result<(), String> {
    let cat = ActiveReflectionCatalog::open(dir).map_err(|e| e.to_string())?;
    println!("active plugins: {:?}", cat.active_plugins());
    for s in cat.list_services() {
        println!("  {s}");
    }
    Ok(())
}

fn btrfs_seal(image: &Path, dir: &Path) -> Result<(), String> {
    let cat = ActiveReflectionCatalog::open(dir).map_err(|e| e.to_string())?;
    let paths: Vec<PathBuf> = cat.blob_paths();
    let report = btrfs::seal_blobs_into_btrfs_image(&paths, image).map_err(|e| e.to_string())?;
    println!(
        "sealed {} blobs into {} ({} bytes, subvolumes={})",
        report.blob_count,
        report.image_path.display(),
        report.image_bytes,
        report.subvolumes
    );
    println!("{}", report.mount_hint);
    Ok(())
}

fn keygen(keyfile: &Path) -> Result<(), String> {
    if keyfile.exists() {
        return Err(format!(
            "{} exists — the keypair is the account identity and its session is account-persistent; refusing to overwrite",
            keyfile.display()
        ));
    }
    let kp = WgKeypair::generate();
    kp.save(keyfile).map_err(|e| e.to_string())?;
    let id = kp.public_identity();
    println!("private key written to {}", keyfile.display());
    println!("public   {}", id.wg_public_key);
    println!("key-id   {}", id.key_id);
    println!("session  {} (account-persistent)", id.session.session_id);
    Ok(())
}
