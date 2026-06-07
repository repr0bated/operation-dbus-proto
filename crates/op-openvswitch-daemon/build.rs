//! Build script to compile protobuf definitions for gRPC streaming

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Compile all proto files
    tonic_build::configure()
        .build_server(true)
        .build_client(false)
        .compile_protos(
            &[
                "proto/ovsdaemon/v1/ovsdb.proto",
                "proto/ovsdaemon/v1/streaming.proto",
            ],
            &["proto"],
        )?;

    println!("cargo:rerun-if-changed=proto/ovsdaemon/v1/ovsdb.proto");
    println!("cargo:rerun-if-changed=proto/ovsdaemon/v1/streaming.proto");

    Ok(())
}
