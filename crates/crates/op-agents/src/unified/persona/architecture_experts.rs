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
