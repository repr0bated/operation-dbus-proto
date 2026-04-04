//! Filesystem Tools

use crate::tool_registry::{BoxedTool, Tool, ToolRegistry};
use anyhow::Result;
use async_trait::async_trait;
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;

pub async fn register_all(registry: &ToolRegistry) -> Result<usize> {
    registry.register(Arc::new(ReadFileTool)).await?;
    registry.register(Arc::new(WriteFileTool)).await?;
    registry.register(Arc::new(ListDirectoryTool)).await?;
    Ok(3)
}

pub struct ReadFileTool;

#[async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &str { "read_file" }
    fn description(&self) -> &str { "Read the contents of a file." }
    fn category(&self) -> &str { "filesystem" }
    fn tags(&self) -> Vec<String> { vec!["filesystem".into(), "read".into()] }
    
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "Path to the file"}
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let path = input.get("path").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing path"))?;
        
        // Security check
        if path.starts_with("/etc/shadow") || path.starts_with("/etc/sudoers") {
            return Ok(json!({"success": false, "error": "Access denied"}));
        }
        
        match tokio::fs::read_to_string(path).await {
            Ok(content) => Ok(json!({"success": true, "path": path, "content": content})),
            Err(e) => Ok(json!({"success": false, "error": e.to_string()}))
        }
    }
}

pub struct WriteFileTool;

#[async_trait]
impl Tool for WriteFileTool {
    fn name(&self) -> &str { "write_file" }
    fn description(&self) -> &str { "Write content to a file." }
    fn category(&self) -> &str { "filesystem" }
    fn tags(&self) -> Vec<String> { vec!["filesystem".into(), "write".into()] }
    
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {"type": "string"},
                "content": {"type": "string"}
            },
            "required": ["path", "content"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let path = input.get("path").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing path"))?;
        let content = input.get("content").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing content"))?;
        
        // Security check - don't write to system dirs
        if path.starts_with("/etc/") || path.starts_with("/boot/") {
            return Ok(json!({"success": false, "error": "Access denied"}));
        }
        
        match tokio::fs::write(path, content).await {
            Ok(_) => Ok(json!({"success": true, "path": path, "bytes_written": content.len()})),
            Err(e) => Ok(json!({"success": false, "error": e.to_string()}))
        }
    }
}

pub struct ListDirectoryTool;

#[async_trait]
impl Tool for ListDirectoryTool {
    fn name(&self) -> &str { "list_directory" }
    fn description(&self) -> &str { "List contents of a directory." }
    fn category(&self) -> &str { "filesystem" }
    fn tags(&self) -> Vec<String> { vec!["filesystem".into(), "list".into()] }
    
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {"type": "string"}
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let path = input.get("path").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing path"))?;
        
        let mut entries = Vec::new();
        let mut dir = tokio::fs::read_dir(path).await?;
        
        while let Some(entry) = dir.next_entry().await? {
            let meta = entry.metadata().await?;
            entries.push(json!({
                "name": entry.file_name().to_string_lossy(),
                "is_dir": meta.is_dir(),
                "size": meta.len()
            }));
        }
        
        Ok(json!({"success": true, "path": path, "entries": entries}))
    }
}
