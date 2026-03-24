//! op-agents: Agent implementations for op-dbus
//!
//! This crate provides agent types and the factory function to create them.
//! Agents are domain-specific AI assistants that can be invoked via D-Bus or MCP.

pub mod agent_catalog;
pub mod agent_registry;
pub mod agents;
pub mod dbus_service;
pub mod router;
pub mod security;

// Re-export key types
pub use agent_catalog::{builtin_agent_descriptors, AgentDescriptor};
pub use agent_registry::{AgentRegistry, AgentStatus};
pub use agents::base::{AgentTask, AgentTrait, TaskResult};
pub use agents::*;
pub use router::{create_router, AgentsServiceRouter, AgentsState};

/// Create an agent by type name
///
/// This is the factory function that agent tools and D-Bus services use.
///
/// # Arguments
/// * `agent_type` - The type of agent (e.g., "rust-pro", "memory", "sequential-thinking")
/// * `agent_id` - Unique identifier for this agent instance
///
/// # Returns
/// A boxed agent trait object, or error if type is unknown
pub fn create_agent(
    agent_type: &str,
    agent_id: String,
) -> Result<Box<dyn AgentTrait + Send + Sync>, String> {
    use agents::{
        aiml::{
            AIEngineerAgent, DataEngineerAgent, DataScientistAgent, MLEngineerAgent,
            MLOpsEngineerAgent, PromptEngineerAgent,
        },
        analysis::{
            CodeReviewerAgent, DebuggerAgent, PerformanceEngineerAgent, SecurityAuditorAgent,
        },
        architecture::{BackendArchitectAgent, FrontendDeveloperAgent, GraphQLArchitectAgent},
        business::{
            BusinessAnalystAgent, CustomerSupportAgent, HRProAgent, LegalAdvisorAgent,
            PaymentIntegrationAgent, SalesAutomatorAgent,
        },
        content::{
            ApiDocumenterAgent, DocsArchitectAgent, MermaidExpertAgent, TutorialEngineerAgent,
        },
        database::{DatabaseArchitectAgent, DatabaseOptimizerAgent, SqlProAgent},
        infrastructure::{
            CloudArchitectAgent, DeploymentAgent, KubernetesAgent, NetworkEngineerAgent,
            TerraformAgent,
        },
        language::{
            BashProAgent, CProAgent, CSharpProAgent, CppProAgent, ElixirProAgent, GolangProAgent,
            JavaProAgent, JavaScriptProAgent, JuliaProAgent, PhpProAgent, PythonProAgent,
            RubyProAgent, RustProAgent, ScalaProAgent, TypeScriptProAgent,
        },
        mobile::{FlutterExpertAgent, IOSDeveloperAgent, MobileDeveloperAgent},
        operations::{DevOpsTroubleshooterAgent, IncidentResponderAgent, TestAutomatorAgent},
        orchestration::{
            ContextManagerAgent, DxOptimizerAgent, MemoryAgent, SequentialThinkingAgent,
            TddOrchestratorAgent,
        },
        security::{
            BackendSecurityCoderAgent, FrontendSecurityCoderAgent, MobileSecurityCoderAgent,
        },
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
    };

    let agent: Box<dyn AgentTrait + Send + Sync> = match agent_type {
        // Language agents
        "rust-pro" | "rust_pro" => Box::new(RustProAgent::new(agent_id)),
        "python-pro" | "python_pro" => Box::new(PythonProAgent::new(agent_id)),
        "javascript-pro" | "javascript_pro" => Box::new(JavaScriptProAgent::new(agent_id)),
        "typescript-pro" | "typescript_pro" => Box::new(TypeScriptProAgent::new(agent_id)),
        "golang-pro" | "golang_pro" | "go-pro" => Box::new(GolangProAgent::new(agent_id)),
        "java-pro" | "java_pro" => Box::new(JavaProAgent::new(agent_id)),
        "csharp-pro" | "csharp_pro" | "c#-pro" => Box::new(CSharpProAgent::new(agent_id)),
        "cpp-pro" | "cpp_pro" | "c++-pro" => Box::new(CppProAgent::new(agent_id)),
        "c-pro" | "c_pro" => Box::new(CProAgent::new(agent_id)),
        "ruby-pro" | "ruby_pro" => Box::new(RubyProAgent::new(agent_id)),
        "php-pro" | "php_pro" => Box::new(PhpProAgent::new(agent_id)),
        "scala-pro" | "scala_pro" => Box::new(ScalaProAgent::new(agent_id)),
        "elixir-pro" | "elixir_pro" => Box::new(ElixirProAgent::new(agent_id)),
        "julia-pro" | "julia_pro" => Box::new(JuliaProAgent::new(agent_id)),
        "bash-pro" | "bash_pro" => Box::new(BashProAgent::new(agent_id)),

        // Architecture agents
        "backend-architect" | "backend_architect" => Box::new(BackendArchitectAgent::new(agent_id)),
        "frontend-developer" | "frontend_developer" => {
            Box::new(FrontendDeveloperAgent::new(agent_id))
        }
        "graphql-architect" | "graphql_architect" => Box::new(GraphQLArchitectAgent::new(agent_id)),

        // Infrastructure agents
        "network-engineer" | "network_engineer" => Box::new(NetworkEngineerAgent::new(agent_id)),
        "deployment" => Box::new(DeploymentAgent::new(agent_id)),
        "kubernetes" | "k8s" => Box::new(KubernetesAgent::new(agent_id)),
        "terraform" => Box::new(TerraformAgent::new(agent_id)),
        "cloud-architect" | "cloud_architect" => Box::new(CloudArchitectAgent::new(agent_id)),

        // Orchestration agents
        "memory" => Box::new(MemoryAgent::new(agent_id)),
        "context-manager" | "context_manager" => Box::new(ContextManagerAgent::new(agent_id)),
        "sequential-thinking" | "sequential_thinking" => {
            Box::new(SequentialThinkingAgent::new(agent_id))
        }
        "dx-optimizer" | "dx_optimizer" => Box::new(DxOptimizerAgent::new(agent_id)),
        "tdd-orchestrator" | "tdd_orchestrator" => Box::new(TddOrchestratorAgent::new(agent_id)),

        // Analysis agents
        "debugger" => Box::new(DebuggerAgent::new(agent_id)),
        "code-reviewer" | "code_reviewer" => Box::new(CodeReviewerAgent::new(agent_id)),
        "performance-engineer" | "performance_engineer" => {
            Box::new(PerformanceEngineerAgent::new(agent_id))
        }
        "security-auditor" | "security_auditor" => Box::new(SecurityAuditorAgent::new(agent_id)),

        // SEO agents
        "search-specialist" | "search_specialist" => Box::new(SearchSpecialistAgent::new(agent_id)),
        "seo-content-writer" | "seo_content_writer" => {
            Box::new(SEOContentWriterAgent::new(agent_id))
        }
        "seo-keyword-strategist" | "seo_keyword_strategist" => {
            Box::new(SEOKeywordStrategistAgent::new(agent_id))
        }
        "seo-meta-optimizer" | "seo_meta_optimizer" => {
            Box::new(SEOMetaOptimizerAgent::new(agent_id))
        }
        "content-marketer" | "content_marketer" => Box::new(ContentMarketerAgent::new(agent_id)),

        // AI/ML agents
        "prompt-engineer" | "prompt_engineer" => Box::new(PromptEngineerAgent::new(agent_id)),
        "ai-engineer" | "ai_engineer" => Box::new(AIEngineerAgent::new(agent_id)),
        "ml-engineer" | "ml_engineer" => Box::new(MLEngineerAgent::new(agent_id)),
        "mlops-engineer" | "mlops_engineer" => Box::new(MLOpsEngineerAgent::new(agent_id)),
        "data-scientist" | "data_scientist" => Box::new(DataScientistAgent::new(agent_id)),
        "data-engineer" | "data_engineer" => Box::new(DataEngineerAgent::new(agent_id)),

        // Database agents
        "database-architect" | "database_architect" => {
            Box::new(DatabaseArchitectAgent::new(agent_id))
        }
        "database-optimizer" | "database_optimizer" => {
            Box::new(DatabaseOptimizerAgent::new(agent_id))
        }
        "sql-pro" | "sql_pro" => Box::new(SqlProAgent::new(agent_id)),

        // Operations agents
        "devops-troubleshooter" | "devops_troubleshooter" => {
            Box::new(DevOpsTroubleshooterAgent::new(agent_id))
        }
        "incident-responder" | "incident_responder" => {
            Box::new(IncidentResponderAgent::new(agent_id))
        }
        "test-automator" | "test_automator" => Box::new(TestAutomatorAgent::new(agent_id)),

        // Security agents
        "backend-security-coder" | "backend_security_coder" => {
            Box::new(BackendSecurityCoderAgent::new(agent_id))
        }
        "frontend-security-coder" | "frontend_security_coder" => {
            Box::new(FrontendSecurityCoderAgent::new(agent_id))
        }
        "mobile-security-coder" | "mobile_security_coder" => {
            Box::new(MobileSecurityCoderAgent::new(agent_id))
        }

        // Business agents
        "business-analyst" | "business_analyst" => Box::new(BusinessAnalystAgent::new(agent_id)),
        "customer-support" | "customer_support" => Box::new(CustomerSupportAgent::new(agent_id)),
        "hr-pro" | "hr_pro" => Box::new(HRProAgent::new(agent_id)),
        "legal-advisor" | "legal_advisor" => Box::new(LegalAdvisorAgent::new(agent_id)),
        "payment-integration" | "payment_integration" => {
            Box::new(PaymentIntegrationAgent::new(agent_id))
        }
        "sales-automator" | "sales_automator" => Box::new(SalesAutomatorAgent::new(agent_id)),

        // Content agents
        "api-documenter" | "api_documenter" => Box::new(ApiDocumenterAgent::new(agent_id)),
        "docs-architect" | "docs_architect" => Box::new(DocsArchitectAgent::new(agent_id)),
        "mermaid-expert" | "mermaid_expert" => Box::new(MermaidExpertAgent::new(agent_id)),
        "tutorial-engineer" | "tutorial_engineer" => Box::new(TutorialEngineerAgent::new(agent_id)),

        // Mobile agents
        "flutter-expert" | "flutter_expert" => Box::new(FlutterExpertAgent::new(agent_id)),
        "ios-developer" | "ios_developer" => Box::new(IOSDeveloperAgent::new(agent_id)),
        "mobile-developer" | "mobile_developer" => Box::new(MobileDeveloperAgent::new(agent_id)),

        // Specialty agents
        "arm-cortex-expert" | "arm_cortex_expert" => Box::new(ARMCortexExpertAgent::new(agent_id)),
        "blockchain-developer" | "blockchain_developer" => {
            Box::new(BlockchainDeveloperAgent::new(agent_id))
        }
        "error-detective" | "error_detective" => Box::new(ErrorDetectiveAgent::new(agent_id)),
        "hybrid-cloud-architect" | "hybrid_cloud_architect" => {
            Box::new(HybridCloudArchitectAgent::new(agent_id))
        }
        "legacy-modernizer" | "legacy_modernizer" => Box::new(LegacyModernizerAgent::new(agent_id)),
        "observability-engineer" | "observability_engineer" => {
            Box::new(ObservabilityEngineerAgent::new(agent_id))
        }
        "quant-analyst" | "quant_analyst" => Box::new(QuantAnalystAgent::new(agent_id)),
        "ui-ux-designer" | "ui_ux_designer" => Box::new(UIUXDesignerAgent::new(agent_id)),
        "unity-developer" | "unity_developer" => Box::new(UnityDeveloperAgent::new(agent_id)),

        // Web framework agents
        "django-pro" | "django_pro" => Box::new(DjangoProAgent::new(agent_id)),
        "fastapi-pro" | "fastapi_pro" => Box::new(FastAPIProAgent::new(agent_id)),
        "temporal-python-pro" | "temporal_python_pro" => {
            Box::new(TemporalPythonProAgent::new(agent_id))
        }

        _ => return Err(format!("Unknown agent type: {}", agent_type)),
    };

    Ok(agent)
}

/// List all available agent types
pub fn list_agent_types() -> Vec<&'static str> {
    vec![
        // Language
        "rust-pro",
        "python-pro",
        "javascript-pro",
        "typescript-pro",
        "golang-pro",
        "java-pro",
        "csharp-pro",
        "cpp-pro",
        "c-pro",
        "ruby-pro",
        "php-pro",
        "scala-pro",
        "elixir-pro",
        "julia-pro",
        "bash-pro",
        // Architecture
        "backend-architect",
        "frontend-developer",
        "graphql-architect",
        // Infrastructure
        "network-engineer",
        "deployment",
        "kubernetes",
        "terraform",
        "cloud-architect",
        // Orchestration
        "memory",
        "context-manager",
        "sequential-thinking",
        "dx-optimizer",
        "tdd-orchestrator",
        // Analysis
        "debugger",
        "code-reviewer",
        "performance-engineer",
        "security-auditor",
        // SEO
        "search-specialist",
        "seo-content-writer",
        "seo-keyword-strategist",
        "seo-meta-optimizer",
        "content-marketer",
        // AI/ML
        "prompt-engineer",
        "ai-engineer",
        "ml-engineer",
        "mlops-engineer",
        "data-scientist",
        "data-engineer",
        // Database
        "database-architect",
        "database-optimizer",
        "sql-pro",
        // Operations
        "devops-troubleshooter",
        "incident-responder",
        "test-automator",
        // Security
        "backend-security-coder",
        "frontend-security-coder",
        "mobile-security-coder",
        // Business
        "business-analyst",
        "customer-support",
        "hr-pro",
        "legal-advisor",
        "payment-integration",
        "sales-automator",
        // Content
        "api-documenter",
        "docs-architect",
        "mermaid-expert",
        "tutorial-engineer",
        // Mobile
        "flutter-expert",
        "ios-developer",
        "mobile-developer",
        // Specialty
        "arm-cortex-expert",
        "blockchain-developer",
        "error-detective",
        "hybrid-cloud-architect",
        "legacy-modernizer",
        "observability-engineer",
        "quant-analyst",
        "ui-ux-designer",
        "unity-developer",
        // Web frameworks
        "django-pro",
        "fastapi-pro",
        "temporal-python-pro",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_agent() {
        let agent = create_agent("memory", "test-1".to_string());
        assert!(agent.is_ok());
        let agent = agent.unwrap();
        assert_eq!(agent.agent_type(), "memory");
    }

    #[test]
    fn test_create_agent_underscore_variant() {
        let agent = create_agent("rust_pro", "test-2".to_string());
        assert!(agent.is_ok());
    }

    #[test]
    fn test_unknown_agent() {
        let agent = create_agent("unknown-agent", "test-3".to_string());
        assert!(agent.is_err());
    }

    #[test]
    fn test_list_agent_types() {
        let types = list_agent_types();
        assert!(types.contains(&"memory"));
        assert!(types.contains(&"rust-pro"));
        assert!(types.contains(&"sequential-thinking"));
        assert!(types.len() > 50); // We have many agents
    }
}
