//! Point IntrospectiveGadget::inspect_legacy_data at a REAL sealed blob file
//! (`/dev/shm/opdbus/plugin-blobs/qdrant.<hash>.blob`, the custom OPBLOB01
//! binary format used by this repo's blob catalog), not synthetic/JSON data.
//!
//! NOTE: `analyze_binary_patterns` (introspective_gadget.rs) is an unbounded
//! O(n^2)-ish scan with no cutoff -- confirmed by reading it, not guessed.
//! Deliberately truncating the input to a small prefix here to avoid a
//! multi-minute (or longer) run; the full 36KB blob would need that loop
//! fixed first before it's safe to run un-truncated. That's a real,
//! separate finding, not addressed by this demo.

use op_inspector::{IntrospectiveGadget, KnowledgeBase};
use std::sync::Arc;
use tokio::sync::RwLock;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let full = std::fs::read(
        "/tmp/claude-1000/-home-jeremy-git-operation-dbus-proto/88ffa2c0-8dc5-4914-bd79-4e1098ff688d/scratchpad/qdrant.blob",
    )?;
    println!("full blob size: {} bytes", full.len());

    // analyze_binary_patterns is now O(n) (fixed) -- run on the FULL,
    // untruncated blob, no more truncation needed.
    let kb = Arc::new(RwLock::new(KnowledgeBase::default()));
    let gadget = IntrospectiveGadget::new(kb).await?;

    let start = std::time::Instant::now();
    let inspection = gadget
        .inspect_legacy_data(&full, "qdrant sealed blob (OPBLOB01, FULL 36878 bytes)")
        .await?;
    println!("inspect_legacy_data on full blob took: {:.2?}", start.elapsed());

    println!("file_size (of truncated sample): {}", inspection.file_size);
    println!(
        "file_header (first 16 bytes): {:?}",
        inspection.file_header.as_ref().map(|h| String::from_utf8_lossy(h).to_string())
    );
    println!("file_header (raw bytes): {:?}", inspection.file_header);
    println!("entropy: {:.4}", inspection.entropy);
    println!("strings_found ({} total), first 20:", inspection.strings_found.len());
    for s in inspection.strings_found.iter().take(20) {
        println!("  {s:?}");
    }
    println!("patterns found: {}", inspection.patterns.len());
    for p in &inspection.patterns {
        println!(
            "  offset={} count={} pattern={:?}",
            p.offset,
            p.count,
            String::from_utf8_lossy(&p.pattern)
        );
    }
    println!("schema_generated.schema_type: {}", inspection.schema_generated.schema_type);
    println!("schema_generated.object_patterns: {:?}", inspection.schema_generated.object_patterns);

    Ok(())
}
