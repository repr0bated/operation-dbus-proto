//! Active blob-backed gRPC reflection.
//!
//! `tonic_reflection::server::Builder` freezes its descriptor index at build
//! time. This module keeps the descriptor files indexed, but advertises service
//! names from the active plugin object blobs so discovery follows D-Bus object
//! lifetime.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use prost::Message;
use prost_types::{
    DescriptorProto, EnumDescriptorProto, FieldDescriptorProto, FileDescriptorProto,
    FileDescriptorSet,
};
use tokio::sync::{mpsc, RwLock};
use tokio_stream::{wrappers::ReceiverStream, StreamExt};
use tonic::{Request, Response, Status, Streaming};
use tonic_reflection::pb::v1::server_reflection_request::MessageRequest;
use tonic_reflection::pb::v1::server_reflection_response::MessageResponse;
use tonic_reflection::pb::v1::server_reflection_server::{
    ServerReflection, ServerReflectionServer,
};
use tonic_reflection::pb::v1::{
    ExtensionNumberResponse, FileDescriptorResponse, ListServiceResponse, ServerReflectionRequest,
    ServerReflectionResponse, ServiceResponse,
};

use crate::plugin_object_blob::PluginObjectBlob;

#[derive(Debug, Clone, Default)]
pub struct ActiveReflectionCatalog {
    inner: Arc<RwLock<ActiveReflectionInner>>,
}

#[derive(Debug, Clone)]
struct ActiveReflectionInner {
    descriptor_sets: Vec<Vec<u8>>,
    blobs: BTreeMap<String, PluginObjectBlob>,
    index: ReflectionIndex,
}

impl Default for ActiveReflectionInner {
    fn default() -> Self {
        let mut inner = Self {
            descriptor_sets: Vec::new(),
            blobs: BTreeMap::new(),
            index: ReflectionIndex::default(),
        };
        inner
            .descriptor_sets
            .push(crate::proto::FILE_DESCRIPTOR_SET.to_vec());
        inner
            .descriptor_sets
            .push(tonic_reflection::pb::v1::FILE_DESCRIPTOR_SET.to_vec());
        inner.rebuild_index();
        inner
    }
}

impl ActiveReflectionCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn upsert_blob(&self, blob: PluginObjectBlob) {
        let mut inner = self.inner.write().await;
        inner.blobs.insert(blob.manifest.plugin_id.clone(), blob);
        inner.rebuild_index();
    }

    pub async fn remove_blob(&self, plugin_id: &str) {
        let mut inner = self.inner.write().await;
        inner.blobs.remove(plugin_id);
        inner.rebuild_index();
    }

    pub async fn list_services(&self) -> Vec<String> {
        self.inner.read().await.index.service_names.clone()
    }

    async fn file_by_filename(&self, filename: &str) -> Result<Vec<u8>, Status> {
        self.inner.read().await.index.file_by_filename(filename)
    }

    async fn symbol_by_name(&self, symbol: &str) -> Result<Vec<u8>, Status> {
        self.inner.read().await.index.symbol_by_name(symbol)
    }
}

impl ActiveReflectionInner {
    fn rebuild_index(&mut self) {
        let active_services = self
            .blobs
            .values()
            .flat_map(|blob| {
                // Advertise legacy service names (mounted via build.rs) for
                // ListServices; per-method typed services are indexed for
                // file/symbol lookups but not listed unless routes are mounted.
                // TODO(unify-routes): advertise operation.method.* services
                // when PerMethodGrpcServices routes are mounted.
                blob.manifest.grpc.services.clone()
            })
            .collect::<BTreeSet<_>>();
        let mut index = ReflectionIndex::new(active_services);

        for encoded in self
            .descriptor_sets
            .iter()
            .chain(self.blobs.values().map(|blob| &blob.descriptor_set))
        {
            let Ok(file_set) = FileDescriptorSet::decode(encoded.as_slice()) else {
                continue;
            };
            index.add_file_descriptor_set(file_set);
        }

        self.index = index;
    }
}

#[derive(Debug, Clone, Default)]
struct ReflectionIndex {
    active_services: BTreeSet<String>,
    service_names: Vec<String>,
    files: HashMap<String, Arc<FileDescriptorProto>>,
    symbols: HashMap<String, Arc<FileDescriptorProto>>,
}

impl ReflectionIndex {
    fn new(active_services: BTreeSet<String>) -> Self {
        Self {
            active_services,
            service_names: Vec::new(),
            files: HashMap::new(),
            symbols: HashMap::new(),
        }
    }

    fn add_file_descriptor_set(&mut self, file_set: FileDescriptorSet) {
        for file in file_set.file {
            let Some(name) = file.name.clone() else {
                continue;
            };
            if self.files.contains_key(&name) {
                continue;
            }

            let file = Arc::new(file);
            self.files.insert(name, file.clone());
            self.process_file(file);
        }

        self.service_names.sort();
        self.service_names.dedup();
    }

    fn process_file(&mut self, file: Arc<FileDescriptorProto>) {
        let prefix = file.package.clone().unwrap_or_default();

        for message in &file.message_type {
            self.process_message(file.clone(), &prefix, message);
        }
        for enum_type in &file.enum_type {
            self.process_enum(file.clone(), &prefix, enum_type);
        }
        for service in &file.service {
            let Some(service_name) = extract_name(&prefix, service.name.as_ref()) else {
                continue;
            };
            if self.active_services.contains(&service_name)
                || service_name == "grpc.reflection.v1.ServerReflection"
            {
                self.service_names.push(service_name.clone());
            }
            self.symbols.insert(service_name.clone(), file.clone());

            for method in &service.method {
                if let Some(method_name) = extract_name(&service_name, method.name.as_ref()) {
                    self.symbols.insert(method_name, file.clone());
                }
            }
        }
    }

    fn process_message(
        &mut self,
        file: Arc<FileDescriptorProto>,
        prefix: &str,
        message: &DescriptorProto,
    ) {
        let Some(message_name) = extract_name(prefix, message.name.as_ref()) else {
            return;
        };
        self.symbols.insert(message_name.clone(), file.clone());

        for nested in &message.nested_type {
            self.process_message(file.clone(), &message_name, nested);
        }
        for enum_type in &message.enum_type {
            self.process_enum(file.clone(), &message_name, enum_type);
        }
        for field in &message.field {
            self.process_field(file.clone(), &message_name, field);
        }
        for oneof in &message.oneof_decl {
            if let Some(oneof_name) = extract_name(&message_name, oneof.name.as_ref()) {
                self.symbols.insert(oneof_name, file.clone());
            }
        }
    }

    fn process_enum(
        &mut self,
        file: Arc<FileDescriptorProto>,
        prefix: &str,
        enum_type: &EnumDescriptorProto,
    ) {
        let Some(enum_name) = extract_name(prefix, enum_type.name.as_ref()) else {
            return;
        };
        self.symbols.insert(enum_name.clone(), file.clone());

        for value in &enum_type.value {
            if let Some(value_name) = extract_name(&enum_name, value.name.as_ref()) {
                self.symbols.insert(value_name, file.clone());
            }
        }
    }

    fn process_field(
        &mut self,
        file: Arc<FileDescriptorProto>,
        prefix: &str,
        field: &FieldDescriptorProto,
    ) {
        if let Some(field_name) = extract_name(prefix, field.name.as_ref()) {
            self.symbols.insert(field_name, file);
        }
    }

    fn symbol_by_name(&self, symbol: &str) -> Result<Vec<u8>, Status> {
        self.encode_file(symbol, self.symbols.get(symbol), "symbol")
    }

    fn file_by_filename(&self, filename: &str) -> Result<Vec<u8>, Status> {
        self.encode_file(filename, self.files.get(filename), "file")
    }

    fn encode_file(
        &self,
        name: &str,
        file: Option<&Arc<FileDescriptorProto>>,
        kind: &str,
    ) -> Result<Vec<u8>, Status> {
        let Some(file) = file else {
            return Err(Status::not_found(format!("{kind} '{name}' not found")));
        };
        let mut encoded = Vec::new();
        file.encode(&mut encoded)
            .map_err(|_| Status::internal("failed to encode descriptor"))?;
        Ok(encoded)
    }
}

fn extract_name(prefix: &str, name: Option<&String>) -> Option<String> {
    let name = name?;
    if prefix.is_empty() {
        Some(name.to_string())
    } else {
        Some(format!("{prefix}.{name}"))
    }
}

#[derive(Debug, Clone)]
pub struct DynamicReflectionService {
    catalog: ActiveReflectionCatalog,
}

impl DynamicReflectionService {
    pub fn new(catalog: ActiveReflectionCatalog) -> Self {
        Self { catalog }
    }

    pub fn into_v1_server(self) -> ServerReflectionServer<Self> {
        ServerReflectionServer::new(self)
    }
}

#[tonic::async_trait]
impl ServerReflection for DynamicReflectionService {
    type ServerReflectionInfoStream = ReceiverStream<Result<ServerReflectionResponse, Status>>;

    async fn server_reflection_info(
        &self,
        request: Request<Streaming<ServerReflectionRequest>>,
    ) -> Result<Response<Self::ServerReflectionInfoStream>, Status> {
        let mut request_rx = request.into_inner();
        let (response_tx, response_rx) =
            mpsc::channel::<Result<ServerReflectionResponse, Status>>(1);
        let catalog = self.catalog.clone();

        tokio::spawn(async move {
            while let Some(request) = request_rx.next().await {
                let Ok(request) = request else {
                    return;
                };

                let response = match request.message_request.clone() {
                    None => Err(Status::invalid_argument("invalid MessageRequest")),
                    Some(MessageRequest::FileByFilename(filename)) => catalog
                        .file_by_filename(&filename)
                        .await
                        .map(file_descriptor_response),
                    Some(MessageRequest::FileContainingSymbol(symbol)) => catalog
                        .symbol_by_name(&symbol)
                        .await
                        .map(file_descriptor_response),
                    Some(MessageRequest::FileContainingExtension(_)) => {
                        Err(Status::not_found("extensions are not supported"))
                    }
                    Some(MessageRequest::AllExtensionNumbersOfType(_)) => {
                        Ok(MessageResponse::AllExtensionNumbersResponse(
                            ExtensionNumberResponse::default(),
                        ))
                    }
                    Some(MessageRequest::ListServices(_)) => {
                        let services = catalog
                            .list_services()
                            .await
                            .into_iter()
                            .map(|name| ServiceResponse { name })
                            .collect();
                        Ok(MessageResponse::ListServicesResponse(ListServiceResponse {
                            service: services,
                        }))
                    }
                };

                let send_result = match response {
                    Ok(message_response) => {
                        response_tx
                            .send(Ok(ServerReflectionResponse {
                                valid_host: request.host.clone(),
                                original_request: Some(request),
                                message_response: Some(message_response),
                            }))
                            .await
                    }
                    Err(status) => response_tx.send(Err(status)).await,
                };

                if send_result.is_err() {
                    return;
                }
            }
        });

        Ok(Response::new(ReceiverStream::new(response_rx)))
    }
}

fn file_descriptor_response(file: Vec<u8>) -> MessageResponse {
    MessageResponse::FileDescriptorResponse(FileDescriptorResponse {
        file_descriptor_proto: vec![file],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn active_reflection_advertises_only_active_blob_services() {
        let catalog = ActiveReflectionCatalog::new();
        let before = catalog.list_services().await;
        assert!(before
            .iter()
            .any(|service| service == "grpc.reflection.v1.ServerReflection"));
        assert!(!before
            .iter()
            .any(|service| service == "operation.plugin.v1.ZeroclawPluginMethods"));

        catalog
            .upsert_blob(crate::zeroclaw_object_blob::from_plugin_schema())
            .await;
        let after = catalog.list_services().await;
        // Legacy service (mounted via build.rs) is advertised.
        assert!(after
            .iter()
            .any(|service| service == "operation.plugin.v1.ZeroclawPluginMethods"));

        catalog.remove_blob("zeroclaw").await;
        let removed = catalog.list_services().await;
        assert!(!removed
            .iter()
            .any(|service| service == "operation.plugin.v1.ZeroclawPluginMethods"));
    }
}
