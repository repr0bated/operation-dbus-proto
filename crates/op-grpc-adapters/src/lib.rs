pub mod adapters;
pub mod interceptor;
pub mod server;

pub mod proto {
    tonic::include_proto!("op.adapters.v1");

    pub const FILE_DESCRIPTOR_SET: &[u8] =
        tonic::include_file_descriptor_set!("adapters_descriptor");
}
