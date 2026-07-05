//! Build script — compiles proto files for the gRPC client.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_dir = "proto";
    let out_dir = "src/proto";

    std::fs::create_dir_all(out_dir)?;

    let mut proto_files = vec![];
    if std::path::Path::new(proto_dir).is_dir() {
        for entry in std::fs::read_dir(proto_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("proto") {
                proto_files.push(path.to_string_lossy().into_owned());
            }
        }
    }

    if proto_files.is_empty() {
        eprintln!("cargo:warning=no .proto files found in {proto_dir}, skipping codegen");
        return Ok(());
    }

    tonic_build::configure()
        .build_server(false)
        .build_client(true)
        .out_dir(out_dir)
        .compile(&proto_files, &[proto_dir])?;

    for f in &proto_files {
        println!("cargo:rerun-if-changed={}", f);
    }
    Ok(())
}
