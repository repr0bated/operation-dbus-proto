use std::env;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    // Compile all domain protos into a single combined FileDescriptorSet so
    // that tonic-reflection exposes every service in one query.
    //
    // Adding a new domain proto:
    //   1. Add the .proto file under proto/
    //   2. Add it to the compile_protos list below
    //   3. Add rerun-if-changed below
    //   4. Add the generated server/client to grpc_server.rs
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .file_descriptor_set_path(out_dir.join("operation_descriptor.bin"))
        .compile_protos(
            &[
                "proto/operation.proto",
                "proto/mail.proto",
                "proto/privacy_network.proto",
                "proto/registration.proto",
                "proto/registry.proto",
            ],
            &["proto"],
        )?;

    println!("cargo:rerun-if-changed=proto/operation.proto");
    println!("cargo:rerun-if-changed=proto/mail.proto");
    println!("cargo:rerun-if-changed=proto/privacy_network.proto");
    println!("cargo:rerun-if-changed=proto/registration.proto");
    println!("cargo:rerun-if-changed=proto/registry.proto");
    println!("cargo:rerun-if-changed=build.rs");

    Ok(())
}
