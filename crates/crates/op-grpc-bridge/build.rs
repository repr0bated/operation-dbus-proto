use std::env;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    // Compile main operation.proto
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .file_descriptor_set_path(out_dir.join("operation_descriptor.bin"))
        .compile_protos(&["proto/operation.proto"], &["proto"])?;

    // Generate and compile plugin proto
    generate_plugin_proto()?;

    println!("cargo:rerun-if-changed=proto/operation.proto");
    println!("cargo:rerun-if-changed=build.rs");

    Ok(())
}

fn generate_plugin_proto() -> Result<(), Box<dyn std::error::Error>> {
    use std::fs;

    // Create a simple generated proto for demonstration
    // In production, this would load schemas from op-state-store
    let proto_content = r#"syntax = "proto3";

package operation.v1.plugins;

import "google/protobuf/struct.proto";
import "google/protobuf/timestamp.proto";

// Generated plugin messages
// This would be auto-generated from plugin schemas

message NetState {
  repeated NetworkInterface interfaces = 1;
}

message NetworkInterface {
  string name = 1;
  string type = 2;
  optional string ipv4 = 3;
  optional string ipv6 = 4;
  optional string mac_address = 5;
  bool enabled = 6;
}

message GetNetStateRequest {
  string plugin_id = 1;
}

message GetNetStateResponse {
  NetState state = 1;
}

message UpdateNetStateRequest {
  string plugin_id = 1;
  NetState state = 2;
}

message UpdateNetStateResponse {
  bool success = 1;
}

enum ChangeType {
  CHANGE_TYPE_ADDED = 0;
  CHANGE_TYPE_UPDATED = 1;
  CHANGE_TYPE_REMOVED = 2;
}

// Service for Net plugin
service NetService {
  rpc GetState(GetNetStateRequest) returns (GetNetStateResponse);
  rpc UpdateState(UpdateNetStateRequest) returns (UpdateNetStateResponse);
  rpc SubscribeChanges(SubscribeNetChangesRequest) returns (stream NetStateChange);
}

message SubscribeNetChangesRequest {
  string plugin_id = 1;
}

message NetStateChange {
  string change_id = 1;
  NetworkInterface changed_interface = 2;
  ChangeType change_type = 3;
  google.protobuf.Timestamp timestamp = 4;
}
"#;

    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    let proto_path = out_dir.join("generated_plugins.proto");
    fs::write(&proto_path, proto_content)?;

    // Compile the generated proto
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile(&[proto_path], &[out_dir])?;

    Ok(())
}
