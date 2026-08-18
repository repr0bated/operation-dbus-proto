//! Build script — compiles proto files for the gRPC client.
//!
//! Two sources feed codegen:
//!   1. `proto/*.proto` — protos owned by this crate (none today).
//!   2. `../op-grpc-bridge/proto/operation.proto` — the canonical control-plane
//!      proto, compiled here for its *client* stubs only. It is referenced in
//!      place rather than copied so the GUI can never drift from the server's
//!      definition. `EventChainServiceClient` (used by the Accountability view)
//!      comes from this file.
//!
//! Generated code lands in `src/proto/` and is pulled in with `include!`, the
//! same arrangement the chat transport already uses.

use std::path::Path;

/// The canonical control-plane proto, owned by `op-grpc-bridge`.
const OPERATION_PROTO: &str = "../op-grpc-bridge/proto/operation.proto";
const OPERATION_PROTO_DIR: &str = "../op-grpc-bridge/proto";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_dir = "proto";
    let out_dir = "src/proto";

    std::fs::create_dir_all(out_dir)?;

    let mut proto_files = vec![];
    let mut include_dirs = vec![];

    if Path::new(proto_dir).is_dir() {
        for entry in std::fs::read_dir(proto_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("proto") {
                proto_files.push(path.to_string_lossy().into_owned());
            }
        }
        include_dirs.push(proto_dir.to_string());
    }

    // The Accountability view needs EventChainService's client stubs. Skip
    // silently if the sibling crate is absent (e.g. a crate-only checkout)
    // rather than failing the build.
    if Path::new(OPERATION_PROTO).is_file() {
        proto_files.push(OPERATION_PROTO.to_string());
        include_dirs.push(OPERATION_PROTO_DIR.to_string());
    } else {
        eprintln!(
            "cargo:warning={OPERATION_PROTO} not found; EventChainService client not generated"
        );
    }

    if proto_files.is_empty() {
        eprintln!("cargo:warning=no .proto files found, skipping codegen");
        return Ok(());
    }

    tonic_build::configure()
        .build_server(false)
        .build_client(true)
        .out_dir(out_dir)
        .compile_protos(&proto_files, &include_dirs)?;

    for f in &proto_files {
        println!("cargo:rerun-if-changed={}", f);
    }
    println!("cargo:rerun-if-changed=build.rs");
    Ok(())
}
