//! Policy and Compliance as First-Class State Objects
//!
//! OP-DBUS treats configuration, policy, and compliance as first-class objects.
//! They are part of the same canonical object universe as operational state.
//!
//! This enables:
//! - Proving what changed, under what policy, with what compliance at the time
//! - Versioned, hashed policy objects referenced by execution records
//! - Replayable policy context as part of history

use std::collections::HashMap;
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{OpDbusError, Result};
use op_state_store::StateStore;

/// A policy object that governs system changes
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Policy {
    /// Unique policy identifier
    pub policy_id: String,
    /// Human-readable name
    pub name: String,
    /// Policy version
    pub version: String,
    /// Policy rules
    pub rules: Vec<PolicyRule>,
    /// Approval requirements
    pub approval_requirements: Vec<ApprovalRequirement>,
    /// Time-based constraints
    pub time_constraints: Option<TimeConstraints>,
    /// Scope (what this policy applies to)
    pub scope: PolicyScope,
    /// Whether this policy is active
    pub active: bool,
    /// Creation timestamp
    pub created_at_ns: u128,
    /// Hash of policy content
    pub content_hash: String,
}

impl Policy {
    /// Create a new policy
    pub fn new(name: &str, version: &str) -> Self {
        let policy_id = uuid::Uuid::new_v4().to_string();
        let mut policy = Self {
            policy_id,
            name: name.to_string(),
            version: version.to_string(),
            rules: Vec::new(),
            approval_requirements: Vec::new(),
            time_constraints: None,
            scope: PolicyScope::Global,
            active: true,
            created_at_ns: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
            content_hash: String::new(),
        };
        policy.compute_hash();
        policy
    }

    /// Add a rule to the policy
    pub fn add_rule(mut self, rule: PolicyRule) -> Self {
        self.rules.push(rule);
        self.compute_hash();
        self
    }

    /// Compute content hash
    fn compute_hash(&mut self) {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(self.name.as_bytes());
        hasher.update(self.version.as_bytes());
        hasher.update(serde_json::to_vec(&self.rules).unwrap_or_default());
        self.content_hash = hex::encode(hasher.finalize());
    }

    /// Check if a tool execution is allowed under this policy
    pub fn allows_tool(&self, tool_name: &str, params: &Value) -> PolicyDecision {
        for rule in &self.rules {
            match rule.evaluate(tool_name, params) {
                RuleResult::Deny(reason) => {
                    return PolicyDecision::Denied {
                        policy_id: self.policy_id.clone(),
                        rule_id: rule.rule_id.clone(),
                        reason,
                    };
                }
                RuleResult::Allow => {}
                RuleResult::NotApplicable => {}
            }
        }

        PolicyDecision::Allowed {
            policy_id: self.policy_id.clone(),
        }
    }
}

/// A rule within a policy
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PolicyRule {
    /// Rule identifier
    pub rule_id: String,
    /// Rule name
    pub name: String,
    /// Rule type
    pub rule_type: RuleType,
    /// Rule condition (JSON expression)
    pub condition: Value,
    /// Action to take when condition matches
    pub action: RuleAction,
}

impl PolicyRule {
    /// Evaluate this rule against a tool execution
    pub fn evaluate(&self, tool_name: &str, params: &Value) -> RuleResult {
        match &self.rule_type {
            RuleType::ToolWhitelist(patterns) => {
                if patterns.iter().any(|p| tool_name.contains(p) || p == "*") {
                    RuleResult::Allow
                } else {
                    RuleResult::Deny(format!("Tool {} not in whitelist", tool_name))
                }
            }
            RuleType::ToolBlacklist(patterns) => {
                if patterns.iter().any(|p| tool_name.contains(p)) {
                    RuleResult::Deny(format!("Tool {} is blacklisted", tool_name))
                } else {
                    RuleResult::Allow
                }
            }
            RuleType::ParameterConstraint { param_path, constraint } => {
                if let Some(value) = self.get_param_value(params, param_path) {
                    if constraint.check(value) {
                        RuleResult::Allow
                    } else {
                        RuleResult::Deny(format!("Parameter {} violates constraint", param_path))
                    }
                } else {
                    RuleResult::NotApplicable
                }
            }
            RuleType::NetworkZone(allowed_zones) => {
                // Check if 'source_ip' is in params (injected by context)
                if let Some(ip) = self.get_param_value(params, "source_ip").and_then(|v| v.as_str()) {
                    let zone = AccessZone::from_ip(ip);
                    if allowed_zones.contains(&zone) {
                        RuleResult::Allow
                    } else {
                        RuleResult::Deny(format!("Source IP {} in {:?} not allowed", ip, zone))
                    }
                } else {
                    // If no source IP provided, assume restrictive default or N/A
                    RuleResult::NotApplicable 
                }
            }
            RuleType::Custom(_) => RuleResult::NotApplicable,
        }
    }

    fn get_param_value<'a>(&self, params: &'a Value, path: &str) -> Option<&'a Value> {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = params;
        for part in parts {
            current = current.get(part)?;
        }
        Some(current)
    }
}

/// Type of policy rule
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum RuleType {
    /// Whitelist of allowed tools (patterns)
    ToolWhitelist(Vec<String>),
    /// Blacklist of denied tools (patterns)
    ToolBlacklist(Vec<String>),
    /// Constraint on a parameter value
    ParameterConstraint {
        param_path: String,
        constraint: ParameterConstraint,
    },
    /// Custom rule with expression
    Custom(String),
    /// Network zone constraint
    NetworkZone(Vec<AccessZone>),
}

/// Security level for resources/tools
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SecurityLevel {
    /// Safe, read-only operations - any IP
    #[default]
    Public,
    /// Normal operations - any IP
    Standard,
    /// System modifications - localhost or private network
    Elevated,
    /// Dangerous commands - localhost only
    Restricted,
}

/// IP-based access control zones
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AccessZone {
    /// 127.0.0.1, ::1 - full access to everything
    Localhost,
    /// Trusted VPN/mesh networks (Netmaker, Tailscale, etc.) - full access
    TrustedMesh,
    /// 192.168.x.x, 10.x.x.x, 172.16-31.x.x - elevated access
    PrivateNetwork,
    /// Public IPs - restricted to safe tools only
    #[default]
    Public,
}

impl AccessZone {
    pub fn from_ip(ip: &str) -> Self {
        let ip = ip.trim();
        if ip == "127.0.0.1" || ip == "::1" || ip == "localhost" || ip.starts_with("127.") {
            return Self::Localhost;
        }
        // Simplified mesh detection for this context
        if ip.starts_with("100.64.") || ip.starts_with("10.101.") || ip.starts_with("fd") {
             return Self::TrustedMesh;
        }
        if ip.starts_with("192.168.") || ip.starts_with("10.") || ip.starts_with("172.16.") {
             return Self::PrivateNetwork;
        }
        Self::Public
    }
}

/// Constraint on a parameter value
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ParameterConstraint {
    pub constraint_type: ConstraintType,
    pub value: Value,
}

impl ParameterConstraint {
    pub fn check(&self, actual: &Value) -> bool {
        match &self.constraint_type {
            ConstraintType::Equals => actual == &self.value,
            ConstraintType::NotEquals => actual != &self.value,
            ConstraintType::Contains => {
                if let (Some(actual_str), Some(value_str)) = (actual.as_str(), self.value.as_str()) {
                    actual_str.contains(value_str)
                } else {
                    false
                }
            }
            ConstraintType::LessThan => {
                if let (Some(a), Some(v)) = (actual.as_f64(), self.value.as_f64()) {
                    a < v
                } else {
                    false
                }
            }
            ConstraintType::GreaterThan => {
                if let (Some(a), Some(v)) = (actual.as_f64(), self.value.as_f64()) {
                    a > v
                } else {
                    false
                }
            }
            ConstraintType::InList => {
                if let Some(list) = self.value.as_array() {
                    list.contains(actual)
                } else {
                    false
                }
            }
            ConstraintType::Regex => {
                if let (Some(actual_str), Some(pattern)) = (actual.as_str(), self.value.as_str()) {
                    regex::Regex::new(pattern)
                        .map(|re| re.is_match(actual_str))
                        .unwrap_or(false)
                } else {
                    false
                }
            }
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ConstraintType {
    Equals,
    NotEquals,
    Contains,
    LessThan,
    GreaterThan,
    InList,
    Regex,
}

/// Action to take when a rule matches
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum RuleAction {
    Allow,
    Deny,
    RequireApproval,
    Log,
    Alert(String),
}

/// Result of evaluating a rule
#[derive(Debug, Clone)]
pub enum RuleResult {
    Allow,
    Deny(String),
    NotApplicable,
}

/// Policy decision
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PolicyDecision {
    Allowed { policy_id: String },
    Denied { policy_id: String, rule_id: String, reason: String },
    RequiresApproval { policy_id: String, approvers: Vec<String> },
}

/// Approval requirement
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ApprovalRequirement {
    pub approver_roles: Vec<String>,
    pub min_approvers: usize,
    pub timeout_hours: Option<u32>,
}

/// Time-based constraints
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TimeConstraints {
    /// Allowed days (0=Sunday, 6=Saturday)
    pub allowed_days: Vec<u8>,
    /// Allowed hours (0-23)
    pub allowed_hours_start: u8,
    pub allowed_hours_end: u8,
    /// Timezone
    pub timezone: String,
    /// Blackout windows
    pub blackout_windows: Vec<BlackoutWindow>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlackoutWindow {
    pub start: String, // ISO 8601
    pub end: String,
    pub reason: String,
}

/// Policy scope
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PolicyScope {
    Global,
    Plugin(String),
    ObjectType(String),
    Namespace(String),
    Custom(Value),
}

/// Compliance profile
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ComplianceProfile {
    /// Profile identifier
    pub profile_id: String,
    /// Name (e.g., "CIS-L1", "PCI-DSS", "HIPAA")
    pub name: String,
    /// Version
    pub version: String,
    /// Controls in this profile
    pub controls: Vec<ComplianceControl>,
    /// Active
    pub active: bool,
}

/// A compliance control
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ComplianceControl {
    pub control_id: String,
    pub title: String,
    pub description: String,
    pub severity: Severity,
    pub check_type: ComplianceCheckType,
    pub remediation: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ComplianceCheckType {
    /// Object must exist
    ObjectExists { object_type: String, filter: Value },
    /// Object property must match
    PropertyValue { object_type: String, property: String, expected: Value },
    /// Tool execution required
    ToolExecuted { tool_name: String, within_hours: u32 },
    /// Custom check
    Custom(String),
}

/// Policy engine for evaluating policies
pub struct PolicyEngine {
    state_store: Arc<dyn StateStore>,
    active_policies: parking_lot::RwLock<HashMap<String, Policy>>,
    compliance_profiles: parking_lot::RwLock<HashMap<String, ComplianceProfile>>,
}

impl PolicyEngine {
    /// Create a new policy engine
    pub fn new(state_store: Arc<dyn StateStore>) -> Self {
        Self {
            state_store,
            active_policies: parking_lot::RwLock::new(HashMap::new()),
            compliance_profiles: parking_lot::RwLock::new(HashMap::new()),
        }
    }

    /// Load policies from state store
    pub async fn load_policies(&self) -> Result<()> {
        // Would load from state_store in production
        // For now, create default policy
        let default_policy = Policy::new("default", "1.0.0")
            .add_rule(PolicyRule {
                rule_id: "allow-system".into(),
                name: "Allow system tools".into(),
                rule_type: RuleType::ToolWhitelist(vec!["system.".into()]),
                condition: Value::Null,
                action: RuleAction::Allow,
            });

        let mut policies = self.active_policies.write();
        policies.insert(default_policy.policy_id.clone(), default_policy);

        Ok(())
    }

    /// Register a policy
    pub fn register_policy(&self, policy: Policy) {
        let mut policies = self.active_policies.write();
        policies.insert(policy.policy_id.clone(), policy);
    }

    /// Check if a tool execution is allowed
    pub fn check_tool_execution(
        &self,
        tool_name: &str,
        params: &Value,
    ) -> PolicyDecision {
        let policies = self.active_policies.read();

        for policy in policies.values() {
            if !policy.active {
                continue;
            }

            let decision = policy.allows_tool(tool_name, params);
            if let PolicyDecision::Denied { .. } = decision {
                return decision;
            }
        }

        PolicyDecision::Allowed {
            policy_id: "default".into(),
        }
    }

    /// Get active policy for a tool
    pub fn get_effective_policy(&self, tool_name: &str) -> Option<Policy> {
        let policies = self.active_policies.read();
        
        // Find most specific matching policy
        for policy in policies.values() {
            if policy.active {
                match &policy.scope {
                    PolicyScope::Global => return Some(policy.clone()),
                    PolicyScope::Plugin(p) if tool_name.starts_with(p) => {
                        return Some(policy.clone());
                    }
                    _ => {}
                }
            }
        }

        None
    }

    /// Register a compliance profile
    pub fn register_compliance_profile(&self, profile: ComplianceProfile) {
        let mut profiles = self.compliance_profiles.write();
        profiles.insert(profile.profile_id.clone(), profile);
    }

    /// Run compliance check
    pub async fn check_compliance(&self, profile_id: &str) -> Result<ComplianceReport> {
        let profiles = self.compliance_profiles.read();
        let profile = profiles.get(profile_id)
            .ok_or_else(|| OpDbusError::PolicyViolation(
                format!("Compliance profile not found: {}", profile_id)
            ))?;

        let mut results = Vec::new();

        for control in &profile.controls {
            let status = self.evaluate_control(control).await?;
            results.push(ControlResult {
                control_id: control.control_id.clone(),
                status,
                details: None,
            });
        }

        let passed = results.iter().filter(|r| r.status == ControlStatus::Pass).count();
        let failed = results.iter().filter(|r| r.status == ControlStatus::Fail).count();

        Ok(ComplianceReport {
            profile_id: profile_id.to_string(),
            timestamp_ns: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
            results,
            summary: ComplianceSummary {
                total: profile.controls.len(),
                passed,
                failed,
                not_applicable: profile.controls.len() - passed - failed,
            },
        })
    }

    async fn evaluate_control(&self, control: &ComplianceControl) -> Result<ControlStatus> {
        // Simplified evaluation
        match &control.check_type {
            ComplianceCheckType::ObjectExists { object_type: _, filter: _ } => {
                // Would query state_store
                Ok(ControlStatus::Pass)
            }
            ComplianceCheckType::PropertyValue { .. } => {
                Ok(ControlStatus::Pass)
            }
            _ => Ok(ControlStatus::NotApplicable),
        }
    }
}

/// Compliance report
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ComplianceReport {
    pub profile_id: String,
    pub timestamp_ns: u128,
    pub results: Vec<ControlResult>,
    pub summary: ComplianceSummary,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ControlResult {
    pub control_id: String,
    pub status: ControlStatus,
    pub details: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub enum ControlStatus {
    Pass,
    Fail,
    NotApplicable,
    Error,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ComplianceSummary {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub not_applicable: usize,
}
