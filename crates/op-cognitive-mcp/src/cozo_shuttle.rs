use std::path::PathBuf;
use anyhow::{Context, Result};
use cozo_lib::Db;
use serde_json::Value;

/// Verdict returned by the compliance graph evaluation
#[derive(Debug, Clone)]
pub struct PolicyVerdict {
    pub allow: bool,
    pub reason: String,
}

/// CozoDB graph database shuttle — relational/graph queries + compliance evaluation
pub struct CozoGraphShuttle {
    db: Db,
    #[allow(dead_code)]
    db_path: Option<PathBuf>,
}

impl CozoGraphShuttle {
    pub fn new_in_memory() -> Result<Self> {
        let db = Db::new()?;
        Ok(Self { db, db_path: None })
    }

    pub fn new_persistent(path: PathBuf) -> Result<Self> {
        let db = Db::new()?;
        Ok(Self { db, db_path: Some(path) })
    }

    pub fn from_env() -> Result<Self> {
        if let Ok(path) = std::env::var("COGNITIVE_MCP_COZO_DB_PATH") {
            Self::new_persistent(PathBuf::from(path))
        } else {
            Self::new_in_memory()
        }
    }

    pub fn run_query(&self, query: &str, params: Option<Value>) -> Result<Value> {
        let params_json = params.unwrap_or(Value::Null);
        self.db.run_script(query, params_json)
            .context("CozoDB query failed")
    }

    /// Evaluate a proposed mutation against the NIST/EU-AI-Act compliance graph.
    ///
    /// Queries for any Deny rules matching (plugin_id, operation). Returns
    /// Allow if no deny rule fires. This is synchronous — CozoDB is in-process.
    pub fn evaluate_mutation(&self, plugin_id: &str, operation: &str) -> PolicyVerdict {
        let query = r#"
            deny_rule[reason] :=
                *compliance_rule[plugin, op, action, reason],
                action = 'Deny',
                (plugin = $plugin || plugin = '*'),
                (op = $op || op = '*')
            ?[reason] := deny_rule[reason]
        "#;
        let params = serde_json::json!({ "plugin": plugin_id, "op": operation });

        match self.run_query(query, Some(params)) {
            Ok(result) => {
                let rows = result.get("rows")
                    .and_then(|r| r.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0);
                if rows > 0 {
                    let reason = result["rows"][0][0]
                        .as_str()
                        .unwrap_or("compliance rule violated")
                        .to_string();
                    PolicyVerdict { allow: false, reason }
                } else {
                    PolicyVerdict { allow: true, reason: "no deny rule matched".to_string() }
                }
            }
            // If the table doesn't exist yet (fresh instance), default allow
            Err(_) => PolicyVerdict { allow: true, reason: "compliance graph not seeded".to_string() },
        }
    }

    pub fn store_relationship(&self, src: &str, rel: &str, dst: &str, props: Option<Value>) -> Result<()> {
        let query = r#"
        ?[src, rel, dst, props] <- [[$src, $rel, $dst, $props]]
        :create _rel {src, rel, dst, props}
        "#;
        let params = serde_json::json!({ "src": src, "rel": rel, "dst": dst, "props": props.unwrap_or(Value::Null) });
        self.run_query(query, Some(params))?;
        Ok(())
    }

    pub fn query_relationships_from(&self, src: &str) -> Result<Value> {
        let query = "?[src, rel, dst, props] := _rel[src, rel, dst, props], src == $src";
        self.run_query(query, Some(serde_json::json!({"src": src})))
    }

    pub fn query_relationships_to(&self, dst: &str) -> Result<Value> {
        let query = "?[src, rel, dst, props] := _rel[src, rel, dst, props], dst == $dst";
        self.run_query(query, Some(serde_json::json!({"dst": dst})))
    }

    pub fn store_node(&self, id: &str, props: Value) -> Result<()> {
        let query = r#"
        ?[id, props] <- [[$id, $props]]
        :create _node {id, props}
        "#;
        self.run_query(query, Some(serde_json::json!({"id": id, "props": props})))?;
        Ok(())
    }

    pub fn query_node(&self, id: &str) -> Result<Value> {
        let query = "?[id, props] := _node[id, props], id == $id";
        self.run_query(query, Some(serde_json::json!({"id": id})))
    }

    pub fn traverse_graph(&self, start_node: &str, max_depth: u32) -> Result<Value> {
        let query = r#"
        start[node] := _node[node, _], node == $start
        path[node, path, depth] := start[node], path = [node], depth = 0
        path[to, path, depth] := path[from, path_so_far, d], _rel[from, _, to, _],
                               path = append(path_so_far, to), depth = d + 1, depth < $max_depth
        ?[node, path, depth] := path[node, path, depth]
        "#;
        self.run_query(query, Some(serde_json::json!({"start": start_node, "max_depth": max_depth})))
    }
}
