fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Client-only: this service proxies to op-dbus's PluginService.CallMethod;
    // it never serves any of operation.proto's services itself.
    tonic_build::configure()
        .build_server(false)
        .build_client(true)
        .compile_protos(
            &["../op-grpc-bridge/proto/operation.proto"],
            &["../op-grpc-bridge/proto"],
        )?;
    println!("cargo:rerun-if-changed=../op-grpc-bridge/proto/operation.proto");
    Ok(())
}
