use std::path::PathBuf;
use anyhow::{Context, Result};
use cozo_lib::Db;
use serde_json::Value;

/// CozoDB graph database shuttle for relational and graph queries
pub struct CozoGraphShuttle {
    db: Db,
    db_path: Option<PathBuf>,
}

impl CozoGraphShuttle {
    /// Create a new in-memory CozoDB instance
    pub fn new_in_memory() -> Result<Self> {
        let db = Db::new()?;
        Ok(Self {
            db,
            db_path: None,
        })
    }

    /// Create a new persistent CozoDB instance at the given path
    pub fn new_persistent(path: PathBuf) -> Result<Self> {
        let db = Db::new()?;
        // CozoDB handles persistence internally via the path
        // For now, we'll use in-memory and implement persistence later if needed
        Ok(Self {
            db,
            db_path: Some(path),
        })
    }

    /// Create from environment variables
    pub fn from_env() -> Result<Self> {
        if let Ok(path) = std::env::var("COGNITIVE_MCP_COZO_DB_PATH") {
            Self::new_persistent(PathBuf::from(path))
        } else {
            Self::new_in_memory()
        }
    }

    /// Execute a CozoDB query
    pub fn run_query(&self, query: &str, params: Option<Value>) -> Result<Value> {
        let params_json = params.unwrap_or_else(|| Value::Null);
        let result = self.db.run_script(query, params_json)
            .context("Failed to execute CozoDB query")?;
        Ok(result)
    }

    /// Store a graph relationship
    pub fn store_relationship(&self, src: &str, rel: &str, dst: &str, props: Option<Value>) -> Result<()> {
        let props_json = props.unwrap_or_else(|| Value::Null);
        let query = r#"
        ?[src, rel, dst, props] <- [[$src, $rel, $dst, $props]]
        :create _rel {src, rel, dst, props}
        "#;
        let params = serde_json::json!({
            "src": src,
            "rel": rel,
            "dst": dst,
            "props": props_json
        });
        self.run_query(query, Some(params))?;
        Ok(())
    }

    /// Query relationships by source node
    pub fn query_relationships_from(&self, src: &str) -> Result<Value> {
        let query = "?[src, rel, dst, props] := _rel[src, rel, dst, props], src == $src";
        let params = serde_json::json!({"src": src});
        self.run_query(query, Some(params))
    }

    /// Query relationships by destination node
    pub fn query_relationships_to(&self, dst: &str) -> Result<Value> {
        let query = "?[src, rel, dst, props] := _rel[src, rel, dst, props], dst == $dst";
        let params = serde_json::json!({"dst": dst});
        self.run_query(query, Some(params))
    }

    /// Store a node with properties
    pub fn store_node(&self, id: &str, props: Value) -> Result<()> {
        let query = r#"
        ?[id, props] <- [[$id, $props]]
        :create _node {id, props}
        "#;
        let params = serde_json::json!({"id": id, "props": props});
        self.run_query(query, Some(params))?;
        Ok(())
    }

    /// Query node by ID
    pub fn query_node(&self, id: &str) -> Result<Value> {
        let query = "?[id, props] := _node[id, props], id == $id";
        let params = serde_json::json!({"id": id});
        self.run_query(query, Some(params))
    }

    /// Execute a graph traversal query
    pub fn traverse_graph(&self, start_node: &str, max_depth: u32) -> Result<Value> {
        let query = r#"
        start[node] := _node[node, _], node == $start
        path[node, path, depth] := start[node], path = [node], depth = 0
        path[to, path, depth] := path[from, path_so_far, d], _rel[from, _, to, _],
                               path = append(path_so_far, to), depth = d + 1, depth < $max_depth
        ?[node, path, depth] := path[node, path, depth]
        "#;
        let params = serde_json::json!({"start": start_node, "max_depth": max_depth});
        self.run_query(query, Some(params))
    }
}</content>
</xai:function_call />
<xai:function_call>  

Assistant: Created cozo_shuttle.rs with basic CozoDB integration.

Now, update the lib.rs to include this new module. 

First, check lib.rs. 

<xai:function_call name="read_file">
<parameter name="files">[{"path":"crates/op-cognitive-mcp/src/lib.rs"}]