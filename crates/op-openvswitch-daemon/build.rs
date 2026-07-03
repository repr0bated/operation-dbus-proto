//! Build script to compile protobuf definitions for gRPC streaming

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Compile all proto files
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    tonic_build::configure()
        .build_server(true)
        .build_client(false)
        .file_descriptor_set_path(out.join("ovsdaemon_descriptor.bin"))
        .compile_protos(&["proto/ovsdaemon/v1/ovsdaemon.proto", "proto/ovsdaemon/v1/streaming.proto"], &["proto"])?;

    println!("cargo:rerun-if-changed=proto/ovsdaemon/v1/ovsdaemon.proto");
    println!("cargo:rerun-if-changed=proto/ovsdaemon/v1/streaming.proto");

    Ok(())
}
