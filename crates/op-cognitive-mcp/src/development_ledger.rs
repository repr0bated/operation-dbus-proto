//! Structured development ledger for Cognitive capabilities.
//!
//! Cozo is used for durable status and relationships; Qdrant remains reserved
//! for semantic retrieval. The ledger is deliberately small and append-history
//! aware so a capability can be planned, implemented, verified, and deployed
//! without losing the evidence trail.

use crate::cozo_shuttle::CozoGraphShuttle;
use anyhow::{Context, Result};
use cozo::{DataValue, ScriptMutability};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::Arc;

pub struct DevelopmentLedger {
    db: Arc<cozo::DbInstance>,
}

const DEVELOPMENT_CATEGORIES: &[(&str, &str, &str)] = &[
    (
        "contract_schema",
        "Contract and schema",
        "Plugin contracts, schemas, generated interfaces, and catalog identity.",
    ),
    (
        "ingress_identity",
        "Ingress and identity",
        "Canonical ingress, authentication, authorization, and session identity.",
    ),
    (
        "runtime_dispatch",
        "Runtime and dispatch",
        "Tool routing, mutation dispatch, orchestration, and execution.",
    ),
    (
        "persistence_memory",
        "Persistence and memory",
        "Cozo state, memory namespaces, durable records, and lifecycle.",
    ),
    (
        "retrieval_indexing",
        "Retrieval and indexing",
        "Qdrant, RAG, semantic search, and index freshness.",
    ),
    (
        "code_intelligence",
        "Code intelligence",
        "Code search, context, suggestions, and repository analysis.",
    ),
    (
        "compliance_governance",
        "Compliance and governance",
        "Policy, controls, approvals, and evidence requirements.",
    ),
    (
        "model_orchestration",
        "Model orchestration",
        "Model selection, provider routing, fallbacks, and quotas.",
    ),
    (
        "generation_surfaces",
        "Generation surfaces",
        "JSON render, canvas, UI generation, and catalog promotion.",
    ),
    (
        "observability_accountability",
        "Observability and accountability",
        "Audit, activity, provenance, accountability, and reporting.",
    ),
    (
        "verification_testing",
        "Verification and testing",
        "Unit, integration, ingress, live, and regression verification.",
    ),
    (
        "deployment_operations",
        "Deployment and operations",
        "Builds, runit services, release, health, and rollback.",
    ),
    (
        "external_integrations",
        "External integrations",
        "Salad, OAuth providers, Gemini, OpenAI, and other connectors.",
    ),
    (
        "data_quality_lifecycle",
        "Data quality and lifecycle",
        "Validation, freshness, migration, retention, and deprecation.",
    ),
];

impl DevelopmentLedger {
    pub fn new(shuttle: Arc<CozoGraphShuttle>) -> Self {
        Self { db: shuttle.db() }
    }

    fn run(&self, script: &str, params: BTreeMap<String, DataValue>) -> Result<cozo::NamedRows> {
        self.db
            .run_script(script, params, ScriptMutability::Mutable)
            .map_err(|e| anyhow::anyhow!("{e}"))
    }

    pub fn execute(&self, input: &Value) -> Result<Value> {
        match input
            .get("operation")
            .and_then(Value::as_str)
            .unwrap_or("list")
        {
            "upsert" => self.upsert(input),
            "record_verification" => self.record_verification(input),
            "list" => self.list(input),
            "summary" => self.summary(),
            "history" => self.history_list(input),
            "categories" => Ok(json!({
                "categories": DEVELOPMENT_CATEGORIES.iter().map(|(id, title, description)| json!({
                    "id": id, "title": title, "description": description
                })).collect::<Vec<_>>()
            })),
            op => anyhow::bail!("unknown cognitive development operation: {op}"),
        }
    }

    fn upsert(&self, input: &Value) -> Result<Value> {
        let id = required(input, "capability_id")?;
        let category = input
            .get("category")
            .and_then(Value::as_str)
            .unwrap_or("runtime");
        if !DEVELOPMENT_CATEGORIES
            .iter()
            .any(|(id, _, _)| *id == category)
            && category != "runtime"
        {
            anyhow::bail!("unknown development category '{category}'")
        }
        let now = chrono::Utc::now().to_rfc3339();
        let mut p = BTreeMap::new();
        for (key, value) in [
            ("id", id),
            (
                "title",
                input.get("title").and_then(Value::as_str).unwrap_or(""),
            ),
            (
                "description",
                input
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or(""),
            ),
            ("category", category),
            (
                "owner",
                input.get("owner").and_then(Value::as_str).unwrap_or(""),
            ),
            (
                "status",
                input
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or("planned"),
            ),
            (
                "surface",
                input
                    .get("schema_surface")
                    .and_then(Value::as_str)
                    .unwrap_or(""),
            ),
            (
                "cap",
                input
                    .get("required_capability")
                    .and_then(Value::as_str)
                    .unwrap_or(""),
            ),
            (
                "subid",
                input.get("subid").and_then(Value::as_str).unwrap_or(""),
            ),
            ("deps", &json_string(input.get("dependencies"), "[]")?),
            ("tests", &json_string(input.get("tests"), "[]")?),
            (
                "commit",
                input
                    .get("deployed_commit")
                    .and_then(Value::as_str)
                    .unwrap_or(""),
            ),
            (
                "blocker",
                input.get("blocker").and_then(Value::as_str).unwrap_or(""),
            ),
            ("now", &now),
        ] {
            p.insert(key.into(), DataValue::Str(value.into()));
        }
        p.insert(
            "live".into(),
            DataValue::Bool(
                input
                    .get("live_verified")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            ),
        );
        self.run(
            r#"
            ?[capability_id, title, description, category, owner, status, schema_surface,
              required_capability, subid, dependencies_json, tests_json,
              live_verified, deployed_commit, blocker, created_at, updated_at]
                <- [[$id, $title, $description, $category, $owner, $status, $surface,
                     $cap, $subid, $deps, $tests, $live, $commit, $blocker, $now, $now]]
            :put cognitive_development {
                capability_id => title, description, category, owner, status, schema_surface,
                required_capability, subid, dependencies_json, tests_json,
                live_verified, deployed_commit, blocker, created_at, updated_at
            }
        "#,
            p,
        )
        .context("upsert cognitive development capability")?;
        self.history(
            id,
            "upsert",
            input
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("planned"),
            "capability upserted",
            input
                .get("deployed_commit")
                .and_then(Value::as_str)
                .unwrap_or(""),
        )?;
        Ok(
            json!({"capability_id": id, "status": input.get("status").and_then(Value::as_str).unwrap_or("planned"), "updated_at": now}),
        )
    }

    fn record_verification(&self, input: &Value) -> Result<Value> {
        let id = required(input, "capability_id")?;
        let checks = input
            .get("checks")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let checks_passed = checks.iter().all(|check| {
            check
                .get("passed")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        });
        let has_checks = !checks.is_empty();
        let verified = input
            .get("live_verified")
            .and_then(Value::as_bool)
            .unwrap_or(!has_checks || checks_passed);
        let requested_status = input.get("status").and_then(Value::as_str);
        let status = if !verified || !checks_passed {
            "blocked"
        } else {
            requested_status.unwrap_or("verified")
        };
        let now = chrono::Utc::now().to_rfc3339();
        let mut p = BTreeMap::new();
        p.insert("id".into(), DataValue::Str(id.into()));
        p.insert("status".into(), DataValue::Str(status.into()));
        p.insert("live".into(), DataValue::Bool(verified));
        p.insert(
            "commit".into(),
            DataValue::Str(
                input
                    .get("commit")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .into(),
            ),
        );
        p.insert(
            "blocker".into(),
            DataValue::Str(
                input
                    .get("blocker")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .into(),
            ),
        );
        p.insert("now".into(), DataValue::Str(now.clone().into()));
        self.run(
            r#"
            ?[capability_id, status, live_verified, deployed_commit, blocker, updated_at]
                <- [[$id, $status, $live, $commit, $blocker, $now]]
            :update cognitive_development {
                capability_id => status, live_verified, deployed_commit, blocker, updated_at
            }
        "#,
            p,
        )
        .context("record cognitive development verification")?;
        let details = input.get("details").and_then(Value::as_str).unwrap_or("");
        let history_details = if checks.is_empty() {
            details.to_string()
        } else {
            json!({"details": details, "checks": checks, "checks_passed": checks_passed})
                .to_string()
        };
        self.history(
            id,
            "verification",
            status,
            &history_details,
            input.get("commit").and_then(Value::as_str).unwrap_or(""),
        )?;
        Ok(
            json!({"capability_id": id, "status": status, "live_verified": verified, "checks_passed": checks_passed, "check_count": checks.len(), "recorded_at": now}),
        )
    }

    fn history(
        &self,
        id: &str,
        event: &str,
        status: &str,
        details: &str,
        commit: &str,
    ) -> Result<()> {
        let recorded_at = chrono::Utc::now().to_rfc3339();
        let mut p = BTreeMap::new();
        for (k, v) in [
            ("id", id),
            ("at", recorded_at.as_str()),
            ("event", event),
            ("status", status),
            ("details", details),
            ("commit", commit),
        ] {
            p.insert(k.into(), DataValue::Str(v.into()));
        }
        self.run(r#"
            ?[capability_id, recorded_at, event, status, details, commit]
                <- [[$id, $at, $event, $status, $details, $commit]]
            :put cognitive_development_history { capability_id, recorded_at => event, status, details, commit }
        "#, p)?;
        Ok(())
    }

    fn history_list(&self, input: &Value) -> Result<Value> {
        let id = required(input, "capability_id")?;
        let mut params = BTreeMap::new();
        params.insert("id".into(), DataValue::Str(id.into()));
        let rows = self.run(
            r#"?[recorded_at, event, status, details, commit]
                := *cognitive_development_history[$id, recorded_at, event, status, details, commit]
                :order -recorded_at"#,
            params,
        )?;
        let history = rows
            .rows
            .iter()
            .map(|r| {
                json!({
                    "recorded_at": as_str(&r[0]), "event": as_str(&r[1]),
                    "status": as_str(&r[2]), "details": as_str(&r[3]), "commit": as_str(&r[4])
                })
            })
            .collect::<Vec<_>>();
        Ok(json!({"capability_id": id, "history": history, "count": history.len()}))
    }

    fn summary(&self) -> Result<Value> {
        let rows = self.run(
            "?[category, status] := *cognitive_development[_id, _title, _description, category, _owner, status, _surface, _cap, _subid, _deps, _tests, _live, _commit, _blocker, _created, _updated]",
            BTreeMap::new(),
        )?;
        let mut counts = BTreeMap::<(String, String), usize>::new();
        for row in &rows.rows {
            *counts
                .entry((as_str(&row[0]), as_str(&row[1])))
                .or_default() += 1;
        }
        let groups = counts.into_iter().map(|((category, status), count)| {
            json!({"category": category, "status": status, "count": count})
        }).collect::<Vec<_>>();
        Ok(json!({"groups": groups, "group_count": groups.len()}))
    }

    fn list(&self, input: &Value) -> Result<Value> {
        let status = input.get("status").and_then(Value::as_str);
        let category = input.get("category").and_then(Value::as_str);
        let (query, mut p) = if status.is_some() || category.is_some() {
            let status_clause = status.map(|_| ", status = $status").unwrap_or("");
            let category_clause = category.map(|_| ", category = $category").unwrap_or("");
            let mut p = BTreeMap::new();
            if let Some(status) = status {
                p.insert("status".into(), DataValue::Str(status.into()));
            }
            if let Some(category) = category {
                p.insert("category".into(), DataValue::Str(category.into()));
            }
            (format!("?[id, title, description, category, owner, status, surface, cap, subid, deps, tests, live, commit, blocker, created, updated] := *cognitive_development[id, title, description, category, owner, status, surface, cap, subid, deps, tests, live, commit, blocker, created, updated]{status_clause}{category_clause} :order id"), p)
        } else {
            ("?[id, title, description, category, owner, status, surface, cap, subid, deps, tests, live, commit, blocker, created, updated] := *cognitive_development[id, title, description, category, owner, status, surface, cap, subid, deps, tests, live, commit, blocker, created, updated] :order id".to_string(), BTreeMap::new())
        };
        let rows = self.run(&query, std::mem::take(&mut p))?;
        let capabilities = rows.rows.iter().map(|r| json!({
            "capability_id": as_str(&r[0]), "title": as_str(&r[1]), "description": as_str(&r[2]),
            "category": as_str(&r[3]), "owner": as_str(&r[4]), "status": as_str(&r[5]), "schema_surface": as_str(&r[6]),
            "required_capability": as_str(&r[7]), "subid": as_str(&r[8]),
            "dependencies": parse_json(&as_str(&r[9])), "tests": parse_json(&as_str(&r[10])),
            "live_verified": matches!(&r[11], DataValue::Bool(true)), "deployed_commit": as_str(&r[12]),
            "blocker": as_str(&r[13]), "created_at": as_str(&r[14]), "updated_at": as_str(&r[15])
        })).collect::<Vec<_>>();
        Ok(json!({"capabilities": capabilities, "count": capabilities.len()}))
    }
}

fn required<'a>(input: &'a Value, key: &str) -> Result<&'a str> {
    input
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow::anyhow!("missing required field '{key}'"))
}
fn json_string(value: Option<&Value>, fallback: &str) -> Result<String> {
    Ok(match value {
        Some(v) if !v.is_null() => serde_json::to_string(v)?,
        _ => fallback.into(),
    })
}
fn as_str(value: &DataValue) -> String {
    if let DataValue::Str(s) = value {
        s.to_string()
    } else {
        value.to_string()
    }
}
fn parse_json(value: &str) -> Value {
    serde_json::from_str(value).unwrap_or_else(|_| json!([]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracks_capability_and_verification_history() {
        let shuttle = Arc::new(CozoGraphShuttle::new_in_memory().expect("cozo"));
        let ledger = DevelopmentLedger::new(shuttle);
        let created = ledger
            .execute(&json!({
                "operation": "upsert",
                "capability_id": "cognitive.memory.query",
                "title": "Memory query",
                "status": "implemented",
                "tests": ["unit", "grpc"],
                "dependencies": ["cozo"]
            }))
            .expect("upsert");
        assert_eq!(created["status"], "implemented");
        let verified = ledger
            .execute(&json!({
                "operation": "record_verification",
                "capability_id": "cognitive.memory.query",
                "status": "verified",
                "live_verified": true,
                "details": "host UDS"
            }))
            .expect("verification");
        assert_eq!(verified["live_verified"], true);
        let listed = ledger
            .execute(&json!({"operation": "list", "status": "verified"}))
            .expect("list");
        assert_eq!(listed["count"], 1);
        assert_eq!(
            listed["capabilities"][0]["capability_id"],
            "cognitive.memory.query"
        );
        let categories = ledger
            .execute(&json!({"operation": "categories"}))
            .expect("categories");
        assert_eq!(categories["categories"].as_array().unwrap().len(), 14);
        let blocked = ledger
            .execute(&json!({
                "operation": "record_verification",
                "capability_id": "cognitive.memory.query",
                "checks": [{"name": "grpc", "passed": false, "details": "not exposed"}]
            }))
            .expect("blocked verification");
        assert_eq!(blocked["status"], "blocked");
        assert_eq!(blocked["checks_passed"], false);
        let summary = ledger
            .execute(&json!({"operation": "summary"}))
            .expect("summary");
        assert_eq!(summary["group_count"], 1);
        let history = ledger
            .execute(&json!({
                "operation": "history",
                "capability_id": "cognitive.memory.query"
            }))
            .expect("history");
        assert_eq!(history["count"], 3);
        assert!(ledger
            .execute(&json!({
                "operation": "upsert",
                "capability_id": "bad.category",
                "category": "not_a_real_category"
            }))
            .is_err());
    }
}
