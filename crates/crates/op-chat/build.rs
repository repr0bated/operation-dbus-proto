//! Build script for op-chat
//!
//! Compiles protocol buffer files for gRPC services.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Check if proto file exists
    let proto_path = "proto/orchestration.proto";
    if !std::path::Path::new(proto_path).exists() {
        // Proto file not present, skip compilation
        // This allows building without protoc installed
        println!(
            "cargo:warning=Proto file not found at {}, skipping gRPC codegen",
            proto_path
        );
        return Ok(());
    }

    // Compile proto files
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .out_dir("src/orchestration/proto")
        .compile(&[proto_path], &["proto"])?;

    // Tell cargo to rerun if proto changes
    println!("cargo:rerun-if-changed={}", proto_path);

    Ok(())
}
