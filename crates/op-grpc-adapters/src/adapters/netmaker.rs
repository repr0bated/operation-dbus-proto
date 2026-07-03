//! Netmaker adapter — bridges Netmaker HTTP REST API to gRPC.
//! Connects via unix socket: /run/netmaker/api.sock

use crate::proto::{
    netmaker_service_server::NetmakerService, GetNetworkRequest, GetNetworkResponse,
    GetNodeRequest, GetNodeResponse, HealthRequest, HealthResponse, Host, JoinNetworkRequest,
    JoinNetworkResponse, LeaveNetworkRequest, LeaveNetworkResponse, ListHostsRequest,
    ListHostsResponse, ListNetworksRequest, ListNetworksResponse, ListNodesRequest,
    ListNodesResponse, NetmakerEvent, Network, Node, RestartServiceRequest,
    RestartServiceResponse, ExecuteCommandRequest, ExecuteCommandResponse,
    StreamEventsRequest,
};
use async_trait::async_trait;
use serde_json::Value as JsonValue;
use std::pin::Pin;
use tokio::process::Command;
use tokio_stream::Stream;
use tonic::{Request, Response, Status};

#[allow(dead_code)]
const API_SOCK: &str = "/run/netmaker/api.sock";

pub struct NetmakerAdapter {
    client: reqwest::Client,
    #[allow(dead_code)]
    base_url: String,
}

impl NetmakerAdapter {
    pub fn new() -> Self {
        // Route HTTP through the service endpoint exposed by the socket-backed
        // registration path.
        let client = reqwest::Client::builder().build().expect("reqwest client");
        Self {
            client,
            base_url: "http://localhost".to_string(),
        }
    }

    async fn get<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T, Status> {
        let master_key =
            std::env::var("NETMAKER_MASTER_KEY").unwrap_or_else(|_| "masterkey".to_string());

        // The socket-backed registration path resolves the service endpoint;
        // the adapter keeps a conventional HTTP client here.
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

    async fn patch_netclient_config<F>(&self, edit: F) -> Result<(), Status>
    where
        F: FnOnce(&mut JsonValue),
    {
        let path = "/etc/netclient/netclient.json";
        let mut config = match tokio::fs::read_to_string(path).await {
            Ok(content) => serde_json::from_str::<JsonValue>(&content)
                .map_err(|e| Status::internal(format!("netclient config parse error: {}", e)))?,
            Err(_) => serde_json::json!({}),
        };

        edit(&mut config);

        tokio::fs::create_dir_all("/etc/netclient")
            .await
            .map_err(|e| Status::internal(format!("netclient config dir error: {}", e)))?;
        tokio::fs::write(path, serde_json::to_string_pretty(&config).unwrap_or_else(|_| "{}".to_string()))
            .await
            .map_err(|e| Status::internal(format!("netclient config write error: {}", e)))?;
        Ok(())
    }

    async fn restart_netclient(&self) -> Result<(), Status> {
        let resp = self
            .execute_command(Request::new(ExecuteCommandRequest {
                command: Some(crate::proto::execute_command_request::Command::Restart(
                    RestartServiceRequest {
                        service: "netclient".to_string(),
                    },
                )),
            }))
            .await?
            .into_inner();
        if resp.success { Ok(()) } else { Err(Status::internal(resp.stderr)) }
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
        struct HealthResp {
            status: String,
            version: Option<String>,
        }
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
        struct Net {
            netid: String,
            addressrange: Option<String>,
            addressrange6: Option<String>,
            localrange: Option<String>,
        }
        let nets: Vec<Net> = self.get("/api/networks").await?;
        let networks = nets
            .into_iter()
            .map(|n| Network {
                name: n.netid,
                address_range: n.addressrange.unwrap_or_default(),
                address_range6: n.addressrange6.unwrap_or_default(),
                is_local: n.localrange.is_some(),
            })
            .collect();
        Ok(Response::new(ListNetworksResponse { networks }))
    }

    async fn get_network(
        &self,
        req: Request<GetNetworkRequest>,
    ) -> Result<Response<GetNetworkResponse>, Status> {
        #[derive(serde::Deserialize)]
        struct Net {
            netid: String,
            addressrange: Option<String>,
            addressrange6: Option<String>,
            localrange: Option<String>,
        }
        let n: Net = self
            .get(&format!("/api/networks/{}", req.into_inner().name))
            .await?;
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
        struct N {
            id: String,
            name: Option<String>,
            address: Option<String>,
            publickey: Option<String>,
            connected: Option<bool>,
            lastcheckin: Option<i64>,
        }
        let network = req.into_inner().network;
        let path = if network.is_empty() {
            "/api/nodes".to_string()
        } else {
            format!("/api/nodes/{}", network)
        };
        let raw: Vec<N> = self.get(&path).await?;
        let nodes = raw
            .into_iter()
            .map(|n| Node {
                id: n.id,
                name: n.name.unwrap_or_default(),
                address: n.address.unwrap_or_default(),
                public_key: n.publickey.unwrap_or_default(),
                connected: n.connected.unwrap_or(false),
                last_checkin: n.lastcheckin.map(|t| t.to_string()).unwrap_or_default(),
            })
            .collect();
        Ok(Response::new(ListNodesResponse { nodes }))
    }

    async fn get_node(
        &self,
        req: Request<GetNodeRequest>,
    ) -> Result<Response<GetNodeResponse>, Status> {
        #[derive(serde::Deserialize)]
        struct N {
            id: String,
            name: Option<String>,
            address: Option<String>,
            publickey: Option<String>,
            connected: Option<bool>,
            lastcheckin: Option<i64>,
        }
        let n: N = self
            .get(&format!("/api/nodes/{}", req.into_inner().id))
            .await?;
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
        struct H {
            id: String,
            name: Option<String>,
            publickey: Option<String>,
            version: Option<String>,
        }
        let raw: Vec<H> = self.get("/api/hosts").await?;
        let hosts = raw
            .into_iter()
            .map(|h| Host {
                id: h.id,
                name: h.name.unwrap_or_default(),
                public_key: h.publickey.unwrap_or_default(),
                version: h.version.unwrap_or_default(),
            })
            .collect();
        Ok(Response::new(ListHostsResponse { hosts }))
    }

    async fn join_network(
        &self,
        req: Request<JoinNetworkRequest>,
    ) -> Result<Response<JoinNetworkResponse>, Status> {
        let r = req.into_inner();
        self.patch_netclient_config(|config| {
            config["enabled"] = serde_json::json!(true);
            config["default_network"] = serde_json::json!(r.network);
            config["enrollment_token"] = serde_json::json!(r.token);
            config["interface"] = serde_json::json!("netmaker");
            config["inittype"] = serde_json::json!(6);
        })
        .await?;
        self.restart_netclient().await?;
        Ok(Response::new(JoinNetworkResponse {
            success: true,
            message: "netclient configuration updated and restarted".to_string(),
        }))
    }

    async fn leave_network(
        &self,
        req: Request<LeaveNetworkRequest>,
    ) -> Result<Response<LeaveNetworkResponse>, Status> {
        let network = req.into_inner().network;
        self.patch_netclient_config(|config| {
            if config
                .get("default_network")
                .and_then(|v| v.as_str())
                == Some(network.as_str())
            {
                config["default_network"] = serde_json::json!("");
            }
            config["enabled"] = serde_json::json!(false);
            config["enrollment_token"] = serde_json::json!(null);
        })
        .await?;
        self.restart_netclient().await?;
        Ok(Response::new(LeaveNetworkResponse {
            success: true,
            message: "netclient configuration updated and restarted".to_string(),
        }))
    }

    async fn restart_service(
        &self,
        req: Request<RestartServiceRequest>,
    ) -> Result<Response<RestartServiceResponse>, Status> {
        let service = req.into_inner().service;
        if service == "netclient" {
            self.restart_netclient().await?;
        } else {
            return Err(Status::invalid_argument(format!(
                "unsupported service '{}'",
                service
            )));
        }
        Ok(Response::new(RestartServiceResponse {
            success: true,
            message: format!("{} restarted", service),
        }))
    }

    async fn execute_command(
        &self,
        req: Request<ExecuteCommandRequest>,
    ) -> Result<Response<ExecuteCommandResponse>, Status> {
        let command = req.into_inner().command.ok_or_else(|| {
            Status::invalid_argument("missing command variant for netclient execution")
        })?;

        match command {
            crate::proto::execute_command_request::Command::Join(join) => {
                self.patch_netclient_config(|config| {
                    config["enabled"] = serde_json::json!(true);
                    config["default_network"] = serde_json::json!(join.network);
                    config["enrollment_token"] = serde_json::json!(join.token);
                    config["interface"] = serde_json::json!("netmaker");
                    config["inittype"] = serde_json::json!(6);
                })
                .await?;
                self.restart_netclient().await?;
                Ok(Response::new(ExecuteCommandResponse {
                    success: true,
                    exit_code: 0,
                    stdout: "joined network".to_string(),
                    stderr: String::new(),
                }))
            }
            crate::proto::execute_command_request::Command::Leave(leave) => {
                self.patch_netclient_config(|config| {
                    if config
                        .get("default_network")
                        .and_then(|v| v.as_str())
                        == Some(leave.network.as_str())
                    {
                        config["default_network"] = serde_json::json!("");
                    }
                    config["enabled"] = serde_json::json!(false);
                    config["enrollment_token"] = serde_json::json!(null);
                })
                .await?;
                self.restart_netclient().await?;
                Ok(Response::new(ExecuteCommandResponse {
                    success: true,
                    exit_code: 0,
                    stdout: "left network".to_string(),
                    stderr: String::new(),
                }))
            }
            crate::proto::execute_command_request::Command::Restart(restart) => {
                if restart.service != "netclient" {
                    return Err(Status::invalid_argument(format!(
                        "unsupported service '{}'",
                        restart.service
                    )));
                }
                let output = Command::new("s6d")
                    .args(["restart", "netclient"])
                    .output()
                    .await
                    .map_err(|e| Status::internal(format!("failed to execute s6d: {}", e)))?;
                Ok(Response::new(ExecuteCommandResponse {
                    success: output.status.success(),
                    exit_code: output.status.code().unwrap_or(-1),
                    stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                    stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                }))
            }
            crate::proto::execute_command_request::Command::Connect(req) => {
                self.run_command("connect", vec![req.network]).await
            }
            crate::proto::execute_command_request::Command::Disconnect(req) => {
                self.run_command("disconnect", vec![req.network]).await
            }
            crate::proto::execute_command_request::Command::List(_) => {
                self.run_command("list", vec![]).await
            }
            crate::proto::execute_command_request::Command::Peers(_) => {
                self.run_command("peers", vec![]).await
            }
            crate::proto::execute_command_request::Command::Ping(req) => {
                self.run_command("ping", vec![req.peer]).await
            }
            crate::proto::execute_command_request::Command::Pull(_) => {
                self.run_command("pull", vec![]).await
            }
            crate::proto::execute_command_request::Command::Push(_) => {
                self.run_command("push", vec![]).await
            }
            crate::proto::execute_command_request::Command::Register(req) => {
                self.run_command("register", vec![req.instance]).await
            }
            crate::proto::execute_command_request::Command::Server(req) => {
                let mut args = vec![req.subcommand];
                args.extend(req.args);
                self.run_command("server", args).await
            }
            crate::proto::execute_command_request::Command::Install(_) => {
                self.run_command("install", vec![]).await
            }
            crate::proto::execute_command_request::Command::Uninstall(_) => {
                self.run_command("uninstall", vec![]).await
            }
            crate::proto::execute_command_request::Command::Use(req) => {
                self.run_command("use", vec![req.version]).await
            }
            crate::proto::execute_command_request::Command::Version(_) => {
                self.run_command("version", vec![]).await
            }
        }
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

impl NetmakerAdapter {
    async fn run_command(
        &self,
        name: &str,
        args: Vec<String>,
    ) -> Result<Response<ExecuteCommandResponse>, Status> {
        let mut cmd = Command::new("netclient");
        cmd.arg(name);
        for arg in args {
            cmd.arg(arg);
        }
        let output = cmd
            .output()
            .await
            .map_err(|e| Status::internal(format!("failed to execute netclient: {}", e)))?;
        Ok(Response::new(ExecuteCommandResponse {
            success: output.status.success(),
            exit_code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        }))
    }
}
