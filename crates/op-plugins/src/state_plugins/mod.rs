//! State plugins - each manages a domain via native protocols
//!
//! These plugins implement the StatePlugin trait from op-state

// pub mod dnsresolver;
// pub mod full_system;
// pub mod keyring;
// pub mod login1;
// pub mod lxc;
pub mod incus;
pub mod mcp;
pub mod net;
// pub mod netmaker;
pub mod openflow;
// pub mod openflow_obfuscation;
// pub mod packagekit;
// pub mod pcidecl;
// pub mod privacy;
pub mod adc;
pub mod config;
pub mod dinit;
pub mod endpoint;
pub mod gcloud_adc;
pub mod keypair;
pub mod privacy_router;
pub mod privacy_routes;
pub mod proxy_server;
pub mod service;
pub mod sessdecl;
// pub mod systemd;
// pub mod systemd_networkd;

pub mod agent_config;
pub mod hardware;
pub mod ovsdb_bridge;
pub mod proxmox;
pub mod rtnetlink;
pub mod schema_contract;
pub mod software;
pub mod users;
pub mod web_ui;
pub mod wireguard;

// Re-export plugin types
// pub use dnsresolver::DnsResolverPlugin;
// pub use full_system::FullSystemPlugin;
// pub use login1::Login1Plugin;
// pub use lxc::LxcPlugin;
pub use incus::IncusPlugin;
pub use mcp::McpStatePlugin;
pub use mcp::{ExecutionResult, ToolDefinition};
pub use net::NetStatePlugin;
// pub use netmaker::NetmakerPlugin;
pub use openflow::OpenFlowPlugin;
// pub use openflow_obfuscation::OpenFlowObfuscationPlugin;
// pub use packagekit::PackageKitPlugin;
// pub use pcidecl::PciDeclPlugin;
// pub use privacy::PrivacyPlugin;
pub use adc::AdcPlugin;
pub use agent_config::AgentConfigPlugin;
pub use config::ConfigPlugin;
pub use dinit::DinitStatePlugin;
pub use endpoint::EndpointPlugin;
pub use gcloud_adc::GcloudAdcPlugin;
pub use hardware::HardwarePlugin;
pub use keypair::KeypairPlugin;
pub use ovsdb_bridge::OvsBridgePlugin;
pub use privacy_router::PrivacyRouterPlugin;
pub use privacy_routes::PrivacyRoutesPlugin;
pub use proxmox::ProxmoxPlugin;
pub use proxy_server::ProxyServerPlugin;
pub use rtnetlink::RtnetlinkPlugin;
pub use sessdecl::SessDeclPlugin;
pub use software::SoftwarePlugin;
// pub use systemd::SystemdStatePlugin;
pub use users::UsersPlugin;
pub use web_ui::WebUiPlugin;
pub use wireguard::WireGuardPlugin;
// pub use systemd_networkd::SystemdNetworkdPlugin; // TODO: Plugin not yet implemented
