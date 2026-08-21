//! Shared validation for model-facing Cognitive MCP ingress.
//!
//! The stdio MCP tools and the bridge-mounted gRPC service both create
//! sessions and can trigger retrieval work. Keeping the basic bounds here
//! prevents one transport from becoming a bypass for the other.

pub const MAX_QUERY_BYTES: usize = 16 * 1024;
pub const MAX_CONVERSATION_ID_BYTES: usize = 256;

pub fn validate_query(query: &str) -> Result<(), String> {
    if query.trim().is_empty() {
        return Err("query must not be empty".to_string());
    }
    if query.len() > MAX_QUERY_BYTES {
        return Err(format!("query exceeds the {MAX_QUERY_BYTES}-byte limit"));
    }
    Ok(())
}

pub fn validate_conversation_id(conversation_id: &str) -> Result<(), String> {
    if conversation_id.len() > MAX_CONVERSATION_ID_BYTES {
        return Err(format!(
            "conversation_id exceeds the {MAX_CONVERSATION_ID_BYTES}-byte limit"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_and_conversation_bounds_are_transport_neutral() {
        assert!(validate_query("where is canonical ingress?").is_ok());
        assert!(validate_query("\t ").is_err());
        assert!(validate_query(&"q".repeat(MAX_QUERY_BYTES + 1)).is_err());
        assert!(validate_conversation_id("").is_ok());
        assert!(validate_conversation_id(&"s".repeat(MAX_CONVERSATION_ID_BYTES + 1)).is_err());
    }
}
