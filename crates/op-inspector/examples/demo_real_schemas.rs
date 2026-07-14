//! Run IntrospectiveGadget against every REAL method-args schema currently
//! live in this host's plugin catalog (dumped via `zcall help <plugin>
//! <method>` across all 141 plugins / 333 methods), instead of a synthetic
//! sample. Confirms the parser/type-inference is robust across the real
//! shape variety found in production (zero-arg schemas with no `properties`
//! key at all, flat `properties`+`required`, schemas with `$defs` for
//! nested types, schemas with/without `description`, etc.) rather than just
//! the two hand-crafted shapes in the earlier synthetic demo.

use op_inspector::{InspectionInput, InspectionSource, IntrospectiveGadget, KnowledgeBase};
use simd_json::prelude::*;
use std::sync::Arc;
use tokio::sync::RwLock;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let raw = std::fs::read_to_string(
        "/tmp/claude-1000/-home-jeremy-git-operation-dbus-proto/88ffa2c0-8dc5-4914-bd79-4e1098ff688d/scratchpad/all_args_schemas.json",
    )?;
    let mut raw_mut = raw.clone();
    let schemas: simd_json::OwnedValue = unsafe { simd_json::from_str(&mut raw_mut)? };
    let schemas = schemas.as_array().expect("top-level array");

    let kb = Arc::new(RwLock::new(KnowledgeBase::default()));
    let gadget = IntrospectiveGadget::new(Arc::clone(&kb)).await?;

    let mut ok = 0usize;
    let mut failed = 0usize;
    let mut skipped_empty = 0usize;
    let mut shape_counts: std::collections::HashMap<String, usize> = Default::default();

    for (i, schema) in schemas.iter().enumerate() {
        // Skip the 12 `{}` noise entries (artifacts of failed zcall calls in
        // the dump, not real method schemas -- a genuine zero-arg schema
        // always has at least $schema/title/type, never a bare {}).
        if schema.as_object().map(|o| o.is_empty()).unwrap_or(true) {
            skipped_empty += 1;
            continue;
        }

        let data = simd_json::to_string(schema)?;
        let input = InspectionInput {
            source: InspectionSource::RawData {
                format_hint: Some("json".to_string()),
                // Unique name per schema -- this is deliberately NOT a
                // merge test (333 different methods are 333 different
                // logical entities, not repeated samples of one shape;
                // merging across them would be semantically wrong). This
                // tests parser/type-inference robustness across real shape
                // diversity instead.
                description: format!("real_method_args_schema_{i}"),
            },
            data: Some(data),
            metadata: Default::default(),
        };

        match gadget.inspect_object(input).await {
            Ok(result) => {
                ok += 1;
                let shape_key = if result.schema.properties.is_empty() {
                    "no-properties (zero-arg)".to_string()
                } else if schema.get("$defs").is_some() {
                    "has-$defs (nested types)".to_string()
                } else {
                    "flat-properties".to_string()
                };
                *shape_counts.entry(shape_key).or_insert(0) += 1;
            }
            Err(e) => {
                failed += 1;
                eprintln!("FAILED on schema #{i}: {e}");
            }
        }
    }

    println!("total real schemas: {}", schemas.len());
    println!("skipped (noise/empty): {skipped_empty}");
    println!("parsed ok: {ok}");
    println!("failed: {failed}");
    println!("shape breakdown of successfully-parsed schemas:");
    for (k, v) in &shape_counts {
        println!("  {v:>4}  {k}");
    }

    Ok(())
}
