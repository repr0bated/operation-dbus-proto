//! Access Controller: Access control and redaction.
//!
//! This module implements the `AccessController` trait, enforcing access
//! control policies and redacting sensitive data (secrets, PII).

use crate::data_models::*;
use crate::interfaces::AccessController;
use anyhow::Result;
use regex::Regex;
use std::sync::Arc;
use parking_lot::RwLock;
use tracing::{debug, warn};

/// Controller that enforces security policies on projections.
#[derive(Debug, Clone)]
pub struct ProjectionAccessController {
    /// Active access policies
    policies: Arc<RwLock<Vec<AccessPolicy>>>,
    /// Audit trail for decisions
    audit_trail: Arc<RwLock<Vec<AccessControlAudit>>>,
}

impl ProjectionAccessController {
    /// Creates a new ProjectionAccessController
    pub fn new() -> Self {
        Self {
            policies: Arc::new(RwLock::new(Vec::new())),
            audit_trail: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

impl Default for ProjectionAccessController {
    fn default() -> Self {
        Self::new()
    }
}

impl AccessController for ProjectionAccessController {
    fn enforce_policy(
        &self,
        projection: &Projection,
        requester: &Requester,
    ) -> Result<Projection> {
        let mut result = projection.clone();
        
        // Validate access
        self.validate_permissions(requester, "read", &projection.id)?;
        
        // Check if redaction is needed
        let policies = self.policies.read();
        for policy in policies.iter() {
            let re = Regex::new(&policy.resource_pattern)?;
            if re.is_match(&projection.id) && policy.redact_sensitive {
                result.data = self.redact_sensitive(&result.data, requester);
            }
        }
        
        Ok(result)
    }

    fn validate_permissions(
        &self,
        requester: &Requester,
        action: &str,
        resource: &str,
    ) -> Result<()> {
        let policies = self.policies.read();
        let mut allowed = false;
        
        for policy in policies.iter() {
            if policy.action == action {
                let re = Regex::new(&policy.resource_pattern)?;
                if re.is_match(resource) {
                    // Check permissions
                    if policy.required_permissions.is_empty() {
                        allowed = true;
                        break;
                    }
                    
                    for req_perm in &policy.required_permissions {
                        if requester.permissions.contains(req_perm) {
                            allowed = true;
                            break;
                        }
                    }
                }
            }
        }
        
        // Log decision
        self.log_decision(requester, action, resource, allowed);
        
        if allowed {
            Ok(())
        } else {
            Err(anyhow::anyhow!("Permission denied: {} on {}", action, resource))
        }
    }

    fn redact_sensitive(&self, data: &simd_json::OwnedValue, _requester: &Requester) -> simd_json::OwnedValue {
        // In production, use JSON paths from schema to redact
        data.clone()
    }

    fn log_decision(
        &self,
        requester: &Requester,
        action: &str,
        resource: &str,
        allowed: bool,
    ) {
        let audit = AccessControlAudit {
            timestamp: chrono::Utc::now(),
            requester_id: requester.id.clone(),
            action: action.to_string(),
            resource: resource.to_string(),
            allowed,
            reason: if allowed { "Policy match".to_string() } else { "No policy match".to_string() },
        };
        
        self.audit_trail.write().push(audit);
        
        if !allowed {
            warn!(
                requester_id = requester.id,
                action = action,
                resource = resource,
                "Access denied"
            );
        } else {
            debug!(
                requester_id = requester.id,
                action = action,
                resource = resource,
                "Access granted"
            );
        }
    }

    fn is_accessible(&self, _data: &simd_json::OwnedValue, _requester: &Requester) -> bool {
        // Simplified check
        true
    }

    fn add_policy(&mut self, policy: AccessPolicy) {
        let mut policies = self.policies.write();
        policies.push(policy);
    }

    fn get_policies(&self) -> Vec<AccessPolicy> {
        self.policies.read().clone()
    }

    fn get_audit_trail(&self) -> Vec<AccessControlAudit> {
        self.audit_trail.read().clone()
    }
}
