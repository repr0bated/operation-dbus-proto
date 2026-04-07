use std::env;
use std::path::Path;

fn main() {
    // Trigger rebuild when embedded assets or build inputs change.
    println!("cargo:rerun-if-changed=ui/dist");
    println!("cargo:rerun-if-changed=ui/package.json");
    println!("cargo:rerun-if-changed=ui/src");
    println!("cargo:rerun-if-changed=ui/index.html");

    let has_index = Path::new("ui/dist/index.html").exists();
    if !has_index {
        let profile = env::var("PROFILE").unwrap_or_else(|_| "dev".to_string());
        if profile == "release" {
            panic!(
                "Missing ui/dist/index.html for release build. Run: cd crates/op-web/ui && npm ci && npm run build:prod"
            );
        }
        println!(
            "cargo:warning=Embedded UI assets missing (ui/dist/index.html). Run: cd crates/op-web/ui && npm ci && npm run build"
        );
    }
}
