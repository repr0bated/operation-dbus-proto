//! Validator behaviour at the seam between the two layers.
//!
//! Grammar (`SpecValidator::new`) and vocabulary (`SpecValidator::with_catalog`)
//! are separate on purpose, and the difference is load-bearing: a grammar-only
//! validator cannot reject a component name, so an admission path that forgets
//! the catalog admits specs the renderer will refuse. These tests pin both
//! halves, including that the grammar half stays silent about names.

use std::path::{Path, PathBuf};

use op_gallery_gen::{CatalogGuard, SpecValidator};

/// The exported catalog that ships in this repo.
fn catalog() -> CatalogGuard {
    let dir: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schemas/json-render");
    CatalogGuard::load(&dir).expect("exported catalog artifact must load")
}

/// A spec that satisfies both grammar and the real catalog.
fn sound_spec() -> serde_json::Value {
    serde_json::json!({
        "root": "root",
        "elements": {
            "root": {
                "type": "card",
                "props": {"title": "Netmaker", "tone": "ok"},
                "children": ["headline", "detail"]
            },
            "headline": {
                "type": "heading",
                "props": {"text": "Mesh", "level": 2}
            },
            "detail": {
                "type": "statCard",
                "props": {
                    "label": "Peers",
                    "value": {"$state": "/plugins/netmaker/peer_count"},
                    "sub": null,
                    "variant": "ok"
                }
            }
        }
    })
}

#[test]
fn catalog_admits_a_spec_that_uses_the_real_vocabulary() {
    let result = SpecValidator::with_catalog(catalog()).validate(&sound_spec());
    assert!(result.valid, "expected admission, got {:?}", result.errors);
    assert_eq!(result.signature.len(), 64);
}

#[test]
fn grammar_alone_does_not_judge_component_names() {
    let spec = serde_json::json!({
        "root": "root",
        "elements": {"root": {"type": "no_such_component", "children": []}}
    });

    let grammar = SpecValidator::new().validate(&spec);
    assert!(
        grammar.valid,
        "grammar has no vocabulary to judge names against: {:?}",
        grammar.errors
    );
    assert!(SpecValidator::new().catalog_hash().is_none());

    let admission = SpecValidator::with_catalog(catalog()).validate(&spec);
    assert!(!admission.valid, "the catalog must reject an unknown name");
    assert!(admission
        .errors
        .iter()
        .any(|e| e.code == "E_UNKNOWN_COMPONENT"));
}

#[test]
fn catalog_rejects_a_missing_required_prop() {
    let spec = serde_json::json!({
        "root": "root",
        "elements": {"root": {"type": "badge", "props": {"text": "up"}}}
    });

    let result = SpecValidator::with_catalog(catalog()).validate(&spec);
    assert!(!result.valid);
    assert!(
        result.errors.iter().any(|e| e.code == "E_PROP_REQUIRED"),
        "expected the absent `tone` to be caught: {:?}",
        result.errors
    );
}

#[test]
fn grammar_rejects_missing_root() {
    let spec = serde_json::json!({
        "elements": {"label1": {"type": "text", "props": {"text": "Hello"}}}
    });

    let result = SpecValidator::new().validate(&spec);
    assert!(!result.valid);
    assert!(result.errors.iter().any(|e| e.code == "E_MISSING_ROOT"));
}

#[test]
fn grammar_rejects_a_child_that_does_not_exist() {
    let spec = serde_json::json!({
        "root": "root",
        "elements": {
            "root": {"type": "card", "props": {"title": null, "tone": null}, "children": ["ghost"]}
        }
    });

    let result = SpecValidator::new().validate(&spec);
    assert!(!result.valid);
    assert!(result.errors.iter().any(|e| e.code == "E_DANGLING_REF"));
}

#[test]
fn signature_is_stable_across_key_order() {
    // Signature drives dedup, so it has to see two orderings of the same spec
    // as one spec.
    let a = serde_json::json!({
        "root": "a",
        "elements": {"a": {"type": "text", "props": {"text": "Test"}}}
    });
    let b = serde_json::json!({
        "elements": {"a": {"props": {"text": "Test"}, "type": "text"}},
        "root": "a"
    });

    let validator = SpecValidator::new();
    assert_eq!(
        validator.validate(&a).signature,
        validator.validate(&b).signature
    );
}

#[test]
fn the_catalog_artifact_matches_its_manifest() {
    // The digest is the only thing that makes a stale export detectable, so its
    // check is worth a test of its own rather than being incidental.
    let guard = catalog();
    let manifest: serde_json::Value = serde_json::from_slice(
        &std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../schemas/json-render/catalog.manifest.json"),
        )
        .expect("manifest must exist"),
    )
    .expect("manifest must be JSON");

    assert_eq!(
        guard.hash(),
        manifest["schemaSha256"].as_str().unwrap(),
        "guard must compile the exact bytes the manifest describes"
    );
    assert_eq!(
        guard.component_count(),
        manifest["componentCount"].as_u64().unwrap() as usize
    );
    assert_eq!(
        guard.json_render_version(),
        manifest["jsonRenderVersion"].as_str().unwrap()
    );
}
