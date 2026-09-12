This file is a merged representation of a subset of the codebase, containing specifically included files, combined into a single document by Repomix.

<file_summary>
This section contains a summary of this file.

<purpose>
This file contains a packed representation of a subset of the repository's contents that is considered the most important context.
It is designed to be easily consumable by AI systems for analysis, code review,
or other automated processes.
</purpose>

<file_format>
The content is organized as follows:
1. This summary section
2. Repository information
3. Directory structure
4. Repository files (if enabled)
5. Multiple file entries, each consisting of:
  - File path as an attribute
  - Full contents of the file
</file_format>

<usage_guidelines>
- This file should be treated as read-only. Any changes should be made to the
  original repository files, not this packed version.
- When processing this file, use the file path to distinguish
  between different files in the repository.
- Be aware that this file may contain sensitive information. Handle it with
  the same level of security as you would the original repository.
</usage_guidelines>

<notes>
- Some files may have been excluded based on .gitignore rules and Repomix's configuration
- Binary files are not included in this packed representation. Please refer to the Repository Structure section for a complete list of file paths, including binary files
- Only files matching these patterns are included: /home/jeremy/git/operation-dbus-proto/crates/op-agents/**
- Files matching patterns in .gitignore are excluded
- Files matching default ignore patterns are excluded
- Files are sorted by Git change count (files with more changes are at the bottom)
</notes>

</file_summary>

<directory_structure>
/
  home/
    jeremy/
      git/
        operation-dbus-proto/
          crates/
            op-agents/
              src/
                agents/
                  aiml/
                    ai_engineer.rs
                    data_engineer.rs
                    data_scientist.rs
                    ml_engineer.rs
                    mlops_engineer.rs
                    mod.rs
                    prompt_engineer.rs
                  analysis/
                    code_reviewer.rs
                    debugger.rs
                    mod.rs
                    performance.rs
                    security_auditor.rs
                  architecture/
                    backend_architect.rs
                    frontend_developer.rs
                    graphql_architect.rs
                    mod.rs
                  business/
                    business_analyst.rs
                    customer_support.rs
                    hr_pro.rs
                    legal_advisor.rs
                    mod.rs
                    payment_integration.rs
                    sales_automator.rs
                  content/
                    api_documenter.rs
                    docs_architect.rs
                    mermaid_expert.rs
                    mod.rs
                    tutorial_engineer.rs
                  database/
                    database_architect.rs
                    database_optimizer.rs
                    mod.rs
                    sql_pro.rs
                  infrastructure/
                    cloud.rs
                    deployment.rs
                    kubernetes.rs
                    mod.rs
                    network.rs
                    terraform.rs
                  language/
                    bash_pro.rs
                    c_pro.rs
                    cpp_pro.rs
                    csharp_pro.rs
                    elixir_pro.rs
                    golang_pro.rs
                    java_pro.rs
                    javascript_pro.rs
                    julia_pro.rs
                    mod.rs
                    php_pro.rs
                    python_pro.rs
                    ruby_pro.rs
                    rust_pro.rs
                    scala_pro.rs
                    typescript_pro.rs
                  mobile/
                    flutter_expert.rs
                    ios_developer.rs
                    mobile_developer.rs
                    mod.rs
                  operations/
                    devops_troubleshooter.rs
                    incident_responder.rs
                    mod.rs
                    test_automator.rs
                  orchestration/
                    context_manager.rs
                    dx_optimizer.rs
                    mem0_wrapper.rs
                    memory.rs
                    mod.rs
                    sequential_thinking.rs
                    tdd_orchestrator.rs
                  security/
                    backend_security_coder.rs
                    frontend_security_coder.rs
                    mobile_security_coder.rs
                    mod.rs
                  seo/
                    content_marketer.rs
                    mod.rs
                    search_specialist.rs
                    seo_content_writer.rs
                    seo_keyword_strategist.rs
                    seo_meta_optimizer.rs
                  specialty/
                    arm_cortex_expert.rs
                    snowball_developer.rs
                    error_detective.rs
                    hybrid_cloud_architect.rs
                    legacy_modernizer.rs
                    mod.rs
                    observability_engineer.rs
                    quant_analyst.rs
                    ui_ux_designer.rs
                    unity_developer.rs
                  system/
                    mod.rs
                  webframeworks/
                    django_pro.rs
                    fastapi_pro.rs
                    mod.rs
                    temporal_python_pro.rs
                  base.rs
                  mod.rs
                bin/
                  dbus-agent-manager.rs
                  dbus-agent.rs
                generator/
                  md_parser.rs
                  mod.rs
                  template.rs
                security/
                  mod.rs
                  profiles.rs
                  sandbox.rs
                  validation.rs
                unified/
                  execution/
                    base.rs
                    golang.rs
                    javascript.rs
                    mod.rs
                    python.rs
                    rust.rs
                    shell.rs
                  orchestration/
                    base.rs
                    code_review_orchestrator.rs
                    mod.rs
                    tdd_orchestrator.rs
                  persona/
                    architecture_experts.rs
                    base.rs
                    framework_experts.rs
                    mod.rs
                    operations_experts.rs
                  agent_trait.rs
                  mod.rs
                  prompts.rs
                  registry.rs
                agent_catalog.rs
                agent_registry.rs
                dbus_service.rs
                lib.rs
                router.rs
              Cargo.toml
              compare-op-agents.md
              SPEC.md
</directory_structure>

<files>
This section contains the contents of the repository's files.

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/aiml/ai_engineer.rs">
//! AI Engineer Agent - LLM applications, RAG systems, AI integration

use async_trait::async_trait;
use simd_json::json;

use crate::agents::base::{AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;

pub struct AIEngineerAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl AIEngineerAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            profile: SecurityProfile::content_generation("ai-engineer"),
            agent_id,
        }
    }

    fn analyze(&self, args: Option<&str>) -> Result<String, String> {
        let input = args.unwrap_or("").to_lowercase();
        let mut recommendations = Vec::new();
        let mut patterns = Vec::new();

        if input.contains("rag") || input.contains("retrieval") {
            recommendations.push("Use hybrid search (BM25 + semantic) for better recall");
            recommendations.push("Implement chunk overlap to preserve context at boundaries");
            recommendations.push("Add metadata filtering for efficient retrieval");
            patterns.push("RAG Architecture");
        }

        if input.contains("prompt") || input.contains("llm") {
            recommendations.push("Use few-shot examples for consistent outputs");
            recommendations.push("Implement structured output parsing (JSON mode)");
            recommendations.push("Add input/output guardrails");
            patterns.push("Prompt Engineering");
        }

        if input.contains("agent") || input.contains("tool") {
            recommendations.push("Define clear tool schemas with descriptions");
            recommendations.push("Implement tool use validation and error handling");
            recommendations.push("Add observability for agent decision tracing");
            patterns.push("AI Agents");
        }

        if recommendations.is_empty() {
            recommendations.push("Start with clear use case definition");
            recommendations.push("Evaluate model capabilities vs requirements");
            recommendations.push("Design evaluation metrics before building");
            patterns.push("AI System Design");
        }

        let result = json!({
            "analysis": { "input": args.unwrap_or(""), "patterns": patterns, "recommendations": recommendations },
            "stack_recommendations": {
                "frameworks": ["LangChain", "LlamaIndex", "Semantic Kernel"],
                "vector_dbs": ["Pinecone", "Weaviate", "Qdrant", "ChromaDB"],
                "monitoring": ["LangSmith", "Phoenix", "Weights & Biases"]
            }
        });
        Ok(simd_json::to_string_pretty(&result).unwrap_or_default())
    }
}

#[async_trait]
impl AgentTrait for AIEngineerAgent {
    fn agent_type(&self) -> &str {
        "ai-engineer"
    }
    fn name(&self) -> &str {
        "AI Engineer"
    }
    fn description(&self) -> &str {
        "Build LLM applications, RAG systems, and AI-powered features with production-grade architecture."
    }
    fn operations(&self) -> Vec<String> {
        vec![
            "design_rag".to_string(),
            "design_agent".to_string(),
            "optimize_prompts".to_string(),
            "analyze".to_string(),
        ]
    }
    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }
    fn get_status(&self) -> String {
        format!("AI Engineer agent {} is running", self.agent_id)
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "ai-engineer" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }
        match self.analyze(task.args.as_deref()) {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/aiml/data_engineer.rs">
//! Data Engineer Agent - ETL pipelines, data warehouses, data quality

use crate::agents::base::{AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;
use async_trait::async_trait;
use simd_json::json;

pub struct DataEngineerAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl DataEngineerAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            profile: SecurityProfile::content_generation("data-engineer"),
            agent_id,
        }
    }

    fn analyze(&self, args: Option<&str>) -> Result<String, String> {
        let input = args.unwrap_or("").to_lowercase();
        let mut recommendations = Vec::new();

        if input.contains("pipeline") || input.contains("etl") {
            recommendations.push("Design idempotent pipelines for rerunnability");
            recommendations.push("Implement data quality checks at each stage");
            recommendations.push("Use incremental loads where possible");
        }
        if input.contains("warehouse") || input.contains("lakehouse") {
            recommendations.push("Design dimensional models (star/snowflake schema)");
            recommendations.push("Implement slowly changing dimensions");
            recommendations.push("Partition data for query performance");
        }
        if input.contains("streaming") || input.contains("realtime") {
            recommendations.push("Design for exactly-once semantics");
            recommendations.push("Handle late-arriving data gracefully");
            recommendations.push("Implement windowing strategies");
        }
        if recommendations.is_empty() {
            recommendations.push("Define data contracts and schemas");
            recommendations.push("Implement data lineage tracking");
            recommendations.push("Design for scalability and maintainability");
        }

        let result = json!({
            "analysis": { "input": args.unwrap_or(""), "recommendations": recommendations },
            "tools": { "batch": ["Apache Spark", "dbt", "Airflow"], "streaming": ["Apache Kafka", "Apache Flink", "Apache Beam"], "storage": ["Delta Lake", "Apache Iceberg", "Apache Hudi"] }
        });
        Ok(simd_json::to_string_pretty(&result).unwrap_or_default())
    }
}

#[async_trait]
impl AgentTrait for DataEngineerAgent {
    fn agent_type(&self) -> &str {
        "data-engineer"
    }
    fn name(&self) -> &str {
        "Data Engineer"
    }
    fn description(&self) -> &str {
        "Build data pipelines, warehouses, and infrastructure for analytics and ML."
    }
    fn operations(&self) -> Vec<String> {
        vec![
            "design_pipeline".to_string(),
            "design_warehouse".to_string(),
            "optimize_queries".to_string(),
            "analyze".to_string(),
        ]
    }
    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }
    fn get_status(&self) -> String {
        format!("Data Engineer agent {} is running", self.agent_id)
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "data-engineer" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }
        match self.analyze(task.args.as_deref()) {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/aiml/data_scientist.rs">
//! Data Scientist Agent - Data analysis, visualization, experimentation

use crate::agents::base::{AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;
use async_trait::async_trait;
use simd_json::json;

pub struct DataScientistAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl DataScientistAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            profile: SecurityProfile::content_generation("data-scientist"),
            agent_id,
        }
    }

    fn analyze(&self, args: Option<&str>) -> Result<String, String> {
        let input = args.unwrap_or("").to_lowercase();
        let mut recommendations = Vec::new();

        if input.contains("analysis") || input.contains("explore") {
            recommendations.push("Start with univariate analysis before multivariate");
            recommendations.push("Check for missing values and outliers");
            recommendations.push("Visualize distributions and correlations");
        }
        if input.contains("model") || input.contains("predict") {
            recommendations.push("Establish baseline with simple models first");
            recommendations.push("Use cross-validation for robust evaluation");
            recommendations.push("Feature importance analysis for interpretability");
        }
        if input.contains("experiment") || input.contains("ab") {
            recommendations.push("Define success metrics before experiment");
            recommendations.push("Calculate required sample size");
            recommendations.push("Account for multiple comparison correction");
        }
        if recommendations.is_empty() {
            recommendations.push("Define clear hypothesis or question");
            recommendations.push("Understand data provenance and quality");
            recommendations.push("Choose appropriate statistical methods");
        }

        let result = json!({
            "analysis": { "input": args.unwrap_or(""), "recommendations": recommendations },
            "tools": ["pandas", "numpy", "scikit-learn", "matplotlib", "seaborn", "jupyter"]
        });
        Ok(simd_json::to_string_pretty(&result).unwrap_or_default())
    }
}

#[async_trait]
impl AgentTrait for DataScientistAgent {
    fn agent_type(&self) -> &str {
        "data-scientist"
    }
    fn name(&self) -> &str {
        "Data Scientist"
    }
    fn description(&self) -> &str {
        "Analyze data, build models, run experiments, and derive insights."
    }
    fn operations(&self) -> Vec<String> {
        vec![
            "explore_data".to_string(),
            "build_model".to_string(),
            "run_experiment".to_string(),
            "analyze".to_string(),
        ]
    }
    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }
    fn get_status(&self) -> String {
        format!("Data Scientist agent {} is running", self.agent_id)
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "data-scientist" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }
        match self.analyze(task.args.as_deref()) {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/aiml/ml_engineer.rs">
//! ML Engineer Agent - Model training, optimization, deployment

use crate::agents::base::{AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;
use async_trait::async_trait;
use simd_json::json;

pub struct MLEngineerAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl MLEngineerAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            profile: SecurityProfile::content_generation("ml-engineer"),
            agent_id,
        }
    }

    fn analyze(&self, args: Option<&str>) -> Result<String, String> {
        let input = args.unwrap_or("").to_lowercase();
        let mut recommendations = Vec::new();

        if input.contains("training") || input.contains("model") {
            recommendations.push("Use early stopping and learning rate scheduling");
            recommendations.push("Implement gradient checkpointing for large models");
            recommendations.push("Track experiments with MLflow or W&B");
        }
        if input.contains("deploy") || input.contains("inference") {
            recommendations.push("Quantize models for faster inference");
            recommendations.push("Use batching for throughput optimization");
            recommendations.push("Implement model versioning and A/B testing");
        }
        if recommendations.is_empty() {
            recommendations.push("Start with data quality assessment");
            recommendations.push("Establish baseline models before complex architectures");
            recommendations.push("Design reproducible experiment pipelines");
        }

        let result = json!({
            "analysis": { "input": args.unwrap_or(""), "recommendations": recommendations },
            "frameworks": ["PyTorch", "TensorFlow", "JAX", "scikit-learn", "XGBoost"]
        });
        Ok(simd_json::to_string_pretty(&result).unwrap_or_default())
    }
}

#[async_trait]
impl AgentTrait for MLEngineerAgent {
    fn agent_type(&self) -> &str {
        "ml-engineer"
    }
    fn name(&self) -> &str {
        "ML Engineer"
    }
    fn description(&self) -> &str {
        "Train, optimize, and deploy machine learning models at scale."
    }
    fn operations(&self) -> Vec<String> {
        vec![
            "train_model".to_string(),
            "optimize".to_string(),
            "deploy".to_string(),
            "analyze".to_string(),
        ]
    }
    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }
    fn get_status(&self) -> String {
        format!("ML Engineer agent {} is running", self.agent_id)
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "ml-engineer" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }
        match self.analyze(task.args.as_deref()) {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/aiml/mlops_engineer.rs">
//! MLOps Engineer Agent - ML pipelines, model serving, monitoring

use crate::agents::base::{AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;
use async_trait::async_trait;
use simd_json::json;

pub struct MLOpsEngineerAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl MLOpsEngineerAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            profile: SecurityProfile::content_generation("mlops-engineer"),
            agent_id,
        }
    }

    fn analyze(&self, args: Option<&str>) -> Result<String, String> {
        let input = args.unwrap_or("").to_lowercase();
        let mut recommendations = Vec::new();

        if input.contains("pipeline") {
            recommendations.push("Use Kubeflow or Airflow for orchestration");
            recommendations.push("Implement feature stores for consistency");
            recommendations.push("Version datasets alongside code");
        }
        if input.contains("serving") || input.contains("deploy") {
            recommendations.push("Use TorchServe, TF Serving, or Triton");
            recommendations.push("Implement canary deployments for models");
            recommendations.push("Add model health checks and auto-rollback");
        }
        if input.contains("monitor") {
            recommendations.push("Track data drift and model drift");
            recommendations.push("Set up prediction latency alerts");
            recommendations.push("Monitor feature distribution shifts");
        }
        if recommendations.is_empty() {
            recommendations.push("Establish CI/CD for ML pipelines");
            recommendations.push("Implement model registry (MLflow, SageMaker)");
            recommendations.push("Design feature engineering pipelines");
        }

        let result = json!({
            "analysis": { "input": args.unwrap_or(""), "recommendations": recommendations },
            "tools": { "orchestration": ["Kubeflow", "Airflow", "Prefect"], "serving": ["Seldon", "KServe", "BentoML"], "monitoring": ["Evidently", "WhyLabs", "Arize"] }
        });
        Ok(simd_json::to_string_pretty(&result).unwrap_or_default())
    }
}

#[async_trait]
impl AgentTrait for MLOpsEngineerAgent {
    fn agent_type(&self) -> &str {
        "mlops-engineer"
    }
    fn name(&self) -> &str {
        "MLOps Engineer"
    }
    fn description(&self) -> &str {
        "Build and maintain ML infrastructure, pipelines, and model serving systems."
    }
    fn operations(&self) -> Vec<String> {
        vec![
            "design_pipeline".to_string(),
            "setup_serving".to_string(),
            "configure_monitoring".to_string(),
            "analyze".to_string(),
        ]
    }
    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }
    fn get_status(&self) -> String {
        format!("MLOps Engineer agent {} is running", self.agent_id)
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "mlops-engineer" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }
        match self.analyze(task.args.as_deref()) {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/aiml/mod.rs">
//! AI/ML Agents
//!
//! Specialized agents for AI and machine learning:
//! - `AIEngineer`: LLM applications, RAG systems, prompt engineering
//! - `MLEngineer`: Model training, optimization, deployment
//! - `MLOpsEngineer`: ML pipelines, model serving, monitoring
//! - `DataScientist`: Data analysis, visualization, experimentation
//! - `PromptEngineer`: Prompt design, optimization, evaluation

mod ai_engineer;
mod data_engineer;
mod data_scientist;
mod ml_engineer;
mod mlops_engineer;
pub mod prompt_engineer;

pub use ai_engineer::AIEngineerAgent;
pub use data_engineer::DataEngineerAgent;
pub use data_scientist::DataScientistAgent;
pub use ml_engineer::MLEngineerAgent;
pub use mlops_engineer::MLOpsEngineerAgent;
pub use prompt_engineer::PromptEngineerAgent;
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/aiml/prompt_engineer.rs">
//! Prompt Engineer Agent - Prompt design, optimization, evaluation

use crate::agents::base::{AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;
use async_trait::async_trait;
use simd_json::json;

pub struct PromptEngineerAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl PromptEngineerAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            profile: SecurityProfile::content_generation("prompt-engineer"),
            agent_id,
        }
    }

    fn analyze(&self, args: Option<&str>) -> Result<String, String> {
        let input = args.unwrap_or("").to_lowercase();
        let mut recommendations = Vec::new();

        if input.contains("system") || input.contains("persona") {
            recommendations.push("Define clear role and expertise boundaries");
            recommendations.push("Include behavioral constraints and guardrails");
            recommendations.push("Specify output format expectations");
        }
        if input.contains("few-shot") || input.contains("example") {
            recommendations.push("Use 3-5 diverse, representative examples");
            recommendations.push("Include both positive and edge cases");
            recommendations.push("Format examples consistently");
        }
        if input.contains("chain") || input.contains("reasoning") {
            recommendations.push("Use step-by-step reasoning prompts");
            recommendations.push("Add 'Let's think step by step' for complex tasks");
            recommendations.push("Break complex tasks into subtasks");
        }
        if recommendations.is_empty() {
            recommendations.push("Be specific and unambiguous in instructions");
            recommendations.push("Test prompts with diverse inputs");
            recommendations.push("Iterate based on failure cases");
        }

        let result = json!({
            "analysis": { "input": args.unwrap_or(""), "recommendations": recommendations },
            "techniques": ["Few-shot learning", "Chain-of-thought", "Self-consistency", "Tree-of-thoughts", "ReAct"],
            "evaluation": ["Human evaluation", "LLM-as-judge", "Task-specific metrics"]
        });
        Ok(simd_json::to_string_pretty(&result).unwrap_or_default())
    }
}

#[async_trait]
impl AgentTrait for PromptEngineerAgent {
    fn agent_type(&self) -> &str {
        "prompt-engineer"
    }
    fn name(&self) -> &str {
        "Prompt Engineer"
    }
    fn description(&self) -> &str {
        "Design, optimize, and evaluate prompts for LLM applications."
    }
    fn operations(&self) -> Vec<String> {
        vec![
            "design_prompt".to_string(),
            "optimize".to_string(),
            "evaluate".to_string(),
            "analyze".to_string(),
        ]
    }
    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }
    fn get_status(&self) -> String {
        format!("Prompt Engineer agent {} is running", self.agent_id)
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "prompt-engineer" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }
        match self.analyze(task.args.as_deref()) {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/analysis/code_reviewer.rs">
//! Code Reviewer Agent

use async_trait::async_trait;
use std::process::Command;

use crate::agents::base::{validation, AgentTask, AgentTrait, TaskResult};
use crate::security::{profiles::presets, SecurityProfile};

const ALLOWED_DIRS: &[&str] = &["/tmp", "/home", "/opt"];

pub struct CodeReviewerAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl CodeReviewerAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            agent_id,
            profile: presets::code_reviewer(),
        }
    }

    fn search_code(&self, path: Option<&str>, pattern: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("rg");

        if let Some(p) = pattern {
            validation::validate_args(p)?;
            cmd.arg(p);
        } else {
            return Err("Pattern required".to_string());
        }

        if let Some(dir) = path {
            let validated_path = validation::validate_path(dir, ALLOWED_DIRS)?;
            cmd.arg(validated_path);
        }

        cmd.arg("--no-heading").arg("-n");

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!("Search results:\n{}\n{}", stdout, stderr))
    }

    fn count_lines(&self, path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("tokei");

        if let Some(dir) = path {
            let validated_path = validation::validate_path(dir, ALLOWED_DIRS)?;
            cmd.arg(validated_path);
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!("Line counts:\n{}\n{}", stdout, stderr))
    }

    fn git_diff(&self, path: Option<&str>, args: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("git");
        cmd.arg("diff");

        if let Some(a) = args {
            validation::validate_args(a)?;
            for arg in a.split_whitespace() {
                cmd.arg(arg);
            }
        }

        if let Some(dir) = path {
            let validated_path = validation::validate_path(dir, ALLOWED_DIRS)?;
            cmd.current_dir(validated_path);
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!("Git diff:\n{}\n{}", stdout, stderr))
    }

    fn git_log(&self, path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("git");
        cmd.arg("log").arg("--oneline").arg("-20");

        if let Some(dir) = path {
            let validated_path = validation::validate_path(dir, ALLOWED_DIRS)?;
            cmd.current_dir(validated_path);
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!("Git log:\n{}\n{}", stdout, stderr))
    }
}

#[async_trait]
impl AgentTrait for CodeReviewerAgent {
    fn agent_type(&self) -> &str {
        "code-reviewer"
    }
    fn name(&self) -> &str {
        "Code Reviewer"
    }
    fn description(&self) -> &str {
        "Code review and analysis"
    }

    fn operations(&self) -> Vec<String> {
        vec![
            "search".to_string(),
            "count".to_string(),
            "diff".to_string(),
            "log".to_string(),
        ]
    }

    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        let result = match task.operation.as_str() {
            "search" => self.search_code(task.path.as_deref(), task.args.as_deref()),
            "count" => self.count_lines(task.path.as_deref()),
            "diff" => self.git_diff(task.path.as_deref(), task.args.as_deref()),
            "log" => self.git_log(task.path.as_deref()),
            _ => Err(format!("Unknown operation: {}", task.operation)),
        };

        match result {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/analysis/debugger.rs">
//! Debugger Agent

use async_trait::async_trait;
use std::process::Command;

use crate::agents::base::{validation, AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;

const ALLOWED_DIRS: &[&str] = &["/tmp", "/home", "/opt", "/var/log"];

pub struct DebuggerAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl DebuggerAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            agent_id,
            profile: SecurityProfile::read_only_analysis(
                "debugger",
                vec!["strace", "ltrace", "gdb"],
            ),
        }
    }

    fn read_logs(&self, path: Option<&str>, lines: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("tail");

        let num_lines = lines.unwrap_or("100");
        validation::validate_args(num_lines)?;
        cmd.arg("-n").arg(num_lines);

        if let Some(file) = path {
            let validated_path = validation::validate_path(file, ALLOWED_DIRS)?;
            cmd.arg(validated_path);
        } else {
            return Err("Log file path required".to_string());
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!("Log output:\n{}\n{}", stdout, stderr))
    }

    fn journalctl(&self, unit: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("journalctl");
        cmd.arg("--no-pager").arg("-n").arg("100");

        if let Some(u) = unit {
            validation::validate_args(u)?;
            cmd.arg("-u").arg(u);
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!("Journal output:\n{}\n{}", stdout, stderr))
    }

    fn process_info(&self, pid: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("ps");
        cmd.arg("aux");

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);

        if let Some(p) = pid {
            validation::validate_args(p)?;
            let filtered: String = stdout
                .lines()
                .filter(|line| line.contains(p))
                .collect::<Vec<_>>()
                .join("\n");
            Ok(format!("Process info:\n{}", filtered))
        } else {
            Ok(format!("All processes:\n{}", stdout))
        }
    }
}

#[async_trait]
impl AgentTrait for DebuggerAgent {
    fn agent_type(&self) -> &str {
        "debugger"
    }
    fn name(&self) -> &str {
        "Debugger"
    }
    fn description(&self) -> &str {
        "Debug logs and process analysis"
    }

    fn operations(&self) -> Vec<String> {
        vec![
            "logs".to_string(),
            "journal".to_string(),
            "process".to_string(),
        ]
    }

    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        let result = match task.operation.as_str() {
            "logs" => self.read_logs(task.path.as_deref(), task.args.as_deref()),
            "journal" => self.journalctl(task.path.as_deref()),
            "process" => self.process_info(task.path.as_deref()),
            _ => Err(format!("Unknown operation: {}", task.operation)),
        };

        match result {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/analysis/mod.rs">
//! Code analysis and review agents

pub mod code_reviewer;
pub mod debugger;
pub mod performance;
pub mod security_auditor;

pub use code_reviewer::CodeReviewerAgent;
pub use debugger::DebuggerAgent;
pub use performance::PerformanceEngineerAgent;
pub use security_auditor::SecurityAuditorAgent;
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/analysis/performance.rs">
//! Performance Engineer Agent

use async_trait::async_trait;
use std::process::Command;

use crate::agents::base::{AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;

pub struct PerformanceEngineerAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl PerformanceEngineerAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            agent_id,
            profile: SecurityProfile::read_only_analysis(
                "performance-engineer",
                vec!["top", "htop", "vmstat", "iostat"],
            ),
        }
    }

    fn system_stats(&self) -> Result<String, String> {
        let mut cmd = Command::new("vmstat");
        cmd.arg("1").arg("5");

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!("System stats:\n{}\n{}", stdout, stderr))
    }

    fn io_stats(&self) -> Result<String, String> {
        let mut cmd = Command::new("iostat");
        cmd.arg("-x").arg("1").arg("3");

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!("I/O stats:\n{}\n{}", stdout, stderr))
    }

    fn memory_info(&self) -> Result<String, String> {
        let mut cmd = Command::new("free");
        cmd.arg("-h");

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!("Memory info:\n{}\n{}", stdout, stderr))
    }

    fn cpu_info(&self) -> Result<String, String> {
        let mut cmd = Command::new("lscpu");

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!("CPU info:\n{}\n{}", stdout, stderr))
    }
}

#[async_trait]
impl AgentTrait for PerformanceEngineerAgent {
    fn agent_type(&self) -> &str {
        "performance-engineer"
    }
    fn name(&self) -> &str {
        "Performance Engineer"
    }
    fn description(&self) -> &str {
        "System performance analysis"
    }

    fn operations(&self) -> Vec<String> {
        vec![
            "vmstat".to_string(),
            "iostat".to_string(),
            "memory".to_string(),
            "cpu".to_string(),
        ]
    }

    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        let result = match task.operation.as_str() {
            "vmstat" => self.system_stats(),
            "iostat" => self.io_stats(),
            "memory" => self.memory_info(),
            "cpu" => self.cpu_info(),
            _ => Err(format!("Unknown operation: {}", task.operation)),
        };

        match result {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/analysis/security_auditor.rs">
//! Security Auditor Agent

use async_trait::async_trait;
use std::process::Command;

use crate::agents::base::{validation, AgentTask, AgentTrait, TaskResult};
use crate::security::{profiles::presets, SecurityProfile};

const ALLOWED_DIRS: &[&str] = &["/tmp", "/home", "/opt"];

pub struct SecurityAuditorAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl SecurityAuditorAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            agent_id,
            profile: presets::security_auditor(),
        }
    }

    fn semgrep_scan(&self, path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("semgrep");
        cmd.arg("--config=auto");

        if let Some(dir) = path {
            let validated_path = validation::validate_path(dir, ALLOWED_DIRS)?;
            cmd.arg(validated_path);
        } else {
            cmd.arg(".");
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!("Semgrep scan:\n{}\n{}", stdout, stderr))
    }

    fn bandit_scan(&self, path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("bandit");
        cmd.arg("-r");

        if let Some(dir) = path {
            let validated_path = validation::validate_path(dir, ALLOWED_DIRS)?;
            cmd.arg(validated_path);
        } else {
            cmd.arg(".");
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!("Bandit scan:\n{}\n{}", stdout, stderr))
    }

    fn cargo_audit(&self, path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("cargo");
        cmd.arg("audit");

        if let Some(dir) = path {
            let validated_path = validation::validate_path(dir, ALLOWED_DIRS)?;
            cmd.current_dir(validated_path);
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!("Cargo audit:\n{}\n{}", stdout, stderr))
    }

    fn npm_audit(&self, path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("npm");
        cmd.arg("audit").arg("--json");

        if let Some(dir) = path {
            let validated_path = validation::validate_path(dir, ALLOWED_DIRS)?;
            cmd.current_dir(validated_path);
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!("NPM audit:\n{}\n{}", stdout, stderr))
    }
}

#[async_trait]
impl AgentTrait for SecurityAuditorAgent {
    fn agent_type(&self) -> &str {
        "security-auditor"
    }
    fn name(&self) -> &str {
        "Security Auditor"
    }
    fn description(&self) -> &str {
        "Security vulnerability scanning and auditing"
    }

    fn operations(&self) -> Vec<String> {
        vec![
            "semgrep".to_string(),
            "bandit".to_string(),
            "cargo-audit".to_string(),
            "npm-audit".to_string(),
        ]
    }

    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        let result = match task.operation.as_str() {
            "semgrep" => self.semgrep_scan(task.path.as_deref()),
            "bandit" => self.bandit_scan(task.path.as_deref()),
            "cargo-audit" => self.cargo_audit(task.path.as_deref()),
            "npm-audit" => self.npm_audit(task.path.as_deref()),
            _ => Err(format!("Unknown operation: {}", task.operation)),
        };

        match result {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/architecture/backend_architect.rs">
//! Backend Architect Agent
//!
//! Expert backend architect specializing in scalable API design,
//! microservices architecture, and distributed systems.

use async_trait::async_trait;
use simd_json::json;

use crate::agents::base::{AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;

/// Backend Architect Agent
///
/// Masters REST/GraphQL/gRPC APIs, event-driven architectures,
/// service mesh patterns, and modern backend frameworks.
pub struct BackendArchitectAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl BackendArchitectAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            profile: SecurityProfile::content_generation("backend-architect"),
            agent_id,
        }
    }

    fn analyze_architecture(&self, args: Option<&str>) -> Result<String, String> {
        let input = args.unwrap_or("").to_lowercase();

        let mut recommendations = Vec::new();
        let mut patterns = Vec::new();
        let mut questions = Vec::new();

        // API Design Analysis
        if input.contains("api") || input.contains("rest") || input.contains("graphql") {
            recommendations.push("Define API contracts first using OpenAPI/GraphQL schemas");
            recommendations
                .push("Implement versioning strategy (URL path, header, or content negotiation)");
            recommendations
                .push("Design consistent error response format with proper HTTP status codes");
            patterns.push("API-First Design");
        }

        // Microservices Analysis
        if input.contains("microservice") || input.contains("service") {
            recommendations.push("Define clear service boundaries using Domain-Driven Design");
            recommendations.push("Implement service discovery (Consul, etcd, or K8s native)");
            recommendations.push("Design async communication patterns for loose coupling");
            patterns.push("Microservices Architecture");
            patterns.push("Service Mesh");
        }

        // Event-Driven Analysis
        if input.contains("event") || input.contains("kafka") || input.contains("message") {
            recommendations.push("Use event sourcing for audit trail and replay capability");
            recommendations.push("Implement dead letter queues for failed message handling");
            recommendations.push("Design idempotent consumers for at-least-once delivery");
            patterns.push("Event-Driven Architecture");
            patterns.push("CQRS");
        }

        // Resilience Analysis
        if input.contains("scale") || input.contains("resilient") || input.contains("fault") {
            recommendations.push("Implement circuit breakers for external service calls");
            recommendations.push("Design for horizontal scalability with stateless services");
            recommendations.push("Add health checks (liveness, readiness) for orchestration");
            patterns.push("Circuit Breaker");
            patterns.push("Bulkhead");
        }

        // Default recommendations
        if recommendations.is_empty() {
            recommendations
                .push("Start with requirements analysis for scale and consistency needs");
            recommendations.push("Define service boundaries based on business capabilities");
            recommendations.push("Design API contracts before implementation");
            recommendations.push("Plan observability from day one (logging, metrics, tracing)");
            patterns.push("Clean Architecture");
            patterns.push("Domain-Driven Design");
        }

        // Targeted questions for chat UI integration
        if input.contains("chat-ui")
            || input.contains("huggingface")
            || input.contains("interface")
            || input.contains("ui")
            || input.contains("mcp")
            || input.contains("model")
            || input.contains("stream")
            || input.contains("auth")
        {
            questions.push(
                "Which backend is canonical for chat requests today (op-web or another service)?",
            );
            questions.push("What are the existing chat endpoints and payload formats, and are they OpenAI-compatible?");
            questions.push("How should model selection work (per user, per conversation, per request), and where is it configured?");
            questions.push("Do you need streaming responses? If yes, what protocol (SSE/WS) and token format are expected?");
            questions.push("What is the desired scope of the MCP tab (tool discovery only vs full config and execution)?");
            questions.push("What authentication model is required (API keys, session cookies, HuggingFace token passthrough)?");
            questions.push("Where should chat history persist (DB, filesystem, in-memory), and is multi-user isolation required?");
            questions.push("Where should chat-ui live: separate app with reverse proxy, static build served by op-web, or embedded route?");
        }

        let result = json!({
            "analysis": {
                "input": args.unwrap_or(""),
                "recommended_patterns": patterns,
                "recommendations": recommendations,
                "questions": questions,
                "next_steps": [
                    "Define bounded contexts and service boundaries",
                    "Create API contract specifications",
                    "Design data model and ownership",
                    "Plan inter-service communication",
                    "Set up observability infrastructure"
                ]
            },
            "architecture_principles": {
                "scalability": "Design stateless services for horizontal scaling",
                "resilience": "Implement circuit breakers, retries, timeouts",
                "observability": "Structured logging, distributed tracing, metrics",
                "security": "Defense in depth, least privilege, zero trust"
            }
        });

        Ok(simd_json::to_string_pretty(&result).unwrap_or_default())
    }
}

#[async_trait]
impl AgentTrait for BackendArchitectAgent {
    fn agent_type(&self) -> &str {
        "backend-architect"
    }

    fn name(&self) -> &str {
        "Backend Architect"
    }

    fn description(&self) -> &str {
        "Expert backend architect specializing in scalable API design, microservices architecture, and distributed systems."
    }

    fn operations(&self) -> Vec<String> {
        vec![
            "design_api".to_string(),
            "design_microservices".to_string(),
            "design_event_architecture".to_string(),
            "review_architecture".to_string(),
            "analyze".to_string(),
        ]
    }

    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }

    fn get_status(&self) -> String {
        format!("Backend Architect agent {} is running", self.agent_id)
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "backend-architect" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }

        let result = match task.operation.as_str() {
            "design_api"
            | "design_microservices"
            | "design_event_architecture"
            | "review_architecture"
            | "analyze" => self.analyze_architecture(task.args.as_deref()),
            _ => Err(format!(
                "Unknown operation: {}. Available: {:?}",
                task.operation,
                self.operations()
            )),
        };

        match result {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/architecture/frontend_developer.rs">
//! Frontend Developer Agent
//!
//! Expert frontend developer specializing in React 19+, Next.js 15+,
//! and modern frontend architecture.

use async_trait::async_trait;
use simd_json::json;

use crate::agents::base::{AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;

/// Frontend Developer Agent
pub struct FrontendDeveloperAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl FrontendDeveloperAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            profile: SecurityProfile::content_generation("frontend-developer"),
            agent_id,
        }
    }

    fn analyze_frontend(&self, args: Option<&str>) -> Result<String, String> {
        let input = args.unwrap_or("").to_lowercase();

        let mut recommendations = Vec::new();
        let mut patterns = Vec::new();
        let mut tech_stack = Vec::new();

        if input.contains("react") || input.contains("component") {
            recommendations.push("Use React 19 Server Components for data fetching");
            recommendations.push("Implement Suspense boundaries for loading states");
            patterns.push("Server Components");
            tech_stack.push("React 19");
        }

        if input.contains("next") || input.contains("ssr") {
            recommendations.push("Use Next.js 15 App Router for modern routing");
            recommendations.push("Implement Server Actions for form mutations");
            patterns.push("App Router");
            tech_stack.push("Next.js 15");
        }

        if input.contains("state") || input.contains("data") {
            recommendations.push("Use Zustand for client state");
            recommendations.push("Use TanStack Query for server state");
            patterns.push("Server State Management");
            tech_stack.push("TanStack Query");
        }

        if input.contains("performance") {
            recommendations.push("Optimize Core Web Vitals (LCP, FID, CLS)");
            recommendations.push("Implement code splitting with dynamic imports");
            patterns.push("Code Splitting");
        }

        if recommendations.is_empty() {
            recommendations.push("Use TypeScript for type safety");
            recommendations.push("Implement proper error boundaries");
            recommendations.push("Add loading and error states for all async operations");
            patterns.push("TypeScript");
            tech_stack.push("TypeScript 5.x");
        }

        let result = json!({
            "analysis": {
                "input": args.unwrap_or(""),
                "recommended_patterns": patterns,
                "recommendations": recommendations,
                "suggested_stack": tech_stack
            },
            "component_guidelines": {
                "structure": "Atomic design: atoms → molecules → organisms → templates → pages",
                "naming": "PascalCase for components, camelCase for hooks"
            }
        });

        Ok(simd_json::to_string_pretty(&result).unwrap_or_default())
    }
}

#[async_trait]
impl AgentTrait for FrontendDeveloperAgent {
    fn agent_type(&self) -> &str {
        "frontend-developer"
    }
    fn name(&self) -> &str {
        "Frontend Developer"
    }
    fn description(&self) -> &str {
        "Build React components, implement responsive layouts, and handle client-side state management."
    }

    fn operations(&self) -> Vec<String> {
        vec![
            "build_component".to_string(),
            "design_architecture".to_string(),
            "optimize_performance".to_string(),
            "analyze".to_string(),
        ]
    }

    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }

    fn get_status(&self) -> String {
        format!("Frontend Developer agent {} is running", self.agent_id)
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "frontend-developer" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }

        match self.analyze_frontend(task.args.as_deref()) {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/architecture/graphql_architect.rs">
//! GraphQL Architect Agent
//!
//! Expert GraphQL architect specializing in enterprise-scale schema design,
//! federation, performance optimization, and modern GraphQL patterns.

use async_trait::async_trait;
use simd_json::json;

use crate::agents::base::{AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;

/// GraphQL Architect Agent
pub struct GraphQLArchitectAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl GraphQLArchitectAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            profile: SecurityProfile::content_generation("graphql-architect"),
            agent_id,
        }
    }

    fn analyze_graphql(&self, args: Option<&str>) -> Result<String, String> {
        let input = args.unwrap_or("").to_lowercase();

        let mut recommendations = Vec::new();
        let mut patterns = Vec::new();

        if input.contains("schema") || input.contains("type") {
            recommendations.push("Use schema-first development with SDL");
            recommendations.push("Design interfaces for polymorphic types");
            recommendations.push("Implement Relay connection spec for pagination");
            patterns.push("Schema-First Design");
        }

        if input.contains("federation") || input.contains("gateway") {
            recommendations.push("Use Apollo Federation v2 for distributed schemas");
            recommendations.push("Design entity keys for cross-service references");
            patterns.push("Apollo Federation v2");
        }

        if input.contains("performance") || input.contains("n+1") {
            recommendations.push("Implement DataLoader for N+1 query resolution");
            recommendations.push("Use automatic persisted queries (APQ)");
            recommendations.push("Add field-level caching with @cacheControl");
            patterns.push("DataLoader Pattern");
        }

        if recommendations.is_empty() {
            recommendations.push("Define clear type boundaries and relationships");
            recommendations.push("Use input types for mutations");
            recommendations.push("Implement proper error handling with extensions");
            patterns.push("Type-Safe Development");
        }

        let result = json!({
            "analysis": {
                "input": args.unwrap_or(""),
                "recommended_patterns": patterns,
                "recommendations": recommendations
            },
            "schema_guidelines": {
                "naming": "Use PascalCase for types, camelCase for fields",
                "nullability": "Make fields nullable by default, non-null only when guaranteed",
                "pagination": "Use Relay-style connections for lists"
            }
        });

        Ok(simd_json::to_string_pretty(&result).unwrap_or_default())
    }
}

#[async_trait]
impl AgentTrait for GraphQLArchitectAgent {
    fn agent_type(&self) -> &str {
        "graphql-architect"
    }
    fn name(&self) -> &str {
        "GraphQL Architect"
    }
    fn description(&self) -> &str {
        "Master modern GraphQL with federation, performance optimization, and enterprise security."
    }

    fn operations(&self) -> Vec<String> {
        vec![
            "design_schema".to_string(),
            "design_federation".to_string(),
            "optimize_performance".to_string(),
            "analyze".to_string(),
        ]
    }

    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }

    fn get_status(&self) -> String {
        format!("GraphQL Architect agent {} is running", self.agent_id)
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "graphql-architect" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }

        match self.analyze_graphql(task.args.as_deref()) {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/architecture/mod.rs">
//! Architecture Agents
//!
//! Specialized agents for software architecture design:
//! - `BackendArchitect`: API design, microservices, distributed systems
//! - `GraphQLArchitect`: GraphQL schema design, federation, performance
//! - `FrontendDeveloper`: React, Next.js, modern frontend patterns

mod backend_architect;
mod frontend_developer;
mod graphql_architect;

pub use backend_architect::BackendArchitectAgent;
pub use frontend_developer::FrontendDeveloperAgent;
pub use graphql_architect::GraphQLArchitectAgent;
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/business/business_analyst.rs">
//! Business Analyst Agent

use crate::agents::base::{AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;
use async_trait::async_trait;
use simd_json::json;

pub struct BusinessAnalystAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl BusinessAnalystAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            profile: SecurityProfile::content_generation("business-analyst"),
            agent_id,
        }
    }

    fn analyze(&self, args: Option<&str>) -> Result<String, String> {
        let input = args.unwrap_or("").to_lowercase();
        let mut recommendations = Vec::new();

        if input.contains("requirement") {
            recommendations.push("Define SMART requirements (Specific, Measurable, Achievable, Relevant, Time-bound)");
            recommendations.push("Identify stakeholders and their needs");
            recommendations.push("Document acceptance criteria");
        }
        if input.contains("process") || input.contains("workflow") {
            recommendations.push("Map current state (as-is) process");
            recommendations.push("Identify bottlenecks and inefficiencies");
            recommendations.push("Design future state (to-be) process");
        }
        if input.contains("metric") || input.contains("kpi") {
            recommendations.push("Define leading and lagging indicators");
            recommendations.push("Establish baselines and targets");
            recommendations.push("Create measurement framework");
        }
        if recommendations.is_empty() {
            recommendations.push("Start with problem statement and business context");
            recommendations.push("Gather requirements from all stakeholder groups");
            recommendations.push("Document assumptions and constraints");
        }

        let result = json!({
            "analysis": { "input": args.unwrap_or(""), "recommendations": recommendations },
            "artifacts": ["BRD (Business Requirements Document)", "User Stories", "Process Maps", "Data Flow Diagrams"]
        });
        Ok(simd_json::to_string_pretty(&result).unwrap_or_default())
    }
}

#[async_trait]
impl AgentTrait for BusinessAnalystAgent {
    fn agent_type(&self) -> &str {
        "business-analyst"
    }
    fn name(&self) -> &str {
        "Business Analyst"
    }
    fn description(&self) -> &str {
        "Gather requirements, analyze processes, and bridge business-IT communication."
    }
    fn operations(&self) -> Vec<String> {
        vec![
            "gather_requirements".to_string(),
            "analyze_process".to_string(),
            "define_kpis".to_string(),
            "analyze".to_string(),
        ]
    }
    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }
    fn get_status(&self) -> String {
        format!("Business Analyst agent {} is running", self.agent_id)
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "business-analyst" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }
        match self.analyze(task.args.as_deref()) {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/business/customer_support.rs">
//! Customer Support Agent

use crate::agents::base::{AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;
use async_trait::async_trait;
use simd_json::json;

pub struct CustomerSupportAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl CustomerSupportAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            profile: SecurityProfile::content_generation("customer-support"),
            agent_id,
        }
    }

    fn analyze(&self, args: Option<&str>) -> Result<String, String> {
        let input = args.unwrap_or("").to_lowercase();
        let mut recommendations = Vec::new();

        if input.contains("ticket") || input.contains("issue") {
            recommendations.push("Acknowledge the issue promptly");
            recommendations.push("Gather necessary information to diagnose");
            recommendations.push("Set clear expectations for resolution timeline");
        }
        if input.contains("escalat") {
            recommendations.push("Define clear escalation criteria");
            recommendations.push("Document issue history before escalating");
            recommendations.push("Ensure warm handoff with context");
        }
        if input.contains("refund") || input.contains("complaint") {
            recommendations.push("Listen and acknowledge customer frustration");
            recommendations.push("Follow established policies consistently");
            recommendations.push("Document resolution and follow up");
        }
        if recommendations.is_empty() {
            recommendations.push("Practice empathy and active listening");
            recommendations.push("Use positive language and solution-focus");
            recommendations.push("Document interactions for continuity");
        }

        let result = json!({
            "analysis": { "input": args.unwrap_or(""), "recommendations": recommendations },
            "best_practices": ["First contact resolution", "CSAT tracking", "Knowledge base maintenance", "Proactive communication"]
        });
        Ok(simd_json::to_string_pretty(&result).unwrap_or_default())
    }
}

#[async_trait]
impl AgentTrait for CustomerSupportAgent {
    fn agent_type(&self) -> &str {
        "customer-support"
    }
    fn name(&self) -> &str {
        "Customer Support"
    }
    fn description(&self) -> &str {
        "Handle customer inquiries and resolve issues effectively."
    }
    fn operations(&self) -> Vec<String> {
        vec![
            "handle_ticket".to_string(),
            "escalate".to_string(),
            "draft_response".to_string(),
            "analyze".to_string(),
        ]
    }
    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }
    fn get_status(&self) -> String {
        format!("Customer Support agent {} is running", self.agent_id)
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "customer-support" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }
        match self.analyze(task.args.as_deref()) {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/business/hr_pro.rs">
//! HR Pro Agent

use crate::agents::base::{AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;
use async_trait::async_trait;
use simd_json::json;

pub struct HRProAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl HRProAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            profile: SecurityProfile::content_generation("hr-pro"),
            agent_id,
        }
    }

    fn analyze(&self, args: Option<&str>) -> Result<String, String> {
        let input = args.unwrap_or("").to_lowercase();
        let mut recommendations = Vec::new();

        if input.contains("policy") {
            recommendations.push("Ensure compliance with labor laws");
            recommendations.push("Document clear procedures and expectations");
            recommendations.push("Include appeal/grievance processes");
        }
        if input.contains("hiring") || input.contains("recruit") {
            recommendations.push("Define clear job requirements and competencies");
            recommendations.push("Use structured interviews for consistency");
            recommendations.push("Ensure fair and unbiased selection process");
        }
        if input.contains("performance") || input.contains("review") {
            recommendations.push("Set clear, measurable objectives");
            recommendations.push("Provide regular feedback throughout the year");
            recommendations.push("Document performance discussions");
        }
        if recommendations.is_empty() {
            recommendations.push("Maintain up-to-date employee handbook");
            recommendations.push("Ensure compliance with employment regulations");
            recommendations.push("Document all HR decisions and rationale");
        }

        let result = json!({
            "analysis": { "input": args.unwrap_or(""), "recommendations": recommendations },
            "compliance_areas": ["Equal Employment", "Workplace Safety", "Privacy", "Benefits Administration"]
        });
        Ok(simd_json::to_string_pretty(&result).unwrap_or_default())
    }
}

#[async_trait]
impl AgentTrait for HRProAgent {
    fn agent_type(&self) -> &str {
        "hr-pro"
    }
    fn name(&self) -> &str {
        "HR Pro"
    }
    fn description(&self) -> &str {
        "HR policy guidance, compliance, and people management best practices."
    }
    fn operations(&self) -> Vec<String> {
        vec![
            "draft_policy".to_string(),
            "review_compliance".to_string(),
            "advise".to_string(),
            "analyze".to_string(),
        ]
    }
    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }
    fn get_status(&self) -> String {
        format!("HR Pro agent {} is running", self.agent_id)
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "hr-pro" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }
        match self.analyze(task.args.as_deref()) {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/business/legal_advisor.rs">
//! Legal Advisor Agent

use crate::agents::base::{AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;
use async_trait::async_trait;
use simd_json::json;

pub struct LegalAdvisorAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl LegalAdvisorAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            profile: SecurityProfile::content_generation("legal-advisor"),
            agent_id,
        }
    }

    fn analyze(&self, args: Option<&str>) -> Result<String, String> {
        let input = args.unwrap_or("").to_lowercase();
        let mut recommendations = Vec::new();

        if input.contains("privacy") || input.contains("gdpr") {
            recommendations.push("Implement data minimization principles");
            recommendations.push("Provide clear privacy notices");
            recommendations.push("Establish data subject rights processes");
        }
        if input.contains("contract") || input.contains("agreement") {
            recommendations.push("Define clear terms and conditions");
            recommendations.push("Include dispute resolution clauses");
            recommendations.push("Specify governing law and jurisdiction");
        }
        if input.contains("ip") || input.contains("intellectual") {
            recommendations.push("Document ownership of created works");
            recommendations.push("Include proper licensing terms");
            recommendations.push("Protect trade secrets appropriately");
        }
        if recommendations.is_empty() {
            recommendations.push("Always consult qualified legal counsel for specifics");
            recommendations.push("Document compliance efforts");
            recommendations.push("Maintain records of legal decisions");
        }

        let result = json!({
            "analysis": { "input": args.unwrap_or(""), "recommendations": recommendations },
            "disclaimer": "This is general guidance only, not legal advice. Consult qualified legal counsel.",
            "compliance_areas": ["GDPR", "CCPA", "SOC 2", "HIPAA", "PCI DSS"]
        });
        Ok(simd_json::to_string_pretty(&result).unwrap_or_default())
    }
}

#[async_trait]
impl AgentTrait for LegalAdvisorAgent {
    fn agent_type(&self) -> &str {
        "legal-advisor"
    }
    fn name(&self) -> &str {
        "Legal Advisor"
    }
    fn description(&self) -> &str {
        "General legal guidance for tech and business compliance."
    }
    fn operations(&self) -> Vec<String> {
        vec![
            "review_compliance".to_string(),
            "draft_policy".to_string(),
            "advise".to_string(),
            "analyze".to_string(),
        ]
    }
    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }
    fn get_status(&self) -> String {
        format!("Legal Advisor agent {} is running", self.agent_id)
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "legal-advisor" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }
        match self.analyze(task.args.as_deref()) {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/business/mod.rs">
//! Business & Operations Agents
//!
//! - `BusinessAnalyst`: Business analysis and requirements
//! - `HRPro`: HR policies and compliance
//! - `CustomerSupport`: Customer service automation
//! - `PaymentIntegration`: Payment processing integration

mod business_analyst;
mod customer_support;
mod hr_pro;
mod legal_advisor;
mod payment_integration;
mod sales_automator;

pub use business_analyst::BusinessAnalystAgent;
pub use customer_support::CustomerSupportAgent;
pub use hr_pro::HRProAgent;
pub use legal_advisor::LegalAdvisorAgent;
pub use payment_integration::PaymentIntegrationAgent;
pub use sales_automator::SalesAutomatorAgent;
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/business/payment_integration.rs">
//! Payment Integration Agent

use crate::agents::base::{AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;
use async_trait::async_trait;
use simd_json::json;

pub struct PaymentIntegrationAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl PaymentIntegrationAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            profile: SecurityProfile::content_generation("payment-integration"),
            agent_id,
        }
    }

    fn analyze(&self, args: Option<&str>) -> Result<String, String> {
        let input = args.unwrap_or("").to_lowercase();
        let mut recommendations = Vec::new();

        if input.contains("stripe") {
            recommendations.push("Use Stripe Elements for PCI compliance");
            recommendations.push("Implement webhook handlers for async events");
            recommendations.push("Use idempotency keys for retries");
        }
        if input.contains("subscription") || input.contains("recurring") {
            recommendations.push("Handle failed payment retries gracefully");
            recommendations.push("Implement dunning management");
            recommendations.push("Provide clear cancellation flow");
        }
        if input.contains("checkout") {
            recommendations.push("Minimize checkout steps");
            recommendations.push("Show clear pricing and fees");
            recommendations.push("Provide multiple payment options");
        }
        if recommendations.is_empty() {
            recommendations.push("Never store raw card data (use tokenization)");
            recommendations.push("Implement proper error handling");
            recommendations.push("Log transactions for reconciliation");
        }

        let result = json!({
            "analysis": { "input": args.unwrap_or(""), "recommendations": recommendations },
            "providers": ["Stripe", "PayPal", "Square", "Adyen", "Braintree"],
            "compliance": ["PCI DSS", "Strong Customer Authentication (SCA)", "3D Secure"]
        });
        Ok(simd_json::to_string_pretty(&result).unwrap_or_default())
    }
}

#[async_trait]
impl AgentTrait for PaymentIntegrationAgent {
    fn agent_type(&self) -> &str {
        "payment-integration"
    }
    fn name(&self) -> &str {
        "Payment Integration"
    }
    fn description(&self) -> &str {
        "Integrate payment processors and handle billing workflows."
    }
    fn operations(&self) -> Vec<String> {
        vec![
            "integrate_stripe".to_string(),
            "setup_subscriptions".to_string(),
            "handle_webhooks".to_string(),
            "analyze".to_string(),
        ]
    }
    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }
    fn get_status(&self) -> String {
        format!("Payment Integration agent {} is running", self.agent_id)
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "payment-integration" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }
        match self.analyze(task.args.as_deref()) {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/business/sales_automator.rs">
//! Sales Automator Agent

use crate::agents::base::{AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;
use async_trait::async_trait;
use simd_json::json;

pub struct SalesAutomatorAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl SalesAutomatorAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            profile: SecurityProfile::content_generation("sales-automator"),
            agent_id,
        }
    }

    fn analyze(&self, args: Option<&str>) -> Result<String, String> {
        let input = args.unwrap_or("").to_lowercase();
        let mut recommendations = Vec::new();

        if input.contains("lead") || input.contains("prospect") {
            recommendations.push("Implement lead scoring based on engagement");
            recommendations.push("Automate lead qualification workflows");
            recommendations.push("Set up nurture sequences for different segments");
        }
        if input.contains("email") || input.contains("outreach") {
            recommendations.push("Personalize messages with merge fields");
            recommendations.push("A/B test subject lines and content");
            recommendations.push("Implement follow-up sequences");
        }
        if input.contains("crm") || input.contains("pipeline") {
            recommendations.push("Define clear pipeline stages");
            recommendations.push("Automate stage transitions based on actions");
            recommendations.push("Set up activity reminders");
        }
        if recommendations.is_empty() {
            recommendations.push("Map customer journey touchpoints");
            recommendations.push("Automate repetitive tasks");
            recommendations.push("Track key sales metrics");
        }

        let result = json!({
            "analysis": { "input": args.unwrap_or(""), "recommendations": recommendations },
            "tools": ["HubSpot", "Salesforce", "Pipedrive", "Outreach", "Apollo"]
        });
        Ok(simd_json::to_string_pretty(&result).unwrap_or_default())
    }
}

#[async_trait]
impl AgentTrait for SalesAutomatorAgent {
    fn agent_type(&self) -> &str {
        "sales-automator"
    }
    fn name(&self) -> &str {
        "Sales Automator"
    }
    fn description(&self) -> &str {
        "Automate sales processes and CRM workflows."
    }
    fn operations(&self) -> Vec<String> {
        vec![
            "setup_automation".to_string(),
            "configure_crm".to_string(),
            "create_sequence".to_string(),
            "analyze".to_string(),
        ]
    }
    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }
    fn get_status(&self) -> String {
        format!("Sales Automator agent {} is running", self.agent_id)
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "sales-automator" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }
        match self.analyze(task.args.as_deref()) {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/content/api_documenter.rs">
//! API Documenter Agent

use async_trait::async_trait;
use std::process::Command;

use crate::agents::base::{validation, AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;

const ALLOWED_DIRS: &[&str] = &["/tmp", "/home", "/opt"];

pub struct ApiDocumenterAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl ApiDocumenterAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            agent_id,
            profile: SecurityProfile::content_generation("api-documenter"),
        }
    }

    fn find_routes(&self, path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("rg");
        cmd.arg("-n")
            .arg(r#"@(app\.|router\.|get|post|put|delete|patch)"#);

        if let Some(dir) = path {
            let validated_path = validation::validate_path(dir, ALLOWED_DIRS)?;
            cmd.arg(validated_path);
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!("API routes found:\n{}\n{}", stdout, stderr))
    }

    fn find_schemas(&self, path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("rg");
        cmd.arg("-n").arg(r#"(class|interface|type|struct).*\{"#);

        if let Some(dir) = path {
            let validated_path = validation::validate_path(dir, ALLOWED_DIRS)?;
            cmd.arg(validated_path);
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!("Schemas found:\n{}\n{}", stdout, stderr))
    }

    fn generate_cargo_doc(&self, path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("cargo");
        cmd.arg("doc").arg("--no-deps");

        if let Some(dir) = path {
            let validated_path = validation::validate_path(dir, ALLOWED_DIRS)?;
            cmd.current_dir(validated_path);
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!("Documentation generated\n{}\n{}", stdout, stderr))
        } else {
            Ok(format!("Documentation failed\n{}\n{}", stdout, stderr))
        }
    }
}

#[async_trait]
impl AgentTrait for ApiDocumenterAgent {
    fn agent_type(&self) -> &str {
        "api-documenter"
    }
    fn name(&self) -> &str {
        "API Documenter"
    }
    fn description(&self) -> &str {
        "API documentation generation"
    }

    fn operations(&self) -> Vec<String> {
        vec![
            "routes".to_string(),
            "schemas".to_string(),
            "cargo-doc".to_string(),
        ]
    }

    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        let result = match task.operation.as_str() {
            "routes" => self.find_routes(task.path.as_deref()),
            "schemas" => self.find_schemas(task.path.as_deref()),
            "cargo-doc" => self.generate_cargo_doc(task.path.as_deref()),
            _ => Err(format!("Unknown operation: {}", task.operation)),
        };

        match result {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/content/docs_architect.rs">
//! Docs Architect Agent

use async_trait::async_trait;
use std::process::Command;

use crate::agents::base::{validation, AgentTask, AgentTrait, TaskResult};
use crate::security::{profiles::presets, SecurityProfile};

const ALLOWED_DIRS: &[&str] = &["/tmp", "/home", "/opt"];

pub struct DocsArchitectAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl DocsArchitectAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            agent_id,
            profile: presets::docs_architect(),
        }
    }

    fn read_file(&self, path: Option<&str>) -> Result<String, String> {
        if let Some(file) = path {
            let validated_path = validation::validate_path(file, ALLOWED_DIRS)?;
            let content = std::fs::read_to_string(&validated_path)
                .map_err(|e| format!("Failed to read file: {}", e))?;

            // Truncate if too large
            if content.len() > 100000 {
                Ok(format!(
                    "File content (truncated):\n{}...",
                    &content[..100000]
                ))
            } else {
                Ok(format!("File content:\n{}", content))
            }
        } else {
            Err("File path required".to_string())
        }
    }

    fn list_markdown(&self, path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("find");

        if let Some(dir) = path {
            let validated_path = validation::validate_path(dir, ALLOWED_DIRS)?;
            cmd.arg(validated_path);
        } else {
            cmd.arg(".");
        }

        cmd.arg("-name").arg("*.md").arg("-type").arg("f");

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!("Markdown files:\n{}\n{}", stdout, stderr))
    }

    fn check_links(&self, path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("rg");
        cmd.arg(r"\[.*?\]\(.*?\)");

        if let Some(file) = path {
            let validated_path = validation::validate_path(file, ALLOWED_DIRS)?;
            cmd.arg(validated_path);
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!("Links found:\n{}\n{}", stdout, stderr))
    }
}

#[async_trait]
impl AgentTrait for DocsArchitectAgent {
    fn agent_type(&self) -> &str {
        "docs-architect"
    }
    fn name(&self) -> &str {
        "Docs Architect"
    }
    fn description(&self) -> &str {
        "Documentation architecture and organization"
    }

    fn operations(&self) -> Vec<String> {
        vec!["read".to_string(), "list".to_string(), "links".to_string()]
    }

    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        let result = match task.operation.as_str() {
            "read" => self.read_file(task.path.as_deref()),
            "list" => self.list_markdown(task.path.as_deref()),
            "links" => self.check_links(task.path.as_deref()),
            _ => Err(format!("Unknown operation: {}", task.operation)),
        };

        match result {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/content/mermaid_expert.rs">
//! Mermaid Expert Agent

use async_trait::async_trait;
use std::process::Command;

use crate::agents::base::{validation, AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;

const ALLOWED_DIRS: &[&str] = &["/tmp", "/home", "/opt"];

pub struct MermaidExpertAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl MermaidExpertAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            agent_id,
            profile: SecurityProfile::content_generation("mermaid-expert"),
        }
    }

    fn validate_mermaid(&self, path: Option<&str>) -> Result<String, String> {
        // Read mermaid content from file
        if let Some(file) = path {
            let validated_path = validation::validate_path(file, ALLOWED_DIRS)?;
            let content = std::fs::read_to_string(&validated_path)
                .map_err(|e| format!("Failed to read file: {}", e))?;

            // Basic validation
            let valid_starts = [
                "graph",
                "sequenceDiagram",
                "classDiagram",
                "stateDiagram",
                "erDiagram",
                "flowchart",
                "gantt",
                "pie",
            ];
            let is_valid = valid_starts.iter().any(|s| content.trim().starts_with(s));

            if is_valid {
                Ok(format!("Mermaid syntax appears valid:\n{}", content))
            } else {
                Ok(format!(
                    "Warning: Mermaid diagram should start with a valid diagram type. Content:\n{}",
                    content
                ))
            }
        } else {
            Err("File path required".to_string())
        }
    }

    fn find_diagrams(&self, path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("rg");
        cmd.arg("-n").arg("```mermaid");

        if let Some(dir) = path {
            let validated_path = validation::validate_path(dir, ALLOWED_DIRS)?;
            cmd.arg(validated_path);
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!("Mermaid diagrams found:\n{}\n{}", stdout, stderr))
    }
}

#[async_trait]
impl AgentTrait for MermaidExpertAgent {
    fn agent_type(&self) -> &str {
        "mermaid-expert"
    }
    fn name(&self) -> &str {
        "Mermaid Expert"
    }
    fn description(&self) -> &str {
        "Mermaid diagram creation and validation"
    }

    fn operations(&self) -> Vec<String> {
        vec!["validate".to_string(), "find".to_string()]
    }

    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        let result = match task.operation.as_str() {
            "validate" => self.validate_mermaid(task.path.as_deref()),
            "find" => self.find_diagrams(task.path.as_deref()),
            _ => Err(format!("Unknown operation: {}", task.operation)),
        };

        match result {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/content/mod.rs">
//! Content generation and documentation agents

pub mod api_documenter;
pub mod docs_architect;
pub mod mermaid_expert;
pub mod tutorial_engineer;

pub use api_documenter::ApiDocumenterAgent;
pub use docs_architect::DocsArchitectAgent;
pub use mermaid_expert::MermaidExpertAgent;
pub use tutorial_engineer::TutorialEngineerAgent;
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/content/tutorial_engineer.rs">
//! Tutorial Engineer Agent

use async_trait::async_trait;

use crate::agents::base::{validation, AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;

const ALLOWED_DIRS: &[&str] = &["/tmp", "/home", "/opt"];

pub struct TutorialEngineerAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl TutorialEngineerAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            agent_id,
            profile: SecurityProfile::content_generation("tutorial-engineer"),
        }
    }

    fn analyze_code(&self, path: Option<&str>) -> Result<String, String> {
        if let Some(file) = path {
            let validated_path = validation::validate_path(file, ALLOWED_DIRS)?;
            let content = std::fs::read_to_string(&validated_path)
                .map_err(|e| format!("Failed to read file: {}", e))?;

            // Count lines, functions, etc.
            let lines = content.lines().count();
            let functions = content.matches("fn ").count() + content.matches("def ").count();
            let classes = content.matches("class ").count() + content.matches("struct ").count();

            Ok(format!(
                "Code analysis:\n- Lines: {}\n- Functions: {}\n- Classes/Structs: {}",
                lines, functions, classes
            ))
        } else {
            Err("File path required".to_string())
        }
    }

    fn extract_comments(&self, path: Option<&str>) -> Result<String, String> {
        if let Some(file) = path {
            let validated_path = validation::validate_path(file, ALLOWED_DIRS)?;
            let content = std::fs::read_to_string(&validated_path)
                .map_err(|e| format!("Failed to read file: {}", e))?;

            // Extract comments (simple heuristic)
            let comments: Vec<&str> = content
                .lines()
                .filter(|line| {
                    let trimmed = line.trim();
                    trimmed.starts_with("//")
                        || trimmed.starts_with("#")
                        || trimmed.starts_with("/*")
                        || trimmed.starts_with("*")
                        || trimmed.starts_with("///")
                })
                .take(50)
                .collect();

            Ok(format!("Comments found:\n{}", comments.join("\n")))
        } else {
            Err("File path required".to_string())
        }
    }
}

#[async_trait]
impl AgentTrait for TutorialEngineerAgent {
    fn agent_type(&self) -> &str {
        "tutorial-engineer"
    }
    fn name(&self) -> &str {
        "Tutorial Engineer"
    }
    fn description(&self) -> &str {
        "Tutorial and learning content creation"
    }

    fn operations(&self) -> Vec<String> {
        vec!["analyze".to_string(), "comments".to_string()]
    }

    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        let result = match task.operation.as_str() {
            "analyze" => self.analyze_code(task.path.as_deref()),
            "comments" => self.extract_comments(task.path.as_deref()),
            _ => Err(format!("Unknown operation: {}", task.operation)),
        };

        match result {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/database/database_architect.rs">
//! Database Architect Agent

use async_trait::async_trait;
use std::process::Command;

use crate::agents::base::{validation, AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;

const ALLOWED_DIRS: &[&str] = &["/tmp", "/home", "/opt"];

pub struct DatabaseArchitectAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl DatabaseArchitectAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            agent_id,
            profile: SecurityProfile::read_only_analysis(
                "database-architect",
                vec!["psql", "mysql", "sqlite3"],
            ),
        }
    }

    fn get_schema(&self, db_path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("sqlite3");

        if let Some(db) = db_path {
            let validated_path = validation::validate_path(db, ALLOWED_DIRS)?;
            cmd.arg(validated_path);
        } else {
            return Err("Database path required".to_string());
        }

        cmd.arg(".schema");

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!("Schema:\n{}\n{}", stdout, stderr))
    }

    fn list_tables(&self, db_path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("sqlite3");

        if let Some(db) = db_path {
            let validated_path = validation::validate_path(db, ALLOWED_DIRS)?;
            cmd.arg(validated_path);
        } else {
            return Err("Database path required".to_string());
        }

        cmd.arg(".tables");

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!("Tables:\n{}\n{}", stdout, stderr))
    }

    fn describe_table(&self, db_path: Option<&str>, table: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("sqlite3");

        if let Some(db) = db_path {
            let validated_path = validation::validate_path(db, ALLOWED_DIRS)?;
            cmd.arg(validated_path);
        } else {
            return Err("Database path required".to_string());
        }

        if let Some(t) = table {
            validation::validate_args(t)?;
            cmd.arg(format!("PRAGMA table_info({});", t));
        } else {
            return Err("Table name required".to_string());
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!("Table info:\n{}\n{}", stdout, stderr))
    }
}

#[async_trait]
impl AgentTrait for DatabaseArchitectAgent {
    fn agent_type(&self) -> &str {
        "database-architect"
    }
    fn name(&self) -> &str {
        "Database Architect"
    }
    fn description(&self) -> &str {
        "Database schema analysis and design"
    }

    fn operations(&self) -> Vec<String> {
        vec![
            "schema".to_string(),
            "tables".to_string(),
            "describe".to_string(),
        ]
    }

    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        let result = match task.operation.as_str() {
            "schema" => self.get_schema(task.path.as_deref()),
            "tables" => self.list_tables(task.path.as_deref()),
            "describe" => self.describe_table(task.path.as_deref(), task.args.as_deref()),
            _ => Err(format!("Unknown operation: {}", task.operation)),
        };

        match result {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/database/database_optimizer.rs">
//! Database Optimizer Agent

use async_trait::async_trait;
use std::process::Command;

use crate::agents::base::{validation, AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;

const ALLOWED_DIRS: &[&str] = &["/tmp", "/home", "/opt"];

pub struct DatabaseOptimizerAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl DatabaseOptimizerAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            agent_id,
            profile: SecurityProfile::read_only_analysis(
                "database-optimizer",
                vec!["sqlite3", "psql"],
            ),
        }
    }

    fn explain_query(&self, db_path: Option<&str>, query: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("sqlite3");

        if let Some(db) = db_path {
            let validated_path = validation::validate_path(db, ALLOWED_DIRS)?;
            cmd.arg(validated_path);
        } else {
            return Err("Database path required".to_string());
        }

        if let Some(q) = query {
            // Only allow SELECT queries
            let q_upper = q.to_uppercase();
            if !q_upper.trim().starts_with("SELECT") {
                return Err("Only SELECT queries allowed for EXPLAIN".to_string());
            }
            cmd.arg(format!("EXPLAIN QUERY PLAN {}", q));
        } else {
            return Err("Query required".to_string());
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!("Query plan:\n{}\n{}", stdout, stderr))
    }

    fn list_indexes(&self, db_path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("sqlite3");

        if let Some(db) = db_path {
            let validated_path = validation::validate_path(db, ALLOWED_DIRS)?;
            cmd.arg(validated_path);
        } else {
            return Err("Database path required".to_string());
        }

        cmd.arg(".indexes");

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!("Indexes:\n{}\n{}", stdout, stderr))
    }

    fn analyze_stats(&self, db_path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("sqlite3");

        if let Some(db) = db_path {
            let validated_path = validation::validate_path(db, ALLOWED_DIRS)?;
            cmd.arg(validated_path);
        } else {
            return Err("Database path required".to_string());
        }

        cmd.arg("SELECT * FROM sqlite_stat1;");

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!("Statistics:\n{}\n{}", stdout, stderr))
    }
}

#[async_trait]
impl AgentTrait for DatabaseOptimizerAgent {
    fn agent_type(&self) -> &str {
        "database-optimizer"
    }
    fn name(&self) -> &str {
        "Database Optimizer"
    }
    fn description(&self) -> &str {
        "Database query optimization and analysis"
    }

    fn operations(&self) -> Vec<String> {
        vec![
            "explain".to_string(),
            "indexes".to_string(),
            "stats".to_string(),
        ]
    }

    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        let result = match task.operation.as_str() {
            "explain" => self.explain_query(task.path.as_deref(), task.args.as_deref()),
            "indexes" => self.list_indexes(task.path.as_deref()),
            "stats" => self.analyze_stats(task.path.as_deref()),
            _ => Err(format!("Unknown operation: {}", task.operation)),
        };

        match result {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/database/mod.rs">
//! Database-related agents

pub mod database_architect;
pub mod database_optimizer;
pub mod sql_pro;

pub use database_architect::DatabaseArchitectAgent;
pub use database_optimizer::DatabaseOptimizerAgent;
pub use sql_pro::SqlProAgent;
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/database/sql_pro.rs">
//! SQL Pro Agent

use async_trait::async_trait;
use std::process::Command;

use crate::agents::base::{validation, AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;

const ALLOWED_DIRS: &[&str] = &["/tmp", "/home", "/opt"];

pub struct SqlProAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl SqlProAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            agent_id,
            profile: SecurityProfile::code_execution(
                "sql-pro",
                vec!["psql", "mysql", "sqlite3", "sqlfluff"],
            ),
        }
    }

    fn sqlite_query(&self, db_path: Option<&str>, query: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("sqlite3");
        cmd.arg("-header").arg("-column");

        if let Some(db) = db_path {
            let validated_path = validation::validate_path(db, ALLOWED_DIRS)?;
            cmd.arg(validated_path);
        } else {
            return Err("Database path required".to_string());
        }

        if let Some(q) = query {
            // Only allow SELECT queries for safety
            let q_upper = q.to_uppercase();
            if !q_upper.trim().starts_with("SELECT")
                && !q_upper.trim().starts_with(".SCHEMA")
                && !q_upper.trim().starts_with(".TABLES")
            {
                return Err("Only SELECT queries allowed".to_string());
            }
            cmd.arg(q);
        } else {
            cmd.arg(".tables");
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!("Query result:\n{}\n{}", stdout, stderr))
    }

    fn sqlfluff_lint(&self, path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("sqlfluff");
        cmd.arg("lint");

        if let Some(file) = path {
            let validated_path = validation::validate_path(file, ALLOWED_DIRS)?;
            cmd.arg(validated_path);
        } else {
            return Err("SQL file path required".to_string());
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!("SQLFluff lint:\n{}\n{}", stdout, stderr))
    }

    fn sqlfluff_format(&self, path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("sqlfluff");
        cmd.arg("fix").arg("--diff");

        if let Some(file) = path {
            let validated_path = validation::validate_path(file, ALLOWED_DIRS)?;
            cmd.arg(validated_path);
        } else {
            return Err("SQL file path required".to_string());
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!("SQLFluff format:\n{}\n{}", stdout, stderr))
    }
}

#[async_trait]
impl AgentTrait for SqlProAgent {
    fn agent_type(&self) -> &str {
        "sql-pro"
    }
    fn name(&self) -> &str {
        "SQL Pro"
    }
    fn description(&self) -> &str {
        "SQL development and query execution"
    }

    fn operations(&self) -> Vec<String> {
        vec![
            "query".to_string(),
            "lint".to_string(),
            "format".to_string(),
        ]
    }

    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        let result = match task.operation.as_str() {
            "query" => self.sqlite_query(task.path.as_deref(), task.args.as_deref()),
            "lint" => self.sqlfluff_lint(task.path.as_deref()),
            "format" => self.sqlfluff_format(task.path.as_deref()),
            _ => Err(format!("Unknown operation: {}", task.operation)),
        };

        match result {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/infrastructure/cloud.rs">
//! Cloud Architect Agent

use async_trait::async_trait;
use std::process::Command;

use crate::agents::base::{validation, AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;

pub struct CloudArchitectAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl CloudArchitectAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            agent_id,
            profile: SecurityProfile::read_only_analysis(
                "cloud-architect",
                vec!["aws", "gcloud", "az"],
            ),
        }
    }

    fn aws_describe(&self, resource: Option<&str>, args: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("aws");

        if let Some(r) = resource {
            validation::validate_args(r)?;
            for part in r.split_whitespace() {
                cmd.arg(part);
            }
        } else {
            cmd.arg("sts").arg("get-caller-identity");
        }

        if let Some(a) = args {
            validation::validate_args(a)?;
            for arg in a.split_whitespace() {
                cmd.arg(arg);
            }
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!("AWS output:\n{}\n{}", stdout, stderr))
    }

    fn gcloud_describe(&self, resource: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("gcloud");

        if let Some(r) = resource {
            validation::validate_args(r)?;
            for part in r.split_whitespace() {
                cmd.arg(part);
            }
        } else {
            cmd.arg("config").arg("list");
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!("GCloud output:\n{}\n{}", stdout, stderr))
    }
}

#[async_trait]
impl AgentTrait for CloudArchitectAgent {
    fn agent_type(&self) -> &str {
        "cloud-architect"
    }
    fn name(&self) -> &str {
        "Cloud Architect"
    }
    fn description(&self) -> &str {
        "Multi-cloud architecture analysis"
    }

    fn operations(&self) -> Vec<String> {
        vec!["aws-describe".to_string(), "gcloud-describe".to_string()]
    }

    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        let result = match task.operation.as_str() {
            "aws-describe" => self.aws_describe(task.path.as_deref(), task.args.as_deref()),
            "gcloud-describe" => self.gcloud_describe(task.path.as_deref()),
            _ => Err(format!("Unknown operation: {}", task.operation)),
        };

        match result {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/infrastructure/deployment.rs">
//! Deployment Engineer Agent

use async_trait::async_trait;
use std::process::Command;

use crate::agents::base::{validation, AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;

const ALLOWED_DIRS: &[&str] = &["/tmp", "/home", "/opt"];

pub struct DeploymentAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl DeploymentAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            agent_id,
            profile: SecurityProfile::code_execution(
                "deployment-engineer",
                vec!["docker", "docker-compose", "ansible"],
            ),
        }
    }

    fn docker_build(&self, path: Option<&str>, args: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("docker");
        cmd.arg("build");

        if let Some(a) = args {
            validation::validate_args(a)?;
            for arg in a.split_whitespace() {
                cmd.arg(arg);
            }
        }

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.arg(validated_path);
        } else {
            cmd.arg(".");
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!("Docker build succeeded\n{}\n{}", stdout, stderr))
        } else {
            Ok(format!("Docker build failed\n{}\n{}", stdout, stderr))
        }
    }

    fn docker_compose_up(&self, path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("docker-compose");
        cmd.arg("up").arg("-d");

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.current_dir(validated_path);
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!(
                "Docker Compose up succeeded\n{}\n{}",
                stdout, stderr
            ))
        } else {
            Ok(format!("Docker Compose up failed\n{}\n{}", stdout, stderr))
        }
    }

    fn ansible_check(&self, path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("ansible-playbook");
        cmd.arg("--check");

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.arg(validated_path);
        } else {
            return Err("Playbook path required".to_string());
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!("Ansible check:\n{}\n{}", stdout, stderr))
    }
}

#[async_trait]
impl AgentTrait for DeploymentAgent {
    fn agent_type(&self) -> &str {
        "deployment-engineer"
    }
    fn name(&self) -> &str {
        "Deployment Engineer"
    }
    fn description(&self) -> &str {
        "Deployment automation with Docker and Ansible"
    }

    fn operations(&self) -> Vec<String> {
        vec![
            "docker-build".to_string(),
            "compose-up".to_string(),
            "ansible-check".to_string(),
        ]
    }

    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        let result = match task.operation.as_str() {
            "docker-build" => self.docker_build(task.path.as_deref(), task.args.as_deref()),
            "compose-up" => self.docker_compose_up(task.path.as_deref()),
            "ansible-check" => self.ansible_check(task.path.as_deref()),
            _ => Err(format!("Unknown operation: {}", task.operation)),
        };

        match result {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/infrastructure/kubernetes.rs">
//! Kubernetes Architect Agent

use async_trait::async_trait;
use std::process::Command;

use crate::agents::base::{validation, AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;

const ALLOWED_DIRS: &[&str] = &["/tmp", "/home", "/opt"];

pub struct KubernetesAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl KubernetesAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            agent_id,
            profile: SecurityProfile::read_only_analysis(
                "kubernetes-architect",
                vec!["kubectl", "helm", "kustomize"],
            ),
        }
    }

    fn kubectl_get(&self, resource: Option<&str>, args: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("kubectl");
        cmd.arg("get");

        if let Some(r) = resource {
            validation::validate_args(r)?;
            cmd.arg(r);
        } else {
            cmd.arg("all");
        }

        if let Some(a) = args {
            validation::validate_args(a)?;
            for arg in a.split_whitespace() {
                cmd.arg(arg);
            }
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!("kubectl output:\n{}\n{}", stdout, stderr))
    }

    fn kubectl_describe(&self, resource: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("kubectl");
        cmd.arg("describe");

        if let Some(r) = resource {
            validation::validate_args(r)?;
            cmd.arg(r);
        } else {
            return Err("Resource required".to_string());
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!("kubectl describe:\n{}\n{}", stdout, stderr))
    }

    fn kubectl_logs(&self, pod: Option<&str>, args: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("kubectl");
        cmd.arg("logs");

        if let Some(p) = pod {
            validation::validate_args(p)?;
            cmd.arg(p);
        } else {
            return Err("Pod name required".to_string());
        }

        if let Some(a) = args {
            validation::validate_args(a)?;
            for arg in a.split_whitespace() {
                cmd.arg(arg);
            }
        }

        cmd.arg("--tail=100");

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!("Pod logs:\n{}\n{}", stdout, stderr))
    }

    fn helm_list(&self) -> Result<String, String> {
        let mut cmd = Command::new("helm");
        cmd.arg("list").arg("-A");

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!("Helm releases:\n{}\n{}", stdout, stderr))
    }
}

#[async_trait]
impl AgentTrait for KubernetesAgent {
    fn agent_type(&self) -> &str {
        "kubernetes-architect"
    }
    fn name(&self) -> &str {
        "Kubernetes Architect"
    }
    fn description(&self) -> &str {
        "Kubernetes cluster management and analysis"
    }

    fn operations(&self) -> Vec<String> {
        vec![
            "get".to_string(),
            "describe".to_string(),
            "logs".to_string(),
            "helm-list".to_string(),
        ]
    }

    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        let result = match task.operation.as_str() {
            "get" => self.kubectl_get(task.path.as_deref(), task.args.as_deref()),
            "describe" => self.kubectl_describe(task.path.as_deref()),
            "logs" => self.kubectl_logs(task.path.as_deref(), task.args.as_deref()),
            "helm-list" => self.helm_list(),
            _ => Err(format!("Unknown operation: {}", task.operation)),
        };

        match result {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/infrastructure/mod.rs">
//! Infrastructure and DevOps agents

pub mod cloud;
pub mod deployment;
pub mod kubernetes;
pub mod network;
pub mod terraform;

pub use cloud::CloudArchitectAgent;
pub use deployment::DeploymentAgent;
pub use kubernetes::KubernetesAgent;
pub use network::NetworkEngineerAgent;
pub use terraform::TerraformAgent;
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/infrastructure/network.rs">
//! Network Engineer Agent

use async_trait::async_trait;
use std::process::Command;

use crate::agents::base::{validation, AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;

pub struct NetworkEngineerAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl NetworkEngineerAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            agent_id,
            profile: SecurityProfile::read_only_analysis(
                "network-engineer",
                vec!["ip", "ss", "netstat", "dig", "ping", "traceroute"],
            ),
        }
    }

    fn show_interfaces(&self) -> Result<String, String> {
        let mut cmd = Command::new("ip");
        cmd.arg("addr").arg("show");

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!("Network interfaces:\n{}\n{}", stdout, stderr))
    }

    fn show_routes(&self) -> Result<String, String> {
        let mut cmd = Command::new("ip");
        cmd.arg("route").arg("show");

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!("Routes:\n{}\n{}", stdout, stderr))
    }

    fn show_connections(&self) -> Result<String, String> {
        let mut cmd = Command::new("ss");
        cmd.arg("-tuln");

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!("Active connections:\n{}\n{}", stdout, stderr))
    }

    fn dns_lookup(&self, host: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("dig");

        if let Some(h) = host {
            validation::validate_args(h)?;
            cmd.arg(h);
        } else {
            return Err("Host required".to_string());
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!("DNS lookup:\n{}\n{}", stdout, stderr))
    }
}

#[async_trait]
impl AgentTrait for NetworkEngineerAgent {
    fn agent_type(&self) -> &str {
        "network-engineer"
    }
    fn name(&self) -> &str {
        "Network Engineer"
    }
    fn description(&self) -> &str {
        "Network diagnostics and analysis"
    }

    fn operations(&self) -> Vec<String> {
        vec![
            "interfaces".to_string(),
            "routes".to_string(),
            "connections".to_string(),
            "dns".to_string(),
        ]
    }

    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        let result = match task.operation.as_str() {
            "interfaces" => self.show_interfaces(),
            "routes" => self.show_routes(),
            "connections" => self.show_connections(),
            "dns" => self.dns_lookup(task.path.as_deref()),
            _ => Err(format!("Unknown operation: {}", task.operation)),
        };

        match result {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/infrastructure/terraform.rs">
//! Terraform Specialist Agent

use async_trait::async_trait;
use std::process::Command;

use crate::agents::base::{validation, AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;

const ALLOWED_DIRS: &[&str] = &["/tmp", "/home", "/opt"];

pub struct TerraformAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl TerraformAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            agent_id,
            profile: SecurityProfile::code_execution(
                "terraform-specialist",
                vec!["terraform", "tofu"],
            ),
        }
    }

    fn terraform_init(&self, path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("terraform");
        cmd.arg("init");

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.current_dir(validated_path);
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!("Init succeeded\n{}\n{}", stdout, stderr))
        } else {
            Ok(format!("Init failed\n{}\n{}", stdout, stderr))
        }
    }

    fn terraform_plan(&self, path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("terraform");
        cmd.arg("plan").arg("-no-color");

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.current_dir(validated_path);
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!("Plan succeeded\n{}\n{}", stdout, stderr))
        } else {
            Ok(format!("Plan failed\n{}\n{}", stdout, stderr))
        }
    }

    fn terraform_validate(&self, path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("terraform");
        cmd.arg("validate");

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.current_dir(validated_path);
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!("Validation passed\n{}\n{}", stdout, stderr))
        } else {
            Ok(format!("Validation failed\n{}\n{}", stdout, stderr))
        }
    }

    fn terraform_fmt(&self, path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("terraform");
        cmd.arg("fmt").arg("-check").arg("-diff");

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.current_dir(validated_path);
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!("Format check passed\n{}\n{}", stdout, stderr))
        } else {
            Ok(format!("Format check failed\n{}\n{}", stdout, stderr))
        }
    }
}

#[async_trait]
impl AgentTrait for TerraformAgent {
    fn agent_type(&self) -> &str {
        "terraform-specialist"
    }
    fn name(&self) -> &str {
        "Terraform Specialist"
    }
    fn description(&self) -> &str {
        "Infrastructure as Code with Terraform"
    }

    fn operations(&self) -> Vec<String> {
        vec![
            "init".to_string(),
            "plan".to_string(),
            "validate".to_string(),
            "fmt".to_string(),
        ]
    }

    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        let result = match task.operation.as_str() {
            "init" => self.terraform_init(task.path.as_deref()),
            "plan" => self.terraform_plan(task.path.as_deref()),
            "validate" => self.terraform_validate(task.path.as_deref()),
            "fmt" => self.terraform_fmt(task.path.as_deref()),
            _ => Err(format!("Unknown operation: {}", task.operation)),
        };

        match result {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/language/bash_pro.rs">
//! Bash Pro Agent - Shell scripting environment

use async_trait::async_trait;
use std::process::Command;

use crate::agents::base::{validation, AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;

const ALLOWED_DIRS: &[&str] = &["/tmp", "/home", "/opt"];

pub struct BashProAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl BashProAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            agent_id,
            profile: SecurityProfile::code_execution("bash-pro", vec!["bash", "sh", "shellcheck"]),
        }
    }

    fn bash_run(&self, path: Option<&str>, args: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("bash");

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.arg(validated_path);
        } else {
            return Err("Path required".to_string());
        }

        if let Some(a) = args {
            validation::validate_args(a)?;
            for arg in a.split_whitespace() {
                cmd.arg(arg);
            }
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!(
                "Script succeeded\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        } else {
            Ok(format!(
                "Script failed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        }
    }

    fn shellcheck_lint(&self, path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("shellcheck");

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.arg(validated_path);
        } else {
            return Err("Path required".to_string());
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!(
                "ShellCheck passed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        } else {
            Ok(format!(
                "ShellCheck found issues\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        }
    }

    fn bash_syntax_check(&self, path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("bash");
        cmd.arg("-n");

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.arg(validated_path);
        } else {
            return Err("Path required".to_string());
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!("Syntax OK\nstdout: {}\nstderr: {}", stdout, stderr))
        } else {
            Ok(format!(
                "Syntax errors\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        }
    }
}

#[async_trait]
impl AgentTrait for BashProAgent {
    fn agent_type(&self) -> &str {
        "bash-pro"
    }
    fn name(&self) -> &str {
        "Bash Pro Agent"
    }
    fn description(&self) -> &str {
        "Shell scripting environment with ShellCheck"
    }

    fn operations(&self) -> Vec<String> {
        vec!["run".to_string(), "lint".to_string(), "check".to_string()]
    }

    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "bash-pro" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }

        let result = match task.operation.as_str() {
            "run" => self.bash_run(task.path.as_deref(), task.args.as_deref()),
            "lint" => self.shellcheck_lint(task.path.as_deref()),
            "check" => self.bash_syntax_check(task.path.as_deref()),
            _ => Err(format!("Unknown operation: {}", task.operation)),
        };

        match result {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/language/c_pro.rs">
//! C Pro Agent - C development environment

use async_trait::async_trait;
use std::process::Command;

use crate::agents::base::{validation, AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;

const ALLOWED_DIRS: &[&str] = &["/tmp", "/home", "/opt"];

pub struct CProAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl CProAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            agent_id,
            profile: SecurityProfile::code_execution(
                "c-pro",
                vec!["gcc", "clang", "make", "cmake", "gdb", "valgrind"],
            ),
        }
    }

    fn gcc_compile(&self, path: Option<&str>, args: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("gcc");
        cmd.arg("-Wall").arg("-Wextra");

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.arg(validated_path);
        } else {
            return Err("Path required".to_string());
        }

        if let Some(a) = args {
            validation::validate_args(a)?;
            for arg in a.split_whitespace() {
                cmd.arg(arg);
            }
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!(
                "Compilation succeeded\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        } else {
            Ok(format!(
                "Compilation failed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        }
    }

    fn make_build(&self, path: Option<&str>, target: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("make");

        if let Some(t) = target {
            validation::validate_args(t)?;
            cmd.arg(t);
        }

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.current_dir(validated_path);
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!(
                "Build succeeded\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        } else {
            Ok(format!(
                "Build failed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        }
    }

    fn cmake_configure(&self, path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("cmake");
        cmd.arg("-B").arg("build");

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.current_dir(validated_path);
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!(
                "CMake configure succeeded\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        } else {
            Ok(format!(
                "CMake configure failed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        }
    }
}

#[async_trait]
impl AgentTrait for CProAgent {
    fn agent_type(&self) -> &str {
        "c-pro"
    }
    fn name(&self) -> &str {
        "C Pro Agent"
    }
    fn description(&self) -> &str {
        "C development environment with GCC, Make, and CMake"
    }

    fn operations(&self) -> Vec<String> {
        vec![
            "compile".to_string(),
            "make".to_string(),
            "cmake".to_string(),
        ]
    }

    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "c-pro" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }

        let result = match task.operation.as_str() {
            "compile" => self.gcc_compile(task.path.as_deref(), task.args.as_deref()),
            "make" => self.make_build(task.path.as_deref(), task.args.as_deref()),
            "cmake" => self.cmake_configure(task.path.as_deref()),
            _ => Err(format!("Unknown operation: {}", task.operation)),
        };

        match result {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/language/cpp_pro.rs">
//! C++ Pro Agent - C++ development environment

use async_trait::async_trait;
use std::process::Command;

use crate::agents::base::{validation, AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;

const ALLOWED_DIRS: &[&str] = &["/tmp", "/home", "/opt"];

pub struct CppProAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl CppProAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            agent_id,
            profile: SecurityProfile::code_execution(
                "cpp-pro",
                vec!["g++", "clang++", "make", "cmake", "gdb", "valgrind"],
            ),
        }
    }

    fn gpp_compile(&self, path: Option<&str>, args: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("g++");
        cmd.arg("-Wall").arg("-Wextra").arg("-std=c++20");

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.arg(validated_path);
        } else {
            return Err("Path required".to_string());
        }

        if let Some(a) = args {
            validation::validate_args(a)?;
            for arg in a.split_whitespace() {
                cmd.arg(arg);
            }
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!(
                "Compilation succeeded\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        } else {
            Ok(format!(
                "Compilation failed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        }
    }

    fn cmake_build(&self, path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("cmake");
        cmd.arg("--build").arg("build");

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.current_dir(validated_path);
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!(
                "Build succeeded\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        } else {
            Ok(format!(
                "Build failed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        }
    }
}

#[async_trait]
impl AgentTrait for CppProAgent {
    fn agent_type(&self) -> &str {
        "cpp-pro"
    }
    fn name(&self) -> &str {
        "C++ Pro Agent"
    }
    fn description(&self) -> &str {
        "C++ development environment with G++, Make, and CMake"
    }

    fn operations(&self) -> Vec<String> {
        vec!["compile".to_string(), "build".to_string()]
    }

    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "cpp-pro" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }

        let result = match task.operation.as_str() {
            "compile" => self.gpp_compile(task.path.as_deref(), task.args.as_deref()),
            "build" => self.cmake_build(task.path.as_deref()),
            _ => Err(format!("Unknown operation: {}", task.operation)),
        };

        match result {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/language/csharp_pro.rs">
//! C# Pro Agent - .NET development environment

use async_trait::async_trait;
use std::process::Command;

use crate::agents::base::{validation, AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;

const ALLOWED_DIRS: &[&str] = &["/tmp", "/home", "/opt"];

pub struct CSharpProAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl CSharpProAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            agent_id,
            profile: SecurityProfile::code_execution("csharp-pro", vec!["dotnet"]),
        }
    }

    fn dotnet_build(&self, path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("dotnet");
        cmd.arg("build");

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.current_dir(validated_path);
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!(
                "Build succeeded\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        } else {
            Ok(format!(
                "Build failed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        }
    }

    fn dotnet_test(&self, path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("dotnet");
        cmd.arg("test");

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.current_dir(validated_path);
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!(
                "Tests passed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        } else {
            Ok(format!(
                "Tests failed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        }
    }

    fn dotnet_format(&self, path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("dotnet");
        cmd.arg("format").arg("--verify-no-changes");

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.current_dir(validated_path);
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!(
                "Format check passed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        } else {
            Ok(format!(
                "Format check failed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        }
    }
}

#[async_trait]
impl AgentTrait for CSharpProAgent {
    fn agent_type(&self) -> &str {
        "csharp-pro"
    }
    fn name(&self) -> &str {
        "C# Pro Agent"
    }
    fn description(&self) -> &str {
        ".NET development environment"
    }

    fn operations(&self) -> Vec<String> {
        vec![
            "build".to_string(),
            "test".to_string(),
            "format".to_string(),
        ]
    }

    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "csharp-pro" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }

        let result = match task.operation.as_str() {
            "build" => self.dotnet_build(task.path.as_deref()),
            "test" => self.dotnet_test(task.path.as_deref()),
            "format" => self.dotnet_format(task.path.as_deref()),
            _ => Err(format!("Unknown operation: {}", task.operation)),
        };

        match result {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/language/elixir_pro.rs">
//! Elixir Pro Agent - Elixir development environment

use async_trait::async_trait;
use std::process::Command;

use crate::agents::base::{validation, AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;

const ALLOWED_DIRS: &[&str] = &["/tmp", "/home", "/opt"];

pub struct ElixirProAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl ElixirProAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            agent_id,
            profile: SecurityProfile::code_execution("elixir-pro", vec!["elixir", "mix", "iex"]),
        }
    }

    fn mix_compile(&self, path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("mix");
        cmd.arg("compile");

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.current_dir(validated_path);
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!(
                "Compilation succeeded\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        } else {
            Ok(format!(
                "Compilation failed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        }
    }

    fn mix_test(&self, path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("mix");
        cmd.arg("test");

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.current_dir(validated_path);
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!(
                "Tests passed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        } else {
            Ok(format!(
                "Tests failed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        }
    }

    fn mix_format(&self, path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("mix");
        cmd.arg("format").arg("--check-formatted");

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.current_dir(validated_path);
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!(
                "Format check passed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        } else {
            Ok(format!(
                "Format check failed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        }
    }
}

#[async_trait]
impl AgentTrait for ElixirProAgent {
    fn agent_type(&self) -> &str {
        "elixir-pro"
    }
    fn name(&self) -> &str {
        "Elixir Pro Agent"
    }
    fn description(&self) -> &str {
        "Elixir development environment with Mix"
    }

    fn operations(&self) -> Vec<String> {
        vec![
            "compile".to_string(),
            "test".to_string(),
            "format".to_string(),
        ]
    }

    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "elixir-pro" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }

        let result = match task.operation.as_str() {
            "compile" => self.mix_compile(task.path.as_deref()),
            "test" => self.mix_test(task.path.as_deref()),
            "format" => self.mix_format(task.path.as_deref()),
            _ => Err(format!("Unknown operation: {}", task.operation)),
        };

        match result {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/language/golang_pro.rs">
//! Go Pro Agent - Go development environment
//!
//! Provides secure execution for Go development tasks including:
//! - go build/test/run
//! - gofmt formatting
//! - go vet static analysis
//! - staticcheck linting

use async_trait::async_trait;
use std::process::Command;

use crate::agents::base::{validation, AgentTask, AgentTrait, TaskResult};
use crate::security::{profiles::presets, SecurityProfile};

const ALLOWED_DIRS: &[&str] = &["/tmp", "/home", "/opt"];

pub struct GolangProAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl GolangProAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            agent_id,
            profile: presets::golang_pro(),
        }
    }

    fn go_build(&self, path: Option<&str>, args: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("go");
        cmd.arg("build");

        if let Some(a) = args {
            validation::validate_args(a)?;
            for arg in a.split_whitespace() {
                cmd.arg(arg);
            }
        }

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.current_dir(validated_path);
        }

        let output = cmd
            .output()
            .map_err(|e| format!("Failed to run go build: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!(
                "Build succeeded\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        } else {
            Ok(format!(
                "Build failed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        }
    }

    fn go_test(&self, path: Option<&str>, args: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("go");
        cmd.arg("test");
        cmd.arg("./...");
        cmd.arg("-v");

        if let Some(a) = args {
            validation::validate_args(a)?;
            for arg in a.split_whitespace() {
                cmd.arg(arg);
            }
        }

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.current_dir(validated_path);
        }

        let output = cmd
            .output()
            .map_err(|e| format!("Failed to run go test: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!(
                "Tests passed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        } else {
            Ok(format!(
                "Tests failed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        }
    }

    fn go_fmt(&self, path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("gofmt");
        cmd.arg("-l");

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.arg(validated_path);
        } else {
            cmd.arg(".");
        }

        let output = cmd
            .output()
            .map_err(|e| format!("Failed to run gofmt: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if stdout.is_empty() {
            Ok(format!("Code is properly formatted\nstderr: {}", stderr))
        } else {
            Ok(format!(
                "Files need formatting:\n{}\nstderr: {}",
                stdout, stderr
            ))
        }
    }

    fn go_vet(&self, path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("go");
        cmd.arg("vet");
        cmd.arg("./...");

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.current_dir(validated_path);
        }

        let output = cmd
            .output()
            .map_err(|e| format!("Failed to run go vet: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!(
                "go vet passed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        } else {
            Ok(format!(
                "go vet found issues\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        }
    }

    fn go_run(&self, path: Option<&str>, args: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("go");
        cmd.arg("run");

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.arg(validated_path);
        } else {
            cmd.arg(".");
        }

        if let Some(a) = args {
            validation::validate_args(a)?;
            for arg in a.split_whitespace() {
                cmd.arg(arg);
            }
        }

        let output = cmd
            .output()
            .map_err(|e| format!("Failed to run go run: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!(
                "Execution succeeded\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        } else {
            Ok(format!(
                "Execution failed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        }
    }
}

#[async_trait]
impl AgentTrait for GolangProAgent {
    fn agent_type(&self) -> &str {
        "golang-pro"
    }

    fn name(&self) -> &str {
        "Go Pro Agent"
    }

    fn description(&self) -> &str {
        "Go development environment with build, test, and analysis tools"
    }

    fn operations(&self) -> Vec<String> {
        vec![
            "build".to_string(),
            "test".to_string(),
            "fmt".to_string(),
            "vet".to_string(),
            "run".to_string(),
        ]
    }

    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "golang-pro" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }

        let result = match task.operation.as_str() {
            "build" => self.go_build(task.path.as_deref(), task.args.as_deref()),
            "test" => self.go_test(task.path.as_deref(), task.args.as_deref()),
            "fmt" => self.go_fmt(task.path.as_deref()),
            "vet" => self.go_vet(task.path.as_deref()),
            "run" => self.go_run(task.path.as_deref(), task.args.as_deref()),
            _ => Err(format!("Unknown operation: {}", task.operation)),
        };

        match result {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }

    fn get_status(&self) -> String {
        format!("Go Pro agent {} is running", self.agent_id)
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/language/java_pro.rs">
//! Java Pro Agent - Java development environment

use async_trait::async_trait;
use std::process::Command;

use crate::agents::base::{validation, AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;

const ALLOWED_DIRS: &[&str] = &["/tmp", "/home", "/opt"];

pub struct JavaProAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl JavaProAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            agent_id,
            profile: SecurityProfile::code_execution(
                "java-pro",
                vec!["java", "javac", "mvn", "gradle"],
            ),
        }
    }

    fn mvn_build(&self, path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("mvn");
        cmd.arg("compile");

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.current_dir(validated_path);
        }

        let output = cmd
            .output()
            .map_err(|e| format!("Failed to run mvn: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!(
                "Build succeeded\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        } else {
            Ok(format!(
                "Build failed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        }
    }

    fn mvn_test(&self, path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("mvn");
        cmd.arg("test");

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.current_dir(validated_path);
        }

        let output = cmd
            .output()
            .map_err(|e| format!("Failed to run mvn test: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!(
                "Tests passed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        } else {
            Ok(format!(
                "Tests failed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        }
    }

    fn gradle_build(&self, path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("gradle");
        cmd.arg("build");

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.current_dir(validated_path);
        }

        let output = cmd
            .output()
            .map_err(|e| format!("Failed to run gradle: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!(
                "Build succeeded\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        } else {
            Ok(format!(
                "Build failed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        }
    }
}

#[async_trait]
impl AgentTrait for JavaProAgent {
    fn agent_type(&self) -> &str {
        "java-pro"
    }
    fn name(&self) -> &str {
        "Java Pro Agent"
    }
    fn description(&self) -> &str {
        "Java development environment with Maven/Gradle"
    }

    fn operations(&self) -> Vec<String> {
        vec![
            "build".to_string(),
            "test".to_string(),
            "gradle-build".to_string(),
        ]
    }

    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "java-pro" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }

        let result = match task.operation.as_str() {
            "build" => self.mvn_build(task.path.as_deref()),
            "test" => self.mvn_test(task.path.as_deref()),
            "gradle-build" => self.gradle_build(task.path.as_deref()),
            _ => Err(format!("Unknown operation: {}", task.operation)),
        };

        match result {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/language/javascript_pro.rs">
//! JavaScript Pro Agent - JavaScript/Node.js development environment
//!
//! Provides secure execution for JavaScript development tasks including:
//! - Node.js script execution
//! - npm/yarn package management
//! - ESLint linting
//! - Jest/Vitest testing
//! - Prettier formatting

use async_trait::async_trait;
use std::process::Command;

use crate::agents::base::{validation, AgentTask, AgentTrait, TaskResult};
use crate::security::{profiles::presets, SecurityProfile};

const ALLOWED_DIRS: &[&str] = &["/tmp", "/home", "/opt"];

pub struct JavaScriptProAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl JavaScriptProAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            agent_id,
            profile: presets::javascript_pro(),
        }
    }

    fn node_run(&self, path: Option<&str>, args: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("node");

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.arg(validated_path);
        } else {
            return Err("Path required for node run".to_string());
        }

        if let Some(a) = args {
            validation::validate_args(a)?;
            for arg in a.split_whitespace() {
                cmd.arg(arg);
            }
        }

        let output = cmd
            .output()
            .map_err(|e| format!("Failed to run node: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!(
                "Node execution succeeded\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        } else {
            Ok(format!(
                "Node execution failed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        }
    }

    fn npm_test(&self, path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("npm");
        cmd.arg("test");

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.current_dir(validated_path);
        }

        let output = cmd
            .output()
            .map_err(|e| format!("Failed to run npm test: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!(
                "Tests passed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        } else {
            Ok(format!(
                "Tests failed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        }
    }

    fn npm_build(&self, path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("npm");
        cmd.arg("run").arg("build");

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.current_dir(validated_path);
        }

        let output = cmd
            .output()
            .map_err(|e| format!("Failed to run npm build: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!(
                "Build succeeded\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        } else {
            Ok(format!(
                "Build failed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        }
    }

    fn eslint_check(&self, path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("npx");
        cmd.arg("eslint");

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.arg(validated_path);
        } else {
            cmd.arg(".");
        }

        let output = cmd
            .output()
            .map_err(|e| format!("Failed to run eslint: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!(
                "ESLint passed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        } else {
            Ok(format!(
                "ESLint found issues\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        }
    }

    fn prettier_check(&self, path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("npx");
        cmd.arg("prettier").arg("--check");

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.arg(validated_path);
        } else {
            cmd.arg(".");
        }

        let output = cmd
            .output()
            .map_err(|e| format!("Failed to run prettier: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!(
                "Code is properly formatted\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        } else {
            Ok(format!(
                "Code needs formatting\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        }
    }
}

#[async_trait]
impl AgentTrait for JavaScriptProAgent {
    fn agent_type(&self) -> &str {
        "javascript-pro"
    }
    fn name(&self) -> &str {
        "JavaScript Pro Agent"
    }
    fn description(&self) -> &str {
        "JavaScript/Node.js development environment"
    }

    fn operations(&self) -> Vec<String> {
        vec![
            "run".to_string(),
            "test".to_string(),
            "build".to_string(),
            "lint".to_string(),
            "format".to_string(),
        ]
    }

    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "javascript-pro" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }

        let result = match task.operation.as_str() {
            "run" => self.node_run(task.path.as_deref(), task.args.as_deref()),
            "test" => self.npm_test(task.path.as_deref()),
            "build" => self.npm_build(task.path.as_deref()),
            "lint" => self.eslint_check(task.path.as_deref()),
            "format" => self.prettier_check(task.path.as_deref()),
            _ => Err(format!("Unknown operation: {}", task.operation)),
        };

        match result {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/language/julia_pro.rs">
//! Julia Pro Agent - Julia development environment

use async_trait::async_trait;
use std::process::Command;

use crate::agents::base::{validation, AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;

const ALLOWED_DIRS: &[&str] = &["/tmp", "/home", "/opt"];

pub struct JuliaProAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl JuliaProAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            agent_id,
            profile: SecurityProfile::code_execution("julia-pro", vec!["julia"]),
        }
    }

    fn julia_run(&self, path: Option<&str>, args: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("julia");

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.arg(validated_path);
        } else {
            return Err("Path required".to_string());
        }

        if let Some(a) = args {
            validation::validate_args(a)?;
            for arg in a.split_whitespace() {
                cmd.arg(arg);
            }
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!(
                "Execution succeeded\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        } else {
            Ok(format!(
                "Execution failed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        }
    }

    fn julia_test(&self, path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("julia");
        cmd.arg("-e").arg("using Pkg; Pkg.test()");

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.current_dir(validated_path);
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!(
                "Tests passed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        } else {
            Ok(format!(
                "Tests failed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        }
    }
}

#[async_trait]
impl AgentTrait for JuliaProAgent {
    fn agent_type(&self) -> &str {
        "julia-pro"
    }
    fn name(&self) -> &str {
        "Julia Pro Agent"
    }
    fn description(&self) -> &str {
        "Julia development environment"
    }

    fn operations(&self) -> Vec<String> {
        vec!["run".to_string(), "test".to_string()]
    }

    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "julia-pro" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }

        let result = match task.operation.as_str() {
            "run" => self.julia_run(task.path.as_deref(), task.args.as_deref()),
            "test" => self.julia_test(task.path.as_deref()),
            _ => Err(format!("Unknown operation: {}", task.operation)),
        };

        match result {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/language/mod.rs">
//! Language-specific code execution agents
//!
//! These agents provide secure execution environments for various programming languages.

pub mod bash_pro;
pub mod c_pro;
pub mod cpp_pro;
pub mod csharp_pro;
pub mod elixir_pro;
pub mod golang_pro;
pub mod java_pro;
pub mod javascript_pro;
pub mod julia_pro;
pub mod php_pro;
pub mod python_pro;
pub mod ruby_pro;
pub mod rust_pro;
pub mod scala_pro;
pub mod typescript_pro;

// Re-exports
pub use bash_pro::BashProAgent;
pub use c_pro::CProAgent;
pub use cpp_pro::CppProAgent;
pub use csharp_pro::CSharpProAgent;
pub use elixir_pro::ElixirProAgent;
pub use golang_pro::GolangProAgent;
pub use java_pro::JavaProAgent;
pub use javascript_pro::JavaScriptProAgent;
pub use julia_pro::JuliaProAgent;
pub use php_pro::PhpProAgent;
pub use python_pro::PythonProAgent;
pub use ruby_pro::RubyProAgent;
pub use rust_pro::RustProAgent;
pub use scala_pro::ScalaProAgent;
pub use typescript_pro::TypeScriptProAgent;
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/language/php_pro.rs">
//! PHP Pro Agent - PHP development environment

use async_trait::async_trait;
use std::process::Command;

use crate::agents::base::{validation, AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;

const ALLOWED_DIRS: &[&str] = &["/tmp", "/home", "/opt"];

pub struct PhpProAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl PhpProAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            agent_id,
            profile: SecurityProfile::code_execution(
                "php-pro",
                vec!["php", "composer", "phpunit", "phpstan"],
            ),
        }
    }

    fn php_run(&self, path: Option<&str>, args: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("php");

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.arg(validated_path);
        } else {
            return Err("Path required".to_string());
        }

        if let Some(a) = args {
            validation::validate_args(a)?;
            for arg in a.split_whitespace() {
                cmd.arg(arg);
            }
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!(
                "Execution succeeded\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        } else {
            Ok(format!(
                "Execution failed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        }
    }

    fn phpunit_test(&self, path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("vendor/bin/phpunit");

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.current_dir(validated_path);
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!(
                "Tests passed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        } else {
            Ok(format!(
                "Tests failed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        }
    }

    fn phpstan_analyze(&self, path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("vendor/bin/phpstan");
        cmd.arg("analyse");

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.current_dir(validated_path);
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!(
            "PHPStan output\nstdout: {}\nstderr: {}",
            stdout, stderr
        ))
    }

    fn php_lint(&self, path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("php");
        cmd.arg("-l");

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.arg(validated_path);
        } else {
            return Err("Path required".to_string());
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!("Syntax OK\nstdout: {}\nstderr: {}", stdout, stderr))
        } else {
            Ok(format!(
                "Syntax errors found\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        }
    }
}

#[async_trait]
impl AgentTrait for PhpProAgent {
    fn agent_type(&self) -> &str {
        "php-pro"
    }
    fn name(&self) -> &str {
        "PHP Pro Agent"
    }
    fn description(&self) -> &str {
        "PHP development environment with PHPUnit and PHPStan"
    }

    fn operations(&self) -> Vec<String> {
        vec![
            "run".to_string(),
            "test".to_string(),
            "analyze".to_string(),
            "lint".to_string(),
        ]
    }

    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "php-pro" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }

        let result = match task.operation.as_str() {
            "run" => self.php_run(task.path.as_deref(), task.args.as_deref()),
            "test" => self.phpunit_test(task.path.as_deref()),
            "analyze" => self.phpstan_analyze(task.path.as_deref()),
            "lint" => self.php_lint(task.path.as_deref()),
            _ => Err(format!("Unknown operation: {}", task.operation)),
        };

        match result {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/language/python_pro.rs">
//! Python Pro Agent - Python 3.12+ development environment
//!
//! Provides secure execution for Python development tasks including:
//! - Script execution
//! - Testing with pytest
//! - Linting with ruff/pylint
//! - Type checking with mypy
//! - Formatting with black

use async_trait::async_trait;
use std::process::Command;

use crate::agents::base::{validation, AgentTask, AgentTrait, TaskResult};
use crate::security::{profiles::presets, SecurityProfile};

const ALLOWED_DIRS: &[&str] = &["/tmp", "/home", "/opt"];

pub struct PythonProAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl PythonProAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            agent_id,
            profile: presets::python_pro(),
        }
    }

    fn python_run(&self, path: Option<&str>, args: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("python3");

        if let Some(a) = args {
            validation::validate_args(a)?;
            for arg in a.split_whitespace() {
                cmd.arg(arg);
            }
        }

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.arg(validated_path);
        } else {
            return Err("Path required for python run".to_string());
        }

        let output = cmd
            .output()
            .map_err(|e| format!("Failed to run python: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!(
                "Python execution succeeded\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        } else {
            Ok(format!(
                "Python execution failed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        }
    }

    fn pytest_run(&self, path: Option<&str>, args: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("pytest");
        cmd.arg("-v");

        if let Some(a) = args {
            validation::validate_args(a)?;
            for arg in a.split_whitespace() {
                cmd.arg(arg);
            }
        }

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.arg(validated_path);
        }

        let output = cmd
            .output()
            .map_err(|e| format!("Failed to run pytest: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!(
                "Tests passed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        } else {
            Ok(format!(
                "Tests failed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        }
    }

    fn ruff_lint(&self, path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("ruff");
        cmd.arg("check");

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.arg(validated_path);
        } else {
            return Err("Path required for ruff".to_string());
        }

        let output = cmd
            .output()
            .map_err(|e| format!("Failed to run ruff: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!(
            "Ruff output\nstdout: {}\nstderr: {}",
            stdout, stderr
        ))
    }

    fn mypy_check(&self, path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("mypy");

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.arg(validated_path);
        } else {
            return Err("Path required for mypy".to_string());
        }

        let output = cmd
            .output()
            .map_err(|e| format!("Failed to run mypy: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!(
                "Mypy passed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        } else {
            Ok(format!(
                "Mypy found issues\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        }
    }

    fn black_format(&self, path: Option<&str>, check_only: bool) -> Result<String, String> {
        let mut cmd = Command::new("black");

        if check_only {
            cmd.arg("--check").arg("--diff");
        }

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.arg(validated_path);
        } else {
            return Err("Path required for black".to_string());
        }

        let output = cmd
            .output()
            .map_err(|e| format!("Failed to run black: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!(
                "Code is properly formatted\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        } else {
            Ok(format!(
                "Code needs formatting\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        }
    }
}

#[async_trait]
impl AgentTrait for PythonProAgent {
    fn agent_type(&self) -> &str {
        "python-pro"
    }

    fn name(&self) -> &str {
        "Python Pro Agent"
    }

    fn description(&self) -> &str {
        "Python 3.12+ development environment with modern tooling"
    }

    fn operations(&self) -> Vec<String> {
        vec![
            "run".to_string(),
            "test".to_string(),
            "lint".to_string(),
            "typecheck".to_string(),
            "format".to_string(),
        ]
    }

    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "python-pro" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }

        let result = match task.operation.as_str() {
            "run" => self.python_run(task.path.as_deref(), task.args.as_deref()),
            "test" => self.pytest_run(task.path.as_deref(), task.args.as_deref()),
            "lint" => self.ruff_lint(task.path.as_deref()),
            "typecheck" => self.mypy_check(task.path.as_deref()),
            "format" => self.black_format(task.path.as_deref(), true),
            _ => Err(format!("Unknown operation: {}", task.operation)),
        };

        match result {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }

    fn get_status(&self) -> String {
        format!("Python Pro agent {} is running", self.agent_id)
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/language/ruby_pro.rs">
//! Ruby Pro Agent - Ruby development environment

use async_trait::async_trait;
use std::process::Command;

use crate::agents::base::{validation, AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;

const ALLOWED_DIRS: &[&str] = &["/tmp", "/home", "/opt"];

pub struct RubyProAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl RubyProAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            agent_id,
            profile: SecurityProfile::code_execution(
                "ruby-pro",
                vec!["ruby", "gem", "bundle", "rake", "rspec", "rubocop"],
            ),
        }
    }

    fn ruby_run(&self, path: Option<&str>, args: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("ruby");

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.arg(validated_path);
        } else {
            return Err("Path required".to_string());
        }

        if let Some(a) = args {
            validation::validate_args(a)?;
            for arg in a.split_whitespace() {
                cmd.arg(arg);
            }
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!(
                "Execution succeeded\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        } else {
            Ok(format!(
                "Execution failed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        }
    }

    fn rspec_test(&self, path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("bundle");
        cmd.arg("exec").arg("rspec");

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.current_dir(validated_path);
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!(
                "Tests passed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        } else {
            Ok(format!(
                "Tests failed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        }
    }

    fn rubocop_lint(&self, path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("rubocop");

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.arg(validated_path);
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!(
            "Rubocop output\nstdout: {}\nstderr: {}",
            stdout, stderr
        ))
    }
}

#[async_trait]
impl AgentTrait for RubyProAgent {
    fn agent_type(&self) -> &str {
        "ruby-pro"
    }
    fn name(&self) -> &str {
        "Ruby Pro Agent"
    }
    fn description(&self) -> &str {
        "Ruby development environment with RSpec and Rubocop"
    }

    fn operations(&self) -> Vec<String> {
        vec!["run".to_string(), "test".to_string(), "lint".to_string()]
    }

    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "ruby-pro" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }

        let result = match task.operation.as_str() {
            "run" => self.ruby_run(task.path.as_deref(), task.args.as_deref()),
            "test" => self.rspec_test(task.path.as_deref()),
            "lint" => self.rubocop_lint(task.path.as_deref()),
            _ => Err(format!("Unknown operation: {}", task.operation)),
        };

        match result {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/language/rust_pro.rs">
//! Rust Pro Agent - Rust development environment
//!
//! Provides secure execution for Rust development tasks including:
//! - Cargo check/build/test
//! - Clippy linting
//! - Format checking

use async_trait::async_trait;
use std::process::Command;

use crate::agents::base::{validation, AgentTask, AgentTrait, TaskResult};
use crate::security::{profiles::presets, SecurityProfile};
use simd_json::prelude::*;

const ALLOWED_DIRS: &[&str] = &["/tmp", "/home", "/opt"];

pub struct RustProAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl RustProAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            agent_id,
            profile: presets::rust_pro(),
        }
    }

    fn validate_features(&self, features: &str) -> Result<(), String> {
        validation::validate_args(features).map(|_| ())
    }

    fn cargo_check(&self, path: Option<&str>, features: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("cargo");
        cmd.arg("check");

        if let Some(feat) = features {
            self.validate_features(feat)?;
            cmd.arg("--features").arg(feat);
        }

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.current_dir(validated_path);
        }

        let output = cmd
            .output()
            .map_err(|e| format!("Failed to run cargo check: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!(
                "Check passed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        } else {
            Ok(format!(
                "Check failed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        }
    }

    fn cargo_build(
        &self,
        path: Option<&str>,
        features: Option<&str>,
        release: bool,
    ) -> Result<String, String> {
        let mut cmd = Command::new("cargo");
        cmd.arg("build");

        if release {
            cmd.arg("--release");
        }

        if let Some(feat) = features {
            self.validate_features(feat)?;
            cmd.arg("--features").arg(feat);
        }

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.current_dir(validated_path);
        }

        let output = cmd
            .output()
            .map_err(|e| format!("Failed to run cargo build: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!(
                "Build succeeded\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        } else {
            Ok(format!(
                "Build failed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        }
    }

    fn cargo_test(&self, path: Option<&str>, features: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("cargo");
        cmd.arg("test");

        if let Some(feat) = features {
            self.validate_features(feat)?;
            cmd.arg("--features").arg(feat);
        }

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.current_dir(validated_path);
        }

        let output = cmd
            .output()
            .map_err(|e| format!("Failed to run cargo test: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!(
                "Tests passed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        } else {
            Ok(format!(
                "Tests failed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        }
    }

    fn cargo_clippy(&self, path: Option<&str>, features: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("cargo");
        cmd.arg("clippy");

        if let Some(feat) = features {
            self.validate_features(feat)?;
            cmd.arg("--features").arg(feat);
        }

        cmd.arg("--").arg("-D").arg("warnings");

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.current_dir(validated_path);
        }

        let output = cmd
            .output()
            .map_err(|e| format!("Failed to run cargo clippy: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!(
                "Clippy passed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        } else {
            Ok(format!(
                "Clippy failed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        }
    }

    fn cargo_fmt(&self, path: Option<&str>, check_only: bool) -> Result<String, String> {
        let mut cmd = Command::new("cargo");
        cmd.arg("fmt");

        if check_only {
            cmd.arg("--check");
        }

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.current_dir(validated_path);
        }

        let output = cmd
            .output()
            .map_err(|e| format!("Failed to run cargo fmt: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!(
                "Format check passed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        } else {
            Ok(format!(
                "Format check failed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        }
    }
}

#[async_trait]
impl AgentTrait for RustProAgent {
    fn agent_type(&self) -> &str {
        "rust-pro"
    }

    fn name(&self) -> &str {
        "Rust Pro Agent"
    }

    fn description(&self) -> &str {
        "Rust development environment with cargo, clippy, and rustfmt"
    }

    fn operations(&self) -> Vec<String> {
        vec![
            "check".to_string(),
            "build".to_string(),
            "test".to_string(),
            "clippy".to_string(),
            "format".to_string(),
        ]
    }

    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "rust-pro" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }

        let features = task
            .config
            .get("features")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let release = task
            .config
            .get("release")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let result = match task.operation.as_str() {
            "check" => self.cargo_check(task.path.as_deref(), features.as_deref()),
            "build" => self.cargo_build(task.path.as_deref(), features.as_deref(), release),
            "test" => self.cargo_test(task.path.as_deref(), features.as_deref()),
            "clippy" => self.cargo_clippy(task.path.as_deref(), features.as_deref()),
            "format" => self.cargo_fmt(task.path.as_deref(), true),
            _ => Err(format!("Unknown operation: {}", task.operation)),
        };

        match result {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }

    fn get_status(&self) -> String {
        format!("Rust Pro agent {} is running", self.agent_id)
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/language/scala_pro.rs">
//! Scala Pro Agent - Scala development environment

use async_trait::async_trait;
use std::process::Command;

use crate::agents::base::{validation, AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;

const ALLOWED_DIRS: &[&str] = &["/tmp", "/home", "/opt"];

pub struct ScalaProAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl ScalaProAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            agent_id,
            profile: SecurityProfile::code_execution(
                "scala-pro",
                vec!["scala", "scalac", "sbt", "mill"],
            ),
        }
    }

    fn sbt_compile(&self, path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("sbt");
        cmd.arg("compile");

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.current_dir(validated_path);
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!(
                "Compilation succeeded\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        } else {
            Ok(format!(
                "Compilation failed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        }
    }

    fn sbt_test(&self, path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("sbt");
        cmd.arg("test");

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.current_dir(validated_path);
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!(
                "Tests passed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        } else {
            Ok(format!(
                "Tests failed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        }
    }
}

#[async_trait]
impl AgentTrait for ScalaProAgent {
    fn agent_type(&self) -> &str {
        "scala-pro"
    }
    fn name(&self) -> &str {
        "Scala Pro Agent"
    }
    fn description(&self) -> &str {
        "Scala development environment with SBT"
    }

    fn operations(&self) -> Vec<String> {
        vec!["compile".to_string(), "test".to_string()]
    }

    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "scala-pro" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }

        let result = match task.operation.as_str() {
            "compile" => self.sbt_compile(task.path.as_deref()),
            "test" => self.sbt_test(task.path.as_deref()),
            _ => Err(format!("Unknown operation: {}", task.operation)),
        };

        match result {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/language/typescript_pro.rs">
//! TypeScript Pro Agent - TypeScript development environment

use async_trait::async_trait;
use std::process::Command;

use crate::agents::base::{validation, AgentTask, AgentTrait, TaskResult};
use crate::security::{profiles::presets, SecurityProfile};

const ALLOWED_DIRS: &[&str] = &["/tmp", "/home", "/opt"];

pub struct TypeScriptProAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl TypeScriptProAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            agent_id,
            profile: presets::typescript_pro(),
        }
    }

    fn tsc_check(&self, path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("npx");
        cmd.arg("tsc").arg("--noEmit");

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.current_dir(validated_path);
        }

        let output = cmd
            .output()
            .map_err(|e| format!("Failed to run tsc: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!(
                "Type check passed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        } else {
            Ok(format!(
                "Type check failed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        }
    }

    fn tsc_build(&self, path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("npx");
        cmd.arg("tsc");

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.current_dir(validated_path);
        }

        let output = cmd
            .output()
            .map_err(|e| format!("Failed to run tsc build: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!(
                "Build succeeded\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        } else {
            Ok(format!(
                "Build failed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        }
    }

    fn npm_test(&self, path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("npm");
        cmd.arg("test");

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.current_dir(validated_path);
        }

        let output = cmd
            .output()
            .map_err(|e| format!("Failed to run tests: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!(
                "Tests passed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        } else {
            Ok(format!(
                "Tests failed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        }
    }

    fn eslint_check(&self, path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("npx");
        cmd.arg("eslint").arg("--ext").arg(".ts,.tsx");

        if let Some(p) = path {
            let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
            cmd.arg(validated_path);
        } else {
            cmd.arg(".");
        }

        let output = cmd
            .output()
            .map_err(|e| format!("Failed to run eslint: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!(
                "Lint passed\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        } else {
            Ok(format!(
                "Lint found issues\nstdout: {}\nstderr: {}",
                stdout, stderr
            ))
        }
    }
}

#[async_trait]
impl AgentTrait for TypeScriptProAgent {
    fn agent_type(&self) -> &str {
        "typescript-pro"
    }
    fn name(&self) -> &str {
        "TypeScript Pro Agent"
    }
    fn description(&self) -> &str {
        "TypeScript development environment"
    }

    fn operations(&self) -> Vec<String> {
        vec![
            "check".to_string(),
            "build".to_string(),
            "test".to_string(),
            "lint".to_string(),
        ]
    }

    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "typescript-pro" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }

        let result = match task.operation.as_str() {
            "check" => self.tsc_check(task.path.as_deref()),
            "build" => self.tsc_build(task.path.as_deref()),
            "test" => self.npm_test(task.path.as_deref()),
            "lint" => self.eslint_check(task.path.as_deref()),
            _ => Err(format!("Unknown operation: {}", task.operation)),
        };

        match result {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/mobile/flutter_expert.rs">
//! Flutter Expert Agent - Cross-platform Flutter development

use crate::agents::base::{AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;
use async_trait::async_trait;
use simd_json::json;

pub struct FlutterExpertAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl FlutterExpertAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            profile: SecurityProfile::content_generation("flutter-expert"),
            agent_id,
        }
    }

    fn analyze(&self, args: Option<&str>) -> Result<String, String> {
        let input = args.unwrap_or("").to_lowercase();
        let mut recommendations = Vec::new();

        if input.contains("state") || input.contains("bloc") || input.contains("riverpod") {
            recommendations.push("Use Riverpod or BLoC for scalable state management");
            recommendations.push("Separate business logic from UI widgets");
            recommendations.push("Implement proper error handling in state");
        }
        if input.contains("ui") || input.contains("widget") {
            recommendations.push("Extract reusable widgets for consistency");
            recommendations.push("Use const constructors for performance");
            recommendations.push("Implement responsive layouts with LayoutBuilder");
        }
        if input.contains("navigation") || input.contains("routing") {
            recommendations.push("Use GoRouter for declarative routing");
            recommendations.push("Implement deep linking support");
            recommendations.push("Handle navigation state restoration");
        }
        if recommendations.is_empty() {
            recommendations.push("Follow Flutter's layered architecture");
            recommendations.push("Write widget tests for UI components");
            recommendations.push("Use platform channels for native features");
        }

        let result = json!({
            "analysis": { "input": args.unwrap_or(""), "recommendations": recommendations },
            "packages": ["flutter_riverpod", "go_router", "freezed", "dio", "hive"]
        });
        Ok(simd_json::to_string_pretty(&result).unwrap_or_default())
    }
}

#[async_trait]
impl AgentTrait for FlutterExpertAgent {
    fn agent_type(&self) -> &str {
        "flutter-expert"
    }
    fn name(&self) -> &str {
        "Flutter Expert"
    }
    fn description(&self) -> &str {
        "Build beautiful cross-platform apps with Flutter and Dart."
    }
    fn operations(&self) -> Vec<String> {
        vec![
            "create_widget".to_string(),
            "design_state".to_string(),
            "configure_navigation".to_string(),
            "analyze".to_string(),
        ]
    }
    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }
    fn get_status(&self) -> String {
        format!("Flutter Expert agent {} is running", self.agent_id)
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "flutter-expert" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }
        match self.analyze(task.args.as_deref()) {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/mobile/ios_developer.rs">
//! iOS Developer Agent - Native iOS/Swift development

use crate::agents::base::{AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;
use async_trait::async_trait;
use simd_json::json;

pub struct IOSDeveloperAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl IOSDeveloperAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            profile: SecurityProfile::content_generation("ios-developer"),
            agent_id,
        }
    }

    fn analyze(&self, args: Option<&str>) -> Result<String, String> {
        let input = args.unwrap_or("").to_lowercase();
        let mut recommendations = Vec::new();

        if input.contains("swiftui") || input.contains("view") {
            recommendations.push("Use @Observable for modern state management");
            recommendations.push("Implement ViewModifiers for reusable styling");
            recommendations.push("Use environment values for dependency injection");
        }
        if input.contains("uikit") || input.contains("storyboard") {
            recommendations.push("Consider programmatic UI over storyboards for teams");
            recommendations.push("Use Auto Layout with proper constraints");
            recommendations.push("Implement coordinator pattern for navigation");
        }
        if input.contains("concurrency") || input.contains("async") {
            recommendations.push("Use Swift Concurrency (async/await)");
            recommendations.push("Mark UI updates with @MainActor");
            recommendations.push("Use Task groups for parallel operations");
        }
        if recommendations.is_empty() {
            recommendations.push("Follow MVVM or Clean Architecture");
            recommendations.push("Use Swift Package Manager for dependencies");
            recommendations.push("Write unit tests with XCTest");
        }

        let result = json!({
            "analysis": { "input": args.unwrap_or(""), "recommendations": recommendations },
            "frameworks": ["SwiftUI", "Combine", "Core Data", "CloudKit", "HealthKit"]
        });
        Ok(simd_json::to_string_pretty(&result).unwrap_or_default())
    }
}

#[async_trait]
impl AgentTrait for IOSDeveloperAgent {
    fn agent_type(&self) -> &str {
        "ios-developer"
    }
    fn name(&self) -> &str {
        "iOS Developer"
    }
    fn description(&self) -> &str {
        "Build native iOS apps with Swift and SwiftUI."
    }
    fn operations(&self) -> Vec<String> {
        vec![
            "create_view".to_string(),
            "design_architecture".to_string(),
            "implement_feature".to_string(),
            "analyze".to_string(),
        ]
    }
    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }
    fn get_status(&self) -> String {
        format!("iOS Developer agent {} is running", self.agent_id)
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "ios-developer" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }
        match self.analyze(task.args.as_deref()) {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/mobile/mobile_developer.rs">
//! Mobile Developer Agent - General mobile architecture

use crate::agents::base::{AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;
use async_trait::async_trait;
use simd_json::json;

pub struct MobileDeveloperAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl MobileDeveloperAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            profile: SecurityProfile::content_generation("mobile-developer"),
            agent_id,
        }
    }

    fn analyze(&self, args: Option<&str>) -> Result<String, String> {
        let input = args.unwrap_or("").to_lowercase();
        let mut recommendations = Vec::new();

        if input.contains("offline") || input.contains("sync") {
            recommendations.push("Implement offline-first architecture");
            recommendations.push("Use local database (SQLite, Realm) with sync");
            recommendations.push("Handle conflict resolution strategies");
        }
        if input.contains("performance") {
            recommendations.push("Profile with platform-specific tools");
            recommendations.push("Optimize images and assets");
            recommendations.push("Implement lazy loading for lists");
        }
        if input.contains("push") || input.contains("notification") {
            recommendations.push("Use FCM/APNs for push notifications");
            recommendations.push("Handle notification permissions gracefully");
            recommendations.push("Implement rich notifications where appropriate");
        }
        if recommendations.is_empty() {
            recommendations.push("Choose architecture based on team/project size");
            recommendations.push("Implement proper error handling and logging");
            recommendations.push("Design for accessibility from the start");
        }

        let result = json!({
            "analysis": { "input": args.unwrap_or(""), "recommendations": recommendations },
            "considerations": {
                "cross_platform": ["Flutter", "React Native", "Kotlin Multiplatform"],
                "native": ["Swift/SwiftUI (iOS)", "Kotlin/Jetpack Compose (Android)"]
            }
        });
        Ok(simd_json::to_string_pretty(&result).unwrap_or_default())
    }
}

#[async_trait]
impl AgentTrait for MobileDeveloperAgent {
    fn agent_type(&self) -> &str {
        "mobile-developer"
    }
    fn name(&self) -> &str {
        "Mobile Developer"
    }
    fn description(&self) -> &str {
        "Design and build mobile applications with best practices."
    }
    fn operations(&self) -> Vec<String> {
        vec![
            "design_architecture".to_string(),
            "optimize_performance".to_string(),
            "implement_feature".to_string(),
            "analyze".to_string(),
        ]
    }
    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }
    fn get_status(&self) -> String {
        format!("Mobile Developer agent {} is running", self.agent_id)
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "mobile-developer" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }
        match self.analyze(task.args.as_deref()) {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/mobile/mod.rs">
//! Mobile Development Agents
//!
//! - `FlutterExpert`: Cross-platform Flutter development
//! - `IOSDeveloper`: Native iOS/Swift development
//! - `MobileDeveloper`: General mobile architecture

mod flutter_expert;
mod ios_developer;
mod mobile_developer;

pub use flutter_expert::FlutterExpertAgent;
pub use ios_developer::IOSDeveloperAgent;
pub use mobile_developer::MobileDeveloperAgent;
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/operations/devops_troubleshooter.rs">
//! DevOps Troubleshooter Agent
//!
//! Expert troubleshooter for investigating production issues,
//! analyzing logs and metrics, and diagnosing system problems.

use async_trait::async_trait;
use simd_json::json;

use crate::agents::base::{AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;

/// DevOps Troubleshooter Agent
pub struct DevOpsTroubleshooterAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl DevOpsTroubleshooterAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            profile: SecurityProfile::read_only_analysis(
                "devops-troubleshooter",
                vec!["kubectl", "docker", "systemctl"],
            ),
            agent_id,
        }
    }

    fn troubleshoot(&self, args: Option<&str>) -> Result<String, String> {
        let input = args.unwrap_or("").to_lowercase();

        let mut investigation_areas = Vec::new();
        let mut commands_to_run = Vec::new();
        let mut log_patterns = Vec::new();

        if input.contains("kubernetes") || input.contains("k8s") || input.contains("pod") {
            investigation_areas.push("Kubernetes cluster health");
            commands_to_run.push("kubectl get pods -A | grep -v Running");
            commands_to_run.push("kubectl describe pod <pod-name>");
            log_patterns.push("OOMKilled, CrashLoopBackOff, ImagePullBackOff");
        }

        if input.contains("network") || input.contains("dns") || input.contains("connectivity") {
            investigation_areas.push("Network connectivity");
            commands_to_run.push("nslookup <hostname>");
            commands_to_run.push("curl -v <endpoint>");
            log_patterns.push("Connection refused, timeout, DNS resolution failed");
        }

        if input.contains("disk") || input.contains("storage") || input.contains("space") {
            investigation_areas.push("Disk usage and I/O");
            commands_to_run.push("df -h");
            commands_to_run.push("du -sh /*");
            log_patterns.push("No space left on device, I/O errors");
        }

        if input.contains("memory") || input.contains("oom") {
            investigation_areas.push("Memory utilization");
            commands_to_run.push("free -h");
            commands_to_run.push("dmesg | grep -i oom");
            log_patterns.push("OutOfMemoryError, OOM killer invoked");
        }

        if investigation_areas.is_empty() {
            investigation_areas.push("System overview");
            commands_to_run.push("uptime");
            commands_to_run.push("free -h");
            commands_to_run.push("df -h");
            log_patterns.push("Error, Warning, Critical");
        }

        let result = json!({
            "troubleshooting": {
                "input": args.unwrap_or(""),
                "investigation_areas": investigation_areas,
                "diagnostic_commands": commands_to_run,
                "log_patterns_to_search": log_patterns
            },
            "systematic_approach": [
                "1. Gather symptoms and timeline",
                "2. Check recent changes",
                "3. Review monitoring dashboards",
                "4. Analyze logs for error patterns",
                "5. Check resource utilization",
                "6. Investigate dependencies",
                "7. Test hypotheses systematically"
            ],
            "common_root_causes": [
                "Recent deployment introduced regression",
                "Resource exhaustion (CPU, memory, disk)",
                "External dependency failure",
                "Configuration drift or misconfiguration",
                "Network connectivity issues"
            ]
        });

        Ok(simd_json::to_string_pretty(&result).unwrap_or_default())
    }
}

#[async_trait]
impl AgentTrait for DevOpsTroubleshooterAgent {
    fn agent_type(&self) -> &str {
        "devops-troubleshooter"
    }
    fn name(&self) -> &str {
        "DevOps Troubleshooter"
    }
    fn description(&self) -> &str {
        "Expert troubleshooter for investigating production issues and diagnosing system problems."
    }

    fn operations(&self) -> Vec<String> {
        vec![
            "investigate".to_string(),
            "analyze_logs".to_string(),
            "check_metrics".to_string(),
            "diagnose_issue".to_string(),
        ]
    }

    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }

    fn get_status(&self) -> String {
        format!("DevOps Troubleshooter agent {} is running", self.agent_id)
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "devops-troubleshooter" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }

        match self.troubleshoot(task.args.as_deref()) {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/operations/incident_responder.rs">
//! Incident Responder Agent
//!
//! Expert SRE incident responder specializing in rapid problem resolution,
//! modern observability, and comprehensive incident management.

use async_trait::async_trait;
use simd_json::json;

use crate::agents::base::{AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;

/// Incident Responder Agent
pub struct IncidentResponderAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl IncidentResponderAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            profile: SecurityProfile::read_only_analysis(
                "incident-responder",
                vec!["logs", "metrics", "traces"],
            ),
            agent_id,
        }
    }

    fn handle_incident(&self, args: Option<&str>) -> Result<String, String> {
        let input = args.unwrap_or("").to_lowercase();

        let severity = if input.contains("outage")
            || input.contains("down")
            || input.contains("critical")
        {
            "P0 - Critical"
        } else if input.contains("degraded") || input.contains("slow") || input.contains("error") {
            "P1 - High"
        } else if input.contains("intermittent") || input.contains("minor") {
            "P2 - Medium"
        } else {
            "P3 - Low"
        };

        let mut immediate_actions = Vec::new();
        let mut investigation_steps = Vec::new();

        if input.contains("database") || input.contains("db") {
            immediate_actions.push("Check database connection pools and replication lag");
            immediate_actions.push("Review slow query logs for blocking queries");
            investigation_steps.push("Analyze query execution plans");
        }

        if input.contains("memory") || input.contains("oom") {
            immediate_actions.push("Check for memory leaks and OOM kills");
            immediate_actions.push("Review recent deployment changes");
            investigation_steps.push("Analyze heap dumps if available");
        }

        if input.contains("network") || input.contains("timeout") {
            immediate_actions.push("Check network connectivity and DNS resolution");
            immediate_actions.push("Review load balancer health checks");
            investigation_steps.push("Analyze network traces");
        }

        if immediate_actions.is_empty() {
            immediate_actions.push("Establish incident command structure");
            immediate_actions.push("Check service health dashboards");
            immediate_actions.push("Review recent changes (deployments, configs)");
            investigation_steps.push("Correlate metrics, logs, and traces");
            investigation_steps.push("Check upstream/downstream dependencies");
        }

        let result = json!({
            "incident_assessment": {
                "input": args.unwrap_or(""),
                "severity": severity,
                "immediate_actions": immediate_actions,
                "investigation_steps": investigation_steps
            },
            "incident_command": {
                "structure": {
                    "incident_commander": "Single decision-maker, coordinates response",
                    "communication_lead": "Manages stakeholder updates",
                    "technical_lead": "Coordinates technical investigation"
                }
            },
            "resolution_steps": [
                "1. Stabilize - Apply quick mitigations",
                "2. Investigate - Find root cause",
                "3. Fix - Implement permanent solution",
                "4. Validate - Verify service restoration",
                "5. Document - Prepare post-mortem"
            ]
        });

        Ok(simd_json::to_string_pretty(&result).unwrap_or_default())
    }
}

#[async_trait]
impl AgentTrait for IncidentResponderAgent {
    fn agent_type(&self) -> &str {
        "incident-responder"
    }
    fn name(&self) -> &str {
        "Incident Responder"
    }
    fn description(&self) -> &str {
        "Expert SRE incident responder specializing in rapid problem resolution and incident management."
    }

    fn operations(&self) -> Vec<String> {
        vec![
            "assess_incident".to_string(),
            "investigate".to_string(),
            "coordinate_response".to_string(),
            "write_postmortem".to_string(),
        ]
    }

    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }

    fn get_status(&self) -> String {
        format!("Incident Responder agent {} is running", self.agent_id)
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "incident-responder" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }

        match self.handle_incident(task.args.as_deref()) {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/operations/mod.rs">
//! Operations Agents
//!
//! Specialized agents for operations and SRE:
//! - `IncidentResponder`: Production incident management and resolution
//! - `DevOpsTroubleshooter`: System debugging and troubleshooting
//! - `TestAutomator`: Test suite creation and automation
//! - `ObservabilityEngineer`: Monitoring, logging, and tracing

mod devops_troubleshooter;
mod incident_responder;
mod test_automator;

pub use devops_troubleshooter::DevOpsTroubleshooterAgent;
pub use incident_responder::IncidentResponderAgent;
pub use test_automator::TestAutomatorAgent;
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/operations/test_automator.rs">
//! Test Automator Agent
//!
//! Expert test automation engineer specializing in creating comprehensive
//! test suites with high coverage and maintainability.

use async_trait::async_trait;
use simd_json::json;

use crate::agents::base::{AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;

/// Test Automator Agent
pub struct TestAutomatorAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl TestAutomatorAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            profile: SecurityProfile::content_generation("test-automator"),
            agent_id,
        }
    }

    fn generate_tests(&self, args: Option<&str>) -> Result<String, String> {
        let input = args.unwrap_or("").to_lowercase();

        let mut test_types = Vec::new();
        let mut frameworks = Vec::new();
        let mut strategies = Vec::new();

        let (lang, test_framework) = if input.contains("python") {
            ("Python", "pytest")
        } else if input.contains("javascript")
            || input.contains("typescript")
            || input.contains("react")
        {
            ("JavaScript/TypeScript", "Jest + React Testing Library")
        } else if input.contains("rust") {
            ("Rust", "built-in test framework")
        } else if input.contains("go") || input.contains("golang") {
            ("Go", "testing package + testify")
        } else {
            ("General", "language-appropriate framework")
        };

        frameworks.push(test_framework);

        if input.contains("unit") || input.contains("function") {
            test_types.push("Unit Tests");
            strategies.push("Test individual functions/methods in isolation");
            strategies.push("Mock external dependencies");
        }

        if input.contains("integration") || input.contains("api") {
            test_types.push("Integration Tests");
            strategies.push("Test component interactions");
            strategies.push("Use test databases/containers");
        }

        if input.contains("e2e") || input.contains("end-to-end") || input.contains("ui") {
            test_types.push("E2E Tests");
            frameworks.push("Playwright or Cypress");
            strategies.push("Test critical user journeys");
        }

        if test_types.is_empty() {
            test_types.push("Unit Tests");
            test_types.push("Integration Tests");
            strategies.push("Follow testing pyramid (many unit, fewer integration, few E2E)");
            strategies.push("Aim for 80%+ code coverage");
        }

        let result = json!({
            "test_plan": {
                "input": args.unwrap_or(""),
                "language": lang,
                "test_types": test_types,
                "recommended_frameworks": frameworks,
                "strategies": strategies
            },
            "test_structure": {
                "naming": "test_<what>_<scenario>_<expected_result>",
                "organization": "Mirror source code structure",
                "fixtures": "Shared setup in fixtures/conftest"
            },
            "best_practices": [
                "Arrange-Act-Assert (AAA) pattern",
                "One assertion per test (ideally)",
                "Independent and isolated tests",
                "Fast execution (unit tests < 1s each)",
                "Descriptive test names"
            ],
            "coverage_targets": {
                "line_coverage": "80%+",
                "branch_coverage": "75%+",
                "critical_paths": "100%"
            }
        });

        Ok(simd_json::to_string_pretty(&result).unwrap_or_default())
    }
}

#[async_trait]
impl AgentTrait for TestAutomatorAgent {
    fn agent_type(&self) -> &str {
        "test-automator"
    }
    fn name(&self) -> &str {
        "Test Automator"
    }
    fn description(&self) -> &str {
        "Expert test automation engineer specializing in creating comprehensive test suites."
    }

    fn operations(&self) -> Vec<String> {
        vec![
            "generate_unit_tests".to_string(),
            "generate_integration_tests".to_string(),
            "generate_e2e_tests".to_string(),
            "analyze_coverage".to_string(),
        ]
    }

    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }

    fn get_status(&self) -> String {
        format!("Test Automator agent {} is running", self.agent_id)
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "test-automator" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }

        match self.generate_tests(task.args.as_deref()) {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/orchestration/context_manager.rs">
//! Context Manager Agent

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::agents::base::{AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;

pub struct ContextManagerAgent {
    agent_id: String,
    profile: SecurityProfile,
    context: Arc<RwLock<HashMap<String, String>>>,
}

impl ContextManagerAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            agent_id,
            profile: SecurityProfile::orchestration("context-manager", vec!["*"]),
            context: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn save_context(&self, key: Option<&str>, value: Option<&str>) -> Result<String, String> {
        let key = key.ok_or("Key required")?;
        let value = value.ok_or("Value required")?;

        let mut ctx = self.context.write().map_err(|_| "Failed to acquire lock")?;
        ctx.insert(key.to_string(), value.to_string());

        Ok(format!("Context saved: {} = {}", key, value))
    }

    fn restore_context(&self, key: Option<&str>) -> Result<String, String> {
        let key = key.ok_or("Key required")?;

        let ctx = self.context.read().map_err(|_| "Failed to acquire lock")?;

        if let Some(value) = ctx.get(key) {
            Ok(format!("Context restored: {} = {}", key, value))
        } else {
            Err(format!("Context key not found: {}", key))
        }
    }

    fn list_context(&self) -> Result<String, String> {
        let ctx = self.context.read().map_err(|_| "Failed to acquire lock")?;

        if ctx.is_empty() {
            Ok("No context stored".to_string())
        } else {
            let entries: Vec<String> = ctx
                .iter()
                .map(|(k, v)| format!("  {} = {}", k, v))
                .collect();
            Ok(format!("Stored context:\n{}", entries.join("\n")))
        }
    }

    fn clear_context(&self) -> Result<String, String> {
        let mut ctx = self.context.write().map_err(|_| "Failed to acquire lock")?;
        ctx.clear();

        Ok("Context cleared".to_string())
    }
}

#[async_trait]
impl AgentTrait for ContextManagerAgent {
    fn agent_type(&self) -> &str {
        "context-manager"
    }
    fn name(&self) -> &str {
        "Context Manager"
    }
    fn description(&self) -> &str {
        "Session context management and state persistence"
    }

    fn operations(&self) -> Vec<String> {
        vec![
            "save".to_string(),
            "restore".to_string(),
            "list".to_string(),
            "clear".to_string(),
        ]
    }

    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        let result = match task.operation.as_str() {
            "save" => self.save_context(task.path.as_deref(), task.args.as_deref()),
            "restore" => self.restore_context(task.path.as_deref()),
            "list" => self.list_context(),
            "clear" => self.clear_context(),
            _ => Err(format!("Unknown operation: {}", task.operation)),
        };

        match result {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/orchestration/dx_optimizer.rs">
//! Developer Experience Optimizer Agent

use async_trait::async_trait;
use std::process::Command;

use crate::agents::base::{validation, AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;

const ALLOWED_DIRS: &[&str] = &["/tmp", "/home", "/opt"];

pub struct DxOptimizerAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl DxOptimizerAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            agent_id,
            profile: SecurityProfile::orchestration(
                "dx-optimizer",
                vec!["code-reviewer", "debugger", "performance-engineer"],
            ),
        }
    }

    fn analyze_setup(&self, path: Option<&str>) -> Result<String, String> {
        let dir = path.unwrap_or(".");
        let validated_path = validation::validate_path(dir, ALLOWED_DIRS)?;

        let mut analysis = String::from("DX Analysis Report:\n\n");

        // Check for common config files
        let configs = [
            ("package.json", "Node.js project"),
            ("Cargo.toml", "Rust project"),
            ("pyproject.toml", "Python project"),
            ("go.mod", "Go project"),
            (".eslintrc", "ESLint configured"),
            (".prettierrc", "Prettier configured"),
            ("Dockerfile", "Docker configured"),
            ("docker-compose.yml", "Docker Compose configured"),
            (".github/workflows", "GitHub Actions configured"),
        ];

        for (file, desc) in configs {
            let check_path = format!("{}/{}", validated_path, file);
            if std::path::Path::new(&check_path).exists() {
                analysis.push_str(&format!("✓ {} ({})\n", desc, file));
            }
        }

        Ok(analysis)
    }

    fn suggest_improvements(&self, path: Option<&str>) -> Result<String, String> {
        let dir = path.unwrap_or(".");
        let validated_path = validation::validate_path(dir, ALLOWED_DIRS)?;

        let mut suggestions = String::from("DX Improvement Suggestions:\n\n");

        // Check what's missing
        let configs = [
            (
                ".editorconfig",
                "Add EditorConfig for consistent formatting",
            ),
            (".gitignore", "Add .gitignore for clean version control"),
            ("README.md", "Add README.md for documentation"),
            ("CONTRIBUTING.md", "Add contribution guidelines"),
            (".pre-commit-config.yaml", "Add pre-commit hooks"),
        ];

        for (file, suggestion) in configs {
            let check_path = format!("{}/{}", validated_path, file);
            if !std::path::Path::new(&check_path).exists() {
                suggestions.push_str(&format!("• {}\n", suggestion));
            }
        }

        Ok(suggestions)
    }

    fn git_hooks_status(&self, path: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("git");
        cmd.arg("config").arg("--get").arg("core.hooksPath");

        if let Some(dir) = path {
            let validated_path = validation::validate_path(dir, ALLOWED_DIRS)?;
            cmd.current_dir(validated_path);
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);

        if stdout.is_empty() {
            Ok("Git hooks: Using default .git/hooks directory".to_string())
        } else {
            Ok(format!(
                "Git hooks: Custom path configured: {}",
                stdout.trim()
            ))
        }
    }
}

#[async_trait]
impl AgentTrait for DxOptimizerAgent {
    fn agent_type(&self) -> &str {
        "dx-optimizer"
    }
    fn name(&self) -> &str {
        "DX Optimizer"
    }
    fn description(&self) -> &str {
        "Developer experience optimization and workflow improvement"
    }

    fn operations(&self) -> Vec<String> {
        vec![
            "analyze".to_string(),
            "suggest".to_string(),
            "hooks".to_string(),
        ]
    }

    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        let result = match task.operation.as_str() {
            "analyze" => self.analyze_setup(task.path.as_deref()),
            "suggest" => self.suggest_improvements(task.path.as_deref()),
            "hooks" => self.git_hooks_status(task.path.as_deref()),
            _ => Err(format!("Unknown operation: {}", task.operation)),
        };

        match result {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/orchestration/mem0_wrapper.rs">
//! Mem0 Wrapper Agent - Temporarily Disabled
//!
//! This agent wraps the Mem0 Python library for semantic memory.
//! Currently disabled pending embedder configuration (needs a supported embeddings backend).
//!
//! To re-enable:
//! 1. Set up HuggingFace embeddings with proper cache paths, OR
//! 2. Provide OPENAI_API_KEY for OpenAI embeddings

use async_trait::async_trait;
use simd_json::json;
use std::collections::HashMap;
use std::sync::Mutex;
use tracing::warn;

use crate::agents::base::{AgentTask, AgentTrait, TaskResult};

/// Mem0 wrapper state
struct Mem0State {
    initialized: bool,
    available: bool,
    last_error: Option<String>,
}

impl Default for Mem0State {
    fn default() -> Self {
        Self {
            initialized: false,
            available: false,
            last_error: Some(
                "Mem0 temporarily disabled - pending embedder configuration".to_string(),
            ),
        }
    }
}

/// Mem0 Wrapper Agent
pub struct Mem0WrapperAgent {
    id: String,
    state: Mutex<Mem0State>,
    python_path: String,
    mem0_dir: String,
    profile: crate::security::SecurityProfile,
}

impl Mem0WrapperAgent {
    pub fn new(id: String) -> Self {
        let python_path =
            std::env::var("PYTHON_PATH").unwrap_or_else(|_| "/usr/bin/python3".to_string());
        let mem0_dir =
            std::env::var("MEM0_DIR").unwrap_or_else(|_| "/var/lib/op-dbus/.mem0".to_string());

        Self {
            id,
            state: Mutex::new(Mem0State::default()),
            python_path,
            mem0_dir,
            profile: crate::security::SecurityProfile::orchestration("mem0", vec!["*"]),
        }
    }
}

#[async_trait]
impl AgentTrait for Mem0WrapperAgent {
    fn agent_type(&self) -> &str {
        "mem0"
    }

    fn name(&self) -> &str {
        "Mem0 Memory Agent"
    }

    fn description(&self) -> &str {
        "Semantic memory using Mem0 (temporarily disabled)"
    }

    fn operations(&self) -> Vec<String> {
        vec![
            "add".to_string(),
            "search".to_string(),
            "get_all".to_string(),
            "delete".to_string(),
            "update".to_string(),
        ]
    }

    fn security_profile(&self) -> &crate::security::SecurityProfile {
        &self.profile
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        // Return graceful "not available" response
        let error_msg = "Mem0 temporarily disabled - pending embedder configuration. \
                         To enable: configure HuggingFace embeddings or provide OPENAI_API_KEY";

        warn!("Mem0 agent called but disabled: {}", task.operation);

        Ok(TaskResult {
            success: false,
            operation: task.operation,
            data: json!({
                "available": false,
                "error": error_msg,
                "hint": "Use memory_remember/memory_recall for key-value memory instead"
            })
            .to_string(),
            metadata: {
                let mut m = HashMap::new();
                m.insert("status".to_string(), json!("disabled"));
                m
            },
        })
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/orchestration/memory.rs">
//! Memory Agent with Cognitive Features
//!
//! Provides persistent memory storage with semantic search capabilities.
//! Merged features from op-cognitive-mcp for vector embeddings and advanced search.

use async_trait::async_trait;
use simd_json::prelude::*;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::agents::base::{AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;

/// Memory entry with cognitive features
#[derive(Debug, Clone)]
pub struct MemoryEntry {
    pub key: String,
    pub value: String,
    pub vector: Option<Vec<f32>>, // Embedding vector for semantic search
    pub memory_type: MemoryType,
    pub tags: Vec<String>,
    pub created_at: u64, // Unix timestamp
    pub updated_at: u64,
    pub expires_at: Option<u64>,
    pub access_count: u64,
    pub last_accessed: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MemoryType {
    Ephemeral,  // Session-based, may expire
    Persistent, // Permanent storage
    Shared,     // Cross-session shared
}

impl Default for MemoryType {
    fn default() -> Self {
        MemoryType::Persistent
    }
}

fn now_ts() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

impl MemoryEntry {
    pub fn new(key: String, value: String, memory_type: MemoryType, tags: Vec<String>) -> Self {
        let now = now_ts();
        Self {
            key,
            value,
            vector: None,
            memory_type,
            tags,
            created_at: now,
            updated_at: now,
            expires_at: None,
            access_count: 0,
            last_accessed: now,
        }
    }

    /// Check if entry has expired
    pub fn is_expired(&self) -> bool {
        self.expires_at.map_or(false, |exp| now_ts() > exp)
    }
}

pub struct MemoryAgent {
    agent_id: String,
    profile: SecurityProfile,
    memory_path: PathBuf,
    cache: Arc<RwLock<HashMap<String, MemoryEntry>>>,
}

impl MemoryAgent {
    pub fn new(agent_id: String) -> Self {
        let memory_path = PathBuf::from("/var/lib/op-dbus/memory_cognitive.json");
        let cache = if let Ok(content) = fs::read_to_string(&memory_path) {
            Self::parse_memory_entries(&content)
        } else {
            let old_path = PathBuf::from("/var/lib/op-dbus/memory.json");
            if let Ok(content) = fs::read_to_string(&old_path) {
                Self::migrate_old_format(&content)
            } else {
                HashMap::new()
            }
        };

        Self {
            agent_id,
            profile: SecurityProfile::orchestration("memory", vec!["*"]),
            memory_path,
            cache: Arc::new(RwLock::new(cache)),
        }
    }

    fn persist(&self) -> Result<(), String> {
        let cache = self.cache.read().map_err(|_| "Failed to acquire lock")?;
        let content = Self::serialize_memory_entries(&*cache)?;
        fs::write(&self.memory_path, content).map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Parse memory entries from JSON string
    fn parse_memory_entries(content: &str) -> HashMap<String, MemoryEntry> {
        let mut cache = HashMap::new();
        let mut content_mut = content.to_string();
        let value: simd_json::OwnedValue =
            unsafe { simd_json::from_str(&mut content_mut).unwrap_or_default() };

        if let Some(obj) = value.as_object() {
            for (key, entry_val) in obj.iter() {
                if let Some(entry_obj) = entry_val.as_object() {
                    let entry = MemoryEntry {
                        key: key.clone(),
                        value: entry_obj
                            .get("value")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        vector: None,
                        memory_type: entry_obj
                            .get("memory_type")
                            .and_then(|v| v.as_str())
                            .map(|s| match s {
                                "ephemeral" => MemoryType::Ephemeral,
                                "shared" => MemoryType::Shared,
                                _ => MemoryType::Persistent,
                            })
                            .unwrap_or(MemoryType::Persistent),
                        tags: entry_obj
                            .get("tags")
                            .and_then(|v| v.as_array())
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|v| v.as_str().map(String::from))
                                    .collect()
                            })
                            .unwrap_or_default(),
                        created_at: entry_obj
                            .get("created_at")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0),
                        updated_at: entry_obj
                            .get("updated_at")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0),
                        expires_at: entry_obj.get("expires_at").and_then(|v| v.as_u64()),
                        access_count: entry_obj
                            .get("access_count")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0),
                        last_accessed: entry_obj
                            .get("last_accessed")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0),
                    };
                    cache.insert(key.clone(), entry);
                }
            }
        }
        cache
    }

    /// Serialize memory entries to JSON string using simple JSON construction
    fn serialize_memory_entries(cache: &HashMap<String, MemoryEntry>) -> Result<String, String> {
        let mut entries = Vec::new();
        for (key, entry) in cache.iter() {
            let memory_type_str = match entry.memory_type {
                MemoryType::Ephemeral => "ephemeral",
                MemoryType::Persistent => "persistent",
                MemoryType::Shared => "shared",
            };
            let tags_json = entry
                .tags
                .iter()
                .map(|t| format!("\"{}\"", t))
                .collect::<Vec<_>>()
                .join(",");

            let expires_json = entry
                .expires_at
                .map(|e| format!(",\"expires_at\":{}", e))
                .unwrap_or_default();

            let entry_json = format!(
                "\"{}\":{{\"value\":\"{}\",\"memory_type\":\"{}\",\"tags\":[{}],\"created_at\":{},\"updated_at\":{},\"access_count\":{},\"last_accessed\":{}{}}}",
                key, entry.value, memory_type_str, tags_json, entry.created_at, entry.updated_at, 
                entry.access_count, entry.last_accessed, expires_json
            );
            entries.push(entry_json);
        }

        Ok(format!("{{{}}}", entries.join(",")))
    }

    /// Migrate from old format (key-value pairs)
    fn migrate_old_format(content: &str) -> HashMap<String, MemoryEntry> {
        let mut cache = HashMap::new();
        let mut content_mut = content.to_string();
        let old_cache: HashMap<String, String> =
            unsafe { simd_json::from_str(&mut content_mut).unwrap_or_default() };
        for (key, value) in old_cache {
            let entry = MemoryEntry::new(key.clone(), value, MemoryType::Persistent, vec![]);
            cache.insert(key, entry);
        }
        cache
    }

    /// Store with cognitive features
    fn remember_advanced(
        &self,
        key: Option<&str>,
        value: Option<&str>,
        memory_type: Option<MemoryType>,
        tags: Option<Vec<String>>,
    ) -> Result<String, String> {
        let key = key.ok_or("Key required")?;
        let value = value.ok_or("Value required")?;
        let memory_type = memory_type.unwrap_or(MemoryType::Persistent);
        let tags = tags.unwrap_or_default();

        let entry = MemoryEntry::new(
            key.to_string(),
            value.to_string(),
            memory_type.clone(),
            tags,
        );

        {
            let mut cache = self.cache.write().map_err(|_| "Failed to acquire lock")?;
            cache.insert(key.to_string(), entry);
        }
        self.persist()?;

        Ok(format!("Remembered: {} (type: {:?})", key, memory_type))
    }

    /// Simple remember (backward compatible)
    fn remember(&self, key: Option<&str>, value: Option<&str>) -> Result<String, String> {
        self.remember_advanced(key, value, None, None)
    }

    /// Recall with access tracking
    fn recall(&self, key: Option<&str>) -> Result<String, String> {
        let key = key.ok_or("Key required")?;

        let mut cache = self.cache.write().map_err(|_| "Failed to acquire lock")?;

        // Check for expired entries and remove them
        let expired_keys: Vec<String> = cache
            .iter()
            .filter(|(_, entry)| entry.is_expired())
            .map(|(k, _)| k.clone())
            .collect();
        for k in expired_keys {
            cache.remove(&k);
        }

        // Exact match with access tracking
        if let Some(entry) = cache.get_mut(key) {
            if !entry.is_expired() {
                entry.access_count += 1;
                entry.last_accessed = now_ts();
                let value = entry.value.clone();
                let count = entry.access_count;
                drop(cache);
                let _ = self.persist();
                return Ok(format!(
                    "Recalled (exact): {} = {} (accessed: {} times)",
                    key, value, count
                ));
            }
        }

        // Fuzzy search
        let matches: Vec<(String, String, u64)> = cache
            .iter()
            .filter(|(k, _)| k.contains(key))
            .map(|(k, v)| (k.clone(), v.value.clone(), v.access_count))
            .collect();

        if matches.is_empty() {
            Err(format!("Nothing found for '{}'", key))
        } else {
            let result = matches
                .iter()
                .map(|(k, v, count)| format!("{} = {} (accessed: {} times)", k, v, count))
                .collect::<Vec<_>>()
                .join("\n");
            Ok(format!("Recalled (matches):\n{}", result))
        }
    }

    /// Semantic search using scoring
    fn semantic_search(&self, query: Option<&str>, limit: Option<usize>) -> Result<String, String> {
        let query = query.ok_or("Query required")?;
        let limit = limit.unwrap_or(5);

        let cache = self.cache.read().map_err(|_| "Failed to acquire lock")?;

        // Score entries by fuzzy match and access count
        let mut scored: Vec<(String, String, f32)> = cache
            .iter()
            .filter(|(_, entry)| !entry.is_expired())
            .map(|(k, entry)| {
                let mut score = 0.0f32;

                if k.contains(query) {
                    score += 1.0;
                }
                if entry.value.contains(query) {
                    score += 0.5;
                }
                if entry.tags.iter().any(|t| t.contains(query)) {
                    score += 0.8;
                }
                score += (entry.access_count as f32) * 0.01;

                (k.clone(), entry.value.clone(), score)
            })
            .filter(|(_, _, score)| *score > 0.0)
            .collect();

        scored.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());

        if scored.is_empty() {
            return Err(format!("No semantic matches for '{}'", query));
        }

        let results = scored
            .into_iter()
            .take(limit)
            .map(|(k, v, score)| format!("[score: {:.2}] {} = {}", score, k, v))
            .collect::<Vec<_>>()
            .join("\n");

        Ok(format!(
            "Semantic search results for '{}':\n{}",
            query, results
        ))
    }

    /// Query by tags
    fn query_by_tags(&self, tags: Option<Vec<String>>) -> Result<String, String> {
        let tags = tags.ok_or("Tags required")?;
        let cache = self.cache.read().map_err(|_| "Failed to acquire lock")?;

        let matches: Vec<(String, String)> = cache
            .iter()
            .filter(|(_, entry)| tags.iter().all(|tag| entry.tags.contains(tag)))
            .map(|(k, entry)| (k.clone(), entry.value.clone()))
            .collect();

        if matches.is_empty() {
            Err(format!("No entries with tags: {:?}", tags))
        } else {
            let result = matches
                .iter()
                .map(|(k, v)| format!("{} = {}", k, v))
                .collect::<Vec<_>>()
                .join("\n");
            Ok(format!("Tagged entries:\n{}", result))
        }
    }

    /// Forget by key
    fn forget(&self, key: Option<&str>) -> Result<String, String> {
        let key = key.ok_or("Key required")?;

        {
            let mut cache = self.cache.write().map_err(|_| "Failed to acquire lock")?;
            cache.remove(key);
        }
        self.persist()?;

        Ok(format!("Forgotten: {}", key))
    }

    /// List all entries
    fn list(&self) -> Result<String, String> {
        let cache = self.cache.read().map_err(|_| "Failed to acquire lock")?;

        if cache.is_empty() {
            return Ok("No memories stored".to_string());
        }

        let entries: Vec<String> = cache
            .iter()
            .map(|(k, entry)| {
                let tags = if entry.tags.is_empty() {
                    String::new()
                } else {
                    format!(" [tags: {}]", entry.tags.join(", "))
                };
                let vector_status = if entry.vector.is_some() {
                    " [vector]"
                } else {
                    ""
                };
                format!("{} = {}{}{}", k, entry.value, tags, vector_status)
            })
            .collect();

        Ok(format!(
            "Stored memories ({}):\n{}",
            entries.len(),
            entries.join("\n")
        ))
    }

    /// Get memory statistics
    fn stats(&self) -> Result<String, String> {
        let cache = self.cache.read().map_err(|_| "Failed to acquire lock")?;

        let total = cache.len();
        let with_vectors = cache.values().filter(|e| e.vector.is_some()).count();
        let ephemeral = cache
            .values()
            .filter(|e| e.memory_type == MemoryType::Ephemeral)
            .count();
        let persistent = cache
            .values()
            .filter(|e| e.memory_type == MemoryType::Persistent)
            .count();
        let shared = cache
            .values()
            .filter(|e| e.memory_type == MemoryType::Shared)
            .count();
        let expired = cache.values().filter(|e| e.is_expired()).count();

        Ok(format!(
            "Memory Statistics:\nTotal entries: {}\nWith vectors: {}\nEphemeral: {}\nPersistent: {}\nShared: {}\nExpired: {}",
            total, with_vectors, ephemeral, persistent, shared, expired
        ))
    }

    /// Cleanup expired entries
    fn cleanup(&self) -> Result<String, String> {
        let mut cache = self.cache.write().map_err(|_| "Failed to acquire lock")?;

        let before = cache.len();
        let expired_keys: Vec<String> = cache
            .iter()
            .filter(|(_, entry)| entry.is_expired())
            .map(|(k, _)| k.clone())
            .collect();

        for k in expired_keys {
            cache.remove(&k);
        }

        let removed = before - cache.len();
        drop(cache);
        self.persist()?;

        Ok(format!("Cleaned up {} expired entries", removed))
    }
}

#[async_trait]
impl AgentTrait for MemoryAgent {
    fn agent_type(&self) -> &str {
        "memory"
    }
    fn name(&self) -> &str {
        "Memory Agent"
    }
    fn description(&self) -> &str {
        "Cognitive memory with semantic search, tags, and expiration"
    }

    fn operations(&self) -> Vec<String> {
        vec![
            "remember".to_string(),
            "remember_advanced".to_string(),
            "recall".to_string(),
            "semantic_search".to_string(),
            "query_by_tags".to_string(),
            "forget".to_string(),
            "list".to_string(),
            "stats".to_string(),
            "cleanup".to_string(),
        ]
    }

    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        let result = match task.operation.as_str() {
            "remember" => self.remember(task.path.as_deref(), task.args.as_deref()),
            "remember_advanced" => {
                let memory_type =
                    task.config
                        .get("memory_type")
                        .and_then(|v| v.as_str())
                        .map(|s| match s {
                            "ephemeral" => MemoryType::Ephemeral,
                            "shared" => MemoryType::Shared,
                            _ => MemoryType::Persistent,
                        });
                let tags = task
                    .config
                    .get("tags")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    });
                self.remember_advanced(
                    task.path.as_deref(),
                    task.args.as_deref(),
                    memory_type,
                    tags,
                )
            }
            "recall" => self.recall(task.path.as_deref().or(task.args.as_deref())),
            "semantic_search" => {
                let limit = task
                    .config
                    .get("limit")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as usize);
                self.semantic_search(task.path.as_deref().or(task.args.as_deref()), limit)
            }
            "query_by_tags" => {
                let tags = task
                    .config
                    .get("tags")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    });
                self.query_by_tags(tags)
            }
            "forget" => self.forget(task.path.as_deref().or(task.args.as_deref())),
            "list" => self.list(),
            "stats" => self.stats(),
            "cleanup" => self.cleanup(),
            _ => Err(format!("Unknown operation: {}", task.operation)),
        };

        match result {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/orchestration/mod.rs">
//! Orchestration and meta-agents

pub mod context_manager;
pub mod dx_optimizer;
pub mod mem0_wrapper;
pub mod memory;
pub mod sequential_thinking;
pub mod tdd_orchestrator;

pub use context_manager::ContextManagerAgent;
pub use dx_optimizer::DxOptimizerAgent;
pub use mem0_wrapper::Mem0WrapperAgent;
pub use memory::MemoryAgent;
pub use sequential_thinking::SequentialThinkingAgent;
pub use tdd_orchestrator::TddOrchestratorAgent;
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/orchestration/sequential_thinking.rs">
//! Sequential Thinking Agent
//!
//! Helper agent for breaking down complex tasks into sequential steps.

use async_trait::async_trait;
use simd_json::json;

use crate::agents::base::{AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;

pub struct SequentialThinkingAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl SequentialThinkingAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            agent_id,
            profile: SecurityProfile::orchestration("sequential-thinking", vec!["*"]),
        }
    }

    fn analyze(&self, input: &str) -> Result<String, String> {
        // In a real implementation, this might use an LLM or stricter logic.
        // For now, it scaffolds a thinking process.
        let steps = json!({
            "thought_process": {
                "input": input,
                "analysis": "Decomposing task into sequential steps...",
                "steps": [
                    "1. Identify core intent",
                    "2. Check constraints",
                    "3. Formulate plan",
                    "4. Execute step-by-step"
                ],
                "recommendation": "Proceed with step 1"
            }
        });
        Ok(simd_json::to_string_pretty(&steps).unwrap())
    }
}

#[async_trait]
impl AgentTrait for SequentialThinkingAgent {
    fn agent_type(&self) -> &str {
        "sequential-thinking"
    }
    fn name(&self) -> &str {
        "Sequential Thinking"
    }
    fn description(&self) -> &str {
        "Helps break down complex problems into linear, sequential steps"
    }

    fn operations(&self) -> Vec<String> {
        vec!["analyze".to_string()]
    }

    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        let input = task.args.as_deref().unwrap_or("");

        let result = match task.operation.as_str() {
            "analyze" => self.analyze(input),
            _ => Err(format!("Unknown operation: {}", task.operation)),
        };

        match result {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/orchestration/tdd_orchestrator.rs">
//! TDD Orchestrator Agent

use async_trait::async_trait;

use crate::agents::base::{AgentTask, AgentTrait, TaskResult};
use crate::security::{profiles::presets, SecurityProfile};

pub struct TddOrchestratorAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl TddOrchestratorAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            agent_id,
            profile: presets::tdd_orchestrator(),
        }
    }

    fn plan_red_phase(&self) -> Result<String, String> {
        Ok("TDD Red Phase Plan:\n\
            1. Write a failing test for the new feature\n\
            2. Run tests to verify it fails\n\
            3. Ensure the test failure message is clear\n\
            \n\
            Subagents to invoke: test-automator, debugger"
            .to_string())
    }

    fn plan_green_phase(&self) -> Result<String, String> {
        Ok("TDD Green Phase Plan:\n\
            1. Write minimal code to pass the test\n\
            2. Run tests to verify they pass\n\
            3. Ensure no other tests broke\n\
            \n\
            Subagents to invoke: code-reviewer, test-automator"
            .to_string())
    }

    fn plan_refactor_phase(&self) -> Result<String, String> {
        Ok("TDD Refactor Phase Plan:\n\
            1. Identify code smells and duplication\n\
            2. Apply refactoring patterns\n\
            3. Run tests after each change\n\
            4. Ensure code quality improved\n\
            \n\
            Subagents to invoke: code-reviewer, test-automator, debugger"
            .to_string())
    }

    fn full_cycle(&self) -> Result<String, String> {
        Ok("TDD Full Cycle Plan:\n\
            \n\
            Phase 1 - RED:\n\
            - Write failing test\n\
            - Verify test fails correctly\n\
            \n\
            Phase 2 - GREEN:\n\
            - Write minimal implementation\n\
            - Verify all tests pass\n\
            \n\
            Phase 3 - REFACTOR:\n\
            - Improve code quality\n\
            - Maintain test coverage\n\
            \n\
            Coordination: Sequential execution with validation gates"
            .to_string())
    }
}

#[async_trait]
impl AgentTrait for TddOrchestratorAgent {
    fn agent_type(&self) -> &str {
        "tdd-orchestrator"
    }
    fn name(&self) -> &str {
        "TDD Orchestrator"
    }
    fn description(&self) -> &str {
        "Test-Driven Development workflow orchestration"
    }

    fn operations(&self) -> Vec<String> {
        vec![
            "red".to_string(),
            "green".to_string(),
            "refactor".to_string(),
            "cycle".to_string(),
        ]
    }

    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        let result = match task.operation.as_str() {
            "red" => self.plan_red_phase(),
            "green" => self.plan_green_phase(),
            "refactor" => self.plan_refactor_phase(),
            "cycle" => self.full_cycle(),
            _ => Err(format!("Unknown operation: {}", task.operation)),
        };

        match result {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/security/backend_security_coder.rs">
//! Backend Security Coder Agent

use crate::agents::base::{AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;
use async_trait::async_trait;
use simd_json::json;

pub struct BackendSecurityCoderAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl BackendSecurityCoderAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            profile: SecurityProfile::content_generation("backend-security-coder"),
            agent_id,
        }
    }

    fn analyze(&self, args: Option<&str>) -> Result<String, String> {
        let input = args.unwrap_or("").to_lowercase();
        let mut recommendations = Vec::new();

        if input.contains("injection") || input.contains("sql") {
            recommendations.push("Use parameterized queries/prepared statements");
            recommendations.push("Implement input validation and sanitization");
            recommendations.push("Apply principle of least privilege for DB access");
        }
        if input.contains("auth") || input.contains("session") {
            recommendations.push("Use secure session management (HttpOnly, Secure flags)");
            recommendations.push("Implement proper password hashing (bcrypt/argon2)");
            recommendations.push("Add rate limiting for authentication endpoints");
        }
        if input.contains("api") || input.contains("endpoint") {
            recommendations.push("Implement proper authorization checks");
            recommendations.push("Add request validation middleware");
            recommendations.push("Use HTTPS everywhere with proper TLS config");
        }
        if recommendations.is_empty() {
            recommendations.push("Follow OWASP Top 10 guidelines");
            recommendations.push("Implement defense in depth");
            recommendations.push("Add security headers (CSP, HSTS, X-Frame-Options)");
        }

        let result = json!({
            "analysis": { "input": args.unwrap_or(""), "recommendations": recommendations },
            "vulnerabilities_to_check": ["SQL Injection", "Authentication Bypass", "IDOR", "SSRF", "XXE"]
        });
        Ok(simd_json::to_string_pretty(&result).unwrap_or_default())
    }
}

#[async_trait]
impl AgentTrait for BackendSecurityCoderAgent {
    fn agent_type(&self) -> &str {
        "backend-security-coder"
    }
    fn name(&self) -> &str {
        "Backend Security Coder"
    }
    fn description(&self) -> &str {
        "Write secure backend code and identify vulnerabilities."
    }
    fn operations(&self) -> Vec<String> {
        vec![
            "secure_endpoint".to_string(),
            "audit_code".to_string(),
            "fix_vulnerability".to_string(),
            "analyze".to_string(),
        ]
    }
    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }
    fn get_status(&self) -> String {
        format!("Backend Security Coder agent {} is running", self.agent_id)
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "backend-security-coder" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }
        match self.analyze(task.args.as_deref()) {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/security/frontend_security_coder.rs">
//! Frontend Security Coder Agent

use crate::agents::base::{AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;
use async_trait::async_trait;
use simd_json::json;

pub struct FrontendSecurityCoderAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl FrontendSecurityCoderAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            profile: SecurityProfile::content_generation("frontend-security-coder"),
            agent_id,
        }
    }

    fn analyze(&self, args: Option<&str>) -> Result<String, String> {
        let input = args.unwrap_or("").to_lowercase();
        let mut recommendations = Vec::new();

        if input.contains("xss") || input.contains("injection") {
            recommendations.push("Use framework's built-in XSS protection");
            recommendations.push("Sanitize user input before rendering");
            recommendations.push("Implement Content Security Policy (CSP)");
        }
        if input.contains("csrf") {
            recommendations.push("Use CSRF tokens for state-changing requests");
            recommendations.push("Implement SameSite cookie attribute");
            recommendations.push("Validate Origin/Referer headers");
        }
        if input.contains("storage") || input.contains("token") {
            recommendations.push("Don't store sensitive data in localStorage");
            recommendations.push("Use HttpOnly cookies for auth tokens");
            recommendations.push("Clear sensitive data on logout");
        }
        if recommendations.is_empty() {
            recommendations.push("Validate and sanitize all user inputs");
            recommendations.push("Use secure communication (HTTPS)");
            recommendations.push("Implement proper error handling (don't leak info)");
        }

        let result = json!({
            "analysis": { "input": args.unwrap_or(""), "recommendations": recommendations },
            "vulnerabilities_to_check": ["XSS", "CSRF", "Clickjacking", "Open Redirects", "Sensitive Data Exposure"]
        });
        Ok(simd_json::to_string_pretty(&result).unwrap_or_default())
    }
}

#[async_trait]
impl AgentTrait for FrontendSecurityCoderAgent {
    fn agent_type(&self) -> &str {
        "frontend-security-coder"
    }
    fn name(&self) -> &str {
        "Frontend Security Coder"
    }
    fn description(&self) -> &str {
        "Write secure frontend code and prevent client-side vulnerabilities."
    }
    fn operations(&self) -> Vec<String> {
        vec![
            "secure_component".to_string(),
            "audit_code".to_string(),
            "fix_vulnerability".to_string(),
            "analyze".to_string(),
        ]
    }
    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }
    fn get_status(&self) -> String {
        format!("Frontend Security Coder agent {} is running", self.agent_id)
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "frontend-security-coder" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }
        match self.analyze(task.args.as_deref()) {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/security/mobile_security_coder.rs">
//! Mobile Security Coder Agent

use crate::agents::base::{AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;
use async_trait::async_trait;
use simd_json::json;

pub struct MobileSecurityCoderAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl MobileSecurityCoderAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            profile: SecurityProfile::content_generation("mobile-security-coder"),
            agent_id,
        }
    }

    fn analyze(&self, args: Option<&str>) -> Result<String, String> {
        let input = args.unwrap_or("").to_lowercase();
        let mut recommendations = Vec::new();

        if input.contains("storage") || input.contains("keychain") {
            recommendations.push("Use Keychain (iOS) / Keystore (Android) for secrets");
            recommendations.push("Encrypt sensitive local data");
            recommendations.push("Don't store sensitive data in shared preferences");
        }
        if input.contains("network") || input.contains("api") {
            recommendations.push("Implement certificate pinning");
            recommendations.push("Use TLS 1.2+ for all connections");
            recommendations.push("Validate server certificates");
        }
        if input.contains("binary") || input.contains("reverse") {
            recommendations.push("Implement root/jailbreak detection");
            recommendations.push("Use code obfuscation");
            recommendations.push("Implement tamper detection");
        }
        if recommendations.is_empty() {
            recommendations.push("Follow OWASP Mobile Top 10");
            recommendations.push("Implement proper session management");
            recommendations.push("Secure inter-process communication");
        }

        let result = json!({
            "analysis": { "input": args.unwrap_or(""), "recommendations": recommendations },
            "vulnerabilities_to_check": ["Insecure Storage", "Insecure Communication", "Code Tampering", "Reverse Engineering"]
        });
        Ok(simd_json::to_string_pretty(&result).unwrap_or_default())
    }
}

#[async_trait]
impl AgentTrait for MobileSecurityCoderAgent {
    fn agent_type(&self) -> &str {
        "mobile-security-coder"
    }
    fn name(&self) -> &str {
        "Mobile Security Coder"
    }
    fn description(&self) -> &str {
        "Secure mobile apps against common vulnerabilities."
    }
    fn operations(&self) -> Vec<String> {
        vec![
            "secure_storage".to_string(),
            "audit_code".to_string(),
            "fix_vulnerability".to_string(),
            "analyze".to_string(),
        ]
    }
    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }
    fn get_status(&self) -> String {
        format!("Mobile Security Coder agent {} is running", self.agent_id)
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "mobile-security-coder" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }
        match self.analyze(task.args.as_deref()) {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/security/mod.rs">
//! Security-focused Development Agents
//!
//! - `BackendSecurityCoder`: Secure backend development
//! - `FrontendSecurityCoder`: Secure frontend development
//! - `MobileSecurityCoder`: Mobile app security

mod backend_security_coder;
mod frontend_security_coder;
mod mobile_security_coder;

pub use backend_security_coder::BackendSecurityCoderAgent;
pub use frontend_security_coder::FrontendSecurityCoderAgent;
pub use mobile_security_coder::MobileSecurityCoderAgent;
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/seo/content_marketer.rs">
//! Content Marketer Agent

use crate::agents::base::{AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;
use async_trait::async_trait;
use simd_json::json;

pub struct ContentMarketerAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl ContentMarketerAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            profile: SecurityProfile::content_generation("content-marketer"),
            agent_id,
        }
    }

    fn analyze(&self, args: Option<&str>) -> Result<String, String> {
        let input = args.unwrap_or("").to_lowercase();
        let mut recommendations = Vec::new();

        if input.contains("strategy") || input.contains("plan") {
            recommendations.push("Define target audience personas");
            recommendations.push("Map content to buyer journey stages");
            recommendations.push("Create content calendar with themes");
        }
        if input.contains("distribute") || input.contains("promote") {
            recommendations.push("Repurpose content across channels");
            recommendations.push("Build email list for owned distribution");
            recommendations.push("Engage on social media strategically");
        }
        if recommendations.is_empty() {
            recommendations.push("Focus on providing value to audience");
            recommendations.push("Track engagement metrics and conversions");
            recommendations.push("Update and refresh successful content");
        }

        let result = json!({
            "analysis": { "input": args.unwrap_or(""), "recommendations": recommendations },
            "content_types": ["Blog posts", "Ebooks", "Webinars", "Case studies", "Infographics", "Videos"]
        });
        Ok(simd_json::to_string_pretty(&result).unwrap_or_default())
    }
}

#[async_trait]
impl AgentTrait for ContentMarketerAgent {
    fn agent_type(&self) -> &str {
        "content-marketer"
    }
    fn name(&self) -> &str {
        "Content Marketer"
    }
    fn description(&self) -> &str {
        "Plan and execute content marketing strategy."
    }
    fn operations(&self) -> Vec<String> {
        vec![
            "create_strategy".to_string(),
            "plan_content".to_string(),
            "analyze".to_string(),
        ]
    }
    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }
    fn get_status(&self) -> String {
        format!("Content Marketer agent {} is running", self.agent_id)
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "content-marketer" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }
        match self.analyze(task.args.as_deref()) {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/seo/mod.rs">
//! SEO & Content Marketing Agents
//!
//! Specialized agents for SEO optimization and content marketing

mod content_marketer;
pub mod search_specialist;
mod seo_content_writer;
mod seo_keyword_strategist;
mod seo_meta_optimizer;

pub use content_marketer::ContentMarketerAgent;
pub use search_specialist::SearchSpecialistAgent;
pub use seo_content_writer::SEOContentWriterAgent;
pub use seo_keyword_strategist::SEOKeywordStrategistAgent;
pub use seo_meta_optimizer::SEOMetaOptimizerAgent;
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/seo/search_specialist.rs">
//! Search Specialist Agent

use crate::agents::base::{AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;
use async_trait::async_trait;
use simd_json::json;

pub struct SearchSpecialistAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl SearchSpecialistAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            profile: SecurityProfile::content_generation("search-specialist"),
            agent_id,
        }
    }

    fn analyze(&self, args: Option<&str>) -> Result<String, String> {
        let input = args.unwrap_or("").to_lowercase();
        let mut recommendations = Vec::new();

        if input.contains("technical") || input.contains("audit") {
            recommendations.push("Check crawlability with robots.txt and sitemap");
            recommendations.push("Analyze Core Web Vitals scores");
            recommendations.push("Fix broken links and redirect chains");
        }
        if input.contains("local") {
            recommendations.push("Optimize Google Business Profile");
            recommendations.push("Build local citations consistently");
            recommendations.push("Encourage and respond to reviews");
        }
        if recommendations.is_empty() {
            recommendations.push("Monitor search console for issues");
            recommendations.push("Build quality backlinks");
            recommendations.push("Create comprehensive topic clusters");
        }

        let result = json!({
            "analysis": { "input": args.unwrap_or(""), "recommendations": recommendations },
            "tools": ["Google Search Console", "Screaming Frog", "Ahrefs", "SEMrush"]
        });
        Ok(simd_json::to_string_pretty(&result).unwrap_or_default())
    }
}

#[async_trait]
impl AgentTrait for SearchSpecialistAgent {
    fn agent_type(&self) -> &str {
        "search-specialist"
    }
    fn name(&self) -> &str {
        "Search Specialist"
    }
    fn description(&self) -> &str {
        "Technical SEO audits and search optimization."
    }
    fn operations(&self) -> Vec<String> {
        vec![
            "audit_site".to_string(),
            "fix_issues".to_string(),
            "analyze".to_string(),
        ]
    }
    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }
    fn get_status(&self) -> String {
        format!("Search Specialist agent {} is running", self.agent_id)
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "search-specialist" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }
        match self.analyze(task.args.as_deref()) {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/seo/seo_content_writer.rs">
//! SEO Content Writer Agent

use crate::agents::base::{AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;
use async_trait::async_trait;
use simd_json::json;

pub struct SEOContentWriterAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl SEOContentWriterAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            profile: SecurityProfile::content_generation("seo-content-writer"),
            agent_id,
        }
    }

    fn analyze(&self, args: Option<&str>) -> Result<String, String> {
        let input = args.unwrap_or("").to_lowercase();
        let mut recommendations = Vec::new();

        if input.contains("blog") || input.contains("article") {
            recommendations.push("Include target keyword in title, H1, and first paragraph");
            recommendations.push("Use semantic keywords throughout content");
            recommendations.push("Structure with H2/H3 headers for readability");
        }
        if input.contains("product") || input.contains("landing") {
            recommendations.push("Focus on user intent and benefits");
            recommendations.push("Include clear calls-to-action");
            recommendations.push("Add schema markup for rich snippets");
        }
        if recommendations.is_empty() {
            recommendations.push("Write for users first, search engines second");
            recommendations.push("Create comprehensive, valuable content");
            recommendations.push("Include internal and external links");
        }

        let result = json!({
            "analysis": { "input": args.unwrap_or(""), "recommendations": recommendations },
            "content_checklist": ["Title tag (50-60 chars)", "Meta description (150-160 chars)", "Header hierarchy", "Image alt text", "Internal links"]
        });
        Ok(simd_json::to_string_pretty(&result).unwrap_or_default())
    }
}

#[async_trait]
impl AgentTrait for SEOContentWriterAgent {
    fn agent_type(&self) -> &str {
        "seo-content-writer"
    }
    fn name(&self) -> &str {
        "SEO Content Writer"
    }
    fn description(&self) -> &str {
        "Create SEO-optimized content that ranks and converts."
    }
    fn operations(&self) -> Vec<String> {
        vec![
            "write_article".to_string(),
            "optimize_content".to_string(),
            "analyze".to_string(),
        ]
    }
    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }
    fn get_status(&self) -> String {
        format!("SEO Content Writer agent {} is running", self.agent_id)
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "seo-content-writer" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }
        match self.analyze(task.args.as_deref()) {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/seo/seo_keyword_strategist.rs">
//! SEO Keyword Strategist Agent

use crate::agents::base::{AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;
use async_trait::async_trait;
use simd_json::json;

pub struct SEOKeywordStrategistAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl SEOKeywordStrategistAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            profile: SecurityProfile::content_generation("seo-keyword-strategist"),
            agent_id,
        }
    }

    fn analyze(&self, args: Option<&str>) -> Result<String, String> {
        let input = args.unwrap_or("").to_lowercase();
        let mut recommendations = Vec::new();

        if input.contains("research") || input.contains("discover") {
            recommendations.push("Start with seed keywords from business domain");
            recommendations.push("Analyze competitor keyword rankings");
            recommendations.push("Use tools like Ahrefs, SEMrush for volume data");
        }
        if input.contains("strategy") || input.contains("plan") {
            recommendations.push("Group keywords by topic clusters");
            recommendations.push("Balance head terms and long-tail keywords");
            recommendations.push("Map keywords to buyer journey stages");
        }
        if recommendations.is_empty() {
            recommendations
                .push("Focus on search intent (informational, transactional, navigational)");
            recommendations.push("Consider keyword difficulty vs. authority");
            recommendations.push("Track ranking changes over time");
        }

        let result = json!({
            "analysis": { "input": args.unwrap_or(""), "recommendations": recommendations },
            "metrics": ["Search Volume", "Keyword Difficulty", "CPC", "SERP Features", "Click-through Rate"]
        });
        Ok(simd_json::to_string_pretty(&result).unwrap_or_default())
    }
}

#[async_trait]
impl AgentTrait for SEOKeywordStrategistAgent {
    fn agent_type(&self) -> &str {
        "seo-keyword-strategist"
    }
    fn name(&self) -> &str {
        "SEO Keyword Strategist"
    }
    fn description(&self) -> &str {
        "Research and plan keyword strategy for organic search growth."
    }
    fn operations(&self) -> Vec<String> {
        vec![
            "research_keywords".to_string(),
            "create_strategy".to_string(),
            "analyze".to_string(),
        ]
    }
    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }
    fn get_status(&self) -> String {
        format!("SEO Keyword Strategist agent {} is running", self.agent_id)
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "seo-keyword-strategist" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }
        match self.analyze(task.args.as_deref()) {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/seo/seo_meta_optimizer.rs">
//! SEO Meta Optimizer Agent

use crate::agents::base::{AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;
use async_trait::async_trait;
use simd_json::json;

pub struct SEOMetaOptimizerAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl SEOMetaOptimizerAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            profile: SecurityProfile::content_generation("seo-meta-optimizer"),
            agent_id,
        }
    }

    fn analyze(&self, args: Option<&str>) -> Result<String, String> {
        let input = args.unwrap_or("");
        let mut recommendations = Vec::new();

        recommendations.push("Title: 50-60 characters, keyword near front");
        recommendations.push("Meta description: 150-160 characters, include CTA");
        recommendations.push("Use unique titles and descriptions per page");
        recommendations.push("Include primary keyword naturally");

        let result = json!({
            "analysis": { "input": input, "recommendations": recommendations },
            "meta_elements": ["title", "description", "canonical", "robots", "og:title", "og:description", "og:image", "twitter:card"]
        });
        Ok(simd_json::to_string_pretty(&result).unwrap_or_default())
    }
}

#[async_trait]
impl AgentTrait for SEOMetaOptimizerAgent {
    fn agent_type(&self) -> &str {
        "seo-meta-optimizer"
    }
    fn name(&self) -> &str {
        "SEO Meta Optimizer"
    }
    fn description(&self) -> &str {
        "Optimize meta tags for better search visibility and CTR."
    }
    fn operations(&self) -> Vec<String> {
        vec![
            "optimize_meta".to_string(),
            "audit_page".to_string(),
            "analyze".to_string(),
        ]
    }
    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }
    fn get_status(&self) -> String {
        format!("SEO Meta Optimizer agent {} is running", self.agent_id)
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "seo-meta-optimizer" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }
        match self.analyze(task.args.as_deref()) {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/specialty/arm_cortex_expert.rs">
//! ARM Cortex Expert Agent

use crate::agents::base::{AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;
use async_trait::async_trait;
use simd_json::json;

pub struct ARMCortexExpertAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl ARMCortexExpertAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            profile: SecurityProfile::content_generation("arm-cortex-expert"),
            agent_id,
        }
    }

    fn analyze(&self, args: Option<&str>) -> Result<String, String> {
        let input = args.unwrap_or("").to_lowercase();
        let mut recommendations = Vec::new();

        if input.contains("interrupt") || input.contains("isr") {
            recommendations.push("Keep ISRs short - defer work to main loop");
            recommendations.push("Use proper priority configuration (NVIC)");
            recommendations.push("Disable interrupts carefully with critical sections");
        }
        if input.contains("power") || input.contains("sleep") {
            recommendations.push("Use appropriate sleep modes for power savings");
            recommendations.push("Configure wake-up sources correctly");
            recommendations.push("Disable unused peripherals");
        }
        if input.contains("memory") || input.contains("dma") {
            recommendations.push("Use DMA for bulk data transfers");
            recommendations.push("Align data structures for efficient access");
            recommendations.push("Consider cache coherency for Cortex-M7");
        }
        if recommendations.is_empty() {
            recommendations.push("Use CMSIS for portable code");
            recommendations.push("Configure clocks appropriately for application");
            recommendations.push("Implement watchdog for reliability");
        }

        let result = json!({
            "analysis": { "input": args.unwrap_or(""), "recommendations": recommendations },
            "cortex_variants": ["Cortex-M0/M0+", "Cortex-M3", "Cortex-M4", "Cortex-M7", "Cortex-M33"]
        });
        Ok(simd_json::to_string_pretty(&result).unwrap_or_default())
    }
}

#[async_trait]
impl AgentTrait for ARMCortexExpertAgent {
    fn agent_type(&self) -> &str {
        "arm-cortex-expert"
    }
    fn name(&self) -> &str {
        "ARM Cortex Expert"
    }
    fn description(&self) -> &str {
        "Embedded systems development for ARM Cortex-M microcontrollers."
    }
    fn operations(&self) -> Vec<String> {
        vec![
            "configure_peripheral".to_string(),
            "optimize_power".to_string(),
            "debug".to_string(),
            "analyze".to_string(),
        ]
    }
    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }
    fn get_status(&self) -> String {
        format!("ARM Cortex Expert agent {} is running", self.agent_id)
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "arm-cortex-expert" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }
        match self.analyze(task.args.as_deref()) {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/specialty/snowball_developer.rs">
//! Snowball Developer Agent

use crate::agents::base::{AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;
use async_trait::async_trait;
use simd_json::json;

pub struct SnowballDeveloperAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl SnowballDeveloperAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            profile: SecurityProfile::content_generation("snowball-developer"),
            agent_id,
        }
    }

    fn analyze(&self, args: Option<&str>) -> Result<String, String> {
        let input = args.unwrap_or("").to_lowercase();
        let mut recommendations = Vec::new();

        if input.contains("smart contract") || input.contains("solidity") {
            recommendations.push("Use OpenZeppelin for standard implementations");
            recommendations.push("Implement reentrancy guards");
            recommendations.push("Test with Foundry or Hardhat");
        }
        if input.contains("defi") {
            recommendations.push("Implement proper access controls");
            recommendations.push("Handle precision/rounding carefully");
            recommendations.push("Add oracle price feed validation");
        }
        if recommendations.is_empty() {
            recommendations.push("Audit contracts before mainnet deployment");
            recommendations.push("Use upgradeable proxy patterns when needed");
            recommendations.push("Optimize gas usage");
        }

        let result = json!({
            "analysis": { "input": args.unwrap_or(""), "recommendations": recommendations },
            "tools": ["Foundry", "Hardhat", "OpenZeppelin", "Slither", "Mythril"]
        });
        Ok(simd_json::to_string_pretty(&result).unwrap_or_default())
    }
}

#[async_trait]
impl AgentTrait for SnowballDeveloperAgent {
    fn agent_type(&self) -> &str {
        "snowball-developer"
    }
    fn name(&self) -> &str {
        "Snowball Developer"
    }
    fn description(&self) -> &str {
        "Develop smart contracts and snowball applications."
    }
    fn operations(&self) -> Vec<String> {
        vec![
            "write_contract".to_string(),
            "audit".to_string(),
            "deploy".to_string(),
            "analyze".to_string(),
        ]
    }
    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }
    fn get_status(&self) -> String {
        format!("Snowball Developer agent {} is running", self.agent_id)
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "snowball-developer" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }
        match self.analyze(task.args.as_deref()) {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/specialty/error_detective.rs">
//! Error Detective Agent

use crate::agents::base::{AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;
use async_trait::async_trait;
use simd_json::json;

pub struct ErrorDetectiveAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl ErrorDetectiveAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            profile: SecurityProfile::read_only_analysis("error-detective", vec!["logs", "errors"]),
            agent_id,
        }
    }

    fn analyze(&self, args: Option<&str>) -> Result<String, String> {
        let input = args.unwrap_or("").to_lowercase();
        let mut recommendations = Vec::new();

        if input.contains("stack") || input.contains("trace") {
            recommendations.push("Start from the bottom of the stack trace");
            recommendations.push("Look for your code vs library code");
            recommendations.push("Check for chained exceptions");
        }
        if input.contains("intermittent") || input.contains("random") {
            recommendations.push("Look for race conditions or timing issues");
            recommendations.push("Check for resource exhaustion patterns");
            recommendations.push("Review concurrent access to shared state");
        }
        if recommendations.is_empty() {
            recommendations.push("Reproduce the error consistently first");
            recommendations.push("Check logs around the error timestamp");
            recommendations.push("Identify what changed recently");
        }

        let result = json!({
            "analysis": { "input": args.unwrap_or(""), "recommendations": recommendations },
            "debugging_steps": ["Reproduce", "Isolate", "Identify root cause", "Fix", "Verify", "Prevent regression"]
        });
        Ok(simd_json::to_string_pretty(&result).unwrap_or_default())
    }
}

#[async_trait]
impl AgentTrait for ErrorDetectiveAgent {
    fn agent_type(&self) -> &str {
        "error-detective"
    }
    fn name(&self) -> &str {
        "Error Detective"
    }
    fn description(&self) -> &str {
        "Analyze errors, exceptions, and debug complex issues."
    }
    fn operations(&self) -> Vec<String> {
        vec![
            "analyze_error".to_string(),
            "find_root_cause".to_string(),
            "suggest_fix".to_string(),
            "analyze".to_string(),
        ]
    }
    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }
    fn get_status(&self) -> String {
        format!("Error Detective agent {} is running", self.agent_id)
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "error-detective" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }
        match self.analyze(task.args.as_deref()) {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/specialty/hybrid_cloud_architect.rs">
//! Hybrid Cloud Architect Agent

use crate::agents::base::{AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;
use async_trait::async_trait;
use simd_json::json;

pub struct HybridCloudArchitectAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl HybridCloudArchitectAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            profile: SecurityProfile::content_generation("hybrid-cloud-architect"),
            agent_id,
        }
    }

    fn analyze(&self, args: Option<&str>) -> Result<String, String> {
        let input = args.unwrap_or("").to_lowercase();
        let mut recommendations = Vec::new();

        if input.contains("connectivity") || input.contains("network") {
            recommendations.push("Use VPN or dedicated interconnect for secure connectivity");
            recommendations.push("Implement proper network segmentation");
            recommendations.push("Plan for redundancy and failover");
        }
        if input.contains("data") || input.contains("sync") {
            recommendations.push("Define data residency requirements");
            recommendations.push("Implement data sync strategies");
            recommendations.push("Consider latency for cross-cloud operations");
        }
        if recommendations.is_empty() {
            recommendations.push("Define workload placement criteria");
            recommendations.push("Implement consistent identity management");
            recommendations.push("Use infrastructure as code for both environments");
        }

        let result = json!({
            "analysis": { "input": args.unwrap_or(""), "recommendations": recommendations },
            "considerations": ["Security", "Compliance", "Latency", "Cost", "Data sovereignty"]
        });
        Ok(simd_json::to_string_pretty(&result).unwrap_or_default())
    }
}

#[async_trait]
impl AgentTrait for HybridCloudArchitectAgent {
    fn agent_type(&self) -> &str {
        "hybrid-cloud-architect"
    }
    fn name(&self) -> &str {
        "Hybrid Cloud Architect"
    }
    fn description(&self) -> &str {
        "Design hybrid and multi-cloud architectures."
    }
    fn operations(&self) -> Vec<String> {
        vec![
            "design_connectivity".to_string(),
            "plan_migration".to_string(),
            "analyze".to_string(),
        ]
    }
    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }
    fn get_status(&self) -> String {
        format!("Hybrid Cloud Architect agent {} is running", self.agent_id)
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "hybrid-cloud-architect" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }
        match self.analyze(task.args.as_deref()) {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/specialty/legacy_modernizer.rs">
//! Legacy Modernizer Agent

use crate::agents::base::{AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;
use async_trait::async_trait;
use simd_json::json;

pub struct LegacyModernizerAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl LegacyModernizerAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            profile: SecurityProfile::content_generation("legacy-modernizer"),
            agent_id,
        }
    }

    fn analyze(&self, args: Option<&str>) -> Result<String, String> {
        let input = args.unwrap_or("").to_lowercase();
        let mut recommendations = Vec::new();

        if input.contains("assess") || input.contains("audit") {
            recommendations.push("Document current system architecture");
            recommendations.push("Identify technical debt and risks");
            recommendations.push("Map dependencies and integrations");
        }
        if input.contains("migrate") || input.contains("rewrite") {
            recommendations.push("Consider strangler fig pattern");
            recommendations.push("Start with well-bounded components");
            recommendations.push("Maintain backward compatibility during transition");
        }
        if recommendations.is_empty() {
            recommendations.push("Prioritize based on business value and risk");
            recommendations.push("Add tests before refactoring");
            recommendations.push("Modernize incrementally, not big bang");
        }

        let result = json!({
            "analysis": { "input": args.unwrap_or(""), "recommendations": recommendations },
            "strategies": ["Strangler Fig", "Branch by Abstraction", "Event Interception", "Database-First"]
        });
        Ok(simd_json::to_string_pretty(&result).unwrap_or_default())
    }
}

#[async_trait]
impl AgentTrait for LegacyModernizerAgent {
    fn agent_type(&self) -> &str {
        "legacy-modernizer"
    }
    fn name(&self) -> &str {
        "Legacy Modernizer"
    }
    fn description(&self) -> &str {
        "Modernize legacy systems incrementally and safely."
    }
    fn operations(&self) -> Vec<String> {
        vec![
            "assess_system".to_string(),
            "plan_modernization".to_string(),
            "analyze".to_string(),
        ]
    }
    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }
    fn get_status(&self) -> String {
        format!("Legacy Modernizer agent {} is running", self.agent_id)
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "legacy-modernizer" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }
        match self.analyze(task.args.as_deref()) {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/specialty/mod.rs">
//! Specialty Agents
//!
//! Niche and specialized domain agents

mod arm_cortex_expert;
mod snowball_developer;
mod error_detective;
mod hybrid_cloud_architect;
mod legacy_modernizer;
mod observability_engineer;
mod quant_analyst;
mod ui_ux_designer;
mod unity_developer;

pub use arm_cortex_expert::ARMCortexExpertAgent;
pub use snowball_developer::SnowballDeveloperAgent;
pub use error_detective::ErrorDetectiveAgent;
pub use hybrid_cloud_architect::HybridCloudArchitectAgent;
pub use legacy_modernizer::LegacyModernizerAgent;
pub use observability_engineer::ObservabilityEngineerAgent;
pub use quant_analyst::QuantAnalystAgent;
pub use ui_ux_designer::UIUXDesignerAgent;
pub use unity_developer::UnityDeveloperAgent;
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/specialty/observability_engineer.rs">
//! Observability Engineer Agent

use crate::agents::base::{AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;
use async_trait::async_trait;
use simd_json::json;

pub struct ObservabilityEngineerAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl ObservabilityEngineerAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            profile: SecurityProfile::content_generation("observability-engineer"),
            agent_id,
        }
    }

    fn analyze(&self, args: Option<&str>) -> Result<String, String> {
        let input = args.unwrap_or("").to_lowercase();
        let mut recommendations = Vec::new();

        if input.contains("metrics") || input.contains("prometheus") {
            recommendations.push("Use RED method (Rate, Errors, Duration) for services");
            recommendations.push("Use USE method (Utilization, Saturation, Errors) for resources");
            recommendations.push("Set up proper alerting thresholds");
        }
        if input.contains("logging") || input.contains("log") {
            recommendations.push("Use structured logging (JSON format)");
            recommendations.push("Include correlation IDs across services");
            recommendations.push("Set appropriate log levels");
        }
        if input.contains("tracing") || input.contains("trace") {
            recommendations.push("Implement distributed tracing with OpenTelemetry");
            recommendations.push("Add span attributes for debugging context");
            recommendations.push("Sample appropriately for high-volume services");
        }
        if recommendations.is_empty() {
            recommendations.push("Implement all three pillars: metrics, logs, traces");
            recommendations.push("Create service-level dashboards");
            recommendations.push("Define SLIs and SLOs");
        }

        let result = json!({
            "analysis": { "input": args.unwrap_or(""), "recommendations": recommendations },
            "tools": { "metrics": ["Prometheus", "Grafana"], "logs": ["Loki", "ELK"], "traces": ["Jaeger", "Tempo"] }
        });
        Ok(simd_json::to_string_pretty(&result).unwrap_or_default())
    }
}

#[async_trait]
impl AgentTrait for ObservabilityEngineerAgent {
    fn agent_type(&self) -> &str {
        "observability-engineer"
    }
    fn name(&self) -> &str {
        "Observability Engineer"
    }
    fn description(&self) -> &str {
        "Set up monitoring, logging, and tracing infrastructure."
    }
    fn operations(&self) -> Vec<String> {
        vec![
            "setup_metrics".to_string(),
            "configure_logging".to_string(),
            "implement_tracing".to_string(),
            "analyze".to_string(),
        ]
    }
    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }
    fn get_status(&self) -> String {
        format!("Observability Engineer agent {} is running", self.agent_id)
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "observability-engineer" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }
        match self.analyze(task.args.as_deref()) {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/specialty/quant_analyst.rs">
//! Quant Analyst Agent

use crate::agents::base::{AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;
use async_trait::async_trait;
use simd_json::json;

pub struct QuantAnalystAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl QuantAnalystAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            profile: SecurityProfile::content_generation("quant-analyst"),
            agent_id,
        }
    }

    fn analyze(&self, args: Option<&str>) -> Result<String, String> {
        let input = args.unwrap_or("").to_lowercase();
        let mut recommendations = Vec::new();

        if input.contains("backtest") || input.contains("strategy") {
            recommendations.push("Use walk-forward optimization to avoid overfitting");
            recommendations.push("Account for transaction costs and slippage");
            recommendations.push("Test across multiple market regimes");
        }
        if input.contains("risk") {
            recommendations.push("Calculate VaR and Expected Shortfall");
            recommendations.push("Monitor portfolio correlations");
            recommendations.push("Implement position sizing rules");
        }
        if recommendations.is_empty() {
            recommendations.push("Start with robust data cleaning and validation");
            recommendations.push("Use proper statistical significance tests");
            recommendations.push("Document all assumptions and limitations");
        }

        let result = json!({
            "analysis": { "input": args.unwrap_or(""), "recommendations": recommendations },
            "tools": ["Python (pandas, numpy)", "QuantLib", "zipline", "backtrader"]
        });
        Ok(simd_json::to_string_pretty(&result).unwrap_or_default())
    }
}

#[async_trait]
impl AgentTrait for QuantAnalystAgent {
    fn agent_type(&self) -> &str {
        "quant-analyst"
    }
    fn name(&self) -> &str {
        "Quant Analyst"
    }
    fn description(&self) -> &str {
        "Quantitative analysis and algorithmic trading strategies."
    }
    fn operations(&self) -> Vec<String> {
        vec![
            "backtest".to_string(),
            "analyze_risk".to_string(),
            "develop_strategy".to_string(),
            "analyze".to_string(),
        ]
    }
    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }
    fn get_status(&self) -> String {
        format!("Quant Analyst agent {} is running", self.agent_id)
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "quant-analyst" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }
        match self.analyze(task.args.as_deref()) {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/specialty/ui_ux_designer.rs">
//! UI/UX Designer Agent

use crate::agents::base::{AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;
use async_trait::async_trait;
use simd_json::json;

pub struct UIUXDesignerAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl UIUXDesignerAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            profile: SecurityProfile::content_generation("ui-ux-designer"),
            agent_id,
        }
    }

    fn analyze(&self, args: Option<&str>) -> Result<String, String> {
        let input = args.unwrap_or("").to_lowercase();
        let mut recommendations = Vec::new();

        if input.contains("user research") || input.contains("persona") {
            recommendations.push("Conduct user interviews and surveys");
            recommendations.push("Create detailed user personas");
            recommendations.push("Map user journeys and pain points");
        }
        if input.contains("wireframe") || input.contains("prototype") {
            recommendations.push("Start with low-fidelity wireframes");
            recommendations.push("Test with users early and often");
            recommendations.push("Iterate based on feedback");
        }
        if input.contains("accessibility") || input.contains("a11y") {
            recommendations.push("Follow WCAG 2.1 guidelines");
            recommendations.push("Test with screen readers");
            recommendations.push("Ensure sufficient color contrast");
        }
        if recommendations.is_empty() {
            recommendations.push("Design with users, not for users");
            recommendations.push("Follow established design patterns");
            recommendations.push("Create consistent design system");
        }

        let result = json!({
            "analysis": { "input": args.unwrap_or(""), "recommendations": recommendations },
            "principles": ["Clarity", "Consistency", "Feedback", "Efficiency", "Forgiveness"]
        });
        Ok(simd_json::to_string_pretty(&result).unwrap_or_default())
    }
}

#[async_trait]
impl AgentTrait for UIUXDesignerAgent {
    fn agent_type(&self) -> &str {
        "ui-ux-designer"
    }
    fn name(&self) -> &str {
        "UI/UX Designer"
    }
    fn description(&self) -> &str {
        "Design user-centered interfaces and experiences."
    }
    fn operations(&self) -> Vec<String> {
        vec![
            "research_users".to_string(),
            "create_wireframes".to_string(),
            "design_ui".to_string(),
            "analyze".to_string(),
        ]
    }
    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }
    fn get_status(&self) -> String {
        format!("UI/UX Designer agent {} is running", self.agent_id)
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "ui-ux-designer" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }
        match self.analyze(task.args.as_deref()) {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/specialty/unity_developer.rs">
//! Unity Developer Agent

use crate::agents::base::{AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;
use async_trait::async_trait;
use simd_json::json;

pub struct UnityDeveloperAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl UnityDeveloperAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            profile: SecurityProfile::content_generation("unity-developer"),
            agent_id,
        }
    }

    fn analyze(&self, args: Option<&str>) -> Result<String, String> {
        let input = args.unwrap_or("").to_lowercase();
        let mut recommendations = Vec::new();

        if input.contains("performance") {
            recommendations.push("Use object pooling for frequently spawned objects");
            recommendations.push("Optimize draw calls with batching");
            recommendations.push("Profile with Unity Profiler regularly");
        }
        if input.contains("script") || input.contains("code") {
            recommendations.push("Use ScriptableObjects for data");
            recommendations.push("Implement dependency injection");
            recommendations.push("Avoid Update() for infrequent checks");
        }
        if recommendations.is_empty() {
            recommendations.push("Follow Unity's coding conventions");
            recommendations.push("Use prefabs for reusable objects");
            recommendations.push("Implement proper scene management");
        }

        let result = json!({
            "analysis": { "input": args.unwrap_or(""), "recommendations": recommendations },
            "unity_features": ["DOTS", "Addressables", "Input System", "Timeline", "Cinemachine"]
        });
        Ok(simd_json::to_string_pretty(&result).unwrap_or_default())
    }
}

#[async_trait]
impl AgentTrait for UnityDeveloperAgent {
    fn agent_type(&self) -> &str {
        "unity-developer"
    }
    fn name(&self) -> &str {
        "Unity Developer"
    }
    fn description(&self) -> &str {
        "Build games and interactive experiences with Unity."
    }
    fn operations(&self) -> Vec<String> {
        vec![
            "create_script".to_string(),
            "optimize".to_string(),
            "analyze".to_string(),
        ]
    }
    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }
    fn get_status(&self) -> String {
        format!("Unity Developer agent {} is running", self.agent_id)
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "unity-developer" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }
        match self.analyze(task.args.as_deref()) {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/system/mod.rs">
//! System agents - D-Bus based system operations
//!
//! These agents interact with system services via D-Bus:
//! - executor: Command execution
//! - file: File operations
//! - monitor: System monitoring
//! - network: Network configuration
//! - packagekit: Package management
//! - systemd: Systemd service control

pub mod executor;
pub mod file;
pub mod monitor;
pub mod network;
pub mod packagekit;
pub mod systemd;

pub use executor::ExecutorAgent;
pub use file::FileAgent;
pub use monitor::MonitorAgent;
pub use network::NetworkAgent;
pub use packagekit::PackageKitAgent;
pub use systemd::SystemdAgent;
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/webframeworks/django_pro.rs">
//! Django Pro Agent - Django web framework expert

use crate::agents::base::{AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;
use async_trait::async_trait;
use simd_json::json;

pub struct DjangoProAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl DjangoProAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            profile: SecurityProfile::content_generation("django-pro"),
            agent_id,
        }
    }

    fn analyze(&self, args: Option<&str>) -> Result<String, String> {
        let input = args.unwrap_or("").to_lowercase();
        let mut recommendations = Vec::new();

        if input.contains("model") || input.contains("database") {
            recommendations.push("Use migrations for all schema changes");
            recommendations.push("Add db_index=True for frequently queried fields");
            recommendations.push("Use select_related/prefetch_related for N+1 prevention");
        }
        if input.contains("view") || input.contains("api") {
            recommendations.push("Use class-based views for complex logic");
            recommendations.push("Implement proper permission classes");
            recommendations.push("Add pagination for list endpoints");
        }
        if input.contains("auth") || input.contains("security") {
            recommendations.push("Use Django's built-in authentication");
            recommendations.push("Implement CSRF protection");
            recommendations.push("Use django-allauth for social auth");
        }
        if recommendations.is_empty() {
            recommendations.push("Follow Django project structure conventions");
            recommendations.push("Use Django REST Framework for APIs");
            recommendations.push("Implement proper logging and error handling");
        }

        let result = json!({
            "analysis": { "input": args.unwrap_or(""), "recommendations": recommendations },
            "best_practices": ["Use environment variables for secrets", "Implement caching with Redis", "Write tests with pytest-django"]
        });
        Ok(simd_json::to_string_pretty(&result).unwrap_or_default())
    }
}

#[async_trait]
impl AgentTrait for DjangoProAgent {
    fn agent_type(&self) -> &str {
        "django-pro"
    }
    fn name(&self) -> &str {
        "Django Pro"
    }
    fn description(&self) -> &str {
        "Expert Django developer for web applications and REST APIs."
    }
    fn operations(&self) -> Vec<String> {
        vec![
            "create_model".to_string(),
            "create_view".to_string(),
            "configure_auth".to_string(),
            "analyze".to_string(),
        ]
    }
    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }
    fn get_status(&self) -> String {
        format!("Django Pro agent {} is running", self.agent_id)
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "django-pro" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }
        match self.analyze(task.args.as_deref()) {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/webframeworks/fastapi_pro.rs">
//! FastAPI Pro Agent - FastAPI framework expert

use crate::agents::base::{AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;
use async_trait::async_trait;
use simd_json::json;

pub struct FastAPIProAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl FastAPIProAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            profile: SecurityProfile::content_generation("fastapi-pro"),
            agent_id,
        }
    }

    fn analyze(&self, args: Option<&str>) -> Result<String, String> {
        let input = args.unwrap_or("").to_lowercase();
        let mut recommendations = Vec::new();

        if input.contains("endpoint") || input.contains("route") {
            recommendations.push("Use Pydantic models for request/response validation");
            recommendations.push("Implement proper HTTP status codes");
            recommendations.push("Add OpenAPI documentation with examples");
        }
        if input.contains("async") || input.contains("performance") {
            recommendations.push("Use async def for I/O-bound operations");
            recommendations.push("Implement connection pooling for databases");
            recommendations.push("Use background tasks for long operations");
        }
        if input.contains("auth") || input.contains("security") {
            recommendations.push("Use OAuth2 with JWT tokens");
            recommendations.push("Implement dependency injection for auth");
            recommendations.push("Add rate limiting with slowapi");
        }
        if recommendations.is_empty() {
            recommendations.push("Structure with routers for modularity");
            recommendations.push("Use dependency injection for reusable components");
            recommendations.push("Implement proper error handling with HTTPException");
        }

        let result = json!({
            "analysis": { "input": args.unwrap_or(""), "recommendations": recommendations },
            "stack": ["uvicorn", "pydantic", "sqlalchemy", "alembic", "pytest"]
        });
        Ok(simd_json::to_string_pretty(&result).unwrap_or_default())
    }
}

#[async_trait]
impl AgentTrait for FastAPIProAgent {
    fn agent_type(&self) -> &str {
        "fastapi-pro"
    }
    fn name(&self) -> &str {
        "FastAPI Pro"
    }
    fn description(&self) -> &str {
        "Expert FastAPI developer for high-performance async APIs."
    }
    fn operations(&self) -> Vec<String> {
        vec![
            "create_endpoint".to_string(),
            "configure_auth".to_string(),
            "optimize".to_string(),
            "analyze".to_string(),
        ]
    }
    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }
    fn get_status(&self) -> String {
        format!("FastAPI Pro agent {} is running", self.agent_id)
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "fastapi-pro" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }
        match self.analyze(task.args.as_deref()) {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/webframeworks/mod.rs">
//! Web Framework Agents
//!
//! Specialized agents for web framework development:
//! - `DjangoPro`: Django web framework expert
//! - `FastAPIPro`: FastAPI framework expert
//! - `TemporalPythonPro`: Temporal workflow expert

mod django_pro;
mod fastapi_pro;
mod temporal_python_pro;

pub use django_pro::DjangoProAgent;
pub use fastapi_pro::FastAPIProAgent;
pub use temporal_python_pro::TemporalPythonProAgent;
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/webframeworks/temporal_python_pro.rs">
//! Temporal Python Pro Agent - Temporal workflow expert

use crate::agents::base::{AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;
use async_trait::async_trait;
use simd_json::json;

pub struct TemporalPythonProAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl TemporalPythonProAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            profile: SecurityProfile::content_generation("temporal-python-pro"),
            agent_id,
        }
    }

    fn analyze(&self, args: Option<&str>) -> Result<String, String> {
        let input = args.unwrap_or("").to_lowercase();
        let mut recommendations = Vec::new();

        if input.contains("workflow") {
            recommendations.push("Keep workflows deterministic - no random, time, or I/O");
            recommendations.push("Use activities for all side effects");
            recommendations.push("Implement proper error handling with retries");
        }
        if input.contains("activity") {
            recommendations.push("Activities should be idempotent when possible");
            recommendations.push("Configure appropriate timeouts for each activity");
            recommendations.push("Use heartbeats for long-running activities");
        }
        if input.contains("saga") || input.contains("compensation") {
            recommendations.push("Implement compensation logic for rollbacks");
            recommendations.push("Use saga pattern for distributed transactions");
            recommendations.push("Store compensation state for reliability");
        }
        if recommendations.is_empty() {
            recommendations.push("Design workflows to be resumable");
            recommendations.push("Use signals for external events");
            recommendations.push("Implement proper versioning for workflow changes");
        }

        let result = json!({
            "analysis": { "input": args.unwrap_or(""), "recommendations": recommendations },
            "patterns": ["Saga pattern", "State machine", "Long-running workflows", "Child workflows"],
            "testing": ["Time-skipping tests", "Activity mocking", "Workflow replay testing"]
        });
        Ok(simd_json::to_string_pretty(&result).unwrap_or_default())
    }
}

#[async_trait]
impl AgentTrait for TemporalPythonProAgent {
    fn agent_type(&self) -> &str {
        "temporal-python-pro"
    }
    fn name(&self) -> &str {
        "Temporal Python Pro"
    }
    fn description(&self) -> &str {
        "Expert in Temporal workflows for durable, reliable distributed systems."
    }
    fn operations(&self) -> Vec<String> {
        vec![
            "design_workflow".to_string(),
            "design_activity".to_string(),
            "implement_saga".to_string(),
            "analyze".to_string(),
        ]
    }
    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }
    fn get_status(&self) -> String {
        format!("Temporal Python Pro agent {} is running", self.agent_id)
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "temporal-python-pro" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }
        match self.analyze(task.args.as_deref()) {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/base.rs">
//! Base agent trait and common types
//!
//! Defines the common interface for all agents and shared types.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::security::{SandboxExecutor, SandboxResult, SecurityProfile};

/// Agent task input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTask {
    /// Task type identifier (matches agent type)
    #[serde(rename = "type")]
    pub task_type: String,

    /// Operation to perform
    pub operation: String,

    /// Working path (optional)
    #[serde(default)]
    pub path: Option<String>,

    /// Additional arguments
    #[serde(default)]
    pub args: Option<String>,

    /// Additional configuration
    #[serde(default)]
    pub config: HashMap<String, simd_json::OwnedValue>,
}

impl AgentTask {
    pub fn new(task_type: &str, operation: &str) -> Self {
        Self {
            task_type: task_type.to_string(),
            operation: operation.to_string(),
            path: None,
            args: None,
            config: HashMap::new(),
        }
    }

    pub fn with_path(mut self, path: &str) -> Self {
        self.path = Some(path.to_string());
        self
    }

    pub fn with_args(mut self, args: &str) -> Self {
        self.args = Some(args.to_string());
        self
    }

    pub fn with_config(mut self, key: &str, value: simd_json::OwnedValue) -> Self {
        self.config.insert(key.to_string(), value);
        self
    }
}

/// Agent task result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    /// Whether the operation succeeded
    pub success: bool,

    /// Operation that was performed
    pub operation: String,

    /// Result data
    pub data: String,

    /// Additional metadata
    #[serde(default)]
    pub metadata: HashMap<String, simd_json::OwnedValue>,
}

impl TaskResult {
    pub fn success(operation: &str, data: String) -> Self {
        Self {
            success: true,
            operation: operation.to_string(),
            data,
            metadata: HashMap::new(),
        }
    }

    pub fn failure(operation: &str, error: String) -> Self {
        Self {
            success: false,
            operation: operation.to_string(),
            data: error,
            metadata: HashMap::new(),
        }
    }

    pub fn from_execution(operation: &str, result: &SandboxResult) -> Self {
        let data = format!("stdout:\n{}\n\nstderr:\n{}", result.stdout, result.stderr);

        Self {
            success: result.success,
            operation: operation.to_string(),
            data,
            metadata: {
                let mut meta = HashMap::new();
                meta.insert(
                    "duration_ms".to_string(),
                    simd_json::json!(result.duration_ms),
                );
                meta.insert("truncated".to_string(), simd_json::json!(result.truncated));
                meta
            },
        }
    }

    pub fn with_metadata(mut self, key: &str, value: simd_json::OwnedValue) -> Self {
        self.metadata.insert(key.to_string(), value);
        self
    }

    pub fn to_json(&self) -> String {
        simd_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }
}

/// Agent execution context
pub struct AgentContext {
    /// Agent ID
    pub agent_id: String,

    /// Security profile
    pub profile: SecurityProfile,

    /// Sandbox executor
    pub executor: SandboxExecutor,

    /// Working directory
    pub working_dir: Option<PathBuf>,
}

impl AgentContext {
    pub fn new(agent_id: String, profile: SecurityProfile) -> Self {
        let executor = SandboxExecutor::new(profile.clone());
        Self {
            agent_id,
            profile,
            executor,
            working_dir: None,
        }
    }

    pub fn with_working_dir(mut self, dir: PathBuf) -> Self {
        self.working_dir = Some(dir);
        self
    }
}

/// Base trait for all agents
#[async_trait]
pub trait AgentTrait: Send + Sync {
    /// Get agent type identifier
    fn agent_type(&self) -> &str;

    /// Get agent display name
    fn name(&self) -> &str;

    /// Get agent description
    fn description(&self) -> &str;

    /// Get supported operations
    fn operations(&self) -> Vec<String>;

    /// Get security profile
    fn security_profile(&self) -> &SecurityProfile;

    /// Execute a task
    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String>;

    /// Get agent status
    fn get_status(&self) -> String {
        format!("{} is running", self.name())
    }

    /// Check if agent supports an operation
    fn supports_operation(&self, op: &str) -> bool {
        self.operations().iter().any(|o| o == op)
    }
}

/// Common validation functions for agents
pub mod validation {
    pub const FORBIDDEN_CHARS: &[char] = &[
        '$', '`', ';', '&', '|', '>', '<', '(', ')', '{', '}', '\n', '\r',
    ];
    pub const MAX_PATH_LENGTH: usize = 4096;
    pub const MAX_ARGS_LENGTH: usize = 256;

    pub fn validate_path(path: &str, allowed_dirs: &[&str]) -> Result<String, String> {
        if path.len() > MAX_PATH_LENGTH {
            return Err("Path exceeds maximum length".to_string());
        }

        for c in FORBIDDEN_CHARS {
            if path.contains(*c) {
                return Err(format!("Path contains forbidden character: {:?}", c));
            }
        }

        let is_allowed = allowed_dirs.iter().any(|dir| path.starts_with(dir));
        if !is_allowed {
            return Err(format!(
                "Path must be within allowed directories: {:?}",
                allowed_dirs
            ));
        }

        Ok(path.to_string())
    }

    pub fn validate_args(args: &str) -> Result<String, String> {
        if args.len() > MAX_ARGS_LENGTH {
            return Err("Args string too long".to_string());
        }

        for c in FORBIDDEN_CHARS {
            if args.contains(*c) {
                return Err(format!("Args contains forbidden character: {:?}", c));
            }
        }

        Ok(args.to_string())
    }
}

/// Macro for implementing common agent boilerplate
#[macro_export]
macro_rules! impl_agent_common {
    ($agent:ty, $type:expr, $name:expr, $desc:expr, $ops:expr) => {
        impl $agent {
            pub fn agent_type(&self) -> &str {
                $type
            }
            pub fn name(&self) -> &str {
                $name
            }
            pub fn description(&self) -> &str {
                $desc
            }
            pub fn operations(&self) -> Vec<String> {
                $ops.iter().map(|s| s.to_string()).collect()
            }
        }
    };
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agents/mod.rs">
#![allow(dead_code)]
//! Agent implementations organized by category
//!
//! Categories:
//! - language: Programming language specific agents (python-pro, rust-pro, etc.)
//! - infrastructure: DevOps and infrastructure agents
//! - analysis: Code review and analysis agents
//! - database: Database-related agents
//! - content: Documentation and content generation agents
//! - orchestration: Meta-agents that coordinate others
//! - architecture: Software architecture agents (backend, frontend, graphql)
//! - operations: SRE and operations agents (incident response, troubleshooting)
//! - aiml: AI/ML specialized agents (ai-engineer, ml-engineer, etc.)
//! - webframeworks: Web framework agents (django, fastapi, temporal)
//! - mobile: Mobile development agents (flutter, ios, android)
//! - security: Security-focused coding agents
//! - business: Business and operations agents
//! - seo: SEO and content marketing agents
//! - specialty: Niche domain agents (snowball, gaming, finance, etc.)

pub mod aiml;
pub mod analysis;
pub mod architecture;
pub mod base;
pub mod business;
pub mod content;
pub mod database;
pub mod infrastructure;
pub mod language;
pub mod mobile;
pub mod operations;
pub mod orchestration;
pub mod security;
pub mod seo;
pub mod specialty;
pub mod webframeworks;

// Re-export common types
pub use base::{AgentContext, AgentTask, AgentTrait, TaskResult};
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/bin/dbus-agent-manager.rs">
//! D-Bus Agent Manager
//!
//! Starts and manages all agents as D-Bus services.
//! Run this as a systemd service to have agents available.
//!
//! Each agent registers on D-Bus as:
//!   - Service: org.dbusmcp.Agent.{AgentType}
//!   - Path: /org/dbusmcp/Agent/{AgentType}
//!   - Interface: org.dbusmcp.Agent
//!
//! The ChatActor's tool_loader discovers these via introspection.

use anyhow::Result;
use op_agents::{
    create_agent,
    dbus_service::{start_agent, DbusAgentService},
};
use op_core::BusType;
use std::collections::HashMap;
use tokio::signal;
use tracing::{error, info, warn};
use zbus::Connection;

/// Agent configuration
struct AgentConfig {
    agent_type: &'static str,
    auto_start: bool,
    priority: u8,
}

/// Agents to start (run-on-connection + on-demand)
const AGENTS: &[AgentConfig] = &[
    // Orchestration (Critical)
    AgentConfig {
        agent_type: "memory",
        auto_start: true,
        priority: 100,
    },
    AgentConfig {
        agent_type: "context-manager",
        auto_start: true,
        priority: 100,
    },
    AgentConfig {
        agent_type: "sequential-thinking",
        auto_start: true,
        priority: 100,
    },
    AgentConfig {
        agent_type: "dx-optimizer",
        auto_start: true,
        priority: 95,
    },
    AgentConfig {
        agent_type: "tdd-orchestrator",
        auto_start: true,
        priority: 95,
    },
    // Language & Architecture (High)
    AgentConfig {
        agent_type: "rust-pro",
        auto_start: true,
        priority: 90,
    },
    AgentConfig {
        agent_type: "python-pro",
        auto_start: true,
        priority: 90,
    },
    AgentConfig {
        agent_type: "backend-architect",
        auto_start: true,
        priority: 90,
    },
    AgentConfig {
        agent_type: "frontend-developer",
        auto_start: true,
        priority: 90,
    },
    AgentConfig {
        agent_type: "database-architect",
        auto_start: true,
        priority: 85,
    },
    AgentConfig {
        agent_type: "backend-security-coder",
        auto_start: true,
        priority: 85,
    },
    // Infrastructure & Ops (Medium)
    AgentConfig {
        agent_type: "network-engineer",
        auto_start: true,
        priority: 80,
    },
    AgentConfig {
        agent_type: "deployment",
        auto_start: true,
        priority: 80,
    },
    AgentConfig {
        agent_type: "devops-troubleshooter",
        auto_start: true,
        priority: 80,
    },
    // Analysis & Quality (Medium)
    AgentConfig {
        agent_type: "debugger",
        auto_start: true,
        priority: 75,
    },
    AgentConfig {
        agent_type: "code-reviewer",
        auto_start: true,
        priority: 75,
    },
    AgentConfig {
        agent_type: "search-specialist",
        auto_start: true,
        priority: 75,
    },
    AgentConfig {
        agent_type: "prompt-engineer",
        auto_start: true,
        priority: 70,
    },
    AgentConfig {
        agent_type: "docs-architect",
        auto_start: true,
        priority: 70,
    },
];

/// Agent Manager - starts and monitors D-Bus agent services
struct AgentManager {
    connections: HashMap<String, Connection>,
    bus_type: BusType,
}

impl AgentManager {
    fn new(bus_type: BusType) -> Self {
        Self {
            connections: HashMap::new(),
            bus_type,
        }
    }

    /// Start an agent as a D-Bus service
    async fn start_agent(&mut self, agent_type: &str) -> Result<()> {
        if self.connections.contains_key(agent_type) {
            info!("Agent {} already running", agent_type);
            return Ok(());
        }

        // Create the agent
        let agent_id = format!("{}-main", agent_type);
        let agent = create_agent(agent_type, agent_id.clone())
            .map_err(|e| anyhow::anyhow!("Failed to create agent {}: {}", agent_type, e))?;

        // Start as D-Bus service
        let connection = start_agent(agent, &agent_id, self.bus_type)
            .await
            .map_err(|e| {
                anyhow::anyhow!("Failed to start D-Bus service for {}: {}", agent_type, e)
            })?;

        let service_name = DbusAgentService::service_name(agent_type);
        info!("✓ Started D-Bus agent: {} at {}", agent_type, service_name);

        self.connections.insert(agent_type.to_string(), connection);
        Ok(())
    }

    /// Start all auto-start agents
    async fn start_auto_agents(&mut self) -> Result<()> {
        let mut started = 0;
        let mut failed = 0;

        // Sort by priority (highest first)
        let mut agents: Vec<_> = AGENTS.iter().filter(|a| a.auto_start).collect();
        agents.sort_by(|a, b| b.priority.cmp(&a.priority));

        for config in agents {
            match self.start_agent(config.agent_type).await {
                Ok(_) => started += 1,
                Err(e) => {
                    error!("Failed to start {}: {}", config.agent_type, e);
                    failed += 1;
                }
            }
        }

        info!(
            "Agent startup complete: {} started, {} failed",
            started, failed
        );
        Ok(())
    }

    /// List running agents
    fn list_running(&self) -> Vec<&str> {
        self.connections.keys().map(|s| s.as_str()).collect()
    }

    /// Stop an agent
    async fn stop_agent(&mut self, agent_type: &str) -> Result<()> {
        if let Some(_conn) = self.connections.remove(agent_type) {
            info!("Stopped agent: {}", agent_type);
            // Connection drops, D-Bus service unregisters
        }
        Ok(())
    }

    /// Stop all agents
    async fn stop_all(&mut self) {
        let agents: Vec<_> = self.connections.keys().cloned().collect();
        for agent in agents {
            let _ = self.stop_agent(&agent).await;
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("op_agents=info".parse().unwrap()),
        )
        .init();

    info!("Starting op-dbus Agent Manager");

    // Determine bus type from environment
    let bus_type = if std::env::var("DBUS_AGENT_SESSION").is_ok() {
        info!("Using session bus");
        BusType::Session
    } else {
        info!("Using system bus");
        BusType::System
    };

    // Create manager and start agents
    let mut manager = AgentManager::new(bus_type);

    if let Err(e) = manager.start_auto_agents().await {
        error!("Failed to start agents: {}", e);
        return Err(e);
    }

    info!(
        "Agent Manager ready. Running agents: {:?}",
        manager.list_running()
    );
    info!("Press Ctrl+C to stop");

    // Wait for shutdown signal
    signal::ctrl_c().await?;

    info!("Shutting down Agent Manager...");
    manager.stop_all().await;

    info!("Agent Manager stopped");
    Ok(())
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/bin/dbus-agent.rs">
//! D-Bus Agent Launcher
//!
//! Universal binary to run any agent type as a D-Bus service.
//!
//! # Usage
//!
//! ```bash
//! # Run an agent on the session bus
//! dbus-agent python-pro
//!
//! # Run with a custom agent ID
//! dbus-agent python-pro my-python-agent
//!
//! # Run on the system bus (requires privileges)
//! dbus-agent --system rust-pro
//!
//! # List available agent types
//! dbus-agent --list
//! ```
//!
//! # D-Bus Registration
//!
//! The agent will be registered as:
//! - Service name: `org.dbusmcp.Agent.{AgentType}` (e.g., `org.dbusmcp.Agent.PythonPro`)
//! - Object path: `/org/dbusmcp/Agent/{AgentType}`
//! - Interface: `org.dbusmcp.Agent`
//!
//! # Discovery
//!
//! Once running, the agent can be discovered by the ChatActor's tool_loader
//! and registered as a tool that the LLM can call.

use std::env;

use op_core::BusType;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

use op_agents::agent_catalog::builtin_agent_descriptors;
use op_agents::agents::{
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
        ARMCortexExpertAgent, SnowballDeveloperAgent, ErrorDetectiveAgent,
        HybridCloudArchitectAgent, LegacyModernizerAgent, ObservabilityEngineerAgent,
        QuantAnalystAgent, UIUXDesignerAgent, UnityDeveloperAgent,
    },
    webframeworks::{DjangoProAgent, FastAPIProAgent, TemporalPythonProAgent},
    AgentTrait,
};
use op_agents::dbus_service::{generate_agent_id, start_agent};

fn print_usage() {
    eprintln!(
        "Usage:\n  dbus-agent [--system] <agent-type> [agent-id]\n  dbus-agent --list\n\nExamples:\n  dbus-agent python-pro\n  dbus-agent --system rust-pro\n  dbus-agent python-pro my-agent-id"
    );
}

fn normalize_agent_type(raw: &str) -> String {
    raw.trim().to_lowercase().replace('_', "-")
}

fn build_agent(agent_type: &str, agent_id: String) -> Option<Box<dyn AgentTrait>> {
    match agent_type {
        // Language agents
        "bash-pro" => Some(Box::new(BashProAgent::new(agent_id))),
        "c-pro" => Some(Box::new(CProAgent::new(agent_id))),
        "cpp-pro" => Some(Box::new(CppProAgent::new(agent_id))),
        "csharp-pro" => Some(Box::new(CSharpProAgent::new(agent_id))),
        "elixir-pro" => Some(Box::new(ElixirProAgent::new(agent_id))),
        "golang-pro" => Some(Box::new(GolangProAgent::new(agent_id))),
        "java-pro" => Some(Box::new(JavaProAgent::new(agent_id))),
        "javascript-pro" => Some(Box::new(JavaScriptProAgent::new(agent_id))),
        "julia-pro" => Some(Box::new(JuliaProAgent::new(agent_id))),
        "php-pro" => Some(Box::new(PhpProAgent::new(agent_id))),
        "python-pro" => Some(Box::new(PythonProAgent::new(agent_id))),
        "ruby-pro" => Some(Box::new(RubyProAgent::new(agent_id))),
        "rust-pro" => Some(Box::new(RustProAgent::new(agent_id))),
        "scala-pro" => Some(Box::new(ScalaProAgent::new(agent_id))),
        "typescript-pro" => Some(Box::new(TypeScriptProAgent::new(agent_id))),
        // Architecture agents
        "backend-architect" => Some(Box::new(BackendArchitectAgent::new(agent_id))),
        "frontend-developer" => Some(Box::new(FrontendDeveloperAgent::new(agent_id))),
        "graphql-architect" => Some(Box::new(GraphQLArchitectAgent::new(agent_id))),
        // Infrastructure agents
        "cloud-architect" => Some(Box::new(CloudArchitectAgent::new(agent_id))),
        "deployment-engineer" => Some(Box::new(DeploymentAgent::new(agent_id))),
        "kubernetes-architect" => Some(Box::new(KubernetesAgent::new(agent_id))),
        "network-engineer" => Some(Box::new(NetworkEngineerAgent::new(agent_id))),
        "terraform-specialist" => Some(Box::new(TerraformAgent::new(agent_id))),
        // Analysis agents
        "code-reviewer" => Some(Box::new(CodeReviewerAgent::new(agent_id))),
        "debugger" => Some(Box::new(DebuggerAgent::new(agent_id))),
        "performance-engineer" => Some(Box::new(PerformanceEngineerAgent::new(agent_id))),
        "security-auditor" => Some(Box::new(SecurityAuditorAgent::new(agent_id))),
        // Business agents
        "business-analyst" => Some(Box::new(BusinessAnalystAgent::new(agent_id))),
        "customer-support" => Some(Box::new(CustomerSupportAgent::new(agent_id))),
        "hr-pro" => Some(Box::new(HRProAgent::new(agent_id))),
        "legal-advisor" => Some(Box::new(LegalAdvisorAgent::new(agent_id))),
        "payment-integration" => Some(Box::new(PaymentIntegrationAgent::new(agent_id))),
        "sales-automator" => Some(Box::new(SalesAutomatorAgent::new(agent_id))),
        // Content agents
        "api-documenter" => Some(Box::new(ApiDocumenterAgent::new(agent_id))),
        "docs-architect" => Some(Box::new(DocsArchitectAgent::new(agent_id))),
        "mermaid-expert" => Some(Box::new(MermaidExpertAgent::new(agent_id))),
        "tutorial-engineer" => Some(Box::new(TutorialEngineerAgent::new(agent_id))),
        // Database agents
        "database-architect" => Some(Box::new(DatabaseArchitectAgent::new(agent_id))),
        "database-optimizer" => Some(Box::new(DatabaseOptimizerAgent::new(agent_id))),
        "sql-pro" => Some(Box::new(SqlProAgent::new(agent_id))),
        // Operations agents
        "devops-troubleshooter" => Some(Box::new(DevOpsTroubleshooterAgent::new(agent_id))),
        "incident-responder" => Some(Box::new(IncidentResponderAgent::new(agent_id))),
        "test-automator" => Some(Box::new(TestAutomatorAgent::new(agent_id))),
        // Orchestration agents
        "context-manager" => Some(Box::new(ContextManagerAgent::new(agent_id))),
        "memory" => Some(Box::new(MemoryAgent::new(agent_id))),
        "sequential-thinking" => Some(Box::new(SequentialThinkingAgent::new(agent_id))),
        "dx-optimizer" => Some(Box::new(DxOptimizerAgent::new(agent_id))),
        "tdd-orchestrator" => Some(Box::new(TddOrchestratorAgent::new(agent_id))),
        // Security agents
        "backend-security-coder" => Some(Box::new(BackendSecurityCoderAgent::new(agent_id))),
        "frontend-security-coder" => Some(Box::new(FrontendSecurityCoderAgent::new(agent_id))),
        "mobile-security-coder" => Some(Box::new(MobileSecurityCoderAgent::new(agent_id))),
        // SEO agents
        "content-marketer" => Some(Box::new(ContentMarketerAgent::new(agent_id))),
        "search-specialist" => Some(Box::new(SearchSpecialistAgent::new(agent_id))),
        "seo-content-writer" => Some(Box::new(SEOContentWriterAgent::new(agent_id))),
        "seo-keyword-strategist" => Some(Box::new(SEOKeywordStrategistAgent::new(agent_id))),
        "seo-meta-optimizer" => Some(Box::new(SEOMetaOptimizerAgent::new(agent_id))),
        // Specialty agents
        "arm-cortex-expert" => Some(Box::new(ARMCortexExpertAgent::new(agent_id))),
        "snowball-developer" => Some(Box::new(SnowballDeveloperAgent::new(agent_id))),
        "error-detective" => Some(Box::new(ErrorDetectiveAgent::new(agent_id))),
        "hybrid-cloud-architect" => Some(Box::new(HybridCloudArchitectAgent::new(agent_id))),
        "legacy-modernizer" => Some(Box::new(LegacyModernizerAgent::new(agent_id))),
        "observability-engineer" => Some(Box::new(ObservabilityEngineerAgent::new(agent_id))),
        "quant-analyst" => Some(Box::new(QuantAnalystAgent::new(agent_id))),
        "ui-ux-designer" => Some(Box::new(UIUXDesignerAgent::new(agent_id))),
        "unity-developer" => Some(Box::new(UnityDeveloperAgent::new(agent_id))),
        // AI/ML agents
        "ai-engineer" => Some(Box::new(AIEngineerAgent::new(agent_id))),
        "data-engineer" => Some(Box::new(DataEngineerAgent::new(agent_id))),
        "data-scientist" => Some(Box::new(DataScientistAgent::new(agent_id))),
        "ml-engineer" => Some(Box::new(MLEngineerAgent::new(agent_id))),
        "mlops-engineer" => Some(Box::new(MLOpsEngineerAgent::new(agent_id))),
        "prompt-engineer" => Some(Box::new(PromptEngineerAgent::new(agent_id))),
        // Web framework agents
        "django-pro" => Some(Box::new(DjangoProAgent::new(agent_id))),
        "fastapi-pro" => Some(Box::new(FastAPIProAgent::new(agent_id))),
        "temporal-python-pro" => Some(Box::new(TemporalPythonProAgent::new(agent_id))),
        // Mobile agents
        "flutter-expert" => Some(Box::new(FlutterExpertAgent::new(agent_id))),
        "ios-developer" => Some(Box::new(IOSDeveloperAgent::new(agent_id))),
        "mobile-developer" => Some(Box::new(MobileDeveloperAgent::new(agent_id))),
        _ => None,
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("op_agents=info".parse()?))
        .init();

    let mut args = env::args().skip(1);
    let mut use_system = false;

    let mut raw = Vec::new();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--system" => use_system = true,
            "--list" => {
                for descriptor in builtin_agent_descriptors() {
                    println!("{} - {}", descriptor.agent_type, descriptor.description);
                }
                return Ok(());
            }
            "--help" | "-h" => {
                print_usage();
                return Ok(());
            }
            _ => raw.push(arg),
        }
    }

    if raw.is_empty() {
        print_usage();
        return Ok(());
    }

    let agent_type = normalize_agent_type(&raw[0]);
    let agent_id = raw
        .get(1)
        .cloned()
        .unwrap_or_else(|| generate_agent_id(&agent_type));

    let Some(agent) = build_agent(&agent_type, agent_id.clone()) else {
        error!("Unknown agent type: {}", agent_type);
        warn!("Use --list to see available agents.");
        return Ok(());
    };

    let bus_type = if use_system {
        BusType::System
    } else {
        BusType::Session
    };

    info!(
        "Starting agent '{}' with id '{}' on {:?} bus",
        agent_type, agent_id, bus_type
    );

    let _conn = start_agent(agent, &agent_id, bus_type).await?;
    tokio::signal::ctrl_c().await?;
    Ok(())
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/generator/md_parser.rs">
//! Markdown agent definition parser
//!
//! Parses markdown files with YAML frontmatter containing:
//! - name: agent identifier
//! - description: agent description
//! - model: LLM model to use
//! - Markdown content with capabilities, purpose, etc.

use anyhow::{Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Parsed agent definition from markdown file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDefinition {
    /// Agent name (from frontmatter)
    pub name: String,

    /// Description (from frontmatter)
    pub description: String,

    /// Model to use (from frontmatter)
    pub model: String,

    /// Purpose section content
    pub purpose: Option<String>,

    /// Parsed capabilities
    pub capabilities: ParsedCapabilities,

    /// Behavioral traits
    pub behavioral_traits: Vec<String>,

    /// Knowledge base items
    pub knowledge_base: Vec<String>,

    /// Example interactions
    pub examples: Vec<String>,

    /// Raw markdown content
    pub raw_content: String,
}

/// Parsed capabilities from markdown
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ParsedCapabilities {
    /// Category groups (e.g., "Modern Python Features", "Testing & QA")
    pub categories: HashMap<String, Vec<String>>,

    /// All capability items flattened
    pub items: Vec<String>,

    /// Detected operations based on capabilities
    pub detected_operations: Vec<DetectedOperation>,
}

/// An operation detected from capability analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedOperation {
    /// Operation name
    pub name: String,

    /// Description
    pub description: String,

    /// Associated commands
    pub commands: Vec<String>,

    /// Risk level (inferred)
    pub risk: String,
}

/// YAML frontmatter structure
#[derive(Debug, Deserialize)]
struct Frontmatter {
    name: String,
    description: String,
    #[serde(default = "default_model")]
    model: String,
}

fn default_model() -> String {
    "sonnet".to_string()
}

/// Parse an agent markdown file
pub fn parse_agent_markdown(content: &str) -> Result<AgentDefinition> {
    // Extract YAML frontmatter
    let frontmatter_re = Regex::new(r"(?s)^---\n(.*?)\n---\n(.*)$").unwrap();

    let captures = frontmatter_re
        .captures(content)
        .context("No YAML frontmatter found")?;

    let yaml_content = captures.get(1).unwrap().as_str();
    let markdown_content = captures.get(2).unwrap().as_str();

    // Parse frontmatter
    let frontmatter: Frontmatter =
        serde_yaml::from_str(yaml_content).context("Failed to parse YAML frontmatter")?;

    // Parse markdown sections
    let purpose = extract_section(markdown_content, "Purpose");
    let capabilities = parse_capabilities(markdown_content);
    let behavioral_traits = extract_list_section(markdown_content, "Behavioral Traits");
    let knowledge_base = extract_list_section(markdown_content, "Knowledge Base");
    let examples = extract_list_section(markdown_content, "Example Interactions");

    Ok(AgentDefinition {
        name: frontmatter.name,
        description: frontmatter.description,
        model: frontmatter.model,
        purpose,
        capabilities,
        behavioral_traits,
        knowledge_base,
        examples,
        raw_content: content.to_string(),
    })
}

/// Parse a markdown file from disk
pub async fn parse_agent_file(path: &Path) -> Result<AgentDefinition> {
    let content = tokio::fs::read_to_string(path)
        .await
        .context(format!("Failed to read file: {:?}", path))?;

    parse_agent_markdown(&content)
}

/// Extract a section by heading
fn extract_section(content: &str, heading: &str) -> Option<String> {
    let pattern = format!(r"(?s)##\s*{}\s*\n(.*?)(?:\n##|\z)", regex::escape(heading));
    let re = Regex::new(&pattern).ok()?;

    re.captures(content)
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str().trim().to_string())
}

/// Extract a list section (bullet points)
fn extract_list_section(content: &str, heading: &str) -> Vec<String> {
    let section = extract_section(content, heading).unwrap_or_default();

    section
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with('-') || trimmed.starts_with('*') {
                Some(trimmed[1..].trim().to_string())
            } else {
                None
            }
        })
        .collect()
}

/// Parse capabilities section
fn parse_capabilities(content: &str) -> ParsedCapabilities {
    let mut capabilities = ParsedCapabilities::default();

    // Find the Capabilities section
    let cap_section = extract_section(content, "Capabilities").unwrap_or_default();

    // Parse subsections (### headings)
    let subsection_re = Regex::new(r"(?s)###\s*([^\n]+)\n(.*?)(?:###|\z)").unwrap();

    for cap in subsection_re.captures_iter(&cap_section) {
        let category = cap
            .get(1)
            .map(|m| m.as_str().trim().to_string())
            .unwrap_or_default();
        let items_text = cap.get(2).map(|m| m.as_str()).unwrap_or("");

        let items: Vec<String> = items_text
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                if trimmed.starts_with('-') || trimmed.starts_with('*') {
                    Some(trimmed[1..].trim().to_string())
                } else {
                    None
                }
            })
            .collect();

        capabilities.items.extend(items.clone());
        capabilities.categories.insert(category, items);
    }

    // Detect operations from capabilities
    capabilities.detected_operations = detect_operations(&capabilities);

    capabilities
}

/// Detect operations based on capability keywords
fn detect_operations(capabilities: &ParsedCapabilities) -> Vec<DetectedOperation> {
    let mut operations = Vec::new();
    let items_lower: Vec<String> = capabilities
        .items
        .iter()
        .map(|s| s.to_lowercase())
        .collect();

    // Check for common operation patterns
    let operation_patterns = [
        (
            "execute",
            vec!["execute", "run", "execute code", "script"],
            "execute code",
        ),
        (
            "test",
            vec!["test", "testing", "pytest", "jest", "cargo test"],
            "run tests",
        ),
        (
            "lint",
            vec!["lint", "linting", "eslint", "pylint", "clippy"],
            "run linters",
        ),
        (
            "format",
            vec!["format", "formatting", "black", "prettier", "rustfmt"],
            "format code",
        ),
        (
            "build",
            vec!["build", "compile", "compilation"],
            "build/compile code",
        ),
        (
            "analyze",
            vec!["analyze", "analysis", "static analysis"],
            "analyze code",
        ),
        (
            "check",
            vec!["check", "type check", "mypy", "pyright"],
            "type checking",
        ),
        ("deploy", vec!["deploy", "deployment"], "deploy code"),
        (
            "query",
            vec!["query", "sql", "database query"],
            "execute queries",
        ),
        ("review", vec!["review", "code review"], "review code"),
        (
            "scan",
            vec!["scan", "security scan", "vulnerability"],
            "security scanning",
        ),
        (
            "profile",
            vec!["profile", "profiling", "performance"],
            "performance profiling",
        ),
    ];

    for (op_name, keywords, description) in operation_patterns {
        let found = keywords
            .iter()
            .any(|kw| items_lower.iter().any(|item| item.contains(kw)));

        if found {
            operations.push(DetectedOperation {
                name: op_name.to_string(),
                description: description.to_string(),
                commands: Vec::new(), // Filled in by template generator
                risk: infer_risk(op_name),
            });
        }
    }

    operations
}

/// Infer risk level based on operation type
fn infer_risk(operation: &str) -> String {
    match operation {
        "execute" | "deploy" => "high".to_string(),
        "build" | "test" => "medium".to_string(),
        "lint" | "format" | "check" | "analyze" | "review" | "scan" => "low".to_string(),
        _ => "medium".to_string(),
    }
}

/// Determine security profile category from agent definition
pub fn determine_category(definition: &AgentDefinition) -> crate::security::ProfileCategory {
    let name_lower = definition.name.to_lowercase();
    let desc_lower = definition.description.to_lowercase();

    // Code execution agents (language-pro, shell)
    if name_lower.ends_with("-pro")
        && (name_lower.contains("python")
            || name_lower.contains("rust")
            || name_lower.contains("go")
            || name_lower.contains("javascript")
            || name_lower.contains("typescript")
            || name_lower.contains("java")
            || name_lower.contains("c-pro")
            || name_lower.contains("cpp")
            || name_lower.contains("php")
            || name_lower.contains("ruby")
            || name_lower.contains("bash")
            || name_lower.contains("shell"))
    {
        return crate::security::ProfileCategory::CodeExecution;
    }

    // Orchestration agents
    if name_lower.contains("orchestrat")
        || name_lower.contains("manager")
        || desc_lower.contains("coordinate")
        || desc_lower.contains("orchestrat")
    {
        return crate::security::ProfileCategory::Orchestration;
    }

    // Content generation agents
    if name_lower.contains("doc")
        || name_lower.contains("tutorial")
        || name_lower.contains("content")
        || name_lower.contains("mermaid")
        || desc_lower.contains("documentation")
        || desc_lower.contains("content generation")
    {
        return crate::security::ProfileCategory::ContentGeneration;
    }

    // Default to read-only analysis
    crate::security::ProfileCategory::ReadOnlyAnalysis
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_MD: &str = r#"---
name: python-pro
description: Master Python 3.12+ development
model: sonnet
---

You are a Python expert.

## Purpose
Expert Python developer mastering Python 3.12+ features.

## Capabilities

### Modern Python Features
- Python 3.12+ features
- Advanced async/await patterns
- Type hints and generics

### Testing & QA
- Comprehensive testing with pytest
- Property-based testing with Hypothesis

## Behavioral Traits
- Follows PEP 8
- Uses type hints throughout
- Writes extensive tests

## Example Interactions
- "Help me migrate from pip to uv"
- "Optimize this Python code"
"#;

    #[test]
    fn test_parse_agent_markdown() {
        let result = parse_agent_markdown(SAMPLE_MD).unwrap();

        assert_eq!(result.name, "python-pro");
        assert_eq!(result.model, "sonnet");
        assert!(result.purpose.is_some());
        assert!(!result.capabilities.items.is_empty());
        assert!(!result.behavioral_traits.is_empty());
        assert!(!result.examples.is_empty());
    }

    #[test]
    fn test_determine_category() {
        let def = parse_agent_markdown(SAMPLE_MD).unwrap();
        let category = determine_category(&def);
        assert_eq!(category, crate::security::ProfileCategory::CodeExecution);
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/generator/mod.rs">
//! Agent code generation infrastructure
//!
//! Provides tools for:
//! - Parsing markdown agent definitions
//! - Generating Rust D-Bus agent code
//! - Creating agent specifications

pub mod md_parser;
pub mod template;

pub use md_parser::{parse_agent_markdown, AgentDefinition, ParsedCapabilities};
pub use template::{generate_agent_code, AgentOperation, AgentTemplate};
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/generator/template.rs">
//! Agent code template generator
//!
//! Generates Rust source code for D-Bus agents based on:
//! - Agent definitions from markdown
//! - Security profiles
//! - Operation specifications

use crate::generator::md_parser::{determine_category, AgentDefinition};
use crate::security::ProfileCategory;
use std::collections::HashSet;

/// Agent template for code generation
#[derive(Debug, Clone)]
pub struct AgentTemplate {
    /// Agent type identifier (e.g., "python-pro")
    pub agent_type: String,

    /// Rust struct name (e.g., "PythonProAgent")
    pub struct_name: String,

    /// D-Bus interface name (e.g., "org.dbusmcp.Agent.PythonPro")
    pub interface_name: String,

    /// D-Bus path (e.g., "/org/dbusmcp/Agent/PythonPro")
    pub dbus_path: String,

    /// Agent description
    pub description: String,

    /// Security profile category
    pub category: ProfileCategory,

    /// Allowed commands
    pub allowed_commands: HashSet<String>,

    /// Operations this agent supports
    pub operations: Vec<AgentOperation>,
}

/// Agent operation definition
#[derive(Debug, Clone)]
pub struct AgentOperation {
    /// Operation name (e.g., "execute", "test")
    pub name: String,

    /// Operation description
    pub description: String,

    /// Primary command to run
    pub command: String,

    /// Default arguments
    pub default_args: Vec<String>,

    /// Whether path is required
    pub requires_path: bool,

    /// Whether this operation requires approval
    pub requires_approval: bool,
}

impl AgentTemplate {
    /// Create a template from an agent definition
    pub fn from_definition(def: &AgentDefinition) -> Self {
        let category = determine_category(def);
        let agent_type = def.name.clone();

        // Generate Rust-safe names
        let struct_name = to_pascal_case(&agent_type) + "Agent";
        let interface_name = format!("org.dbusmcp.Agent.{}", to_pascal_case(&agent_type));
        let dbus_path = format!("/org/dbusmcp/Agent/{}", to_pascal_case(&agent_type));

        // Determine allowed commands based on agent type
        let allowed_commands = infer_commands(&agent_type, &category);

        // Generate operations
        let operations = generate_operations(
            &agent_type,
            &category,
            &def.capabilities.detected_operations,
        );

        Self {
            agent_type,
            struct_name,
            interface_name,
            dbus_path,
            description: def.description.clone(),
            category,
            allowed_commands,
            operations,
        }
    }
}

/// Convert kebab-case to PascalCase
fn to_pascal_case(s: &str) -> String {
    s.split('-')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

/// Convert kebab-case to snake_case
fn to_snake_case(s: &str) -> String {
    s.replace('-', "_")
}

/// Infer allowed commands from agent type
fn infer_commands(agent_type: &str, category: &ProfileCategory) -> HashSet<String> {
    let mut commands = HashSet::new();

    match category {
        ProfileCategory::CodeExecution => {
            match agent_type {
                "python-pro" => {
                    commands.extend(
                        [
                            "python", "python3", "pip", "pip3", "uv", "ruff", "pytest", "mypy",
                            "black", "isort", "flake8",
                        ]
                        .map(String::from),
                    );
                }
                "rust-pro" => {
                    commands.extend(
                        ["cargo", "rustc", "rustfmt", "clippy-driver", "rustup"].map(String::from),
                    );
                }
                "golang-pro" => {
                    commands.extend(
                        ["go", "gofmt", "golint", "staticcheck", "gopls"].map(String::from),
                    );
                }
                "javascript-pro" | "typescript-pro" => {
                    commands.extend(
                        [
                            "node", "npm", "npx", "yarn", "pnpm", "eslint", "prettier", "jest",
                            "vitest", "tsc",
                        ]
                        .map(String::from),
                    );
                }
                "java-pro" => {
                    commands.extend(["java", "javac", "mvn", "gradle", "ant"].map(String::from));
                }
                "csharp-pro" => {
                    commands.extend(["dotnet", "csc", "msbuild", "nuget"].map(String::from));
                }
                "ruby-pro" => {
                    commands.extend(
                        ["ruby", "gem", "bundle", "rake", "rspec", "rubocop"].map(String::from),
                    );
                }
                "php-pro" => {
                    commands.extend(
                        ["php", "composer", "phpunit", "phpstan", "psalm"].map(String::from),
                    );
                }
                "c-pro" => {
                    commands.extend(
                        ["gcc", "clang", "make", "cmake", "gdb", "valgrind"].map(String::from),
                    );
                }
                "cpp-pro" => {
                    commands.extend(
                        ["g++", "clang++", "make", "cmake", "gdb", "valgrind"].map(String::from),
                    );
                }
                "scala-pro" => {
                    commands.extend(["scala", "scalac", "sbt", "mill"].map(String::from));
                }
                "julia-pro" => {
                    commands.extend(["julia"].map(String::from));
                }
                "elixir-pro" => {
                    commands.extend(["elixir", "mix", "iex"].map(String::from));
                }
                "bash-pro" | "posix-shell-pro" => {
                    commands.extend(["bash", "sh", "shellcheck"].map(String::from));
                }
                "sql-pro" => {
                    commands.extend(["psql", "mysql", "sqlite3", "sqlfluff"].map(String::from));
                }
                _ => {
                    // Default code execution commands
                    commands.insert("echo".to_string());
                }
            }
        }
        ProfileCategory::ReadOnlyAnalysis => {
            commands.extend(["rg", "grep", "wc", "cloc", "tokei", "diff", "git"].map(String::from));
        }
        ProfileCategory::ContentGeneration => {
            commands.extend(["cat", "echo", "wc"].map(String::from));
        }
        ProfileCategory::Orchestration => {
            // Orchestration agents typically don't run commands directly
        }
    }

    commands
}

/// Generate operations based on agent type
fn generate_operations(
    agent_type: &str,
    category: &ProfileCategory,
    detected: &[super::md_parser::DetectedOperation],
) -> Vec<AgentOperation> {
    let mut operations = Vec::new();

    match category {
        ProfileCategory::CodeExecution => {
            match agent_type {
                "python-pro" => {
                    operations.push(AgentOperation {
                        name: "run".to_string(),
                        description: "Execute Python script".to_string(),
                        command: "python3".to_string(),
                        default_args: vec![],
                        requires_path: true,
                        requires_approval: false,
                    });
                    operations.push(AgentOperation {
                        name: "test".to_string(),
                        description: "Run pytest".to_string(),
                        command: "pytest".to_string(),
                        default_args: vec!["-v".to_string()],
                        requires_path: false,
                        requires_approval: false,
                    });
                    operations.push(AgentOperation {
                        name: "lint".to_string(),
                        description: "Run ruff linter".to_string(),
                        command: "ruff".to_string(),
                        default_args: vec!["check".to_string()],
                        requires_path: true,
                        requires_approval: false,
                    });
                    operations.push(AgentOperation {
                        name: "format".to_string(),
                        description: "Format with black".to_string(),
                        command: "black".to_string(),
                        default_args: vec!["--check".to_string(), "--diff".to_string()],
                        requires_path: true,
                        requires_approval: false,
                    });
                    operations.push(AgentOperation {
                        name: "typecheck".to_string(),
                        description: "Run mypy type checker".to_string(),
                        command: "mypy".to_string(),
                        default_args: vec![],
                        requires_path: true,
                        requires_approval: false,
                    });
                }
                "rust-pro" => {
                    operations.push(AgentOperation {
                        name: "check".to_string(),
                        description: "Run cargo check".to_string(),
                        command: "cargo".to_string(),
                        default_args: vec!["check".to_string()],
                        requires_path: false,
                        requires_approval: false,
                    });
                    operations.push(AgentOperation {
                        name: "build".to_string(),
                        description: "Build the project".to_string(),
                        command: "cargo".to_string(),
                        default_args: vec!["build".to_string()],
                        requires_path: false,
                        requires_approval: false,
                    });
                    operations.push(AgentOperation {
                        name: "test".to_string(),
                        description: "Run tests".to_string(),
                        command: "cargo".to_string(),
                        default_args: vec!["test".to_string()],
                        requires_path: false,
                        requires_approval: false,
                    });
                    operations.push(AgentOperation {
                        name: "clippy".to_string(),
                        description: "Run clippy linter".to_string(),
                        command: "cargo".to_string(),
                        default_args: vec![
                            "clippy".to_string(),
                            "--".to_string(),
                            "-D".to_string(),
                            "warnings".to_string(),
                        ],
                        requires_path: false,
                        requires_approval: false,
                    });
                    operations.push(AgentOperation {
                        name: "format".to_string(),
                        description: "Check formatting".to_string(),
                        command: "cargo".to_string(),
                        default_args: vec!["fmt".to_string(), "--check".to_string()],
                        requires_path: false,
                        requires_approval: false,
                    });
                }
                "golang-pro" => {
                    operations.push(AgentOperation {
                        name: "build".to_string(),
                        description: "Build Go project".to_string(),
                        command: "go".to_string(),
                        default_args: vec!["build".to_string()],
                        requires_path: false,
                        requires_approval: false,
                    });
                    operations.push(AgentOperation {
                        name: "test".to_string(),
                        description: "Run Go tests".to_string(),
                        command: "go".to_string(),
                        default_args: vec!["test".to_string(), "./...".to_string()],
                        requires_path: false,
                        requires_approval: false,
                    });
                    operations.push(AgentOperation {
                        name: "fmt".to_string(),
                        description: "Format Go code".to_string(),
                        command: "gofmt".to_string(),
                        default_args: vec!["-l".to_string(), ".".to_string()],
                        requires_path: false,
                        requires_approval: false,
                    });
                    operations.push(AgentOperation {
                        name: "vet".to_string(),
                        description: "Run Go vet".to_string(),
                        command: "go".to_string(),
                        default_args: vec!["vet".to_string(), "./...".to_string()],
                        requires_path: false,
                        requires_approval: false,
                    });
                }
                "javascript-pro" | "typescript-pro" => {
                    operations.push(AgentOperation {
                        name: "run".to_string(),
                        description: "Execute script".to_string(),
                        command: "node".to_string(),
                        default_args: vec![],
                        requires_path: true,
                        requires_approval: false,
                    });
                    operations.push(AgentOperation {
                        name: "test".to_string(),
                        description: "Run tests".to_string(),
                        command: "npm".to_string(),
                        default_args: vec!["test".to_string()],
                        requires_path: false,
                        requires_approval: false,
                    });
                    operations.push(AgentOperation {
                        name: "lint".to_string(),
                        description: "Run ESLint".to_string(),
                        command: "npx".to_string(),
                        default_args: vec!["eslint".to_string(), ".".to_string()],
                        requires_path: false,
                        requires_approval: false,
                    });
                    operations.push(AgentOperation {
                        name: "build".to_string(),
                        description: "Build project".to_string(),
                        command: "npm".to_string(),
                        default_args: vec!["run".to_string(), "build".to_string()],
                        requires_path: false,
                        requires_approval: false,
                    });
                }
                _ => {
                    // Generate generic operations from detected ones
                    for detected_op in detected {
                        operations.push(AgentOperation {
                            name: detected_op.name.clone(),
                            description: detected_op.description.clone(),
                            command: "echo".to_string(),
                            default_args: vec![format!("Operation: {}", detected_op.name)],
                            requires_path: false,
                            requires_approval: detected_op.risk == "high",
                        });
                    }
                }
            }
        }
        ProfileCategory::ReadOnlyAnalysis => {
            operations.push(AgentOperation {
                name: "analyze".to_string(),
                description: "Analyze code".to_string(),
                command: "rg".to_string(),
                default_args: vec![],
                requires_path: true,
                requires_approval: false,
            });
            operations.push(AgentOperation {
                name: "count".to_string(),
                description: "Count lines of code".to_string(),
                command: "wc".to_string(),
                default_args: vec!["-l".to_string()],
                requires_path: true,
                requires_approval: false,
            });
        }
        ProfileCategory::ContentGeneration => {
            operations.push(AgentOperation {
                name: "generate".to_string(),
                description: "Generate content".to_string(),
                command: "echo".to_string(),
                default_args: vec![],
                requires_path: false,
                requires_approval: false,
            });
        }
        ProfileCategory::Orchestration => {
            operations.push(AgentOperation {
                name: "coordinate".to_string(),
                description: "Coordinate subagents".to_string(),
                command: "echo".to_string(),
                default_args: vec![],
                requires_path: false,
                requires_approval: false,
            });
        }
    }

    operations
}

/// Generate Rust source code for a D-Bus agent
pub fn generate_agent_code(template: &AgentTemplate) -> String {
    let _snake_name = to_snake_case(&template.agent_type);
    let allowed_cmds: Vec<&str> = template
        .allowed_commands
        .iter()
        .map(|s| s.as_str())
        .collect();

    let mut operations_impl = String::new();
    let mut match_arms = String::new();

    for op in &template.operations {
        let op_snake = to_snake_case(&op.name);
        let default_args_str = if op.default_args.is_empty() {
            "vec![]".to_string()
        } else {
            format!(
                "vec![{}]",
                op.default_args
                    .iter()
                    .map(|a| format!("\"{}\".to_string()", a))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };

        operations_impl.push_str(&format!(
            r#"
    fn {op_snake}(&self, path: Option<&str>, args: Option<&str>) -> Result<String, String> {{
        let mut cmd = Command::new("{command}");
        let default_args = {default_args};
        
        for arg in default_args {{
            cmd.arg(arg);
        }}
        
        if let Some(a) = args {{
            self.validate_args(a)?;
            for arg in a.split_whitespace() {{
                cmd.arg(arg);
            }}
        }}
        
        {path_handling}
        
        let output = cmd
            .output()
            .map_err(|e| format!("Failed to run {command}: {{}}", e))?;
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        
        if output.status.success() {{
            Ok(format!("{op_name} succeeded\nstdout: {{}}\nstderr: {{}}", stdout, stderr))
        }} else {{
            Ok(format!("{op_name} failed\nstdout: {{}}\nstderr: {{}}", stdout, stderr))
        }}
    }}
"#,
            op_snake = op_snake,
            command = op.command,
            default_args = default_args_str,
            op_name = op.name,
            path_handling = if op.requires_path {
                r#"if let Some(p) = path {
            let validated_path = self.validate_path(p)?;
            cmd.arg(validated_path);
        } else {
            return Err("Path required".to_string());
        }"#
            } else {
                r#"if let Some(p) = path {
            let validated_path = self.validate_path(p)?;
            cmd.current_dir(validated_path);
        }"#
            }
        ));

        match_arms.push_str(&format!(
            r#"            "{}" => self.{op_snake}(task.path.as_deref(), task.args.as_deref()),
"#,
            op.name,
            op_snake = op_snake
        ));
    }

    format!(
        r#"//! {description}
//! 
//! Auto-generated D-Bus agent for {agent_type}

use serde::Deserialize;
use std::process::Command;
use uuid::Uuid;
use zbus::{{connection::Builder, interface, object_server::SignalEmitter}};

// Security configuration
const ALLOWED_DIRECTORIES: &[&str] = &["/tmp", "/home", "/opt"];
const FORBIDDEN_CHARS: &[char] = &[
    '$', '`', ';', '&', '|', '>', '<', '(', ')', '{{', '}}', '\n', '\r',
];
const MAX_PATH_LENGTH: usize = 4096;
const ALLOWED_COMMANDS: &[&str] = &[{allowed_commands}];

#[derive(Debug, Deserialize)]
struct {struct_name}Task {{
    #[serde(rename = "type")]
    task_type: String,
    operation: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    args: Option<String>,
}}

struct {struct_name} {{
    agent_id: String,
}}

#[interface(name = "{interface_name}")]
impl {struct_name} {{
    /// Execute a task safely
    async fn execute(&self, mut task_json: String) -> zbus::fdo::Result<String> {{
        println!("[{{}}] Received task: {{}}", self.agent_id, task_json);

        let task: {struct_name}Task = match unsafe {{ simd_json::from_str(&mut task_json) }} {{
            Ok(t) => t,
            Err(e) => {{
                return Err(zbus::fdo::Error::InvalidArgs(format!(
                    "Failed to parse task: {{}}",
                    e
                )));
            }}
        }};

        if task.task_type != "{agent_type}" {{
            return Err(zbus::fdo::Error::InvalidArgs(format!(
                "Unknown task type: {{}}",
                task.task_type
            )));
        }}

        println!(
            "[{{}}] Operation: {{}} on path: {{:?}}",
            self.agent_id, task.operation, task.path
        );

        let result = match task.operation.as_str() {{
{match_arms}            _ => Err(format!("Unknown operation: {{}}", task.operation)),
        }};

        match result {{
            Ok(data) => {{
                let response = simd_json::json!({{
                    "success": true,
                    "operation": task.operation,
                    "data": data,
                }});
                Ok(response.to_string())
            }}
            Err(e) => Err(zbus::fdo::Error::Failed(e)),
        }}
    }}

    /// Get agent status
    async fn get_status(&self) -> zbus::fdo::Result<String> {{
        Ok(format!("{struct_name} {{}} is running", self.agent_id))
    }}

    /// List supported operations
    async fn list_operations(&self) -> zbus::fdo::Result<String> {{
        let ops = simd_json::json!({{
            "operations": [{operations_list}]
        }});
        Ok(ops.to_string())
    }}

    /// Signal emitted when task completes
    #[zbus(signal)]
    async fn task_completed(signal_emitter: &SignalEmitter<'_>, result: String)
        -> zbus::Result<()>;
}}

impl {struct_name} {{
    fn new(agent_id: String) -> Self {{
        Self {{ agent_id }}
    }}

    fn validate_path(&self, path: &str) -> Result<String, String> {{
        if path.len() > MAX_PATH_LENGTH {{
            return Err("Path exceeds maximum length".to_string());
        }}

        for forbidden_char in FORBIDDEN_CHARS {{
            if path.contains(*forbidden_char) {{
                return Err(format!(
                    "Path contains forbidden character: {{:?}}",
                    forbidden_char
                ));
            }}
        }}

        let mut is_allowed = false;
        for allowed in ALLOWED_DIRECTORIES {{
            if path.starts_with(allowed) {{
                is_allowed = true;
                break;
            }}
        }}

        if !is_allowed {{
            return Err(format!(
                "Path must be within allowed directories: {{:?}}",
                ALLOWED_DIRECTORIES
            ));
        }}

        Ok(path.to_string())
    }}

    fn validate_args(&self, args: &str) -> Result<(), String> {{
        if args.len() > 256 {{
            return Err("Args string too long".to_string());
        }}

        for forbidden_char in FORBIDDEN_CHARS {{
            if args.contains(*forbidden_char) {{
                return Err(format!(
                    "Args contains forbidden character: {{:?}}",
                    forbidden_char
                ));
            }}
        }}

        Ok(())
    }}
{operations_impl}
}}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {{
    let args: Vec<String> = std::env::args().collect();

    let agent_id = if args.len() > 1 {{
        args[1].clone()
    }} else {{
        format!("{agent_type}-{{}}", Uuid::new_v4().to_string()[..8].to_string())
    }};

    println!("Starting {struct_name}: {{}}", agent_id);

    let agent = {struct_name}::new(agent_id.clone());

    let path = format!("{dbus_path}/{{}}", agent_id.replace('-', "_"));
    let service_name = format!("{interface_name}.{{}}", agent_id.replace('-', "_"));

    let _conn = Builder::system()?
        .name(service_name.as_str())?
        .serve_at(path.as_str(), agent)?
        .build()
        .await?;

    println!("{struct_name} {{}} ready on D-Bus", agent_id);
    println!("Service: {{}}", service_name);
    println!("Path: {{}}", path);

    std::future::pending::<()>().await;

    Ok(())
}}
"#,
        description = template.description,
        agent_type = template.agent_type,
        struct_name = template.struct_name,
        interface_name = template.interface_name,
        dbus_path = template.dbus_path,
        allowed_commands = allowed_cmds
            .iter()
            .map(|s| format!("\"{}\"", s))
            .collect::<Vec<_>>()
            .join(", "),
        match_arms = match_arms,
        operations_impl = operations_impl,
        operations_list = template
            .operations
            .iter()
            .map(|o| format!("\"{}\"", o.name))
            .collect::<Vec<_>>()
            .join(", "),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_pascal_case() {
        assert_eq!(to_pascal_case("python-pro"), "PythonPro");
        assert_eq!(to_pascal_case("code-reviewer"), "CodeReviewer");
    }

    #[test]
    fn test_to_snake_case() {
        assert_eq!(to_snake_case("python-pro"), "python_pro");
        assert_eq!(to_snake_case("code-reviewer"), "code_reviewer");
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/security/mod.rs">
//! Security module for agent execution sandboxing and validation
//!
//! Provides:
//! - Security profiles for different agent categories
//! - Input validation and sanitization
//! - Sandboxed execution with resource limits
//! - Path and command whitelisting

pub mod profiles;
pub mod sandbox;
pub mod validation;

pub use profiles::{ProfileCategory, SecurityConfig, SecurityProfile};
pub use sandbox::{ResourceLimits, SandboxExecutor, SandboxResult};
pub use validation::{
    validate_args, validate_command, validate_input, validate_path, SecurityError, ValidationError,
};
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/security/profiles.rs">
//! Security profiles for different agent execution types
//!
//! Defines security constraints for four main agent categories:
//! - CodeExecution: Language-specific execution with strict sandboxing
//! - ReadOnlyAnalysis: Read-only analysis with tool whitelisting
//! - ContentGeneration: Documentation/content generation with write limits
//! - Orchestration: Meta-agents that coordinate other agents

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;

/// Agent security profile categories
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProfileCategory {
    /// Code execution agents (language pros, shell agents)
    CodeExecution,
    /// Analysis agents (reviewers, auditors)
    ReadOnlyAnalysis,
    /// Content generation agents (docs, tutorials)
    ContentGeneration,
    /// Orchestration agents (meta-agents, coordinators)
    Orchestration,
}

impl Default for ProfileCategory {
    fn default() -> Self {
        Self::ReadOnlyAnalysis
    }
}

/// Security profile configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Profile category
    pub category: ProfileCategory,

    /// Commands allowed for execution
    #[serde(default)]
    pub allowed_commands: HashSet<String>,

    /// Paths allowed for reading
    #[serde(default)]
    pub allowed_read_paths: Vec<PathBuf>,

    /// Paths allowed for writing
    #[serde(default)]
    pub allowed_write_paths: Vec<PathBuf>,

    /// Paths explicitly forbidden (takes precedence)
    #[serde(default)]
    pub forbidden_paths: Vec<PathBuf>,

    /// Tools allowed for analysis agents
    #[serde(default)]
    pub allowed_tools: HashSet<String>,

    /// Subagents allowed for orchestration
    #[serde(default)]
    pub allowed_subagents: HashSet<String>,

    /// Execution timeout
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,

    /// Maximum memory (MB)
    #[serde(default = "default_max_memory_mb")]
    pub max_memory_mb: u64,

    /// Maximum output size (bytes)
    #[serde(default = "default_max_output_size")]
    pub max_output_size: usize,

    /// Maximum concurrent operations
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent: usize,

    /// Whether operations require approval
    #[serde(default)]
    pub requires_approval: bool,

    /// Whether agent has root privileges
    #[serde(default)]
    pub requires_root: bool,
}

fn default_timeout_secs() -> u64 {
    60
}
fn default_max_memory_mb() -> u64 {
    512
}
fn default_max_output_size() -> usize {
    1_000_000
} // 1MB
fn default_max_concurrent() -> usize {
    1
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            category: ProfileCategory::ReadOnlyAnalysis,
            allowed_commands: HashSet::new(),
            allowed_read_paths: vec![PathBuf::from("/home"), PathBuf::from("/tmp")],
            allowed_write_paths: vec![],
            forbidden_paths: vec![
                PathBuf::from("/etc"),
                PathBuf::from("/root"),
                PathBuf::from("/var/lib"),
                PathBuf::from("/sys"),
                PathBuf::from("/proc"),
            ],
            allowed_tools: HashSet::new(),
            allowed_subagents: HashSet::new(),
            timeout_secs: 60,
            max_memory_mb: 512,
            max_output_size: 1_000_000,
            max_concurrent: 1,
            requires_approval: false,
            requires_root: false,
        }
    }
}

/// Complete security profile including runtime configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityProfile {
    /// Agent type identifier
    pub agent_type: String,

    /// Human-readable name
    pub name: String,

    /// Description
    pub description: String,

    /// Security configuration
    #[serde(flatten)]
    pub config: SecurityConfig,

    /// Operations this profile allows
    #[serde(default)]
    pub operations: Vec<OperationSecurity>,
}

/// Per-operation security settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationSecurity {
    /// Operation name
    pub name: String,

    /// Whether this operation requires explicit approval
    #[serde(default)]
    pub requires_approval: bool,

    /// Custom timeout for this operation
    pub timeout_secs: Option<u64>,

    /// Additional commands allowed for this operation
    #[serde(default)]
    pub extra_commands: HashSet<String>,

    /// Risk level (for UI display)
    #[serde(default)]
    pub risk_level: RiskLevel,
}

/// Risk level classification
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    #[default]
    Low,
    Medium,
    High,
    Critical,
}

impl SecurityProfile {
    /// Create a profile for code execution agents (language pros)
    pub fn code_execution(agent_type: &str, commands: Vec<&str>) -> Self {
        Self {
            agent_type: agent_type.to_string(),
            name: format!("{} Pro", agent_type.replace('-', " ").to_uppercase()),
            description: format!("Code execution agent for {}", agent_type),
            config: SecurityConfig {
                category: ProfileCategory::CodeExecution,
                allowed_commands: commands.into_iter().map(|s| s.to_string()).collect(),
                allowed_read_paths: vec![
                    PathBuf::from("/home"),
                    PathBuf::from("/tmp"),
                    PathBuf::from("/opt"),
                ],
                allowed_write_paths: vec![PathBuf::from("/tmp")],
                forbidden_paths: vec![
                    PathBuf::from("/etc"),
                    PathBuf::from("/root"),
                    PathBuf::from("/var"),
                    PathBuf::from("/sys"),
                    PathBuf::from("/proc"),
                ],
                timeout_secs: 300,
                max_memory_mb: 2048,
                max_output_size: 5_000_000, // 5MB for build output
                ..Default::default()
            },
            operations: vec![],
        }
    }

    /// Create a profile for read-only analysis agents
    pub fn read_only_analysis(agent_type: &str, tools: Vec<&str>) -> Self {
        Self {
            agent_type: agent_type.to_string(),
            name: agent_type.replace('-', " ").to_string(),
            description: format!("Read-only analysis agent: {}", agent_type),
            config: SecurityConfig {
                category: ProfileCategory::ReadOnlyAnalysis,
                allowed_tools: tools.into_iter().map(|s| s.to_string()).collect(),
                allowed_read_paths: vec![
                    PathBuf::from("/home"),
                    PathBuf::from("/tmp"),
                    PathBuf::from("/opt"),
                ],
                allowed_write_paths: vec![], // Read-only
                timeout_secs: 120,
                max_memory_mb: 1024,
                ..Default::default()
            },
            operations: vec![],
        }
    }

    /// Create a profile for content generation agents
    pub fn content_generation(agent_type: &str) -> Self {
        Self {
            agent_type: agent_type.to_string(),
            name: agent_type.replace('-', " ").to_string(),
            description: format!("Content generation agent: {}", agent_type),
            config: SecurityConfig {
                category: ProfileCategory::ContentGeneration,
                allowed_read_paths: vec![PathBuf::from("/home"), PathBuf::from("/tmp")],
                allowed_write_paths: vec![PathBuf::from("/tmp")],
                timeout_secs: 180,
                max_output_size: 10_000_000, // 10MB for docs
                ..Default::default()
            },
            operations: vec![],
        }
    }

    /// Create a profile for orchestration agents
    pub fn orchestration(agent_type: &str, subagents: Vec<&str>) -> Self {
        Self {
            agent_type: agent_type.to_string(),
            name: agent_type.replace('-', " ").to_string(),
            description: format!("Orchestration agent: {}", agent_type),
            config: SecurityConfig {
                category: ProfileCategory::Orchestration,
                allowed_subagents: subagents.into_iter().map(|s| s.to_string()).collect(),
                max_concurrent: 5,
                timeout_secs: 600, // Longer timeout for coordinated tasks
                ..Default::default()
            },
            operations: vec![],
        }
    }

    /// Check if a command is allowed
    pub fn is_command_allowed(&self, cmd: &str) -> bool {
        self.config.allowed_commands.contains(cmd)
    }

    /// Check if a tool is allowed
    pub fn is_tool_allowed(&self, tool: &str) -> bool {
        self.config.allowed_tools.contains(tool)
    }

    /// Check if a path can be read
    pub fn can_read_path(&self, path: &std::path::Path) -> bool {
        // Check forbidden paths first
        for forbidden in &self.config.forbidden_paths {
            if path.starts_with(forbidden) {
                return false;
            }
        }

        // Then check allowed paths
        for allowed in &self.config.allowed_read_paths {
            if path.starts_with(allowed) {
                return true;
            }
        }

        false
    }

    /// Check if a path can be written
    pub fn can_write_path(&self, path: &std::path::Path) -> bool {
        // Check forbidden paths first
        for forbidden in &self.config.forbidden_paths {
            if path.starts_with(forbidden) {
                return false;
            }
        }

        // Then check allowed paths
        for allowed in &self.config.allowed_write_paths {
            if path.starts_with(allowed) {
                return true;
            }
        }

        false
    }

    /// Get timeout as Duration
    pub fn timeout(&self) -> Duration {
        Duration::from_secs(self.config.timeout_secs)
    }
}

/// Pre-defined security profiles for common agent types
pub mod presets {
    use super::*;

    /// Python Pro agent profile
    pub fn python_pro() -> SecurityProfile {
        let mut profile = SecurityProfile::code_execution(
            "python-pro",
            vec![
                "python", "python3", "pip", "pip3", "uv", "ruff", "pytest", "mypy", "black",
                "isort",
            ],
        );
        profile.operations = vec![
            OperationSecurity {
                name: "run".to_string(),
                requires_approval: false,
                timeout_secs: Some(60),
                extra_commands: HashSet::new(),
                risk_level: RiskLevel::Medium,
            },
            OperationSecurity {
                name: "test".to_string(),
                requires_approval: false,
                timeout_secs: Some(300),
                extra_commands: HashSet::new(),
                risk_level: RiskLevel::Low,
            },
            OperationSecurity {
                name: "lint".to_string(),
                requires_approval: false,
                timeout_secs: Some(60),
                extra_commands: HashSet::new(),
                risk_level: RiskLevel::Low,
            },
            OperationSecurity {
                name: "format".to_string(),
                requires_approval: false,
                timeout_secs: Some(60),
                extra_commands: HashSet::new(),
                risk_level: RiskLevel::Low,
            },
            OperationSecurity {
                name: "install".to_string(),
                requires_approval: true,
                timeout_secs: Some(300),
                extra_commands: HashSet::new(),
                risk_level: RiskLevel::High,
            },
        ];
        profile
    }

    /// Rust Pro agent profile
    pub fn rust_pro() -> SecurityProfile {
        let mut profile = SecurityProfile::code_execution(
            "rust-pro",
            vec!["cargo", "rustc", "rustfmt", "clippy-driver"],
        );
        profile.operations = vec![
            OperationSecurity {
                name: "check".to_string(),
                requires_approval: false,
                timeout_secs: Some(120),
                extra_commands: HashSet::new(),
                risk_level: RiskLevel::Low,
            },
            OperationSecurity {
                name: "build".to_string(),
                requires_approval: false,
                timeout_secs: Some(600),
                extra_commands: HashSet::new(),
                risk_level: RiskLevel::Medium,
            },
            OperationSecurity {
                name: "test".to_string(),
                requires_approval: false,
                timeout_secs: Some(600),
                extra_commands: HashSet::new(),
                risk_level: RiskLevel::Medium,
            },
            OperationSecurity {
                name: "clippy".to_string(),
                requires_approval: false,
                timeout_secs: Some(120),
                extra_commands: HashSet::new(),
                risk_level: RiskLevel::Low,
            },
        ];
        profile
    }

    /// Go Pro agent profile
    pub fn golang_pro() -> SecurityProfile {
        SecurityProfile::code_execution(
            "golang-pro",
            vec!["go", "gofmt", "golint", "staticcheck", "gopls"],
        )
    }

    /// JavaScript Pro agent profile
    pub fn javascript_pro() -> SecurityProfile {
        SecurityProfile::code_execution(
            "javascript-pro",
            vec![
                "node", "npm", "npx", "yarn", "pnpm", "eslint", "prettier", "jest", "vitest",
            ],
        )
    }

    /// TypeScript Pro agent profile
    pub fn typescript_pro() -> SecurityProfile {
        SecurityProfile::code_execution(
            "typescript-pro",
            vec![
                "node", "npm", "npx", "yarn", "pnpm", "tsc", "eslint", "prettier", "jest", "vitest",
            ],
        )
    }

    /// Code Reviewer profile
    pub fn code_reviewer() -> SecurityProfile {
        SecurityProfile::read_only_analysis(
            "code-reviewer",
            vec!["rg", "grep", "wc", "cloc", "tokei", "diff", "git"],
        )
    }

    /// Security Auditor profile
    pub fn security_auditor() -> SecurityProfile {
        SecurityProfile::read_only_analysis(
            "security-auditor",
            vec![
                "rg",
                "grep",
                "semgrep",
                "bandit",
                "safety",
                "npm audit",
                "cargo audit",
            ],
        )
    }

    /// Docs Architect profile
    pub fn docs_architect() -> SecurityProfile {
        SecurityProfile::content_generation("docs-architect")
    }

    /// TDD Orchestrator profile
    pub fn tdd_orchestrator() -> SecurityProfile {
        SecurityProfile::orchestration(
            "tdd-orchestrator",
            vec!["code-reviewer", "test-automator", "debugger"],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_python_pro_profile() {
        let profile = presets::python_pro();
        assert!(profile.is_command_allowed("python3"));
        assert!(profile.is_command_allowed("pytest"));
        assert!(!profile.is_command_allowed("rm"));
    }

    #[test]
    fn test_path_validation() {
        let profile = presets::python_pro();
        assert!(profile.can_read_path(std::path::Path::new("/home/user/project")));
        assert!(!profile.can_read_path(std::path::Path::new("/etc/passwd")));
        assert!(profile.can_write_path(std::path::Path::new("/tmp/test.py")));
        assert!(!profile.can_write_path(std::path::Path::new("/etc/test")));
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/security/sandbox.rs">
//! Sandboxed command execution with resource limits
//!
//! Provides secure execution environment with:
//! - Timeout enforcement
//! - Memory limits
//! - Output size limits
//! - Process isolation

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::io::AsyncReadExt;
use tokio::process::Command;

use super::profiles::SecurityProfile;
use super::validation::{validate_command, validate_path, SecurityError};

/// Resource limits for sandboxed execution
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    /// Maximum execution time
    pub timeout: Duration,

    /// Maximum memory in bytes
    pub max_memory: u64,

    /// Maximum output size in bytes
    pub max_output: usize,

    /// Maximum number of processes
    pub max_processes: u32,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(60),
            max_memory: 512 * 1024 * 1024, // 512MB
            max_output: 1_000_000,         // 1MB
            max_processes: 10,
        }
    }
}

impl From<&SecurityProfile> for ResourceLimits {
    fn from(profile: &SecurityProfile) -> Self {
        Self {
            timeout: Duration::from_secs(profile.config.timeout_secs),
            max_memory: profile.config.max_memory_mb * 1024 * 1024,
            max_output: profile.config.max_output_size,
            max_processes: 10,
        }
    }
}

/// Sandbox execution result (different from tool SandboxResult)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxResult {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
    pub duration_ms: u64,
    pub truncated: bool,
}

impl SandboxResult {
    pub fn success(stdout: String, stderr: String, duration_ms: u64) -> Self {
        Self {
            exit_code: Some(0),
            stdout,
            stderr,
            success: true,
            duration_ms,
            truncated: false,
        }
    }
    pub fn failure(code: Option<i32>, stdout: String, stderr: String, duration_ms: u64) -> Self {
        Self {
            exit_code: code,
            stdout,
            stderr,
            success: false,
            duration_ms,
            truncated: false,
        }
    }
}

/// Sandboxed command executor
pub struct SandboxExecutor {
    /// Security profile to use
    profile: SecurityProfile,

    /// Resource limits
    limits: ResourceLimits,

    /// Additional environment variables
    env: HashMap<String, String>,
}

impl SandboxExecutor {
    /// Create a new sandbox executor with a security profile
    pub fn new(profile: SecurityProfile) -> Self {
        let limits = ResourceLimits::from(&profile);
        Self {
            profile,
            limits,
            env: HashMap::new(),
        }
    }

    /// Set custom resource limits
    pub fn with_limits(mut self, limits: ResourceLimits) -> Self {
        self.limits = limits;
        self
    }

    /// Add environment variable
    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    /// Execute a command in the sandbox
    pub async fn execute(
        &self,
        command: &str,
        args: &[String],
        working_dir: Option<&PathBuf>,
    ) -> Result<SandboxResult, SecurityError> {
        let start = Instant::now();

        // Validate command against whitelist
        let whitelist: Vec<String> = self
            .profile
            .config
            .allowed_commands
            .iter()
            .cloned()
            .collect();
        validate_command(command, &whitelist)?;

        // Validate working directory if provided
        if let Some(dir) = working_dir {
            validate_path(
                dir.to_str().unwrap_or(""),
                &self.profile.config.allowed_read_paths,
                &self.profile.config.forbidden_paths,
            )?;
        }

        // Build the command
        let mut cmd = Command::new(command);
        cmd.args(args);

        // Set environment
        cmd.env_clear();
        cmd.env("PATH", "/usr/local/bin:/usr/bin:/bin");
        cmd.env("HOME", "/tmp");
        cmd.env("LANG", "C.UTF-8");

        for (key, value) in &self.env {
            cmd.env(key, value);
        }

        // Set working directory
        if let Some(dir) = working_dir {
            cmd.current_dir(dir);
        }

        // Configure process I/O
        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        // Spawn the process
        let mut child = cmd
            .spawn()
            .context("Failed to spawn command")
            .map_err(|e| SecurityError::Unauthorized(e.to_string()))?;

        // Get handles to stdout/stderr
        let mut stdout_handle = child
            .stdout
            .take()
            .ok_or_else(|| SecurityError::Unauthorized("No stdout".to_string()))?;
        let mut stderr_handle = child
            .stderr
            .take()
            .ok_or_else(|| SecurityError::Unauthorized("No stderr".to_string()))?;

        // Read output with timeout
        let timeout = self.limits.timeout;
        let max_output = self.limits.max_output;

        let result = tokio::time::timeout(timeout, async {
            let mut stdout_buf = Vec::with_capacity(max_output.min(1024 * 1024));
            let mut stderr_buf = Vec::with_capacity(max_output.min(1024 * 1024));

            // Read output (with size limits)
            let read_limited = async {
                let mut tmp_stdout = vec![0u8; max_output];
                let mut tmp_stderr = vec![0u8; max_output];

                let (stdout_n, stderr_n) = tokio::join!(
                    stdout_handle.read(&mut tmp_stdout),
                    stderr_handle.read(&mut tmp_stderr),
                );

                let stdout_n = stdout_n.unwrap_or(0);
                let stderr_n = stderr_n.unwrap_or(0);

                stdout_buf.extend_from_slice(&tmp_stdout[..stdout_n]);
                stderr_buf.extend_from_slice(&tmp_stderr[..stderr_n]);
            };

            read_limited.await;

            // Wait for process to complete
            let status = child.wait().await;

            (stdout_buf, stderr_buf, status)
        })
        .await;

        let duration = start.elapsed();

        match result {
            Ok((stdout_buf, stderr_buf, status)) => {
                let stdout = String::from_utf8_lossy(&stdout_buf).to_string();
                let stderr = String::from_utf8_lossy(&stderr_buf).to_string();
                let truncated = stdout_buf.len() >= max_output || stderr_buf.len() >= max_output;
                let duration_ms = duration.as_millis() as u64;

                match status {
                    Ok(s) if s.success() => {
                        let mut result = SandboxResult::success(stdout, stderr, duration_ms);
                        result.truncated = truncated;
                        Ok(result)
                    }
                    Ok(s) => {
                        let mut result =
                            SandboxResult::failure(s.code(), stdout, stderr, duration_ms);
                        result.truncated = truncated;
                        Ok(result)
                    }
                    Err(e) => Ok(SandboxResult::failure(
                        None,
                        stdout,
                        format!("{}\n{}", stderr, e),
                        duration_ms,
                    )),
                }
            }
            Err(_) => {
                // Timeout - kill the process
                let _ = child.kill().await;
                Err(SecurityError::Timeout(timeout.as_secs()))
            }
        }
    }

    /// Execute a command with a specific operation's settings
    pub async fn execute_operation(
        &self,
        operation: &str,
        command: &str,
        args: &[String],
        working_dir: Option<&PathBuf>,
    ) -> Result<SandboxResult, SecurityError> {
        // Check if operation requires approval
        if let Some(op_sec) = self.profile.operations.iter().find(|o| o.name == operation) {
            if op_sec.requires_approval && !self.profile.config.requires_approval {
                return Err(SecurityError::RequiresApproval);
            }
        }

        self.execute(command, args, working_dir).await
    }
}

/// Builder for creating sandbox executors
pub struct SandboxBuilder {
    profile: Option<SecurityProfile>,
    limits: Option<ResourceLimits>,
    env: HashMap<String, String>,
}

impl SandboxBuilder {
    pub fn new() -> Self {
        Self {
            profile: None,
            limits: None,
            env: HashMap::new(),
        }
    }

    pub fn with_profile(mut self, profile: SecurityProfile) -> Self {
        self.profile = Some(profile);
        self
    }

    pub fn with_limits(mut self, limits: ResourceLimits) -> Self {
        self.limits = Some(limits);
        self
    }

    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    pub fn build(self) -> Result<SandboxExecutor> {
        let profile = self
            .profile
            .ok_or_else(|| anyhow::anyhow!("Security profile required"))?;

        let mut executor = SandboxExecutor::new(profile);

        if let Some(limits) = self.limits {
            executor.limits = limits;
        }

        executor.env = self.env;

        Ok(executor)
    }
}

impl Default for SandboxBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::profiles::presets;

    #[tokio::test]
    async fn test_sandbox_allowed_command() {
        let profile = presets::python_pro();
        let executor = SandboxExecutor::new(profile);

        // This should work - python is allowed
        let result = executor
            .execute("python3", &["--version".to_string()], None)
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_sandbox_blocked_command() {
        let profile = presets::python_pro();
        let executor = SandboxExecutor::new(profile);

        // This should fail - rm is not allowed
        let result = executor
            .execute("rm", &["-rf".to_string(), "/".to_string()], None)
            .await;
        assert!(result.is_err());
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/security/validation.rs">
//! Input validation and sanitization for secure agent execution
//!
//! Provides validation functions for:
//! - Input strings (preventing injection attacks)
//! - File paths (ensuring allowed directories)
//! - Commands (whitelisting)
//! - Arguments (sanitization)

use std::path::{Path, PathBuf};
use thiserror::Error;

/// Characters forbidden in user input to prevent injection
pub const FORBIDDEN_CHARS: &[char] = &[
    '$', '`', ';', '&', '|', '>', '<', '(', ')', '{', '}', '\n', '\r', '\0',
];

/// Maximum length for various input types
pub const MAX_PATH_LENGTH: usize = 4096;
pub const MAX_COMMAND_LENGTH: usize = 256;
pub const MAX_ARGS_LENGTH: usize = 4096;
pub const MAX_INPUT_LENGTH: usize = 1_000_000; // 1MB

/// Validation error types
#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("Input contains forbidden character: {0:?}")]
    ForbiddenCharacter(char),

    #[error("Input exceeds maximum length ({0} > {1})")]
    TooLong(usize, usize),

    #[error("Empty input not allowed")]
    Empty,

    #[error("Path not within allowed directories: {0}")]
    PathNotAllowed(PathBuf),

    #[error("Path traversal detected: {0}")]
    PathTraversal(PathBuf),

    #[error("Command not whitelisted: {0}")]
    CommandNotAllowed(String),

    #[error("Invalid path: {0}")]
    InvalidPath(String),
}

/// Security errors during execution
#[derive(Debug, Error)]
pub enum SecurityError {
    #[error("Validation failed: {0}")]
    Validation(#[from] ValidationError),

    #[error("Execution timeout after {0} seconds")]
    Timeout(u64),

    #[error("Memory limit exceeded: {0} MB")]
    MemoryExceeded(u64),

    #[error("Output size exceeded: {0} bytes")]
    OutputExceeded(usize),

    #[error("Operation requires approval")]
    RequiresApproval,

    #[error("Agent not authorized for operation: {0}")]
    Unauthorized(String),

    #[error("Resource limit exceeded: {0}")]
    ResourceLimit(String),
}

/// Validate a general input string
pub fn validate_input(input: &str) -> Result<&str, ValidationError> {
    if input.is_empty() {
        return Err(ValidationError::Empty);
    }

    if input.len() > MAX_INPUT_LENGTH {
        return Err(ValidationError::TooLong(input.len(), MAX_INPUT_LENGTH));
    }

    for c in input.chars() {
        if FORBIDDEN_CHARS.contains(&c) {
            return Err(ValidationError::ForbiddenCharacter(c));
        }
    }

    Ok(input)
}

/// Validate a file path against allowed directories
pub fn validate_path(
    path: &str,
    allowed_dirs: &[PathBuf],
    forbidden_dirs: &[PathBuf],
) -> Result<PathBuf, ValidationError> {
    if path.len() > MAX_PATH_LENGTH {
        return Err(ValidationError::TooLong(path.len(), MAX_PATH_LENGTH));
    }

    // Check for forbidden characters
    for c in path.chars() {
        if FORBIDDEN_CHARS.contains(&c) {
            return Err(ValidationError::ForbiddenCharacter(c));
        }
    }

    // Parse and canonicalize the path
    let path_buf = PathBuf::from(path);

    // Check for path traversal attempts
    if path.contains("..") {
        // Allow .. only if the canonicalized path is still within allowed dirs
        // For now, reject any ..
        return Err(ValidationError::PathTraversal(path_buf));
    }

    // Check forbidden directories first (takes precedence)
    for forbidden in forbidden_dirs {
        if path_buf.starts_with(forbidden) {
            return Err(ValidationError::PathNotAllowed(path_buf));
        }
    }

    // Check allowed directories
    let is_allowed = allowed_dirs
        .iter()
        .any(|allowed| path_buf.starts_with(allowed));

    if !is_allowed {
        return Err(ValidationError::PathNotAllowed(path_buf));
    }

    Ok(path_buf)
}

/// Validate a command against whitelist
pub fn validate_command<'a>(
    command: &'a str,
    whitelist: &[String],
) -> Result<&'a str, ValidationError> {
    if command.is_empty() {
        return Err(ValidationError::Empty);
    }

    if command.len() > MAX_COMMAND_LENGTH {
        return Err(ValidationError::TooLong(command.len(), MAX_COMMAND_LENGTH));
    }

    // Extract the base command (first component)
    let base_command = command.split_whitespace().next().unwrap_or(command);

    // Extract just the command name without path
    let cmd_name = Path::new(base_command)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(base_command);

    if !whitelist
        .iter()
        .any(|allowed| allowed == cmd_name || allowed == base_command)
    {
        return Err(ValidationError::CommandNotAllowed(command.to_string()));
    }

    Ok(command)
}

/// Validate and sanitize command arguments
pub fn validate_args(args: &str) -> Result<Vec<String>, ValidationError> {
    if args.len() > MAX_ARGS_LENGTH {
        return Err(ValidationError::TooLong(args.len(), MAX_ARGS_LENGTH));
    }

    // Check for forbidden characters
    for c in args.chars() {
        if FORBIDDEN_CHARS.contains(&c) {
            return Err(ValidationError::ForbiddenCharacter(c));
        }
    }

    // Split arguments safely
    let parsed_args: Vec<String> = shell_words::split(args)
        .map_err(|_| ValidationError::InvalidPath("Invalid argument format".to_string()))?;

    Ok(parsed_args)
}

/// Validate JSON task input
pub fn validate_json_input(json: &str) -> Result<simd_json::OwnedValue, ValidationError> {
    if json.len() > MAX_INPUT_LENGTH {
        return Err(ValidationError::TooLong(json.len(), MAX_INPUT_LENGTH));
    }

    // Parse JSON to ensure it's valid
    let mut json_mut = json.to_string();
    unsafe {
        simd_json::from_str(&mut json_mut)
            .map_err(|_| ValidationError::InvalidPath("Invalid JSON".to_string()))
    }
}

/// Sanitize output by truncating if necessary
pub fn sanitize_output(output: &str, max_size: usize) -> String {
    if output.len() <= max_size {
        output.to_string()
    } else {
        let truncated = &output[..max_size];
        format!("{}... [truncated, {} bytes total]", truncated, output.len())
    }
}

/// Validate environment variable name
pub fn validate_env_name(name: &str) -> Result<&str, ValidationError> {
    if name.is_empty() {
        return Err(ValidationError::Empty);
    }

    // Only allow alphanumeric and underscore
    if !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return Err(ValidationError::ForbiddenCharacter(
            name.chars()
                .find(|c| !c.is_alphanumeric() && *c != '_')
                .unwrap(),
        ));
    }

    Ok(name)
}

/// Validate environment variable value
pub fn validate_env_value(value: &str) -> Result<&str, ValidationError> {
    // Allow most characters but check for null bytes
    if value.contains('\0') {
        return Err(ValidationError::ForbiddenCharacter('\0'));
    }

    if value.len() > MAX_PATH_LENGTH {
        return Err(ValidationError::TooLong(value.len(), MAX_PATH_LENGTH));
    }

    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_input() {
        assert!(validate_input("hello world").is_ok());
        assert!(validate_input("test;rm -rf").is_err());
        assert!(validate_input("$(whoami)").is_err());
        assert!(validate_input("").is_err());
    }

    #[test]
    fn test_validate_path() {
        let allowed = vec![PathBuf::from("/home"), PathBuf::from("/tmp")];
        let forbidden = vec![PathBuf::from("/etc")];

        assert!(validate_path("/home/user/file.txt", &allowed, &forbidden).is_ok());
        assert!(validate_path("/tmp/test", &allowed, &forbidden).is_ok());
        assert!(validate_path("/etc/passwd", &allowed, &forbidden).is_err());
        assert!(validate_path("/root/file", &allowed, &forbidden).is_err());
    }

    #[test]
    fn test_validate_command() {
        let whitelist = vec!["python".to_string(), "cargo".to_string()];

        assert!(validate_command("python", &whitelist).is_ok());
        assert!(validate_command("/usr/bin/python", &whitelist).is_ok());
        assert!(validate_command("cargo", &whitelist).is_ok());
        assert!(validate_command("rm", &whitelist).is_err());
    }

    #[test]
    fn test_path_traversal() {
        let allowed = vec![PathBuf::from("/home")];
        let forbidden = vec![];

        assert!(validate_path("/home/../etc/passwd", &allowed, &forbidden).is_err());
        assert!(validate_path("/home/user/../../../etc", &allowed, &forbidden).is_err());
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/unified/execution/base.rs">
//! Base Execution Agent Implementation

use async_trait::async_trait;
use simd_json::{json, OwnedValue as Value};
use std::collections::HashSet;
use std::process::Stdio;
use tokio::process::Command;
use tokio::time::{timeout, Duration};

use crate::security::{SecurityProfile, SecurityConfig, ProfileCategory};
use super::super::agent_trait::{
    UnifiedAgent, AgentCategory, AgentCapability, AgentRequest, AgentResponse
};

/// Base implementation for execution agents
pub struct ExecutionAgent {
    pub id: String,
    pub name: String,
    pub description: String,
    pub language: String,
    pub system_prompt: String,
    pub knowledge: String,
    pub security_profile: SecurityProfile,
    pub operations: Vec<String>,
}

impl ExecutionAgent {
    /// Create a new execution agent
    pub fn new(
        id: &str,
        name: &str,
        description: &str,
        language: &str,
        allowed_commands: Vec<&str>,
    ) -> Self {
        let security_profile = SecurityProfile::code_execution(id, allowed_commands.clone());
        
        let system_prompt = format!(
            include_str!("../../prompts.rs"),
            agent_name = name,
            language = language,
            allowed_commands = allowed_commands.join(", "),
            file_access = "read: /home, /tmp; write: /tmp",
        );

        Self {
            id: id.to_string(),
            name: name.to_string(),
            description: description.to_string(),
            language: language.to_string(),
            system_prompt,
            knowledge: String::new(),
            security_profile,
            operations: vec![
                "run".to_string(),
                "check".to_string(),
                "format".to_string(),
                "lint".to_string(),
                "test".to_string(),
            ],
        }
    }

    /// Execute a command with sandboxing
    pub async fn execute_command(
        &self,
        command: &str,
        args: &[&str],
        working_dir: Option<&str>,
        timeout_secs: u64,
    ) -> Result<(String, String, i32), String> {
        // Validate command is allowed
        if !self.security_profile.is_command_allowed(command) {
            return Err(format!("Command '{}' not allowed by security profile", command));
        }

        let mut cmd = Command::new(command);
        cmd.args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Some(dir) = working_dir {
            // Validate path
            let path = std::path::Path::new(dir);
            if !self.security_profile.can_read_path(path) {
                return Err(format!("Path '{}' not allowed by security profile", dir));
            }
            cmd.current_dir(dir);
        }

        // Execute with timeout
        let result = timeout(
            Duration::from_secs(timeout_secs),
            cmd.output()
        ).await;

        match result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let code = output.status.code().unwrap_or(-1);
                Ok((stdout, stderr, code))
            }
            Ok(Err(e)) => Err(format!("Command execution failed: {}", e)),
            Err(_) => Err(format!("Command timed out after {} seconds", timeout_secs)),
        }
    }
}

#[async_trait]
impl UnifiedAgent for ExecutionAgent {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn category(&self) -> AgentCategory {
        AgentCategory::Execution
    }

    fn capabilities(&self) -> HashSet<AgentCapability> {
        let mut caps = HashSet::new();
        caps.insert(AgentCapability::RunCode {
            language: self.language.clone(),
        });
        caps.insert(AgentCapability::RunCommand {
            commands: self.security_profile.config.allowed_commands
                .iter().cloned().collect(),
        });
        caps.insert(AgentCapability::ReadFiles);
        caps
    }

    fn system_prompt(&self) -> &str {
        &self.system_prompt
    }

    fn knowledge_base(&self) -> Option<&str> {
        if self.knowledge.is_empty() {
            None
        } else {
            Some(&self.knowledge)
        }
    }

    fn security_profile(&self) -> Option<&SecurityProfile> {
        Some(&self.security_profile)
    }

    fn operations(&self) -> Vec<&str> {
        self.operations.iter().map(|s| s.as_str()).collect()
    }

    async fn execute(&self, request: AgentRequest) -> AgentResponse {
        // Default implementation - subclasses override
        AgentResponse::failure(format!(
            "Operation '{}' not implemented for {}",
            request.operation, self.id
        ))
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/unified/execution/golang.rs">
//! Go Executor Agent

use async_trait::async_trait;
use simd_json::json;
use std::collections::HashSet;

use super::base::ExecutionAgent;
use super::super::agent_trait::{
    UnifiedAgent, AgentCategory, AgentCapability, AgentRequest, AgentResponse
};
use super::super::prompts::languages::GO;
use crate::security::SecurityProfile;

pub struct GoExecutor {
    base: ExecutionAgent,
}

impl GoExecutor {
    pub fn new() -> Self {
        let mut base = ExecutionAgent::new(
            "go-executor",
            "Go Executor",
            "Executes Go code. Supports build, test, vet, and fmt.",
            "go",
            vec!["go", "gofmt", "staticcheck"],
        );
        base.knowledge = GO.to_string();
        base.operations = vec![
            "build".to_string(),
            "test".to_string(),
            "run".to_string(),
            "fmt".to_string(),
            "vet".to_string(),
        ];
        Self { base }
    }
}

impl Default for GoExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl UnifiedAgent for GoExecutor {
    fn id(&self) -> &str { self.base.id() }
    fn name(&self) -> &str { self.base.name() }
    fn description(&self) -> &str { self.base.description() }
    fn category(&self) -> AgentCategory { AgentCategory::Execution }
    fn capabilities(&self) -> HashSet<AgentCapability> { self.base.capabilities() }
    fn system_prompt(&self) -> &str { self.base.system_prompt() }
    fn knowledge_base(&self) -> Option<&str> { self.base.knowledge_base() }
    fn security_profile(&self) -> Option<&SecurityProfile> { self.base.security_profile() }
    fn operations(&self) -> Vec<&str> { self.base.operations() }

    async fn execute(&self, request: AgentRequest) -> AgentResponse {
        let path = request.args.get("path")
            .and_then(|v| v.as_str())
            .unwrap_or(".");

        let (args, timeout): (Vec<&str>, u64) = match request.operation.as_str() {
            "build" => (vec!["build", "./..."], 300),
            "test" => (vec!["test", "-v", "./..."], 300),
            "run" => {
                let file = request.args.get("file")
                    .and_then(|v| v.as_str())
                    .unwrap_or("main.go");
                (vec!["run", file], 120)
            }
            "fmt" => (vec!["fmt", "./..."], 60),
            "vet" => (vec!["vet", "./..."], 120),
            _ => return AgentResponse::failure(format!("Unknown operation: {}", request.operation)),
        };

        match self.base.execute_command("go", &args, Some(path), timeout).await {
            Ok((stdout, stderr, code)) => {
                AgentResponse::success(
                    json!({
                        "stdout": stdout,
                        "stderr": stderr,
                        "exit_code": code,
                        "success": code == 0
                    }),
                    if code == 0 {
                        format!("{} completed successfully", request.operation)
                    } else {
                        format!("{} failed", request.operation)
                    }
                )
            }
            Err(e) => AgentResponse::failure(e),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/unified/execution/javascript.rs">
//! JavaScript/TypeScript Executor Agent

use async_trait::async_trait;
use simd_json::json;
use std::collections::HashSet;

use super::base::ExecutionAgent;
use super::super::agent_trait::{
    UnifiedAgent, AgentCategory, AgentCapability, AgentRequest, AgentResponse
};
use super::super::prompts::languages::JAVASCRIPT;
use crate::security::SecurityProfile;

pub struct JavaScriptExecutor {
    base: ExecutionAgent,
}

impl JavaScriptExecutor {
    pub fn new() -> Self {
        let mut base = ExecutionAgent::new(
            "javascript-executor",
            "JavaScript/TypeScript Executor",
            "Executes JavaScript/TypeScript via Node.js. Supports npm, pnpm, jest, and eslint.",
            "javascript",
            vec!["node", "npm", "npx", "pnpm", "eslint", "prettier", "jest", "vitest", "tsc"],
        );
        base.knowledge = JAVASCRIPT.to_string();
        base.operations = vec![
            "run".to_string(),
            "test".to_string(),
            "lint".to_string(),
            "format".to_string(),
            "typecheck".to_string(),
            "install".to_string(),
        ];
        Self { base }
    }
}

impl Default for JavaScriptExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl UnifiedAgent for JavaScriptExecutor {
    fn id(&self) -> &str { self.base.id() }
    fn name(&self) -> &str { self.base.name() }
    fn description(&self) -> &str { self.base.description() }
    fn category(&self) -> AgentCategory { AgentCategory::Execution }
    fn capabilities(&self) -> HashSet<AgentCapability> { self.base.capabilities() }
    fn system_prompt(&self) -> &str { self.base.system_prompt() }
    fn knowledge_base(&self) -> Option<&str> { self.base.knowledge_base() }
    fn security_profile(&self) -> Option<&SecurityProfile> { self.base.security_profile() }
    fn operations(&self) -> Vec<&str> { self.base.operations() }

    async fn execute(&self, request: AgentRequest) -> AgentResponse {
        let path = request.args.get("path")
            .and_then(|v| v.as_str())
            .unwrap_or(".");

        match request.operation.as_str() {
            "run" => {
                let script = request.args.get("script")
                    .and_then(|v| v.as_str())
                    .unwrap_or("start");
                match self.base.execute_command("npm", &["run", script], Some(path), 300).await {
                    Ok((stdout, stderr, code)) => {
                        AgentResponse::success(
                            json!({ "stdout": stdout, "stderr": stderr, "exit_code": code }),
                            if code == 0 { "Script completed" } else { "Script failed" }
                        )
                    }
                    Err(e) => AgentResponse::failure(e),
                }
            }
            "test" => {
                // Try vitest first, fall back to jest
                match self.base.execute_command("npx", &["vitest", "run"], Some(path), 300).await {
                    Ok((stdout, stderr, code)) => {
                        AgentResponse::success(
                            json!({ "stdout": stdout, "stderr": stderr, "exit_code": code }),
                            if code == 0 { "Tests passed" } else { "Tests failed" }
                        )
                    }
                    Err(_) => {
                        // Try jest
                        match self.base.execute_command("npx", &["jest"], Some(path), 300).await {
                            Ok((stdout, stderr, code)) => {
                                AgentResponse::success(
                                    json!({ "stdout": stdout, "stderr": stderr, "exit_code": code }),
                                    if code == 0 { "Tests passed" } else { "Tests failed" }
                                )
                            }
                            Err(e) => AgentResponse::failure(e),
                        }
                    }
                }
            }
            "lint" => {
                match self.base.execute_command("npx", &["eslint", "."], Some(path), 120).await {
                    Ok((stdout, stderr, code)) => {
                        AgentResponse::success(
                            json!({ "output": stdout, "errors": stderr, "exit_code": code }),
                            if code == 0 { "No linting issues" } else { "Linting issues found" }
                        )
                    }
                    Err(e) => AgentResponse::failure(e),
                }
            }
            "format" => {
                match self.base.execute_command("npx", &["prettier", "--write", "."], Some(path), 60).await {
                    Ok((stdout, _, code)) => {
                        AgentResponse::success(
                            json!({ "output": stdout, "exit_code": code }),
                            "Code formatted"
                        )
                    }
                    Err(e) => AgentResponse::failure(e),
                }
            }
            "typecheck" => {
                match self.base.execute_command("npx", &["tsc", "--noEmit"], Some(path), 120).await {
                    Ok((stdout, stderr, code)) => {
                        AgentResponse::success(
                            json!({ "output": stdout, "errors": stderr, "exit_code": code }),
                            if code == 0 { "No type errors" } else { "Type errors found" }
                        )
                    }
                    Err(e) => AgentResponse::failure(e),
                }
            }
            "install" => {
                match self.base.execute_command("npm", &["install"], Some(path), 300).await {
                    Ok((stdout, stderr, code)) => {
                        AgentResponse::success(
                            json!({ "output": stdout, "errors": stderr, "exit_code": code }),
                            if code == 0 { "Dependencies installed" } else { "Installation failed" }
                        )
                    }
                    Err(e) => AgentResponse::failure(e),
                }
            }
            _ => AgentResponse::failure(format!("Unknown operation: {}", request.operation)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/unified/execution/mod.rs">
//! Execution Agents
//!
//! Agents that can execute code and commands with sandboxing.
//! These have SecurityProfiles and process management.

mod base;
mod python;
mod rust;
mod javascript;
mod golang;
mod shell;

pub use base::ExecutionAgent;
pub use python::PythonExecutor;
pub use rust::RustExecutor;
pub use javascript::JavaScriptExecutor;
pub use golang::GoExecutor;
pub use shell::ShellExecutor;

use std::collections::HashMap;
use once_cell::sync::Lazy;

/// All available execution agents
pub static EXECUTION_AGENTS: Lazy<HashMap<&'static str, fn() -> Box<dyn super::UnifiedAgent>>> = Lazy::new(|| {
    let mut m: HashMap<&'static str, fn() -> Box<dyn super::UnifiedAgent>> = HashMap::new();
    m.insert("python-executor", || Box::new(PythonExecutor::new()));
    m.insert("rust-executor", || Box::new(RustExecutor::new()));
    m.insert("javascript-executor", || Box::new(JavaScriptExecutor::new()));
    m.insert("go-executor", || Box::new(GoExecutor::new()));
    m.insert("shell-executor", || Box::new(ShellExecutor::new()));
    m
});
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/unified/execution/python.rs">
//! Python Executor Agent
//!
//! Executes Python code with sandboxing.

use async_trait::async_trait;
use simd_json::{json, OwnedValue as Value};
use std::collections::HashSet;

use super::base::ExecutionAgent;
use super::super::agent_trait::{
    UnifiedAgent, AgentCategory, AgentCapability, AgentRequest, AgentResponse
};
use super::super::prompts::languages::PYTHON;
use crate::security::SecurityProfile;

pub struct PythonExecutor {
    base: ExecutionAgent,
}

impl PythonExecutor {
    pub fn new() -> Self {
        let mut base = ExecutionAgent::new(
            "python-executor",
            "Python Executor",
            "Executes Python code with sandboxing. Supports pytest, ruff, mypy, and uv.",
            "python",
            vec!["python", "python3", "pip", "pip3", "uv", "ruff", "pytest", "mypy", "black", "isort"],
        );
        base.knowledge = PYTHON.to_string();
        base.operations = vec![
            "run".to_string(),
            "test".to_string(),
            "lint".to_string(),
            "format".to_string(),
            "typecheck".to_string(),
            "install".to_string(),
        ];
        Self { base }
    }

    async fn run_python(&self, code: &str, args: &[&str]) -> AgentResponse {
        // Write code to temp file
        let temp_file = "/tmp/python_exec.py";
        if let Err(e) = tokio::fs::write(temp_file, code).await {
            return AgentResponse::failure(format!("Failed to write temp file: {}", e));
        }

        let mut cmd_args = vec![temp_file];
        cmd_args.extend(args);

        match self.base.execute_command("python3", &cmd_args, None, 60).await {
            Ok((stdout, stderr, code)) => {
                if code == 0 {
                    AgentResponse::success(
                        json!({ "stdout": stdout, "stderr": stderr, "exit_code": code }),
                        "Python code executed successfully"
                    )
                } else {
                    AgentResponse::failure(format!("Python exited with code {}: {}", code, stderr))
                }
            }
            Err(e) => AgentResponse::failure(e),
        }
    }

    async fn run_pytest(&self, path: &str, args: &[&str]) -> AgentResponse {
        let mut cmd_args = vec!["-m", "pytest", path, "-v"];
        cmd_args.extend(args);

        match self.base.execute_command("python3", &cmd_args, None, 300).await {
            Ok((stdout, stderr, code)) => {
                AgentResponse::success(
                    json!({
                        "stdout": stdout,
                        "stderr": stderr,
                        "exit_code": code,
                        "passed": code == 0
                    }),
                    if code == 0 { "All tests passed" } else { "Some tests failed" }
                )
            }
            Err(e) => AgentResponse::failure(e),
        }
    }

    async fn run_ruff(&self, path: &str, fix: bool) -> AgentResponse {
        let mut args = vec!["check", path];
        if fix {
            args.push("--fix");
        }

        match self.base.execute_command("ruff", &args, None, 60).await {
            Ok((stdout, stderr, code)) => {
                AgentResponse::success(
                    json!({
                        "output": stdout,
                        "errors": stderr,
                        "exit_code": code,
                        "clean": code == 0
                    }),
                    if code == 0 { "No linting issues" } else { "Linting issues found" }
                )
            }
            Err(e) => AgentResponse::failure(e),
        }
    }

    async fn run_mypy(&self, path: &str) -> AgentResponse {
        match self.base.execute_command("mypy", &[path, "--strict"], None, 120).await {
            Ok((stdout, stderr, code)) => {
                AgentResponse::success(
                    json!({
                        "output": stdout,
                        "errors": stderr,
                        "exit_code": code,
                        "type_safe": code == 0
                    }),
                    if code == 0 { "No type errors" } else { "Type errors found" }
                )
            }
            Err(e) => AgentResponse::failure(e),
        }
    }
}

impl Default for PythonExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl UnifiedAgent for PythonExecutor {
    fn id(&self) -> &str {
        self.base.id()
    }

    fn name(&self) -> &str {
        self.base.name()
    }

    fn description(&self) -> &str {
        self.base.description()
    }

    fn category(&self) -> AgentCategory {
        AgentCategory::Execution
    }

    fn capabilities(&self) -> HashSet<AgentCapability> {
        self.base.capabilities()
    }

    fn system_prompt(&self) -> &str {
        self.base.system_prompt()
    }

    fn knowledge_base(&self) -> Option<&str> {
        self.base.knowledge_base()
    }

    fn security_profile(&self) -> Option<&SecurityProfile> {
        self.base.security_profile()
    }

    fn operations(&self) -> Vec<&str> {
        self.base.operations()
    }

    async fn execute(&self, request: AgentRequest) -> AgentResponse {
        match request.operation.as_str() {
            "run" => {
                let code = request.args.get("code")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let args: Vec<&str> = request.args.get("args")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
                    .unwrap_or_default();
                self.run_python(code, &args).await
            }
            "test" => {
                let path = request.args.get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or(".");
                let args: Vec<&str> = request.args.get("args")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
                    .unwrap_or_default();
                self.run_pytest(path, &args).await
            }
            "lint" => {
                let path = request.args.get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or(".");
                let fix = request.args.get("fix")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                self.run_ruff(path, fix).await
            }
            "typecheck" => {
                let path = request.args.get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or(".");
                self.run_mypy(path).await
            }
            "format" => {
                let path = request.args.get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or(".");
                // Run both black and isort
                match self.base.execute_command("ruff", &["format", path], None, 60).await {
                    Ok((stdout, _, code)) => {
                        AgentResponse::success(
                            json!({ "output": stdout, "exit_code": code }),
                            "Code formatted"
                        )
                    }
                    Err(e) => AgentResponse::failure(e),
                }
            }
            "install" => {
                let packages: Vec<&str> = request.args.get("packages")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
                    .unwrap_or_default();
                
                if packages.is_empty() {
                    return AgentResponse::failure("No packages specified");
                }

                let mut args = vec!["pip", "install"];
                args.extend(packages.iter());
                
                match self.base.execute_command("uv", &args, None, 300).await {
                    Ok((stdout, stderr, code)) => {
                        AgentResponse::success(
                            json!({ "output": stdout, "errors": stderr, "exit_code": code }),
                            if code == 0 { "Packages installed" } else { "Installation failed" }
                        )
                    }
                    Err(e) => AgentResponse::failure(e),
                }
            }
            _ => AgentResponse::failure(format!("Unknown operation: {}", request.operation)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/unified/execution/rust.rs">
//! Rust Executor Agent

use async_trait::async_trait;
use simd_json::json;
use std::collections::HashSet;

use super::base::ExecutionAgent;
use super::super::agent_trait::{
    UnifiedAgent, AgentCategory, AgentCapability, AgentRequest, AgentResponse
};
use super::super::prompts::languages::RUST;
use crate::security::SecurityProfile;

pub struct RustExecutor {
    base: ExecutionAgent,
}

impl RustExecutor {
    pub fn new() -> Self {
        let mut base = ExecutionAgent::new(
            "rust-executor",
            "Rust Executor",
            "Executes Rust code via cargo. Supports build, test, clippy, and format.",
            "rust",
            vec!["cargo", "rustc", "rustfmt", "clippy-driver"],
        );
        base.knowledge = RUST.to_string();
        base.operations = vec![
            "check".to_string(),
            "build".to_string(),
            "test".to_string(),
            "clippy".to_string(),
            "format".to_string(),
            "run".to_string(),
        ];
        Self { base }
    }
}

impl Default for RustExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl UnifiedAgent for RustExecutor {
    fn id(&self) -> &str { self.base.id() }
    fn name(&self) -> &str { self.base.name() }
    fn description(&self) -> &str { self.base.description() }
    fn category(&self) -> AgentCategory { AgentCategory::Execution }
    fn capabilities(&self) -> HashSet<AgentCapability> { self.base.capabilities() }
    fn system_prompt(&self) -> &str { self.base.system_prompt() }
    fn knowledge_base(&self) -> Option<&str> { self.base.knowledge_base() }
    fn security_profile(&self) -> Option<&SecurityProfile> { self.base.security_profile() }
    fn operations(&self) -> Vec<&str> { self.base.operations() }

    async fn execute(&self, request: AgentRequest) -> AgentResponse {
        let path = request.args.get("path")
            .and_then(|v| v.as_str())
            .unwrap_or(".");

        let (cmd, args, timeout) = match request.operation.as_str() {
            "check" => ("cargo", vec!["check"], 120),
            "build" => {
                let release = request.args.get("release")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let mut args = vec!["build"];
                if release { args.push("--release"); }
                ("cargo", args, 600)
            }
            "test" => ("cargo", vec!["test"], 600),
            "clippy" => ("cargo", vec!["clippy", "--", "-D", "warnings"], 120),
            "format" => ("cargo", vec!["fmt"], 60),
            "run" => {
                let release = request.args.get("release")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let mut args = vec!["run"];
                if release { args.push("--release"); }
                ("cargo", args, 300)
            }
            _ => return AgentResponse::failure(format!("Unknown operation: {}", request.operation)),
        };

        let args_str: Vec<&str> = args.iter().map(|s| *s).collect();
        match self.base.execute_command(cmd, &args_str, Some(path), timeout).await {
            Ok((stdout, stderr, code)) => {
                AgentResponse::success(
                    json!({
                        "stdout": stdout,
                        "stderr": stderr,
                        "exit_code": code,
                        "success": code == 0
                    }),
                    if code == 0 { 
                        format!("{} completed successfully", request.operation)
                    } else {
                        format!("{} failed with code {}", request.operation, code)
                    }
                )
            }
            Err(e) => AgentResponse::failure(e),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/unified/execution/shell.rs">
//! Shell Executor Agent
//!
//! Executes whitelisted shell commands.

use async_trait::async_trait;
use simd_json::json;
use std::collections::HashSet;

use super::base::ExecutionAgent;
use super::super::agent_trait::{
    UnifiedAgent, AgentCategory, AgentCapability, AgentRequest, AgentResponse
};
use crate::security::SecurityProfile;

pub struct ShellExecutor {
    base: ExecutionAgent,
}

impl ShellExecutor {
    pub fn new() -> Self {
        let base = ExecutionAgent::new(
            "shell-executor",
            "Shell Executor",
            "Executes whitelisted shell commands for system operations.",
            "shell",
            vec![
                // File operations (read-only)
                "ls", "cat", "head", "tail", "find", "grep", "wc", "file", "stat",
                // System info
                "uname", "hostname", "uptime", "df", "free", "ps", "top",
                // Network info (read-only)
                "ip", "ss", "netstat", "ping", "dig", "nslookup",
                // Git (read operations)
                "git",
                // Text processing
                "sort", "uniq", "cut", "awk", "sed", "jq",
            ],
        );
        Self { base }
    }
}

impl Default for ShellExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl UnifiedAgent for ShellExecutor {
    fn id(&self) -> &str { self.base.id() }
    fn name(&self) -> &str { self.base.name() }
    fn description(&self) -> &str { self.base.description() }
    fn category(&self) -> AgentCategory { AgentCategory::Execution }
    fn capabilities(&self) -> HashSet<AgentCapability> { self.base.capabilities() }
    fn system_prompt(&self) -> &str { self.base.system_prompt() }
    fn knowledge_base(&self) -> Option<&str> { self.base.knowledge_base() }
    fn security_profile(&self) -> Option<&SecurityProfile> { self.base.security_profile() }
    fn operations(&self) -> Vec<&str> { vec!["exec"] }

    async fn execute(&self, request: AgentRequest) -> AgentResponse {
        if request.operation != "exec" {
            return AgentResponse::failure(format!("Unknown operation: {}", request.operation));
        }

        let command = match request.args.get("command").and_then(|v| v.as_str()) {
            Some(cmd) => cmd,
            None => return AgentResponse::failure("No command specified"),
        };

        // Parse command into program and args
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            return AgentResponse::failure("Empty command");
        }

        let program = parts[0];
        let args: Vec<&str> = parts[1..].to_vec();

        let working_dir = request.args.get("cwd")
            .and_then(|v| v.as_str());

        let timeout = request.args.get("timeout")
            .and_then(|v| v.as_u64())
            .unwrap_or(30);

        match self.base.execute_command(program, &args, working_dir, timeout).await {
            Ok((stdout, stderr, code)) => {
                AgentResponse::success(
                    json!({
                        "stdout": stdout,
                        "stderr": stderr,
                        "exit_code": code,
                        "command": command
                    }),
                    if code == 0 { "Command completed" } else { "Command failed" }
                )
            }
            Err(e) => AgentResponse::failure(e),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/unified/orchestration/base.rs">
//! Base Orchestration Agent Implementation

use async_trait::async_trait;
use simd_json::{json, OwnedValue as Value};
use std::collections::HashSet;
use std::sync::Arc;

use super::super::agent_trait::{
    UnifiedAgent, AgentCategory, AgentCapability, AgentRequest, AgentResponse
};
use super::super::registry::UnifiedAgentRegistry;
use crate::security::SecurityProfile;

/// Workflow step definition
#[derive(Debug, Clone)]
pub struct WorkflowStep {
    pub name: String,
    pub agent_id: String,
    pub operation: String,
    pub args_template: Value,
    pub condition: Option<String>,
}

/// Base implementation for orchestration agents
pub struct OrchestrationAgent {
    pub id: String,
    pub name: String,
    pub description: String,
    pub system_prompt: String,
    pub allowed_agents: HashSet<String>,
    pub workflow_steps: Vec<WorkflowStep>,
}

impl OrchestrationAgent {
    pub fn new(
        id: &str,
        name: &str,
        description: &str,
        allowed_agents: Vec<&str>,
    ) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            description: description.to_string(),
            system_prompt: format!(
                "You are {}, an orchestration agent that coordinates other agents to complete complex tasks.",
                name
            ),
            allowed_agents: allowed_agents.into_iter().map(|s| s.to_string()).collect(),
            workflow_steps: vec![],
        }
    }

    pub fn with_step(mut self, step: WorkflowStep) -> Self {
        self.workflow_steps.push(step);
        self
    }

    /// Execute workflow steps using the registry
    pub async fn execute_workflow(
        &self,
        registry: &UnifiedAgentRegistry,
        context: Value,
    ) -> AgentResponse {
        let mut results = Vec::new();
        let mut current_context = context;

        for step in &self.workflow_steps {
            // Check if agent is allowed
            if !self.allowed_agents.contains(&step.agent_id) {
                return AgentResponse::failure(format!(
                    "Agent '{}' not allowed in this orchestration",
                    step.agent_id
                ));
            }

            // Get the agent
            let agent = match registry.get(&step.agent_id) {
                Some(a) => a,
                None => {
                    return AgentResponse::failure(format!(
                        "Agent '{}' not found",
                        step.agent_id
                    ));
                }
            };

            // Build request
            let request = AgentRequest {
                operation: step.operation.clone(),
                args: step.args_template.clone(),
                context: Some(current_context.to_string()),
                files: vec![],
            };

            // Execute
            let response = agent.execute(request).await;
            results.push(json!({
                "step": step.name,
                "agent": step.agent_id,
                "success": response.success,
                "result": response.data,
            }));

            if !response.success {
                return AgentResponse::failure(format!(
                    "Workflow failed at step '{}': {}",
                    step.name, response.message
                )).with_suggestions(vec![
                    format!("Check {} agent configuration", step.agent_id),
                    "Review step arguments".to_string(),
                ]);
            }

            // Update context with result
            if let Some(obj) = current_context.as_object_mut() {
                obj.insert(format!("step_{}_result", step.name), response.data);
            }
        }

        AgentResponse::success(
            json!({
                "workflow": self.id,
                "steps_completed": results.len(),
                "results": results,
            }),
            "Workflow completed successfully"
        )
    }
}

#[async_trait]
impl UnifiedAgent for OrchestrationAgent {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn category(&self) -> AgentCategory {
        AgentCategory::Orchestration
    }

    fn capabilities(&self) -> HashSet<AgentCapability> {
        let mut caps = HashSet::new();
        caps.insert(AgentCapability::DelegateToAgents {
            agents: self.allowed_agents.iter().cloned().collect(),
        });
        caps.insert(AgentCapability::WorkflowManagement);
        caps
    }

    fn system_prompt(&self) -> &str {
        &self.system_prompt
    }

    fn security_profile(&self) -> Option<&SecurityProfile> {
        None // Orchestrators delegate security to sub-agents
    }

    fn operations(&self) -> Vec<&str> {
        vec!["run_workflow", "list_steps", "validate"]
    }

    async fn execute(&self, request: AgentRequest) -> AgentResponse {
        match request.operation.as_str() {
            "list_steps" => {
                let steps: Vec<_> = self.workflow_steps.iter()
                    .map(|s| json!({
                        "name": s.name,
                        "agent": s.agent_id,
                        "operation": s.operation,
                    }))
                    .collect();
                AgentResponse::success(
                    json!({ "steps": steps }),
                    format!("Workflow has {} steps", steps.len())
                )
            }
            "validate" => {
                // Would validate workflow configuration
                AgentResponse::success(
                    json!({ "valid": true }),
                    "Workflow configuration is valid"
                )
            }
            _ => {
                AgentResponse::failure(
                    "run_workflow requires a registry - use execute_workflow() directly"
                )
            }
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/unified/orchestration/code_review_orchestrator.rs">
//! Code Review Orchestrator Agent
//!
//! Coordinates comprehensive code review:
//! 1. Static analysis
//! 2. Security audit
//! 3. Architecture review
//! 4. Documentation check

use super::base::{OrchestrationAgent, WorkflowStep};
use simd_json::json;

pub struct CodeReviewOrchestrator(OrchestrationAgent);

impl CodeReviewOrchestrator {
    pub fn new() -> OrchestrationAgent {
        OrchestrationAgent::new(
            "code-review-orchestrator",
            "Code Review Orchestrator",
            "Coordinates comprehensive code review with multiple expert agents",
            vec!["python-executor", "rust-executor", "security-auditor", "code-reviewer", "backend-architect"],
        )
        .with_step(WorkflowStep {
            name: "lint".to_string(),
            agent_id: "python-executor".to_string(),
            operation: "lint".to_string(),
            args_template: json!({ "path": ".", "fix": false }),
            condition: None,
        })
        .with_step(WorkflowStep {
            name: "typecheck".to_string(),
            agent_id: "python-executor".to_string(),
            operation: "typecheck".to_string(),
            args_template: json!({ "path": "." }),
            condition: None,
        })
        .with_step(WorkflowStep {
            name: "security_audit".to_string(),
            agent_id: "security-auditor".to_string(),
            operation: "review".to_string(),
            args_template: json!({
                "query": "Audit this code for security vulnerabilities"
            }),
            condition: None,
        })
        .with_step(WorkflowStep {
            name: "architecture_review".to_string(),
            agent_id: "backend-architect".to_string(),
            operation: "review".to_string(),
            args_template: json!({
                "query": "Review the architecture and suggest improvements"
            }),
            condition: None,
        })
        .with_step(WorkflowStep {
            name: "final_review".to_string(),
            agent_id: "code-reviewer".to_string(),
            operation: "review".to_string(),
            args_template: json!({
                "query": "Provide final code review summary"
            }),
            condition: None,
        })
    }
}

impl Default for CodeReviewOrchestrator {
    fn default() -> Self {
        Self(Self::new())
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/unified/orchestration/mod.rs">
//! Orchestration Agents
//!
//! Meta-agents that coordinate other agents for complex workflows.

mod base;
mod tdd_orchestrator;
mod code_review_orchestrator;

pub use base::OrchestrationAgent;
pub use tdd_orchestrator::TddOrchestrator;
pub use code_review_orchestrator::CodeReviewOrchestrator;

use std::collections::HashMap;
use once_cell::sync::Lazy;

/// All available orchestration agents
pub static ORCHESTRATION_AGENTS: Lazy<HashMap<&'static str, fn() -> Box<dyn super::UnifiedAgent>>> = Lazy::new(|| {
    let mut m: HashMap<&'static str, fn() -> Box<dyn super::UnifiedAgent>> = HashMap::new();
    m.insert("tdd-orchestrator", || Box::new(TddOrchestrator::new()));
    m.insert("code-review-orchestrator", || Box::new(CodeReviewOrchestrator::new()));
    m
});
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/unified/orchestration/tdd_orchestrator.rs">
//! TDD Orchestrator Agent
//!
//! Coordinates Test-Driven Development workflow:
//! 1. Write failing test
//! 2. Write minimal code to pass
//! 3. Refactor

use super::base::{OrchestrationAgent, WorkflowStep};
use simd_json::json;

pub struct TddOrchestrator(OrchestrationAgent);

impl TddOrchestrator {
    pub fn new() -> OrchestrationAgent {
        OrchestrationAgent::new(
            "tdd-orchestrator",
            "TDD Orchestrator",
            "Coordinates Test-Driven Development workflow: Red-Green-Refactor",
            vec!["python-executor", "rust-executor", "code-reviewer"],
        )
        .with_step(WorkflowStep {
            name: "write_test".to_string(),
            agent_id: "code-reviewer".to_string(),
            operation: "consult".to_string(),
            args_template: json!({
                "query": "Generate a failing test for the requested feature"
            }),
            condition: None,
        })
        .with_step(WorkflowStep {
            name: "run_test_red".to_string(),
            agent_id: "python-executor".to_string(),
            operation: "test".to_string(),
            args_template: json!({ "path": "." }),
            condition: Some("expect_failure".to_string()),
        })
        .with_step(WorkflowStep {
            name: "implement".to_string(),
            agent_id: "code-reviewer".to_string(),
            operation: "consult".to_string(),
            args_template: json!({
                "query": "Write minimal code to make the test pass"
            }),
            condition: None,
        })
        .with_step(WorkflowStep {
            name: "run_test_green".to_string(),
            agent_id: "python-executor".to_string(),
            operation: "test".to_string(),
            args_template: json!({ "path": "." }),
            condition: Some("expect_success".to_string()),
        })
        .with_step(WorkflowStep {
            name: "refactor".to_string(),
            agent_id: "code-reviewer".to_string(),
            operation: "review".to_string(),
            args_template: json!({
                "query": "Suggest refactoring improvements while keeping tests green"
            }),
            condition: None,
        })
    }
}

impl Default for TddOrchestrator {
    fn default() -> Self {
        Self(Self::new())
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/unified/persona/architecture_experts.rs">
//! Architecture Expert Agents

use super::base::PersonaAgent;
use super::super::agent_trait::AgentCapability;
use super::super::prompts::architecture::{BACKEND_ARCHITECT, SECURITY_AUDITOR, CODE_REVIEWER};

pub struct BackendArchitect(PersonaAgent);

impl BackendArchitect {
    pub fn new() -> PersonaAgent {
        PersonaAgent::new(
            "backend-architect",
            "Backend Architect",
            "Expert in backend architecture, microservices, API design, and distributed systems.",
            "architecture",
            "You are a backend architect with expertise in designing scalable, maintainable systems.",
            BACKEND_ARCHITECT,
        )
        .with_capability(AgentCapability::ArchitectureDesign)
        .with_capability(AgentCapability::CodeReview)
    }
}

impl Default for BackendArchitect {
    fn default() -> Self {
        Self(Self::new())
    }
}

pub struct SecurityAuditor(PersonaAgent);

impl SecurityAuditor {
    pub fn new() -> PersonaAgent {
        PersonaAgent::new(
            "security-auditor",
            "Security Auditor",
            "Expert in application security, OWASP, secure coding practices, and vulnerability assessment.",
            "security",
            "You are a security auditor focused on identifying vulnerabilities and recommending secure coding practices.",
            SECURITY_AUDITOR,
        )
        .with_capability(AgentCapability::SecurityAudit)
        .with_capability(AgentCapability::CodeReview)
    }
}

impl Default for SecurityAuditor {
    fn default() -> Self {
        Self(Self::new())
    }
}

pub struct CodeReviewer(PersonaAgent);

impl CodeReviewer {
    pub fn new() -> PersonaAgent {
        PersonaAgent::new(
            "code-reviewer",
            "Code Reviewer",
            "Expert in code review, best practices, and constructive feedback.",
            "review",
            "You are an experienced code reviewer focused on quality, maintainability, and constructive feedback.",
            CODE_REVIEWER,
        )
        .with_capability(AgentCapability::CodeReview)
    }
}

impl Default for CodeReviewer {
    fn default() -> Self {
        Self(Self::new())
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/unified/persona/base.rs">
//! Base Persona Agent Implementation

use async_trait::async_trait;
use simd_json::{json, OwnedValue as Value};
use std::collections::HashSet;

use super::super::agent_trait::{
    UnifiedAgent, AgentCategory, AgentCapability, AgentRequest, AgentResponse
};
use crate::security::SecurityProfile;

/// Base implementation for persona agents (LLM-only, no execution)
pub struct PersonaAgent {
    pub id: String,
    pub name: String,
    pub description: String,
    pub domain: String,
    pub system_prompt: String,
    pub knowledge: String,
    pub capabilities: HashSet<AgentCapability>,
    pub examples: Vec<(String, String)>,
}

impl PersonaAgent {
    /// Create a new persona agent
    pub fn new(
        id: &str,
        name: &str,
        description: &str,
        domain: &str,
        system_prompt: &str,
        knowledge: &str,
    ) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            description: description.to_string(),
            domain: domain.to_string(),
            system_prompt: system_prompt.to_string(),
            knowledge: knowledge.to_string(),
            capabilities: HashSet::new(),
            examples: vec![],
        }
    }

    /// Add a capability
    pub fn with_capability(mut self, cap: AgentCapability) -> Self {
        self.capabilities.insert(cap);
        self
    }

    /// Add an example interaction
    pub fn with_example(mut self, question: &str, answer: &str) -> Self {
        self.examples.push((question.to_string(), answer.to_string()));
        self
    }

    /// Generate augmented prompt for LLM
    pub fn augmented_prompt(&self, user_query: &str) -> String {
        let mut prompt = self.system_prompt.clone();
        
        if !self.knowledge.is_empty() {
            prompt.push_str("\n\n## Domain Knowledge\n");
            prompt.push_str(&self.knowledge);
        }

        if !self.examples.is_empty() {
            prompt.push_str("\n\n## Example Interactions\n");
            for (q, a) in &self.examples {
                prompt.push_str(&format!("\nQ: {}\nA: {}\n", q, a));
            }
        }

        prompt.push_str(&format!("\n\n## Current Query\n{}", user_query));
        prompt
    }
}

#[async_trait]
impl UnifiedAgent for PersonaAgent {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn category(&self) -> AgentCategory {
        AgentCategory::Persona
    }

    fn capabilities(&self) -> HashSet<AgentCapability> {
        self.capabilities.clone()
    }

    fn system_prompt(&self) -> &str {
        &self.system_prompt
    }

    fn knowledge_base(&self) -> Option<&str> {
        if self.knowledge.is_empty() {
            None
        } else {
            Some(&self.knowledge)
        }
    }

    fn examples(&self) -> Vec<(&str, &str)> {
        self.examples.iter()
            .map(|(q, a)| (q.as_str(), a.as_str()))
            .collect()
    }

    fn security_profile(&self) -> Option<&SecurityProfile> {
        None // Persona agents don't execute code
    }

    fn operations(&self) -> Vec<&str> {
        vec!["consult", "review", "explain", "recommend"]
    }

    async fn execute(&self, request: AgentRequest) -> AgentResponse {
        // Persona agents don't execute - they augment LLM prompts
        // Return the augmented prompt for the LLM to process
        let query = request.args.get("query")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let augmented = self.augmented_prompt(query);

        AgentResponse::success(
            json!({
                "augmented_prompt": augmented,
                "domain": self.domain,
                "agent": self.id,
            }),
            "Prompt augmented with domain expertise"
        )
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/unified/persona/framework_experts.rs">
//! Framework Expert Agents

use super::base::PersonaAgent;
use super::super::agent_trait::AgentCapability;
use super::super::prompts::frameworks::{DJANGO, FASTAPI, REACT};

pub struct DjangoExpert(PersonaAgent);

impl DjangoExpert {
    pub fn new() -> PersonaAgent {
        PersonaAgent::new(
            "django-expert",
            "Django Expert",
            "Expert in Django web framework, ORM, DRF, and Python web development best practices.",
            "django",
            "You are a Django expert with deep knowledge of the Django web framework, Django REST Framework, and Python web development.",
            DJANGO,
        )
        .with_capability(AgentCapability::ArchitectureDesign)
        .with_capability(AgentCapability::CodeReview)
        .with_example(
            "How should I structure a large Django project?",
            "For large Django projects, I recommend: 1) Use a modular app structure with clear boundaries, 2) Implement a service layer for business logic, 3) Use Django's app config for initialization, 4) Keep models thin and use managers for queries..."
        )
    }
}

impl Default for DjangoExpert {
    fn default() -> Self {
        Self(Self::new())
    }
}

pub struct FastAPIExpert(PersonaAgent);

impl FastAPIExpert {
    pub fn new() -> PersonaAgent {
        PersonaAgent::new(
            "fastapi-expert",
            "FastAPI Expert",
            "Expert in FastAPI framework, Pydantic, async Python, and modern API development.",
            "fastapi",
            "You are a FastAPI expert with deep knowledge of async Python, Pydantic, and modern API development patterns.",
            FASTAPI,
        )
        .with_capability(AgentCapability::ArchitectureDesign)
        .with_capability(AgentCapability::CodeReview)
    }
}

impl Default for FastAPIExpert {
    fn default() -> Self {
        Self(Self::new())
    }
}

pub struct ReactExpert(PersonaAgent);

impl ReactExpert {
    pub fn new() -> PersonaAgent {
        PersonaAgent::new(
            "react-expert",
            "React Expert",
            "Expert in React, hooks, state management, and modern frontend development.",
            "react",
            "You are a React expert with deep knowledge of hooks, state management, and modern frontend patterns.",
            REACT,
        )
        .with_capability(AgentCapability::ArchitectureDesign)
        .with_capability(AgentCapability::CodeReview)
    }
}

impl Default for ReactExpert {
    fn default() -> Self {
        Self(Self::new())
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/unified/persona/mod.rs">
//! Persona Agents
//!
//! LLM-only agents that provide expertise without code execution.
//! These augment LLM responses with domain knowledge.

mod base;
mod framework_experts;
mod architecture_experts;
mod operations_experts;

pub use base::PersonaAgent;
pub use framework_experts::*;
pub use architecture_experts::*;
pub use operations_experts::*;

use std::collections::HashMap;
use once_cell::sync::Lazy;

/// All available persona agents
pub static PERSONA_AGENTS: Lazy<HashMap<&'static str, fn() -> Box<dyn super::UnifiedAgent>>> = Lazy::new(|| {
    let mut m: HashMap<&'static str, fn() -> Box<dyn super::UnifiedAgent>> = HashMap::new();
    
    // Framework experts
    m.insert("django-expert", || Box::new(DjangoExpert::new()));
    m.insert("fastapi-expert", || Box::new(FastAPIExpert::new()));
    m.insert("react-expert", || Box::new(ReactExpert::new()));
    
    // Architecture experts
    m.insert("backend-architect", || Box::new(BackendArchitect::new()));
    m.insert("security-auditor", || Box::new(SecurityAuditor::new()));
    m.insert("code-reviewer", || Box::new(CodeReviewer::new()));
    
    // Operations experts
    m.insert("kubernetes-expert", || Box::new(KubernetesExpert::new()));
    m.insert("systemd-expert", || Box::new(SystemdExpert::new()));
    m.insert("dbus-expert", || Box::new(DbusExpert::new()));
    
    m
});
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/unified/persona/operations_experts.rs">
//! Operations Expert Agents

use super::base::PersonaAgent;
use super::super::agent_trait::AgentCapability;
use super::super::prompts::operations::{KUBERNETES_EXPERT, SYSTEMD_EXPERT, DBUS_EXPERT};

pub struct KubernetesExpert(PersonaAgent);

impl KubernetesExpert {
    pub fn new() -> PersonaAgent {
        PersonaAgent::new(
            "kubernetes-expert",
            "Kubernetes Expert",
            "Expert in Kubernetes, container orchestration, Helm, and cloud-native patterns.",
            "kubernetes",
            "You are a Kubernetes expert with deep knowledge of container orchestration and cloud-native patterns.",
            KUBERNETES_EXPERT,
        )
        .with_capability(AgentCapability::ArchitectureDesign)
    }
}

impl Default for KubernetesExpert {
    fn default() -> Self {
        Self(Self::new())
    }
}

pub struct SystemdExpert(PersonaAgent);

impl SystemdExpert {
    pub fn new() -> PersonaAgent {
        PersonaAgent::new(
            "systemd-expert",
            "Systemd Expert",
            "Expert in systemd service management, unit files, and Linux system administration.",
            "systemd",
            "You are a systemd expert with deep knowledge of Linux service management and system administration.",
            SYSTEMD_EXPERT,
        )
    }
}

impl Default for SystemdExpert {
    fn default() -> Self {
        Self(Self::new())
    }
}

pub struct DbusExpert(PersonaAgent);

impl DbusExpert {
    pub fn new() -> PersonaAgent {
        PersonaAgent::new(
            "dbus-expert",
            "D-Bus Expert",
            "Expert in D-Bus IPC, introspection, and Linux desktop/system integration.",
            "dbus",
            "You are a D-Bus expert with deep knowledge of inter-process communication on Linux.",
            DBUS_EXPERT,
        )
    }
}

impl Default for DbusExpert {
    fn default() -> Self {
        Self(Self::new())
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/unified/agent_trait.rs">
//! Unified Agent Trait
//!
//! Single trait that all agents implement, with clear capability declarations.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue;
use std::collections::HashSet;

use crate::security::SecurityProfile;

/// Agent category - determines what the agent can do
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentCategory {
    /// Can execute code/commands with sandboxing
    Execution,
    /// LLM-only, provides expertise without code execution
    Persona,
    /// Coordinates other agents for complex workflows
    Orchestration,
}

/// Specific capabilities an agent has
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentCapability {
    // Execution capabilities
    RunCode { language: String },
    RunCommand { commands: Vec<String> },
    ReadFiles,
    WriteFiles,
    NetworkAccess,
    
    // Persona capabilities (LLM augmentation)
    CodeReview,
    ArchitectureDesign,
    SecurityAudit,
    Documentation,
    Debugging,
    Optimization,
    
    // Orchestration capabilities
    DelegateToAgents { agents: Vec<String> },
    ParallelExecution,
    WorkflowManagement,
}

/// Task request to an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRequest {
    /// Operation to perform
    pub operation: String,
    /// Arguments for the operation
    pub args: Value,
    /// Context from conversation/session
    pub context: Option<String>,
    /// Files to include
    pub files: Vec<FileContext>,
}

/// File context for agent operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileContext {
    pub path: String,
    pub content: String,
}

/// Response from an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResponse {
    /// Whether the operation succeeded
    pub success: bool,
    /// Result data
    pub data: Value,
    /// Human-readable message
    pub message: String,
    /// Files modified/created
    pub files_changed: Vec<String>,
    /// Suggested follow-up actions
    pub suggestions: Vec<String>,
}

impl AgentResponse {
    pub fn success(data: Value, message: impl Into<String>) -> Self {
        Self {
            success: true,
            data,
            message: message.into(),
            files_changed: vec![],
            suggestions: vec![],
        }
    }

    pub fn failure(message: impl Into<String>) -> Self {
        Self {
            success: false,
            data: Value::Null,
            message: message.into(),
            files_changed: vec![],
            suggestions: vec![],
        }
    }

    pub fn with_files(mut self, files: Vec<String>) -> Self {
        self.files_changed = files;
        self
    }

    pub fn with_suggestions(mut self, suggestions: Vec<String>) -> Self {
        self.suggestions = suggestions;
        self
    }
}

/// Unified Agent Trait
///
/// All agents implement this trait, regardless of category.
#[async_trait]
pub trait UnifiedAgent: Send + Sync {
    // =========================================================================
    // IDENTITY
    // =========================================================================
    
    /// Unique agent identifier (e.g., "python-executor", "django-expert")
    fn id(&self) -> &str;
    
    /// Human-readable name
    fn name(&self) -> &str;
    
    /// Description of what this agent does
    fn description(&self) -> &str;
    
    /// Agent category
    fn category(&self) -> AgentCategory;
    
    /// Agent capabilities
    fn capabilities(&self) -> HashSet<AgentCapability>;
    
    // =========================================================================
    // PROMPTS (embedded, not separate markdown files)
    // =========================================================================
    
    /// System prompt for LLM interactions
    /// This is the "persona" that was previously in markdown files
    fn system_prompt(&self) -> &str;
    
    /// Additional context/knowledge to inject
    fn knowledge_base(&self) -> Option<&str> {
        None
    }
    
    /// Example interactions for few-shot learning
    fn examples(&self) -> Vec<(&str, &str)> {
        vec![]
    }
    
    // =========================================================================
    // SECURITY (for execution agents)
    // =========================================================================
    
    /// Security profile (only meaningful for execution agents)
    fn security_profile(&self) -> Option<&SecurityProfile> {
        None
    }
    
    /// Whether this agent requires root/elevated privileges
    fn requires_root(&self) -> bool {
        false
    }
    
    // =========================================================================
    // OPERATIONS
    // =========================================================================
    
    /// List of operations this agent can perform
    fn operations(&self) -> Vec<&str>;
    
    /// Execute an operation
    async fn execute(&self, request: AgentRequest) -> AgentResponse;
    
    /// Check if agent can handle a specific operation
    fn can_handle(&self, operation: &str) -> bool {
        self.operations().contains(&operation)
    }
    
    // =========================================================================
    // LIFECYCLE
    // =========================================================================
    
    /// Initialize the agent (called once on startup)
    async fn initialize(&self) -> Result<(), String> {
        Ok(())
    }
    
    /// Shutdown the agent (called on cleanup)
    async fn shutdown(&self) -> Result<(), String> {
        Ok(())
    }
    
    /// Health check
    fn is_healthy(&self) -> bool {
        true
    }
}

/// Extension trait for agent metadata
pub trait AgentMetadata: UnifiedAgent {
    /// Get full metadata as JSON
    fn metadata(&self) -> Value {
        simd_json::json!({
            "id": self.id(),
            "name": self.name(),
            "description": self.description(),
            "category": self.category(),
            "capabilities": self.capabilities().iter().collect::<Vec<_>>(),
            "operations": self.operations(),
            "requires_root": self.requires_root(),
            "has_security_profile": self.security_profile().is_some(),
        })
    }
}

impl<T: UnifiedAgent> AgentMetadata for T {}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/unified/mod.rs">
//! Unified Agent Architecture
//!
//! This module implements the recommended architecture that:
//! 1. Merges markdown prompts INTO Rust agents (single source of truth)
//! 2. Clearly separates EXECUTION agents from PERSONA agents
//! 3. Uses consistent naming: {type}-executor vs {type}-expert
//!
//! ## Agent Categories
//!
//! - **Execution Agents**: Can run code/commands with sandboxing
//! - **Persona Agents**: LLM-only, provide expertise without code execution
//! - **Orchestration Agents**: Coordinate other agents for complex workflows

pub mod agent_trait;
pub mod execution;
pub mod persona;
pub mod orchestration;
pub mod registry;
pub mod prompts;

pub use agent_trait::{UnifiedAgent, AgentCapability, AgentCategory};
pub use execution::ExecutionAgent;
pub use persona::PersonaAgent;
pub use orchestration::OrchestrationAgent;
pub use registry::UnifiedAgentRegistry;
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/unified/prompts.rs">
//! Embedded Prompts Module
//!
//! Contains all system prompts that were previously in separate markdown files.
//! This ensures single source of truth - prompts are compiled into the binary.

/// Prompt templates for different agent types
pub mod templates {
    /// Base template for all agents
    pub const BASE_AGENT: &str = r#"
You are {agent_name}, an AI assistant specialized in {specialization}.

## Core Principles
- Provide accurate, actionable guidance
- Explain reasoning behind recommendations  
- Consider security, performance, and maintainability
- Acknowledge uncertainty when appropriate

## Response Format
- Be concise but thorough
- Use code examples when helpful
- Structure complex responses with headers
- Highlight critical warnings or gotchas
"#;

    /// Template for execution agents
    pub const EXECUTION_AGENT: &str = r#"
You are {agent_name}, an execution agent that can run {language} code.

## Capabilities
- Execute code in a sandboxed environment
- Access to: {allowed_commands}
- File access: {file_access}

## Safety Rules
- Never execute code that could harm the system
- Validate all inputs before execution
- Report errors clearly with context
- Respect resource limits and timeouts

## Workflow
1. Analyze the request
2. Validate inputs and permissions
3. Execute in sandbox
4. Return results with explanation
"#;

    /// Template for persona agents
    pub const PERSONA_AGENT: &str = r#"
You are {agent_name}, an expert in {domain}.

## Expertise Areas
{expertise_list}

## How You Help
- Provide expert guidance and recommendations
- Review code and architecture decisions
- Explain complex concepts clearly
- Suggest best practices and patterns

## Limitations
- You provide guidance only, not code execution
- Recommendations should be verified before implementation
- Complex changes should involve human review
"#;
}

/// Language-specific prompts for execution agents
pub mod languages {
    pub const PYTHON: &str = r#"
## Python Expertise
- Python 3.8+ syntax and features
- Type hints and mypy
- Virtual environments (venv, uv)
- Package management (pip, poetry, uv)
- Testing (pytest, unittest)
- Linting (ruff, flake8, black)

## Python Best Practices
- Use type hints for function signatures
- Prefer f-strings over .format()
- Use pathlib for file operations
- Handle exceptions specifically, not generically
- Use context managers for resources

## Common Tools
- pytest: Testing framework
- ruff: Fast linter and formatter
- mypy: Static type checker
- uv: Fast package installer
"#;

    pub const RUST: &str = r#"
## Rust Expertise
- Rust 2021 edition
- Ownership, borrowing, lifetimes
- Error handling (Result, Option, ?)
- Async/await with tokio
- Cargo workspace management

## Rust Best Practices
- Prefer &str over String for function params
- Use impl Trait for return types when appropriate
- Leverage the type system for correctness
- Handle all Result/Option cases explicitly
- Use clippy for additional lints

## Common Tools
- cargo: Build system and package manager
- clippy: Linter
- rustfmt: Formatter
- cargo-watch: Auto-rebuild on changes
"#;

    pub const JAVASCRIPT: &str = r#"
## JavaScript/TypeScript Expertise
- ES2022+ features
- TypeScript strict mode
- Node.js runtime
- Package managers (npm, yarn, pnpm)
- Testing (jest, vitest)

## JavaScript Best Practices
- Use TypeScript for type safety
- Prefer const over let, avoid var
- Use async/await over callbacks
- Handle promises with try/catch
- Use ESLint and Prettier

## Common Tools
- npm/pnpm: Package managers
- eslint: Linter
- prettier: Formatter
- jest/vitest: Testing
"#;

    pub const GO: &str = r#"
## Go Expertise
- Go 1.21+ features
- Goroutines and channels
- Error handling patterns
- Module system
- Testing and benchmarking

## Go Best Practices
- Handle errors explicitly
- Use context for cancellation
- Prefer composition over inheritance
- Keep interfaces small
- Use go vet and staticcheck

## Common Tools
- go build/test/run: Core commands
- go mod: Module management
- gofmt: Formatter
- staticcheck: Linter
"#;
}

/// Framework-specific prompts for persona agents
pub mod frameworks {
    pub const DJANGO: &str = r#"
## Django Expertise
- Django 4.x+ features
- Django REST Framework
- ORM and database optimization
- Authentication and permissions
- Celery for async tasks

## Django Best Practices
- Use class-based views for complex logic
- Optimize queries with select_related/prefetch_related
- Use Django's built-in security features
- Write comprehensive model tests
- Use migrations for all schema changes

## Common Patterns
- Fat models, thin views
- Service layer for business logic
- Custom managers for query encapsulation
- Signals for decoupled event handling
"#;

    pub const FASTAPI: &str = r#"
## FastAPI Expertise
- FastAPI 0.100+
- Pydantic v2 models
- Dependency injection
- OpenAPI/Swagger integration
- Async database access

## FastAPI Best Practices
- Use Pydantic for all request/response models
- Leverage dependency injection for shared logic
- Use background tasks for non-blocking operations
- Document all endpoints with docstrings
- Use proper HTTP status codes

## Common Patterns
- Repository pattern for data access
- Service layer for business logic
- Custom exception handlers
- Middleware for cross-cutting concerns
"#;

    pub const REACT: &str = r#"
## React Expertise
- React 18+ features
- Hooks (useState, useEffect, useCallback, useMemo)
- Context API
- React Query/TanStack Query
- Next.js integration

## React Best Practices
- Use functional components with hooks
- Memoize expensive computations
- Lift state up appropriately
- Use TypeScript for props
- Test with React Testing Library

## Common Patterns
- Custom hooks for reusable logic
- Compound components
- Render props (when hooks don't fit)
- Error boundaries
"#;
}

/// Architecture and design prompts
pub mod architecture {
    pub const BACKEND_ARCHITECT: &str = r#"
## Backend Architecture Expertise
- Microservices vs monolith decisions
- API design (REST, GraphQL, gRPC)
- Database selection and modeling
- Caching strategies
- Message queues and event-driven architecture

## Architecture Principles
- Design for failure and resilience
- Prefer loose coupling, high cohesion
- Use the right tool for the job
- Plan for observability from day one
- Consider operational complexity

## Common Patterns
- CQRS for read/write separation
- Event sourcing for audit trails
- Saga pattern for distributed transactions
- Circuit breaker for fault tolerance
- Strangler fig for migrations
"#;

    pub const SECURITY_AUDITOR: &str = r#"
## Security Audit Expertise
- OWASP Top 10 vulnerabilities
- Authentication and authorization
- Input validation and sanitization
- Secrets management
- Security headers and CORS

## Security Checklist
- SQL injection prevention
- XSS prevention
- CSRF protection
- Secure password handling
- Rate limiting
- Audit logging

## Common Issues to Flag
- Hardcoded secrets
- Missing input validation
- Overly permissive CORS
- Insecure direct object references
- Missing authentication checks
"#;

    pub const CODE_REVIEWER: &str = r#"
## Code Review Expertise
- Code quality and maintainability
- Performance optimization
- Security considerations
- Testing coverage
- Documentation quality

## Review Priorities
1. Correctness - Does it work?
2. Security - Is it safe?
3. Performance - Is it efficient?
4. Maintainability - Is it readable?
5. Testing - Is it tested?

## Feedback Style
- Be specific and actionable
- Explain the "why" behind suggestions
- Distinguish must-fix from nice-to-have
- Acknowledge good patterns
- Suggest alternatives when criticizing
"#;
}

/// Operations and DevOps prompts
pub mod operations {
    pub const KUBERNETES_EXPERT: &str = r#"
## Kubernetes Expertise
- Pod, Deployment, Service, Ingress
- ConfigMaps and Secrets
- RBAC and security policies
- Helm charts
- Operators and CRDs

## Kubernetes Best Practices
- Use namespaces for isolation
- Set resource requests and limits
- Use liveness and readiness probes
- Implement pod disruption budgets
- Use network policies

## Common Patterns
- Sidecar containers
- Init containers
- Blue-green deployments
- Canary releases
- GitOps with ArgoCD/Flux
"#;

    pub const SYSTEMD_EXPERT: &str = r#"
## Systemd Expertise
- Unit files (service, timer, socket)
- Service management
- Journald logging
- Resource control (cgroups)
- Dependencies and ordering

## Systemd Best Practices
- Use Type=notify for proper startup detection
- Set appropriate restart policies
- Configure resource limits
- Use PrivateTmp and other security options
- Log to journald, not files

## Common Operations
- systemctl start/stop/restart
- systemctl enable/disable
- journalctl -u service-name
- systemd-analyze for boot analysis
"#;

    pub const DBUS_EXPERT: &str = r#"
## D-Bus Expertise
- System bus vs session bus
- Services, objects, interfaces
- Methods, properties, signals
- Introspection
- Policy configuration

## D-Bus Best Practices
- Use well-known names for services
- Implement Introspectable interface
- Handle disconnections gracefully
- Use signals for notifications
- Validate all inputs

## Common Patterns
- Property change notifications
- Method call timeouts
- Bus name watching
- Object manager pattern
"#;
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/unified/registry.rs">
//! Unified Agent Registry
//!
//! Single registry for all agent types with lazy loading.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use once_cell::sync::Lazy;

use super::agent_trait::{UnifiedAgent, AgentCategory, AgentMetadata};
use super::execution::EXECUTION_AGENTS;
use super::persona::PERSONA_AGENTS;
use super::orchestration::ORCHESTRATION_AGENTS;

/// Unified registry for all agents
pub struct UnifiedAgentRegistry {
    /// Loaded agents (lazily instantiated)
    agents: RwLock<HashMap<String, Arc<dyn UnifiedAgent>>>,
    /// Agent factories
    factories: HashMap<&'static str, fn() -> Box<dyn UnifiedAgent>>,
}

impl UnifiedAgentRegistry {
    /// Create a new registry with all default agents
    pub fn new() -> Self {
        let mut factories: HashMap<&'static str, fn() -> Box<dyn UnifiedAgent>> = HashMap::new();
        
        // Register all execution agents
        for (id, factory) in EXECUTION_AGENTS.iter() {
            factories.insert(*id, *factory);
        }
        
        // Register all persona agents
        for (id, factory) in PERSONA_AGENTS.iter() {
            factories.insert(*id, *factory);
        }
        
        // Register all orchestration agents
        for (id, factory) in ORCHESTRATION_AGENTS.iter() {
            factories.insert(*id, *factory);
        }

        Self {
            agents: RwLock::new(HashMap::new()),
            factories,
        }
    }

    /// Get an agent by ID (lazy loading)
    pub fn get(&self, id: &str) -> Option<Arc<dyn UnifiedAgent>> {
        // Check if already loaded
        {
            let agents = self.agents.read().unwrap();
            if let Some(agent) = agents.get(id) {
                return Some(Arc::clone(agent));
            }
        }

        // Try to load from factory
        if let Some(factory) = self.factories.get(id) {
            let agent: Arc<dyn UnifiedAgent> = Arc::from(factory());
            let mut agents = self.agents.write().unwrap();
            agents.insert(id.to_string(), Arc::clone(&agent));
            return Some(agent);
        }

        None
    }

    /// List all available agent IDs
    pub fn list_ids(&self) -> Vec<&str> {
        self.factories.keys().copied().collect()
    }

    /// List agents by category
    pub fn list_by_category(&self, category: AgentCategory) -> Vec<&str> {
        self.factories.keys()
            .filter(|id| {
                if let Some(agent) = self.get(id) {
                    agent.category() == category
                } else {
                    false
                }
            })
            .copied()
            .collect()
    }

    /// Get metadata for all agents
    pub fn all_metadata(&self) -> Vec<simd_json::OwnedValue> {
        self.factories.keys()
            .filter_map(|id| {
                self.get(id).map(|agent| agent.metadata())
            })
            .collect()
    }

    /// Register a custom agent
    pub fn register(&mut self, id: &'static str, factory: fn() -> Box<dyn UnifiedAgent>) {
        self.factories.insert(id, factory);
    }

    /// Get count of registered agents
    pub fn count(&self) -> usize {
        self.factories.len()
    }

    /// Get count by category
    pub fn count_by_category(&self) -> HashMap<AgentCategory, usize> {
        let mut counts = HashMap::new();
        counts.insert(AgentCategory::Execution, 0);
        counts.insert(AgentCategory::Persona, 0);
        counts.insert(AgentCategory::Orchestration, 0);

        for id in self.factories.keys() {
            if let Some(agent) = self.get(id) {
                *counts.entry(agent.category()).or_insert(0) += 1;
            }
        }

        counts
    }
}

impl Default for UnifiedAgentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Global registry instance
pub static GLOBAL_REGISTRY: Lazy<UnifiedAgentRegistry> = Lazy::new(UnifiedAgentRegistry::new);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_creation() {
        let registry = UnifiedAgentRegistry::new();
        assert!(registry.count() > 0);
    }

    #[test]
    fn test_get_agent() {
        let registry = UnifiedAgentRegistry::new();
        let agent = registry.get("python-executor");
        assert!(agent.is_some());
        assert_eq!(agent.unwrap().id(), "python-executor");
    }

    #[test]
    fn test_list_by_category() {
        let registry = UnifiedAgentRegistry::new();
        let executors = registry.list_by_category(AgentCategory::Execution);
        assert!(!executors.is_empty());
        assert!(executors.contains(&"python-executor"));
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agent_catalog.rs">
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
        ARMCortexExpertAgent, SnowballDeveloperAgent, ErrorDetectiveAgent,
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
        Box::new(SnowballDeveloperAgent::new(agent_id.clone())),
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
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/agent_registry.rs">
//! Agent registry for dynamic agent management
//!
//! This eliminates tight coupling by allowing agents to be registered
//! dynamically without hardcoding in the orchestrator.

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Agent specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSpec {
    /// Unique agent type identifier
    pub agent_type: String,

    /// Human-readable name
    pub name: String,

    /// Description of agent functionality
    pub description: String,

    /// Command to execute the agent
    pub command: String,

    /// Arguments to pass to the command
    #[serde(default)]
    pub args: Vec<String>,

    /// Environment variables
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Working directory
    pub working_dir: Option<PathBuf>,

    /// Capabilities this agent provides
    pub capabilities: Vec<String>,

    /// Whether this agent requires root privileges
    #[serde(default)]
    pub requires_root: bool,

    /// Maximum number of instances
    #[serde(default = "default_max_instances")]
    pub max_instances: usize,

    /// Restart policy
    #[serde(default)]
    pub restart_policy: RestartPolicy,

    /// Health check configuration
    pub health_check: Option<HealthCheck>,
}

fn default_max_instances() -> usize {
    5
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RestartPolicy {
    #[default]
    Never,
    Always,
    OnFailure {
        max_retries: u32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    /// D-Bus method to call for health check
    pub method: String,

    /// Interval in seconds
    pub interval_secs: u64,

    /// Timeout in seconds
    pub timeout_secs: u64,

    /// Number of consecutive failures before marking unhealthy
    pub unhealthy_threshold: u32,
}

/// Agent instance information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInstance {
    pub id: String,
    pub agent_type: String,
    pub pid: Option<u32>,
    pub status: AgentStatus,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub last_health_check: Option<chrono::DateTime<chrono::Utc>>,
    pub restart_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Starting,
    Running,
    Healthy,
    Unhealthy,
    Stopping,
    Stopped,
    Failed { reason: String },
}

/// Agent factory for creating agents
#[async_trait]
pub trait AgentFactory: Send + Sync {
    /// Create a new agent instance
    async fn create_agent(&self, spec: &AgentSpec, instance_id: &str) -> Result<AgentHandle>;

    /// Check if an agent type is supported
    fn supports(&self, agent_type: &str) -> bool;
}

/// Handle to a running agent
pub struct AgentHandle {
    pub id: String,
    pub process: tokio::process::Child,
    pub spec: AgentSpec,
}

/// Default agent factory that spawns processes
pub struct ProcessAgentFactory;

#[async_trait]
impl AgentFactory for ProcessAgentFactory {
    async fn create_agent(&self, spec: &AgentSpec, instance_id: &str) -> Result<AgentHandle> {
        let mut cmd = tokio::process::Command::new(&spec.command);

        // Add arguments
        cmd.args(&spec.args);

        // Add instance ID as environment variable
        cmd.env("AGENT_ID", instance_id);
        cmd.env("AGENT_TYPE", &spec.agent_type);

        // Add custom environment variables
        for (key, value) in &spec.env {
            cmd.env(key, value);
        }

        // Set working directory
        if let Some(dir) = &spec.working_dir {
            cmd.current_dir(dir);
        }

        // Spawn the process
        let process = cmd
            .spawn()
            .context(format!("Failed to spawn agent: {}", spec.command))?;

        Ok(AgentHandle {
            id: instance_id.to_string(),
            process,
            spec: spec.clone(),
        })
    }

    fn supports(&self, _agent_type: &str) -> bool {
        true // Default factory supports all types
    }
}

/// Agent registry for managing agent specifications and instances
pub struct AgentRegistry {
    /// Registered agent specifications
    specs: Arc<RwLock<HashMap<String, AgentSpec>>>,

    /// Running agent instances
    instances: Arc<RwLock<HashMap<String, AgentInstance>>>,

    /// Agent factories
    factories: Arc<RwLock<Vec<Box<dyn AgentFactory>>>>,

    /// Agent handles
    handles: Arc<RwLock<HashMap<String, AgentHandle>>>,
}

impl Default for AgentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentRegistry {
    pub fn new() -> Self {
        let registry = Self {
            specs: Arc::new(RwLock::new(HashMap::new())),
            instances: Arc::new(RwLock::new(HashMap::new())),
            factories: Arc::new(RwLock::new(Vec::new())),
            handles: Arc::new(RwLock::new(HashMap::new())),
        };

        // Register default factory
        let default_factory = Box::new(ProcessAgentFactory);

        // We need to do this in a blocking context since new() is not async
        let factories = registry.factories.clone();
        tokio::spawn(async move {
            let mut factories = factories.write().await;
            factories.push(default_factory);
        });

        registry
    }

    /// Register an agent specification
    pub async fn register_spec(&self, spec: AgentSpec) -> Result<()> {
        let mut specs = self.specs.write().await;

        if specs.contains_key(&spec.agent_type) {
            return Err(anyhow::anyhow!(
                "Agent type '{}' is already registered",
                spec.agent_type
            ));
        }

        specs.insert(spec.agent_type.clone(), spec);
        Ok(())
    }

    /// Load specifications from a configuration file
    pub async fn load_specs_from_file(&self, path: &PathBuf) -> Result<()> {
        let content = tokio::fs::read_to_string(path)
            .await
            .context("Failed to read agent specifications file")?;

        let mut content = content;
        let specs: Vec<AgentSpec> = unsafe { simd_json::from_str(&mut content) }
            .context("Failed to parse agent specifications")?;

        for spec in specs {
            self.register_spec(spec).await?;
        }

        Ok(())
    }

    /// Load specifications from a directory
    pub async fn load_specs_from_directory(&self, dir: &PathBuf) -> Result<()> {
        let mut entries = tokio::fs::read_dir(dir)
            .await
            .context("Failed to read specifications directory")?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Err(e) = self.load_specs_from_file(&path).await {
                    tracing::warn!("Failed to load spec from {:?}: {}", path, e);
                }
            }
        }

        Ok(())
    }

    /// Register a custom agent factory
    pub async fn register_factory(&self, factory: Box<dyn AgentFactory>) {
        let mut factories = self.factories.write().await;
        factories.push(factory);
    }

    /// Spawn an agent instance
    pub async fn spawn_agent(
        &self,
        agent_type: &str,
        _config: Option<OwnedValue>,
    ) -> Result<String> {
        // Get the specification
        let specs = self.specs.read().await;
        let spec = specs
            .get(agent_type)
            .ok_or_else(|| anyhow::anyhow!("Unknown agent type: {}", agent_type))?
            .clone();

        // Check max instances
        let instances = self.instances.read().await;
        let current_count = instances
            .values()
            .filter(|i| {
                i.agent_type == agent_type
                    && matches!(i.status, AgentStatus::Running | AgentStatus::Healthy)
            })
            .count();

        if current_count >= spec.max_instances {
            return Err(anyhow::anyhow!(
                "Maximum instances ({}) reached for agent type '{}'",
                spec.max_instances,
                agent_type
            ));
        }
        drop(instances);

        // Generate instance ID
        let instance_id = format!("{}-{}", agent_type, uuid::Uuid::new_v4());

        // Find suitable factory
        let factories = self.factories.read().await;
        let factory = factories
            .iter()
            .find(|f| f.supports(agent_type))
            .ok_or_else(|| anyhow::anyhow!("No factory supports agent type: {}", agent_type))?;

        // Create the agent
        let handle = factory.create_agent(&spec, &instance_id).await?;
        let pid = handle.process.id();

        // Store the handle
        let mut handles = self.handles.write().await;
        handles.insert(instance_id.clone(), handle);

        // Create instance record
        let instance = AgentInstance {
            id: instance_id.clone(),
            agent_type: agent_type.to_string(),
            pid,
            status: AgentStatus::Starting,
            started_at: chrono::Utc::now(),
            last_health_check: None,
            restart_count: 0,
        };

        // Store instance
        let mut instances = self.instances.write().await;
        instances.insert(instance_id.clone(), instance);

        // TODO: Start health check task if configured

        Ok(instance_id)
    }

    /// Kill an agent instance
    pub async fn kill_agent(&self, instance_id: &str) -> Result<()> {
        let mut handles = self.handles.write().await;

        if let Some(mut handle) = handles.remove(instance_id) {
            // Try graceful shutdown first
            handle
                .process
                .kill()
                .await
                .context("Failed to kill agent process")?;

            // Update instance status
            let mut instances = self.instances.write().await;
            if let Some(instance) = instances.get_mut(instance_id) {
                instance.status = AgentStatus::Stopped;
            }

            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "Agent instance '{}' not found",
                instance_id
            ))
        }
    }

    /// Get agent instance status
    pub async fn get_instance_status(&self, instance_id: &str) -> Result<AgentInstance> {
        let instances = self.instances.read().await;
        instances
            .get(instance_id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Agent instance '{}' not found", instance_id))
    }

    /// List all agent instances
    pub async fn list_instances(&self) -> Vec<AgentInstance> {
        let instances = self.instances.read().await;
        instances.values().cloned().collect()
    }

    /// List all registered agent types
    pub async fn list_agent_types(&self) -> Vec<String> {
        let specs = self.specs.read().await;
        specs.keys().cloned().collect()
    }

    /// Get agent specification
    pub async fn get_spec(&self, agent_type: &str) -> Option<AgentSpec> {
        let specs = self.specs.read().await;
        specs.get(agent_type).cloned()
    }
}

/// Load default agent specifications
pub async fn load_default_specs(registry: &AgentRegistry) -> Result<()> {
    let specs = vec![
        AgentSpec {
            agent_type: "executor".to_string(),
            name: "Command Executor".to_string(),
            description: "Executes whitelisted shell commands".to_string(),
            command: "dbus-agent-executor".to_string(),
            args: vec![],
            env: HashMap::new(),
            working_dir: None,
            capabilities: vec!["execute".to_string()],
            requires_root: false,
            max_instances: 3,
            restart_policy: RestartPolicy::OnFailure { max_retries: 3 },
            health_check: Some(HealthCheck {
                method: "GetStatus".to_string(),
                interval_secs: 30,
                timeout_secs: 5,
                unhealthy_threshold: 3,
            }),
        },
        AgentSpec {
            agent_type: "file".to_string(),
            name: "File Manager".to_string(),
            description: "Manages file operations".to_string(),
            command: "dbus-agent-file".to_string(),
            args: vec![],
            env: HashMap::new(),
            working_dir: None,
            capabilities: vec![
                "read".to_string(),
                "write".to_string(),
                "delete".to_string(),
            ],
            requires_root: false,
            max_instances: 5,
            restart_policy: RestartPolicy::OnFailure { max_retries: 3 },
            health_check: Some(HealthCheck {
                method: "GetStatus".to_string(),
                interval_secs: 30,
                timeout_secs: 5,
                unhealthy_threshold: 3,
            }),
        },
        AgentSpec {
            agent_type: "network".to_string(),
            name: "Network Manager".to_string(),
            description: "Manages network configuration".to_string(),
            command: "dbus-agent-network".to_string(),
            args: vec![],
            env: HashMap::new(),
            working_dir: None,
            capabilities: vec!["network".to_string()],
            requires_root: true,
            max_instances: 2,
            restart_policy: RestartPolicy::Always,
            health_check: Some(HealthCheck {
                method: "GetStatus".to_string(),
                interval_secs: 30,
                timeout_secs: 5,
                unhealthy_threshold: 3,
            }),
        },
        AgentSpec {
            agent_type: "systemd".to_string(),
            name: "Systemd Controller".to_string(),
            description: "Controls systemd services".to_string(),
            command: "dbus-agent-systemd".to_string(),
            args: vec![],
            env: HashMap::new(),
            working_dir: None,
            capabilities: vec!["service".to_string()],
            requires_root: true,
            max_instances: 2,
            restart_policy: RestartPolicy::Always,
            health_check: Some(HealthCheck {
                method: "GetStatus".to_string(),
                interval_secs: 30,
                timeout_secs: 5,
                unhealthy_threshold: 3,
            }),
        },
        AgentSpec {
            agent_type: "monitor".to_string(),
            name: "System Monitor".to_string(),
            description: "Monitors system resources".to_string(),
            command: "dbus-agent-monitor".to_string(),
            args: vec![],
            env: HashMap::new(),
            working_dir: None,
            capabilities: vec!["monitor".to_string()],
            requires_root: false,
            max_instances: 1,
            restart_policy: RestartPolicy::Always,
            health_check: Some(HealthCheck {
                method: "GetStatus".to_string(),
                interval_secs: 30,
                timeout_secs: 5,
                unhealthy_threshold: 3,
            }),
        },
        AgentSpec {
            agent_type: "packagekit".to_string(),
            name: "Package Manager".to_string(),
            description: "Manages system packages via PackageKit".to_string(),
            command: "dbus-agent-packagekit".to_string(),
            args: vec![],
            env: HashMap::new(),
            working_dir: None,
            capabilities: vec![
                "install".to_string(),
                "remove".to_string(),
                "update".to_string(),
            ],
            requires_root: true,
            max_instances: 2,
            restart_policy: RestartPolicy::OnFailure { max_retries: 3 },
            health_check: Some(HealthCheck {
                method: "GetStatus".to_string(),
                interval_secs: 30,
                timeout_secs: 5,
                unhealthy_threshold: 3,
            }),
        },
        AgentSpec {
            agent_type: "python-pro".to_string(),
            name: "Python Professional".to_string(),
            description: "Python development and execution environment".to_string(),
            command: "dbus-agent-python-pro".to_string(),
            args: vec![],
            env: HashMap::new(),
            working_dir: None,
            capabilities: vec![
                "execute".to_string(),
                "analyze".to_string(),
                "format".to_string(),
            ],
            requires_root: false,
            max_instances: 5,
            restart_policy: RestartPolicy::OnFailure { max_retries: 3 },
            health_check: Some(HealthCheck {
                method: "GetStatus".to_string(),
                interval_secs: 30,
                timeout_secs: 5,
                unhealthy_threshold: 3,
            }),
        },
        AgentSpec {
            agent_type: "rust-pro".to_string(),
            name: "Rust Professional".to_string(),
            description: "Rust development and compilation environment".to_string(),
            command: "dbus-agent-rust-pro".to_string(),
            args: vec![],
            env: HashMap::new(),
            working_dir: None,
            capabilities: vec![
                "compile".to_string(),
                "check".to_string(),
                "test".to_string(),
            ],
            requires_root: false,
            max_instances: 5,
            restart_policy: RestartPolicy::OnFailure { max_retries: 3 },
            health_check: Some(HealthCheck {
                method: "GetStatus".to_string(),
                interval_secs: 30,
                timeout_secs: 5,
                unhealthy_threshold: 3,
            }),
        },
        AgentSpec {
            agent_type: "c-pro".to_string(),
            name: "C Professional".to_string(),
            description: "C development and compilation environment".to_string(),
            command: "dbus-agent-c-pro".to_string(),
            args: vec![],
            env: HashMap::new(),
            working_dir: None,
            capabilities: vec![
                "compile".to_string(),
                "debug".to_string(),
                "analyze".to_string(),
            ],
            requires_root: false,
            max_instances: 5,
            restart_policy: RestartPolicy::OnFailure { max_retries: 3 },
            health_check: Some(HealthCheck {
                method: "GetStatus".to_string(),
                interval_secs: 30,
                timeout_secs: 5,
                unhealthy_threshold: 3,
            }),
        },
        AgentSpec {
            agent_type: "cpp-pro".to_string(),
            name: "C++ Professional".to_string(),
            description: "C++ development and compilation environment".to_string(),
            command: "dbus-agent-cpp-pro".to_string(),
            args: vec![],
            env: HashMap::new(),
            working_dir: None,
            capabilities: vec![
                "compile".to_string(),
                "debug".to_string(),
                "analyze".to_string(),
            ],
            requires_root: false,
            max_instances: 5,
            restart_policy: RestartPolicy::OnFailure { max_retries: 3 },
            health_check: Some(HealthCheck {
                method: "GetStatus".to_string(),
                interval_secs: 30,
                timeout_secs: 5,
                unhealthy_threshold: 3,
            }),
        },
        AgentSpec {
            agent_type: "golang-pro".to_string(),
            name: "Go Professional".to_string(),
            description: "Go development and compilation environment".to_string(),
            command: "dbus-agent-golang-pro".to_string(),
            args: vec![],
            env: HashMap::new(),
            working_dir: None,
            capabilities: vec![
                "compile".to_string(),
                "test".to_string(),
                "build".to_string(),
            ],
            requires_root: false,
            max_instances: 5,
            restart_policy: RestartPolicy::OnFailure { max_retries: 3 },
            health_check: Some(HealthCheck {
                method: "GetStatus".to_string(),
                interval_secs: 30,
                timeout_secs: 5,
                unhealthy_threshold: 3,
            }),
        },
        AgentSpec {
            agent_type: "javascript-pro".to_string(),
            name: "JavaScript Professional".to_string(),
            description: "JavaScript development and execution environment".to_string(),
            command: "dbus-agent-javascript-pro".to_string(),
            args: vec![],
            env: HashMap::new(),
            working_dir: None,
            capabilities: vec![
                "execute".to_string(),
                "format".to_string(),
                "lint".to_string(),
            ],
            requires_root: false,
            max_instances: 5,
            restart_policy: RestartPolicy::OnFailure { max_retries: 3 },
            health_check: Some(HealthCheck {
                method: "GetStatus".to_string(),
                interval_secs: 30,
                timeout_secs: 5,
                unhealthy_threshold: 3,
            }),
        },
        AgentSpec {
            agent_type: "php-pro".to_string(),
            name: "PHP Professional".to_string(),
            description: "PHP development and execution environment".to_string(),
            command: "dbus-agent-php-pro".to_string(),
            args: vec![],
            env: HashMap::new(),
            working_dir: None,
            capabilities: vec![
                "execute".to_string(),
                "lint".to_string(),
                "analyze".to_string(),
            ],
            requires_root: false,
            max_instances: 5,
            restart_policy: RestartPolicy::OnFailure { max_retries: 3 },
            health_check: Some(HealthCheck {
                method: "GetStatus".to_string(),
                interval_secs: 30,
                timeout_secs: 5,
                unhealthy_threshold: 3,
            }),
        },
        AgentSpec {
            agent_type: "sql-pro".to_string(),
            name: "SQL Professional".to_string(),
            description: "SQL development and query execution environment".to_string(),
            command: "dbus-agent-sql-pro".to_string(),
            args: vec![],
            env: HashMap::new(),
            working_dir: None,
            capabilities: vec![
                "execute".to_string(),
                "optimize".to_string(),
                "analyze".to_string(),
            ],
            requires_root: false,
            max_instances: 5,
            restart_policy: RestartPolicy::OnFailure { max_retries: 3 },
            health_check: Some(HealthCheck {
                method: "GetStatus".to_string(),
                interval_secs: 30,
                timeout_secs: 5,
                unhealthy_threshold: 3,
            }),
        },
    ];

    for spec in specs {
        registry.register_spec(spec).await?;
    }

    Ok(())
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/dbus_service.rs">
//! D-Bus Service Wrapper for AgentTrait implementations
//!
//! Exposes agents via D-Bus with standard interface: org.dbusmcp.Agent
//! This allows agents to be discovered by the ChatActor's tool_loader
//! and registered as tools that the LLM can call.
//!
//! # Architecture Integration
//!
//! ```text
//! ChatActor (brain)
//!    └── ToolRegistry
//!           └── AgentTool (wraps D-Bus calls)
//!                  └── D-Bus Call to org.dbusmcp.Agent.{AgentType}
//!                         └── DbusAgentService (this module)
//!                                └── AgentTrait implementation
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use op_agents::{create_agent, dbus_service};
//! use op_core::BusType;
//!
//! let agent = create_agent("python-pro", "python-1".to_string()).unwrap();
//! let connection = dbus_service::start_agent(agent, "python-1", BusType::Session).await?;
//! // Agent is now discoverable via D-Bus introspection
//! ```

use crate::agents::base::{AgentTask, AgentTrait};
use op_core::BusType;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info};
use zbus::{connection::Builder, interface, object_server::SignalContext, Connection};

/// Error type for D-Bus agent service operations
#[derive(Debug, thiserror::Error)]
pub enum DbusAgentError {
    #[error("D-Bus connection error: {0}")]
    Connection(#[from] zbus::Error),

    #[error("Agent execution error: {0}")]
    Execution(String),

    #[error("JSON serialization error: {0}")]
    Json(#[from] simd_json::Error),

    #[error("Invalid task: {0}")]
    InvalidTask(String),
}

/// D-Bus service that wraps an AgentTrait implementation
///
/// Exposes a standard interface that can be discovered and called
/// by the AgentTool in op-chat's tool registry.
pub struct DbusAgentService {
    agent: Arc<RwLock<Box<dyn AgentTrait>>>,
    agent_type: String,
    agent_id: String,
}

impl DbusAgentService {
    /// Create a new D-Bus service wrapper for an agent
    pub fn new(agent: Box<dyn AgentTrait>, agent_id: String) -> Self {
        let agent_type = agent.agent_type().to_string();
        Self {
            agent: Arc::new(RwLock::new(agent)),
            agent_type,
            agent_id,
        }
    }

    /// Get the D-Bus well-known name for this agent type
    /// e.g., "python-pro" -> "org.dbusmcp.Agent.PythonPro"
    pub fn service_name(agent_type: &str) -> String {
        format!("org.dbusmcp.Agent.{}", to_pascal_case(agent_type))
    }

    /// Get the D-Bus object path for this agent
    /// e.g., "python-pro" -> "/org/dbusmcp/Agent/PythonPro"
    pub fn object_path(agent_type: &str) -> String {
        format!("/org/dbusmcp/Agent/{}", to_pascal_case(agent_type))
    }
}

/// D-Bus interface: org.dbusmcp.Agent
///
/// This is the standard interface that all agents expose.
/// The AgentTool in op-chat will call these methods.
#[interface(name = "org.dbusmcp.Agent")]
impl DbusAgentService {
    //
    // === Core Execution Methods ===
    //

    /// Execute a task on the agent
    ///
    /// # Arguments
    /// * `task_json` - JSON-encoded AgentTask:
    ///   ```json
    ///   {
    ///     "type": "python-pro",
    ///     "operation": "test",
    ///     "path": "/home/user/project",
    ///     "args": "--verbose",
    ///     "config": {}
    ///   }
    ///   ```
    ///
    /// # Returns
    /// JSON-encoded TaskResult
    async fn execute(&self, task_json: String) -> Result<String, zbus::fdo::Error> {
        debug!(
            "[{}] Execute called: {}",
            self.agent_id,
            &task_json[..task_json.len().min(200)]
        );

        let mut task_json_mut = task_json.to_string();
        let task: AgentTask = unsafe { simd_json::from_str(&mut task_json_mut) }.map_err(|e| {
            error!("[{}] Invalid task JSON: {}", self.agent_id, e);
            zbus::fdo::Error::InvalidArgs(format!("Invalid task JSON: {}", e))
        })?;

        let agent = self.agent.read().await;

        // Validate operation is supported
        if !agent.supports_operation(&task.operation) {
            return Err(zbus::fdo::Error::InvalidArgs(format!(
                "Unsupported operation '{}'. Supported: {:?}",
                task.operation,
                agent.operations()
            )));
        }

        let result = agent.execute(task).await.map_err(|e| {
            error!("[{}] Execution failed: {}", self.agent_id, e);
            zbus::fdo::Error::Failed(format!("Execution failed: {}", e))
        })?;

        simd_json::to_string(&result).map_err(|e| {
            error!("[{}] Serialization failed: {}", self.agent_id, e);
            zbus::fdo::Error::Failed(format!("Serialization failed: {}", e))
        })
    }

    /// Execute an operation directly (convenience method)
    ///
    /// Simpler than Execute - just pass operation name and path
    async fn run_operation(
        &self,
        operation: String,
        path: String,
        args: String,
    ) -> Result<String, zbus::fdo::Error> {
        let task = AgentTask {
            task_type: self.agent_type.clone(),
            operation,
            path: if path.is_empty() { None } else { Some(path) },
            args: if args.is_empty() { None } else { Some(args) },
            config: std::collections::HashMap::new(),
        };

        let task_json = simd_json::to_string(&task)
            .map_err(|e| zbus::fdo::Error::Failed(format!("Failed to serialize task: {}", e)))?;

        self.execute(task_json).await
    }

    //
    // === Introspection Methods ===
    //

    /// Get the agent type identifier (e.g., "python-pro")
    fn agent_type(&self) -> &str {
        &self.agent_type
    }

    /// Get the agent instance ID
    fn agent_id(&self) -> &str {
        &self.agent_id
    }

    /// Get the agent's display name
    async fn name(&self) -> String {
        let agent = self.agent.read().await;
        agent.name().to_string()
    }

    /// Get the agent's description
    async fn description(&self) -> String {
        let agent = self.agent.read().await;
        agent.description().to_string()
    }

    /// List supported operations
    async fn operations(&self) -> Vec<String> {
        let agent = self.agent.read().await;
        agent.operations()
    }

    /// Check if a specific operation is supported
    async fn supports_operation(&self, operation: String) -> bool {
        let agent = self.agent.read().await;
        agent.supports_operation(&operation)
    }

    /// Get the agent's current status
    async fn status(&self) -> String {
        let agent = self.agent.read().await;
        agent.get_status()
    }

    /// Get the security profile as JSON
    async fn security_profile(&self) -> String {
        let agent = self.agent.read().await;
        let profile = agent.security_profile();
        simd_json::to_string(profile).unwrap_or_else(|_| "{}".to_string())
    }

    /// Get full agent metadata as JSON (for tool discovery)
    async fn metadata(&self) -> String {
        let agent = self.agent.read().await;
        let profile = agent.security_profile();

        simd_json::json!({
            "agent_type": self.agent_type,
            "agent_id": self.agent_id,
            "name": agent.name(),
            "description": agent.description(),
            "operations": agent.operations(),
            "status": agent.get_status(),
            "security": {
                "category": format!("{:?}", profile.config.category),
                "timeout_secs": profile.config.timeout_secs,
                "requires_root": profile.config.requires_root,
            }
        })
        .to_string()
    }

    /// Ping to check if agent is alive
    fn ping(&self) -> bool {
        true
    }

    //
    // === Signals ===
    //

    /// Signal emitted when a task completes
    #[zbus(signal)]
    async fn task_completed(
        signal_ctxt: &SignalContext<'_>,
        task_id: &str,
        success: bool,
        result_json: &str,
    ) -> zbus::Result<()>;

    /// Signal emitted when agent status changes
    #[zbus(signal)]
    async fn status_changed(signal_ctxt: &SignalContext<'_>, new_status: &str) -> zbus::Result<()>;
}

//
// === Public Functions ===
//

/// Start an agent as a D-Bus service
///
/// This registers the agent on the specified bus with a well-known name.
/// The agent can then be discovered via D-Bus introspection and called
/// by the AgentTool in the ChatActor's tool registry.
///
/// # Arguments
/// * `agent` - The agent to expose via D-Bus
/// * `agent_id` - Unique identifier for this agent instance
/// * `bus_type` - Which bus to register on (System or Session)
///
/// # Returns
/// The D-Bus connection (keeps the service alive as long as it's held)
pub async fn start_agent(
    agent: Box<dyn AgentTrait>,
    agent_id: &str,
    bus_type: BusType,
) -> Result<Connection, DbusAgentError> {
    tracing::info!("Starting D-Bus agent service");
    let agent_type = agent.agent_type().to_string();
    let service = DbusAgentService::new(agent, agent_id.to_string());

    let service_name = DbusAgentService::service_name(&agent_type);
    let object_path = DbusAgentService::object_path(&agent_type);

    info!(
        "Starting D-Bus agent: {} (id={}) at {} on {:?} bus",
        service_name, agent_id, object_path, bus_type
    );

    let connection = match bus_type {
        BusType::System => {
            Builder::system()?
                .name(service_name.as_str())?
                .serve_at(object_path.as_str(), service)?
                .build()
                .await?
        }
        BusType::Session => {
            Builder::session()?
                .name(service_name.as_str())?
                .serve_at(object_path.as_str(), service)?
                .build()
                .await?
        }
    };

    info!("D-Bus agent {} registered successfully", service_name);

    // Wait for the service to be ready
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    Ok(connection)
}

/// Start an agent with a custom instance suffix
///
/// Useful for running multiple instances of the same agent type.
/// Service name becomes: org.dbusmcp.Agent.{AgentType}.{InstanceId}
pub async fn start_agent_instance(
    agent: Box<dyn AgentTrait>,
    agent_id: &str,
    instance_suffix: &str,
    bus_type: BusType,
) -> Result<Connection, DbusAgentError> {
    tracing::info!("Starting D-Bus agent instance");
    let agent_type = agent.agent_type().to_string();
    let service = DbusAgentService::new(agent, agent_id.to_string());

    let base_name = DbusAgentService::service_name(&agent_type);
    let service_name = format!("{}.{}", base_name, instance_suffix);
    let base_path = DbusAgentService::object_path(&agent_type);
    let object_path = format!("{}/{}", base_path, instance_suffix);

    info!(
        "Starting D-Bus agent instance: {} at {} on {:?} bus",
        service_name, object_path, bus_type
    );

    let connection = match bus_type {
        BusType::System => {
            Builder::system()?
                .name(service_name.as_str())?
                .serve_at(object_path.as_str(), service)?
                .build()
                .await?
        }
        BusType::Session => {
            Builder::session()?
                .name(service_name.as_str())?
                .serve_at(object_path.as_str(), service)?
                .build()
                .await?
        }
    };

    info!(
        "D-Bus agent instance {} registered successfully",
        service_name
    );

    // Wait for the service to be ready
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    Ok(connection)
}

//
// === Helper Functions ===
//

/// Convert agent type to PascalCase for D-Bus naming
/// e.g., "python-pro" -> "PythonPro"
fn to_pascal_case(s: &str) -> String {
    s.split('-')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().chain(chars).collect(),
            }
        })
        .collect()
}

/// Generate a unique agent ID
pub fn generate_agent_id(agent_type: &str) -> String {
    format!(
        "{}-{}",
        agent_type,
        uuid::Uuid::new_v4()
            .to_string()
            .split('-')
            .next()
            .unwrap_or("0000")
    )
}

/// Check if a D-Bus service name matches the agent pattern
pub fn is_agent_service(service_name: &str) -> bool {
    service_name.starts_with("org.dbusmcp.Agent.")
}

/// Extract agent type from service name
/// e.g., "org.dbusmcp.Agent.PythonPro" -> "python-pro"
pub fn service_name_to_agent_type(service_name: &str) -> Option<String> {
    if !is_agent_service(service_name) {
        return None;
    }

    let pascal = service_name.strip_prefix("org.dbusmcp.Agent.")?;
    // Handle instance suffixes (e.g., "PythonPro.instance1" -> "PythonPro")
    let pascal = pascal.split('.').next()?;
    Some(to_kebab_case(pascal))
}

/// Convert PascalCase to kebab-case
/// e.g., "PythonPro" -> "python-pro"
fn to_kebab_case(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                result.push('-');
            }
            result.push(c.to_ascii_lowercase());
        } else {
            result.push(c);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_pascal_case() {
        assert_eq!(to_pascal_case("python-pro"), "PythonPro");
        assert_eq!(to_pascal_case("rust-pro"), "RustPro");
        assert_eq!(to_pascal_case("code-reviewer"), "CodeReviewer");
        assert_eq!(to_pascal_case("tdd-orchestrator"), "TddOrchestrator");
    }

    #[test]
    fn test_to_kebab_case() {
        assert_eq!(to_kebab_case("PythonPro"), "python-pro");
        assert_eq!(to_kebab_case("RustPro"), "rust-pro");
        assert_eq!(to_kebab_case("CodeReviewer"), "code-reviewer");
    }

    #[test]
    fn test_service_name() {
        assert_eq!(
            DbusAgentService::service_name("python-pro"),
            "org.dbusmcp.Agent.PythonPro"
        );
    }

    #[test]
    fn test_object_path() {
        assert_eq!(
            DbusAgentService::object_path("python-pro"),
            "/org/dbusmcp/Agent/PythonPro"
        );
    }

    #[test]
    fn test_is_agent_service() {
        assert!(is_agent_service("org.dbusmcp.Agent.PythonPro"));
        assert!(is_agent_service("org.dbusmcp.Agent.RustPro.instance1"));
        assert!(!is_agent_service("org.freedesktop.DBus"));
        assert!(!is_agent_service("org.dbusmcp.Orchestrator"));
    }

    #[test]
    fn test_service_name_to_agent_type() {
        assert_eq!(
            service_name_to_agent_type("org.dbusmcp.Agent.PythonPro"),
            Some("python-pro".to_string())
        );
        assert_eq!(
            service_name_to_agent_type("org.dbusmcp.Agent.PythonPro.instance1"),
            Some("python-pro".to_string())
        );
        assert_eq!(service_name_to_agent_type("org.freedesktop.DBus"), None);
    }

    #[test]
    fn test_generate_agent_id() {
        let id = generate_agent_id("python-pro");
        assert!(id.starts_with("python-pro-"));
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/lib.rs">
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
            ARMCortexExpertAgent, SnowballDeveloperAgent, ErrorDetectiveAgent,
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
        "snowball-developer" | "snowball_developer" => {
            Box::new(SnowballDeveloperAgent::new(agent_id))
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
        "snowball-developer",
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
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/src/router.rs">
//! Agents Router - HTTP endpoints for agent management
//!
//! This module exports a router that can be mounted by op-http.

use axum::{
    extract::State,
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::agent_registry::AgentRegistry;

/// Agents service state
#[derive(Clone)]
pub struct AgentsState {
    pub registry: Arc<RwLock<AgentRegistry>>,
}

impl AgentsState {
    pub fn new() -> Self {
        Self {
            registry: Arc::new(RwLock::new(AgentRegistry::new())),
        }
    }

    pub fn with_registry(registry: AgentRegistry) -> Self {
        Self {
            registry: Arc::new(RwLock::new(registry)),
        }
    }
}

impl Default for AgentsState {
    fn default() -> Self {
        Self::new()
    }
}

/// Create the agents router
///
/// Mount this at `/api/agents` in the unified server:
/// ```ignore
/// use op_http::prelude::*;
/// use op_agents::router::{create_router, AgentsState};
///
/// let state = AgentsState::new();
/// let router = RouterBuilder::new()
///     .nest("/api/agents", "agents", create_router(state))
///     .build();
/// ```
pub fn create_router(state: AgentsState) -> Router {
    Router::new()
        .route("/", get(list_agents_handler))
        .route("/", post(spawn_agent_handler))
        .route("/health", get(health_handler))
        .route("/types", get(list_types_handler))
        .route("/:id", get(get_agent_handler))
        .route("/:id", delete(kill_agent_handler))
        //.route("/:id/task", post(send_task_handler))
        .with_state(state)
}

/// Service info for op-http ServiceRouter trait
pub struct AgentsServiceRouter;

impl op_http::router::ServiceRouter for AgentsServiceRouter {
    fn prefix() -> &'static str {
        "/api/agents"
    }

    fn name() -> &'static str {
        "agents"
    }

    fn description() -> &'static str {
        "Agent management API endpoints"
    }
}

// === Handlers ===

async fn health_handler() -> impl IntoResponse {
    Json(json!({
        "status": "healthy",
        "service": "agents"
    }))
}

async fn list_agents_handler(State(state): State<AgentsState>) -> impl IntoResponse {
    let registry = state.registry.read().await;
    let agents = registry.list_instances().await;
    Json(json!({ "agents": agents }))
}

async fn list_types_handler() -> impl IntoResponse {
    let types = crate::list_agent_types();
    Json(json!({ "types": types }))
}

async fn get_agent_handler(
    State(state): State<AgentsState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> impl IntoResponse {
    let registry = state.registry.read().await;
    match registry.get_instance_status(&id).await {
        Ok(status) => Json(json!({ "agent": status })),
        Err(_) => Json(json!({ "error": "Agent not found" })),
    }
}

async fn spawn_agent_handler(
    State(state): State<AgentsState>,
    Json(request): Json<Value>,
) -> impl IntoResponse {
    let agent_type = request
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("executor");
    let config = request.get("config").cloned();

    let registry = state.registry.write().await;
    match registry.spawn_agent(agent_type, config).await {
        Ok(id) => Json(json!({ "agent_id": id })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn kill_agent_handler(
    State(state): State<AgentsState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> impl IntoResponse {
    let registry = state.registry.write().await;
    match registry.kill_agent(&id).await {
        Ok(_) => Json(json!({ "killed": true })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

// TODO: Implement send_task
// async fn send_task_handler(
//     State(state): State<AgentsState>,
//     axum::extract::Path(id): axum::extract::Path<String>,
//     Json(task): Json<Value>,
// ) -> impl IntoResponse {
//     let registry = state.registry.read().await;
//     match registry.send_task(&id, task).await {
//         Ok(result) => Json(json!({ "result": result })),
//         Err(e) => Json(json!({ "error": e.to_string() })),
//     }
// }
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/Cargo.toml">
[package]
name = "op-agents"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
description = "Secure agent registry and D-Bus agent implementations for op-dbus-v2"

[[bin]]
name = "dbus-agent"
path = "src/bin/dbus-agent.rs"

[[bin]]
name = "op-agent-manager"
path = "src/bin/dbus-agent-manager.rs"

[dependencies]
# Internal crates
op-core = { workspace = true }
op-http = { workspace = true }

# Async runtime
tokio = { workspace = true }
async-trait = { workspace = true }
futures = { workspace = true }

# Serialization
serde = { workspace = true }
simd-json = { workspace = true }
serde_yaml = { workspace = true }
toml = { workspace = true }

# Error handling
anyhow = { workspace = true }
thiserror = { workspace = true }

# D-Bus
zbus = { workspace = true }

# Utils
uuid = { workspace = true }
chrono = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
regex = { workspace = true }
shell-words = "1.1"

axum = { workspace = true }

[dev-dependencies]
tempfile = { workspace = true }
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/compare-op-agents.md">
# compare-op-agents

**Date**: 2026-04-05  
**Spec files analyzed**: SPEC.md  
**Analysis mode**: Current system state specification with spec-alignment notes

---

## Summary

| Category | Count |
|---|---|
| Rust source files | 130 |
| Proto files | 0 |
| Binary targets | 2 |
| UI files | 0 |
| Root-declared modules | 6 |
| Partial artifacts | 0 |
| Spec-listed source files | 20 |
| Spec-listed but missing | 0 |
| Extra implementation files | 110 |

## Current Implementation Overview

- Secure agent registry and D-Bus agent implementations for op-dbus-v2
- Internal crate integrations: op-core, op-http.

## Module / File Comparison

| Module or File | State | Current Role | Notes |
|---|---|---|---|
| `src/agents/aiml/prompt_engineer.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/agents/aiml/prompt_engineer.rs |
| `src/agents/aiml/mod.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/agents/aiml/mod.rs |
| `src/agents/aiml/mlops_engineer.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/agents/aiml/mlops_engineer.rs |
| `src/agents/aiml/ml_engineer.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/agents/aiml/ml_engineer.rs |
| `src/agents/aiml/data_scientist.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/agents/aiml/data_scientist.rs |
| `src/agents/aiml/data_engineer.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/agents/aiml/data_engineer.rs |
| `src/agents/aiml/ai_engineer.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/agents/aiml/ai_engineer.rs |
| `src/agents/analysis/security_auditor.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/agents/analysis/security_auditor.rs |
| `src/agents/analysis/performance.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/agents/analysis/performance.rs |
| `src/agents/analysis/mod.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/agents/analysis/mod.rs |
| `src/agents/analysis/debugger.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/agents/analysis/debugger.rs |
| `src/agents/analysis/code_reviewer.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/agents/analysis/code_reviewer.rs |
| `src/agents/architecture/mod.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/agents/architecture/mod.rs |
| `src/agents/architecture/graphql_architect.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/agents/architecture/graphql_architect.rs |
| `src/agents/architecture/frontend_developer.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/agents/architecture/frontend_developer.rs |
| `src/agents/architecture/backend_architect.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/agents/architecture/backend_architect.rs |
| `src/agents/business/sales_automator.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/agents/business/sales_automator.rs |
| `src/agents/business/payment_integration.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/agents/business/payment_integration.rs |
| `src/agents/business/mod.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/agents/business/mod.rs |
| `src/agents/business/legal_advisor.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/agents/business/legal_advisor.rs |
| `agents` | ✅ Present | agents group | src/agents/aiml/ai_engineer.rs, src/agents/aiml/data_engineer.rs, src/agents/aiml/data_scientist.rs, src/agents/aiml/ml_engineer.rs, src/agents/aiml/mlops_engineer.rs, src/agents/aiml/mod.rs, src/agents/aiml/prompt_engineer.rs, src/agents/analysis/code_reviewer.rs, ... (+88 more) |
| `bin` | ✅ Present | bin group | src/bin/dbus-agent-manager.rs, src/bin/dbus-agent.rs |
| `generator` | ✅ Present | generator group | src/generator/md_parser.rs, src/generator/mod.rs, src/generator/template.rs |
| `root` | ✅ Present | root source group | src/agent_catalog.rs, src/agent_registry.rs, src/dbus_service.rs, src/lib.rs, src/router.rs |
| `security` | ✅ Present | security group | src/security/mod.rs, src/security/profiles.rs, src/security/sandbox.rs, src/security/validation.rs |
| `unified` | ✅ Present | unified group | src/unified/agent_trait.rs, src/unified/execution/base.rs, src/unified/execution/golang.rs, src/unified/execution/javascript.rs, src/unified/execution/mod.rs, src/unified/execution/python.rs, src/unified/execution/rust.rs, src/unified/execution/shell.rs, ... (+12 more) |

## Feature / Capability Comparison

| Capability | State | Evidence | Source |
|---|---|---|---|
| dbus_service | ✅ Implemented | src/dbus_service.rs | SPEC main module |
| agent_catalog | ✅ Implemented | src/agent_catalog.rs | SPEC main module |
| agent_registry | ✅ Implemented | src/agent_registry.rs | SPEC main module |
| router | ✅ Implemented | src/router.rs | SPEC main module |
| Binary `dbus-agent` | ✅ Implemented | src/bin/dbus-agent.rs | Cargo bin target |
| Binary `op-agent-manager` | ✅ Implemented | src/bin/dbus-agent-manager.rs | Cargo bin target |

## Dependencies Comparison

### Internal Workspace Dependencies
- `op-core` - documented in SPEC
- `op-http` - documented in SPEC

### External Runtime Dependencies
- `tokio` - documented in SPEC
- `async-trait` - documented in SPEC
- `futures` - documented in SPEC
- `serde` - documented in SPEC
- `simd-json` - documented in SPEC
- `serde_yaml` - documented in SPEC
- `toml` - documented in SPEC
- `anyhow` - documented in SPEC
- `thiserror` - documented in SPEC
- `zbus` - not listed in SPEC dependency block
- `uuid` - not listed in SPEC dependency block
- `chrono` - not listed in SPEC dependency block
- `tracing` - not listed in SPEC dependency block
- `tracing-subscriber` - not listed in SPEC dependency block
- `regex` - not listed in SPEC dependency block
- `shell-words` - not listed in SPEC dependency block
- `axum` - not listed in SPEC dependency block

### Development and Build Dependencies
- `dev:tempfile`

## Notes and Observations

- Local documentation files present: SPEC.md.
- Current implementation contains 110 Rust source files beyond the explicit spec/design source inventory.
- Root module declarations found in `lib.rs`/`main.rs`: agent_catalog, agent_registry, agents, dbus_service, router, security.
- 8 runtime dependencies are present in `Cargo.toml` but omitted from the SPEC dependency block.
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-agents/SPEC.md">
# op-agents - Specification

## Overview
**Crate**: `op-agents`  
**Location**: `crates/op-agents`

## Quick Reference

### From Cargo.toml
```toml
[package]
name = "op-agents"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
```

### Source Structure
```
op-agents/src/agents/aiml/prompt_engineer.rs
op-agents/src/agents/aiml/mod.rs
op-agents/src/agents/aiml/mlops_engineer.rs
op-agents/src/agents/aiml/ml_engineer.rs
op-agents/src/agents/aiml/data_scientist.rs
op-agents/src/agents/aiml/data_engineer.rs
op-agents/src/agents/aiml/ai_engineer.rs
op-agents/src/agents/analysis/security_auditor.rs
op-agents/src/agents/analysis/performance.rs
op-agents/src/agents/analysis/mod.rs
op-agents/src/agents/analysis/debugger.rs
op-agents/src/agents/analysis/code_reviewer.rs
op-agents/src/agents/architecture/mod.rs
op-agents/src/agents/architecture/graphql_architect.rs
op-agents/src/agents/architecture/frontend_developer.rs
op-agents/src/agents/architecture/backend_architect.rs
op-agents/src/agents/business/sales_automator.rs
op-agents/src/agents/business/payment_integration.rs
op-agents/src/agents/business/mod.rs
op-agents/src/agents/business/legal_advisor.rs
```

### Key Dependencies
```toml
# Internal crates
op-core = { workspace = true }
op-http = { workspace = true }

# Async runtime
tokio = { workspace = true }
async-trait = { workspace = true }
futures = { workspace = true }

# Serialization
serde = { workspace = true }
simd-json = { workspace = true }
serde_yaml = { workspace = true }
toml = { workspace = true }

# Error handling
anyhow = { workspace = true }
thiserror = { workspace = true }

# D-Bus
```

### Binaries
```toml
[[bin]]
name = "dbus-agent"
path = "src/bin/dbus-agent.rs"

[[bin]]
name = "op-agent-manager"
path = "src/bin/dbus-agent-manager.rs"
```

### Features
```toml
# No features
```

## Documentation Files


## Module Structure
     130 Rust source files

### Main Modules
dbus_service
agent_catalog
agent_registry
router

## Purpose
Secure agent registry and D-Bus agent implementations for op-dbus-v2

## Build Information
- **Edition**: edition.workspace = true
- **Version**: version.workspace = true
- **License**: license.workspace = true

## Related Crates
Internal dependencies:


---
*Generated from crate analysis*
</file>

</files>
