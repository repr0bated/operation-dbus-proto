use std::env;
use std::path::Path;

fn main() {
    // Trigger rebuild when embedded assets or build inputs change.
    println!("cargo:rerun-if-changed=../../lovable/dist");
    println!("cargo:rerun-if-changed=../../lovable/package.json");
    println!("cargo:rerun-if-changed=../../lovable/src");
    println!("cargo:rerun-if-changed=../../lovable/index.html");

    let has_index = Path::new("../../lovable/dist/index.html").exists();
    if !has_index {
        let profile = env::var("PROFILE").unwrap_or_else(|_| "dev".to_string());
        if profile == "release" {
            panic!(
                "Missing lovable/dist/index.html for release build. Run: cd lovable && npm ci && npm run build"
            );
        }
        println!(
            "cargo:warning=Embedded UI assets missing (lovable/dist/index.html). Run: cd lovable && npm ci && npm run build"
        );
    }
}
