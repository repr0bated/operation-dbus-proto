//! Point IntrospectiveGadget::inspect_xml_data at the REAL repomix-generated
//! XML dump of this repo (16MB, ~thousands of embedded source files).
//!
//! Relevant to a RAG use case: repomix wraps each source file in a
//! `<file path="...">...</file>` element -- if `analyze_xml_elements`
//! correctly identifies `file` elements with `path` attributes, that's a
//! ready-made chunking boundary for indexing. But note this parser is
//! REGEX-based, not a real XML parser (see extract_xml_root /
//! analyze_xml_elements in introspective_gadget.rs) -- it will also match
//! anything inside the embedded SOURCE CODE that merely looks like `<word>`
//! (Rust generics like `Vec<String>`, comparison operators, etc). This is a
//! real accuracy stress test, not just a performance one.

use op_inspector::{IntrospectiveGadget, KnowledgeBase};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let path = "/home/jeremy/git/operation-dbus-proto/repomix-output.xml";
    let xml = std::fs::read_to_string(path)?;
    println!("repomix-output.xml size: {} bytes ({:.1} MB)", xml.len(), xml.len() as f64 / 1_000_000.0);

    let kb = Arc::new(RwLock::new(KnowledgeBase::default()));
    let gadget = IntrospectiveGadget::new(kb).await?;

    let start = Instant::now();
    let inspection = gadget.inspect_xml_data(&xml, "repomix-output.xml (full repo dump)").await?;
    let elapsed = start.elapsed();

    println!("elapsed: {:.2?}", elapsed);
    println!("root_element: {:?}", inspection.root_element);
    println!("namespaces found: {}", inspection.namespaces.len());
    println!("total elements matched: {}", inspection.elements.len());

    // How many of the matched "elements" are genuine repomix structural tags
    // (file_summary, directory_structure, file, files) vs. false positives
    // from embedded source code (Rust generics, comparisons, etc.)?
    let known_repomix_tags = [
        "file_summary", "/file_summary", "purpose", "/purpose",
        "file_format", "/file_format", "usage_guidelines", "/usage_guidelines",
        "notes", "/notes", "directory_structure", "/directory_structure",
        "files", "/files", "file", "/file",
    ];
    let mut real_count = 0usize;
    let mut file_elements_with_path = 0usize;
    let mut false_positive_examples: Vec<String> = Vec::new();

    for el in &inspection.elements {
        if known_repomix_tags.contains(&el.name.as_str()) {
            real_count += 1;
            if el.name == "file" && el.attributes.contains_key("path") {
                file_elements_with_path += 1;
            }
        } else if false_positive_examples.len() < 10 {
            false_positive_examples.push(el.name.clone());
        }
    }

    println!("\ngenuine repomix structural tags matched: {real_count}");
    println!("`file` elements with a `path` attribute (usable RAG chunk boundaries): {file_elements_with_path}");
    println!("false-positive matches (first 10, from embedded source code): {false_positive_examples:?}");
    println!("false-positive rate: {:.1}% of all {} matches",
        100.0 * (inspection.elements.len() - real_count) as f64 / inspection.elements.len() as f64,
        inspection.elements.len());

    Ok(())
}
