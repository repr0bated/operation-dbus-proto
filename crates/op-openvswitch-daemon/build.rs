// build.rs - Compile protobuf definitions

use std::io::Result;

fn main() -> Result<()> {
    println!("cargo:rerun-if-changed=proto/ovsdaemon/v1/ovsdaemon.proto");

    #[allow(deprecated)]
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile(&["proto/ovsdaemon/v1/ovsdaemon.proto"], &["proto"])?;

    Ok(())
}
