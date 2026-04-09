use std::env;
use std::path::Path;

fn main() {
    let ui_dist = "ui/dist";

    println!("cargo:rerun-if-changed={}", ui_dist);
    if let Ok(entries) = std::fs::read_dir(ui_dist) {
        for entry in entries.flatten() {
            println!("cargo:rerun-if-changed={}", entry.path().display());
        }
    }
    if let Ok(entries) = std::fs::read_dir(format!("{}/assets", ui_dist)) {
        for entry in entries.flatten() {
            println!("cargo:rerun-if-changed={}", entry.path().display());
        }
    }
    println!("cargo:rerun-if-changed=ui/package.json");
    println!("cargo:rerun-if-changed=ui/src");
    println!("cargo:rerun-if-changed=ui/index.html");
    println!("cargo:rerun-if-changed=ui/public");

    let has_index = Path::new(&format!("{}/index.html", ui_dist)).exists();
    if !has_index {
        let profile = env::var("PROFILE").unwrap_or_else(|_| "dev".to_string());
        if profile == "release" {
            panic!(
                "Missing {} for release build. Run: cd crates/op-web/ui && npm ci && npm run build",
                format!("{}/index.html", ui_dist)
            );
        }
        println!(
                "cargo:warning=Embedded UI assets missing ({}). Run: cd crates/op-web/ui && npm ci && npm run build",
                format!("{}/index.html", ui_dist)
            );
    }
}
