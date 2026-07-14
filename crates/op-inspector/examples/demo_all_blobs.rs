//! Run inspect_legacy_data across the ENTIRE live sealed-blob catalog (66
//! real .blob files copied from /dev/shm/opdbus/plugin-blobs), not just one.
//! Each file is truncated to a bounded prefix (see NOTE in demo_real_blob.rs
//! -- analyze_binary_patterns is an unbounded O(n^2)-ish scan with no
//! cutoff, confirmed by reading it) so this stays fast across all 66 files.

use op_inspector::{IntrospectiveGadget, KnowledgeBase};
use std::sync::Arc;
use tokio::sync::RwLock;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let dir = "/tmp/claude-1000/-home-jeremy-git-operation-dbus-proto/88ffa2c0-8dc5-4914-bd79-4e1098ff688d/scratchpad/all-blobs";
    let mut paths: Vec<_> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|e| e == "blob").unwrap_or(false))
        .collect();
    paths.sort();

    println!("blob files found: {}", paths.len());

    let kb = Arc::new(RwLock::new(KnowledgeBase::default()));
    let gadget = IntrospectiveGadget::new(kb).await?;

    let mut ok = 0usize;
    let mut failed = 0usize;
    let mut magic_matches = 0usize;
    let mut total_entropy = 0.0f64;
    let mut total_strings = 0usize;
    let mut total_patterns = 0usize;
    let mut max_pattern: Option<(String, usize, String)> = None; // (file, count, pattern)

    let start = std::time::Instant::now();

    for path in &paths {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let full = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("  {name}: read failed: {e}");
                failed += 1;
                continue;
            }
        };
        // analyze_binary_patterns is now O(n) (fixed) -- run on the FULL,
        // untruncated file so cross-connections across the whole blob are
        // visible, not just within a truncated prefix.
        match gadget.inspect_legacy_data(&full, &name).await {
            Ok(inspection) => {
                ok += 1;
                let is_magic = inspection
                    .file_header
                    .as_deref()
                    .map(|h| h.starts_with(b"OPBLOB01"))
                    .unwrap_or(false);
                if is_magic {
                    magic_matches += 1;
                }
                total_entropy += inspection.entropy;
                total_strings += inspection.strings_found.len();
                total_patterns += inspection.patterns.len();

                if let Some(top) = inspection.patterns.first() {
                    let better = max_pattern
                        .as_ref()
                        .map(|(_, c, _)| top.count > *c)
                        .unwrap_or(true);
                    if better {
                        max_pattern = Some((
                            name.clone(),
                            top.count,
                            String::from_utf8_lossy(&top.pattern).to_string(),
                        ));
                    }
                }
            }
            Err(e) => {
                eprintln!("  {name}: inspect_legacy_data failed: {e}");
                failed += 1;
            }
        }
    }

    let elapsed = start.elapsed();

    println!("\n--- summary across {} real blobs ---", paths.len());
    println!("ok: {ok}  failed: {failed}");
    println!(
        "OPBLOB01 magic correctly detected: {magic_matches}/{ok} (should be {ok}/{ok} -- every real sealed blob starts with this magic)"
    );
    println!("avg entropy: {:.4}", total_entropy / ok.max(1) as f64);
    println!("total strings extracted: {total_strings}");
    println!("total patterns found: {total_patterns}");
    if let Some((file, count, pattern)) = &max_pattern {
        println!("highest-count pattern overall: {pattern:?} x{count} in {file}");
    }
    println!("elapsed: {:.2?} for {} files ({:.1} ms/file avg)", elapsed, paths.len(), elapsed.as_millis() as f64 / paths.len().max(1) as f64);

    Ok(())
}
