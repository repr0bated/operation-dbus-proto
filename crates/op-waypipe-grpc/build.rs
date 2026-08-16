use std::env;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let proto = "proto/waypipe_tunnel.proto";

    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .file_descriptor_set_path(out_dir.join("waypipe_descriptor.bin"))
        .compile_protos(&[proto], &["proto"])?;

    println!("cargo:rerun-if-changed={proto}");
    println!("cargo:rerun-if-changed=build.rs");
    Ok(())
}
