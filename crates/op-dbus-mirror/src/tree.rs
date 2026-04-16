//! Tree-walking and path management for D-Bus mirror

use simd_json::OwnedValue as Value;
use std::collections::HashMap;

/// Represents a node in the D-Bus hierarchy
pub struct MirrorNode {
    pub name: String,
    pub children: HashMap<String, MirrorNode>,
    pub data: Option<Value>,
}

impl MirrorNode {
    pub fn new(name: String) -> Self {
        Self {
            name,
            children: HashMap::new(),
            data: None,
        }
    }

    /// Insert a path into the tree
    pub fn insert(&mut self, path: &str, data: Value) {
        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        self.insert_recursive(&parts, data);
    }

    fn insert_recursive(&mut self, parts: &[&str], data: Value) {
        if parts.is_empty() {
            self.data = Some(data);
            return;
        }

        let first = parts[0];
        let remaining = &parts[1..];

        let entry = self
            .children
            .entry(first.to_string())
            .or_insert_with(|| MirrorNode::new(first.to_string()));

        entry.insert_recursive(remaining, data);
    }
}
