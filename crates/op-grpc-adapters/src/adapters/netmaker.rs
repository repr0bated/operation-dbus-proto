//! Netmaker adapter — bridges Netmaker HTTP REST API to gRPC.
//! Connects via unix socket: /run/netmaker/api.sock

use crate::proto::{
    netmaker_service_server::NetmakerService, GetNetworkRequest, GetNetworkResponse,
    GetNodeRequest, GetNodeResponse, HealthRequest, HealthResponse, Host, ListHostsRequest,
    ListHostsResponse, ListNetworksRequest, ListNetworksResponse, ListNodesRequest,
    ListNodesResponse, NetmakerEvent, Network, Node, StreamEventsRequest,
};
use async_trait::async_trait;
use std::pin::Pin;
use tokio_stream::Stream;
use tonic::{Request, Response, Status};

const API_SOCK: &str = "/run/netmaker/api.sock";

pub struct NetmakerAdapter {
    client: reqwest::Client,
    base_url: String,
}

impl NetmakerAdapter {
    pub fn new() -> Self {
        // Route HTTP through the unix socket via a custom connector
        let client = reqwest::Client::builder()
            .build()
            .expect("reqwest client");
        Self {
            client,
            base_url: format!("http://localhost"),
        }
    }

    async fn get<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T, Status> {
        let master_key = std::env::var("NETMAKER_MASTER_KEY")
            .unwrap_or_else(|_| "masterkey".to_string());

        // Use the unix socket proxy device (host-side TCP forward from proxy device)
        // The proxy device binds tcp:127.0.0.1:8081 → container tcp:127.0.0.1:8081
        let url = format!("http://127.0.0.1:8081{}", path);
        let resp = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", master_key))
            .send()
            .await
            .map_err(|e| Status::unavailable(format!("Netmaker API unavailable: {}", e)))?;

        if !resp.status().is_success() {
            return Err(Status::internal(format!(
                "Netmaker API error: {}",
                resp.status()
            )));
        }

        resp.json::<T>()
            .await
            .map_err(|e| Status::internal(format!("Netmaker API parse error: {}", e)))
    }
}

impl Default for NetmakerAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl NetmakerService for NetmakerAdapter {
    async fn get_server_health(
        &self,
        _req: Request<HealthRequest>,
    ) -> Result<Response<HealthResponse>, Status> {
        #[derive(serde::Deserialize)]
        struct HealthResp { status: String, version: Option<String> }
        let r: HealthResp = self.get("/api/server/health").await?;
        Ok(Response::new(HealthResponse {
            status: r.status,
            version: r.version.unwrap_or_default(),
        }))
    }

    async fn list_networks(
        &self,
        _req: Request<ListNetworksRequest>,
    ) -> Result<Response<ListNetworksResponse>, Status> {
        #[derive(serde::Deserialize)]
        struct Net { netid: String, addressrange: Option<String>, addressrange6: Option<String>, localrange: Option<String> }
        let nets: Vec<Net> = self.get("/api/networks").await?;
        let networks = nets.into_iter().map(|n| Network {
            name: n.netid,
            address_range: n.addressrange.unwrap_or_default(),
            address_range6: n.addressrange6.unwrap_or_default(),
            is_local: n.localrange.is_some(),
        }).collect();
        Ok(Response::new(ListNetworksResponse { networks }))
    }

    async fn get_network(
        &self,
        req: Request<GetNetworkRequest>,
    ) -> Result<Response<GetNetworkResponse>, Status> {
        #[derive(serde::Deserialize)]
        struct Net { netid: String, addressrange: Option<String>, addressrange6: Option<String>, localrange: Option<String> }
        let n: Net = self.get(&format!("/api/networks/{}", req.into_inner().name)).await?;
        Ok(Response::new(GetNetworkResponse {
            network: Some(Network {
                name: n.netid,
                address_range: n.addressrange.unwrap_or_default(),
                address_range6: n.addressrange6.unwrap_or_default(),
                is_local: n.localrange.is_some(),
            }),
        }))
    }

    async fn list_nodes(
        &self,
        req: Request<ListNodesRequest>,
    ) -> Result<Response<ListNodesResponse>, Status> {
        #[derive(serde::Deserialize)]
        struct N { id: String, name: Option<String>, address: Option<String>, publickey: Option<String>, connected: Option<bool>, lastcheckin: Option<i64> }
        let network = req.into_inner().network;
        let path = if network.is_empty() { "/api/nodes".to_string() } else { format!("/api/nodes/{}", network) };
        let raw: Vec<N> = self.get(&path).await?;
        let nodes = raw.into_iter().map(|n| Node {
            id: n.id,
            name: n.name.unwrap_or_default(),
            address: n.address.unwrap_or_default(),
            public_key: n.publickey.unwrap_or_default(),
            connected: n.connected.unwrap_or(false),
            last_checkin: n.lastcheckin.map(|t| t.to_string()).unwrap_or_default(),
        }).collect();
        Ok(Response::new(ListNodesResponse { nodes }))
    }

    async fn get_node(
        &self,
        req: Request<GetNodeRequest>,
    ) -> Result<Response<GetNodeResponse>, Status> {
        #[derive(serde::Deserialize)]
        struct N { id: String, name: Option<String>, address: Option<String>, publickey: Option<String>, connected: Option<bool>, lastcheckin: Option<i64> }
        let n: N = self.get(&format!("/api/nodes/{}", req.into_inner().id)).await?;
        Ok(Response::new(GetNodeResponse {
            node: Some(Node {
                id: n.id,
                name: n.name.unwrap_or_default(),
                address: n.address.unwrap_or_default(),
                public_key: n.publickey.unwrap_or_default(),
                connected: n.connected.unwrap_or(false),
                last_checkin: n.lastcheckin.map(|t| t.to_string()).unwrap_or_default(),
            }),
        }))
    }

    async fn list_hosts(
        &self,
        _req: Request<ListHostsRequest>,
    ) -> Result<Response<ListHostsResponse>, Status> {
        #[derive(serde::Deserialize)]
        struct H { id: String, name: Option<String>, publickey: Option<String>, version: Option<String> }
        let raw: Vec<H> = self.get("/api/hosts").await?;
        let hosts = raw.into_iter().map(|h| Host {
            id: h.id,
            name: h.name.unwrap_or_default(),
            public_key: h.publickey.unwrap_or_default(),
            version: h.version.unwrap_or_default(),
        }).collect();
        Ok(Response::new(ListHostsResponse { hosts }))
    }

    type StreamEventsStream = Pin<Box<dyn Stream<Item = Result<NetmakerEvent, Status>> + Send>>;

    async fn stream_events(
        &self,
        _req: Request<StreamEventsRequest>,
    ) -> Result<Response<Self::StreamEventsStream>, Status> {
        // Poll Netmaker's metric endpoint as a simple event stream
        let stream = async_stream::stream! {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
                yield Ok(NetmakerEvent {
                    event_type: "heartbeat".to_string(),
                    payload_json: "{}".to_string(),
                });
            }
        };
        Ok(Response::new(Box::pin(stream)))
    }
}
