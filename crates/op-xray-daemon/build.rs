//! Build script for op-xray-daemon.
//!
//! Compiles the vendored xray-core commander protos (StatsService,
//! RoutingService, LoggerService — see proto/VENDORED.md for provenance).

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_files = [
        "proto/app/stats/command/command.proto",
        "proto/app/router/command/command.proto",
        "proto/app/log/command/config.proto",
    ];

    tonic_build::configure()
        .build_server(false)
        .build_client(true)
        .compile_protos(&proto_files, &["proto"])?;

    for proto_path in &proto_files {
        println!("cargo:rerun-if-changed={}", proto_path);
    }

    Ok(())
}
