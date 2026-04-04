//! Agent catalog for tool registration.
//!
//! Builds a list of agent descriptors with operations for MCP/tool exposure.

use crate::agents::{
    aiml::{
        AIEngineerAgent, DataEngineerAgent, DataScientistAgent, MLEngineerAgent,
        MLOpsEngineerAgent, PromptEngineerAgent,
    },
    analysis::{CodeReviewerAgent, DebuggerAgent, PerformanceEngineerAgent, SecurityAuditorAgent},
    architecture::{BackendArchitectAgent, FrontendDeveloperAgent, GraphQLArchitectAgent},
    business::{
        BusinessAnalystAgent, CustomerSupportAgent, HRProAgent, LegalAdvisorAgent,
        PaymentIntegrationAgent, SalesAutomatorAgent,
    },
    content::{ApiDocumenterAgent, DocsArchitectAgent, MermaidExpertAgent, TutorialEngineerAgent},
    database::{DatabaseArchitectAgent, DatabaseOptimizerAgent, SqlProAgent},
    infrastructure::{
        CloudArchitectAgent, DeploymentAgent, KubernetesAgent, NetworkEngineerAgent, TerraformAgent,
    },
    language::{
        BashProAgent, CProAgent, CSharpProAgent, CppProAgent, ElixirProAgent, GolangProAgent,
        JavaProAgent, JavaScriptProAgent, JuliaProAgent, PhpProAgent, PythonProAgent, RubyProAgent,
        RustProAgent, ScalaProAgent, TypeScriptProAgent,
    },
    mobile::{FlutterExpertAgent, IOSDeveloperAgent, MobileDeveloperAgent},
    operations::{DevOpsTroubleshooterAgent, IncidentResponderAgent, TestAutomatorAgent},
    orchestration::{
        ContextManagerAgent, DxOptimizerAgent, MemoryAgent, SequentialThinkingAgent,
        TddOrchestratorAgent,
    },
    security::{BackendSecurityCoderAgent, FrontendSecurityCoderAgent, MobileSecurityCoderAgent},
    seo::{
        ContentMarketerAgent, SEOContentWriterAgent, SEOKeywordStrategistAgent,
        SEOMetaOptimizerAgent, SearchSpecialistAgent,
    },
    specialty::{
        ARMCortexExpertAgent, BlockchainDeveloperAgent, ErrorDetectiveAgent,
        HybridCloudArchitectAgent, LegacyModernizerAgent, ObservabilityEngineerAgent,
        QuantAnalystAgent, UIUXDesignerAgent, UnityDeveloperAgent,
    },
    webframeworks::{DjangoProAgent, FastAPIProAgent, TemporalPythonProAgent},
    AgentTrait,
};

/// Minimal descriptor for tool registration.
#[derive(Debug, Clone)]
pub struct AgentDescriptor {
    pub agent_type: String,
    pub name: String,
    pub description: String,
    pub operations: Vec<String>,
}

fn describe_agent(agent: &dyn AgentTrait) -> AgentDescriptor {
    AgentDescriptor {
        agent_type: agent.agent_type().to_string(),
        name: agent.name().to_string(),
        description: agent.description().to_string(),
        operations: agent.operations(),
    }
}

/// List built-in agents suitable for MCP/tool exposure.
pub fn builtin_agent_descriptors() -> Vec<AgentDescriptor> {
    let agent_id = "catalog".to_string();

    let agents: Vec<Box<dyn AgentTrait>> = vec![
        // Language agents
        Box::new(BashProAgent::new(agent_id.clone())),
        Box::new(CProAgent::new(agent_id.clone())),
        Box::new(CppProAgent::new(agent_id.clone())),
        Box::new(CSharpProAgent::new(agent_id.clone())),
        Box::new(ElixirProAgent::new(agent_id.clone())),
        Box::new(GolangProAgent::new(agent_id.clone())),
        Box::new(JavaProAgent::new(agent_id.clone())),
        Box::new(JavaScriptProAgent::new(agent_id.clone())),
        Box::new(JuliaProAgent::new(agent_id.clone())),
        Box::new(PhpProAgent::new(agent_id.clone())),
        Box::new(PythonProAgent::new(agent_id.clone())),
        Box::new(RubyProAgent::new(agent_id.clone())),
        Box::new(RustProAgent::new(agent_id.clone())),
        Box::new(ScalaProAgent::new(agent_id.clone())),
        Box::new(TypeScriptProAgent::new(agent_id.clone())),
        // Architecture agents
        Box::new(BackendArchitectAgent::new(agent_id.clone())),
        Box::new(FrontendDeveloperAgent::new(agent_id.clone())),
        Box::new(GraphQLArchitectAgent::new(agent_id.clone())),
        // Infrastructure agents
        Box::new(CloudArchitectAgent::new(agent_id.clone())),
        Box::new(DeploymentAgent::new(agent_id.clone())),
        Box::new(KubernetesAgent::new(agent_id.clone())),
        Box::new(NetworkEngineerAgent::new(agent_id.clone())),
        Box::new(TerraformAgent::new(agent_id.clone())),
        // Analysis agents
        Box::new(CodeReviewerAgent::new(agent_id.clone())),
        Box::new(DebuggerAgent::new(agent_id.clone())),
        Box::new(PerformanceEngineerAgent::new(agent_id.clone())),
        Box::new(SecurityAuditorAgent::new(agent_id.clone())),
        // Business agents
        Box::new(BusinessAnalystAgent::new(agent_id.clone())),
        Box::new(CustomerSupportAgent::new(agent_id.clone())),
        Box::new(HRProAgent::new(agent_id.clone())),
        Box::new(LegalAdvisorAgent::new(agent_id.clone())),
        Box::new(PaymentIntegrationAgent::new(agent_id.clone())),
        Box::new(SalesAutomatorAgent::new(agent_id.clone())),
        // Content agents
        Box::new(ApiDocumenterAgent::new(agent_id.clone())),
        Box::new(DocsArchitectAgent::new(agent_id.clone())),
        Box::new(MermaidExpertAgent::new(agent_id.clone())),
        Box::new(TutorialEngineerAgent::new(agent_id.clone())),
        // Database agents
        Box::new(DatabaseArchitectAgent::new(agent_id.clone())),
        Box::new(DatabaseOptimizerAgent::new(agent_id.clone())),
        Box::new(SqlProAgent::new(agent_id.clone())),
        // Operations agents
        Box::new(DevOpsTroubleshooterAgent::new(agent_id.clone())),
        Box::new(IncidentResponderAgent::new(agent_id.clone())),
        Box::new(TestAutomatorAgent::new(agent_id.clone())),
        // Orchestration agents
        Box::new(ContextManagerAgent::new(agent_id.clone())),
        Box::new(DxOptimizerAgent::new(agent_id.clone())),
        Box::new(TddOrchestratorAgent::new(agent_id.clone())),
        // Security agents
        Box::new(BackendSecurityCoderAgent::new(agent_id.clone())),
        Box::new(FrontendSecurityCoderAgent::new(agent_id.clone())),
        Box::new(MobileSecurityCoderAgent::new(agent_id.clone())),
        // SEO agents
        Box::new(ContentMarketerAgent::new(agent_id.clone())),
        Box::new(SearchSpecialistAgent::new(agent_id.clone())),
        Box::new(SEOContentWriterAgent::new(agent_id.clone())),
        Box::new(SEOKeywordStrategistAgent::new(agent_id.clone())),
        Box::new(SEOMetaOptimizerAgent::new(agent_id.clone())),
        // Specialty agents
        Box::new(ARMCortexExpertAgent::new(agent_id.clone())),
        Box::new(BlockchainDeveloperAgent::new(agent_id.clone())),
        Box::new(ErrorDetectiveAgent::new(agent_id.clone())),
        Box::new(HybridCloudArchitectAgent::new(agent_id.clone())),
        Box::new(LegacyModernizerAgent::new(agent_id.clone())),
        Box::new(MemoryAgent::new(agent_id.clone())),
        Box::new(SequentialThinkingAgent::new(agent_id.clone())),
        Box::new(ObservabilityEngineerAgent::new(agent_id.clone())),
        Box::new(QuantAnalystAgent::new(agent_id.clone())),
        Box::new(UIUXDesignerAgent::new(agent_id.clone())),
        Box::new(UnityDeveloperAgent::new(agent_id.clone())),
        // AI/ML agents
        Box::new(AIEngineerAgent::new(agent_id.clone())),
        Box::new(DataEngineerAgent::new(agent_id.clone())),
        Box::new(DataScientistAgent::new(agent_id.clone())),
        Box::new(MLEngineerAgent::new(agent_id.clone())),
        Box::new(MLOpsEngineerAgent::new(agent_id.clone())),
        Box::new(PromptEngineerAgent::new(agent_id.clone())),
        // Web frameworks
        Box::new(DjangoProAgent::new(agent_id.clone())),
        Box::new(FastAPIProAgent::new(agent_id.clone())),
        Box::new(TemporalPythonProAgent::new(agent_id.clone())),
        // Mobile
        Box::new(FlutterExpertAgent::new(agent_id.clone())),
        Box::new(IOSDeveloperAgent::new(agent_id.clone())),
        Box::new(MobileDeveloperAgent::new(agent_id)),
    ];

    agents
        .iter()
        .map(|agent| describe_agent(agent.as_ref()))
        .collect()
}
