//! Stand-in `zeroclawlabs` types so this repo compiles without `/srv/git/zeroclaw`.
//! On the live host, point `workspace.dependencies.zeroclaw` at that checkout.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct Config {}

pub mod config {
    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct AgentConfig {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct AutonomyConfig {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct BackupConfig {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct BrowserConfig {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct ChannelsConfig {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct CloudOpsConfig {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct ComposioConfig {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct ConversationalAiConfig {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct CostConfig {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct CronConfig {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct DataRetentionConfig {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct DelegateAgentConfig {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct DelegateToolConfig {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct EmbeddingRouteConfig {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct GatewayConfig {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct GoogleWorkspaceConfig {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct HardwareConfig {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct HeartbeatConfig {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct HooksConfig {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct HttpRequestConfig {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct IdentityConfig {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct JiraConfig {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct KnowledgeConfig {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct LinkedInConfig {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct McpConfig {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct MemoryConfig {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct Microsoft365Config {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct ModelRouteConfig {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct MultimodalConfig {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct NodeTransportConfig {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct NodesConfig {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct NotionConfig {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct ObservabilityConfig {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct PeripheralsConfig {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct PluginsConfig {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct ProjectIntelConfig {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct ProxyConfig {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct QueryClassificationConfig {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct ReliabilityConfig {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct RuntimeConfig {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct SchedulerConfig {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct SecretsConfig {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct SecurityConfig {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct SecurityOpsConfig {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct SkillsConfig {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct StorageConfig {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct SwarmConfig {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct TextBrowserConfig {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct TranscriptionConfig {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct TtsConfig {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct TunnelConfig {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct WebFetchConfig {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct WebSearchConfig {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
    pub struct WorkspaceConfig {}

    pub mod schema {
        use schemars::JsonSchema;
        use serde::{Deserialize, Serialize};

        #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
        pub struct ModelProviderConfig {}
    }
}

pub mod tools {
    pub mod browser_delegate {
        use schemars::JsonSchema;
        use serde::{Deserialize, Serialize};

        #[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
        pub struct BrowserDelegateConfig {}
    }
}
