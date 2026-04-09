//! JSON-RPC 2.0 protocol types for OP-DBUS
//!
//! Provides JSON-RPC request/response types and error handling.

use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;
use simd_json::prelude::*;
use simd_json::ValueBuilder;

/// JSON-RPC version constant
pub const JSONRPC_VERSION: &str = "2.0";

// ============================================================================
// STANDARD ERROR CODES
// ============================================================================

/// Standard JSON-RPC error codes
pub mod error_codes {
    pub const PARSE_ERROR: i32 = -32700;
    pub const INVALID_REQUEST: i32 = -32600;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL_ERROR: i32 = -32603;

    // Custom error codes (application-specific)
    pub const DATABASE_ERROR: i32 = -32000;
    pub const CONNECTION_ERROR: i32 = -32001;
    pub const NOT_FOUND: i32 = -32002;
    pub const PERMISSION_DENIED: i32 = -32003;
    pub const TOOL_ERROR: i32 = -32004;
    pub const POLICY_ERROR: i32 = -32005;
}

// ============================================================================
// JSON-RPC REQUEST
// ============================================================================

/// JSON-RPC 2.0 request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    #[serde(default)]
    pub params: Value,
    pub id: Value,
}

impl JsonRpcRequest {
    /// Create a new JSON-RPC request
    pub fn new(method: impl Into<String>, params: Value) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            method: method.into(),
            params,
            id: 1.into(),
        }
    }

    /// Create with a specific ID
    pub fn with_id(method: impl Into<String>, params: Value, id: Value) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            method: method.into(),
            params,
            id,
        }
    }

    /// Create a notification (no id)
    pub fn notification(method: impl Into<String>, params: Value) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            method: method.into(),
            params,
            id: Value::null(),
        }
    }
}

// ============================================================================
// JSON-RPC RESPONSE
// ============================================================================

/// JSON-RPC 2.0 response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
    pub id: Value,
}

impl JsonRpcResponse {
    /// Create a success response
    pub fn success(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            result: Some(result),
            error: None,
            id,
        }
    }

    /// Create an error response from JsonRpcError
    pub fn error(id: Value, err: JsonRpcError) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            result: None,
            error: Some(err),
            id,
        }
    }

    /// Create an error response with code and message
    pub fn error_with_code(id: Value, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
                data: None,
            }),
            id,
        }
    }

    /// Create an error response with data
    pub fn error_with_data(id: Value, code: i32, message: impl Into<String>, data: Value) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
                data: Some(data),
            }),
            id,
        }
    }

    /// Check if this response is a success
    pub fn is_success(&self) -> bool {
        self.error.is_none() && self.result.is_some()
    }

    /// Check if this response is an error
    pub fn is_error(&self) -> bool {
        self.error.is_some()
    }
}

// ============================================================================
// JSON-RPC ERROR
// ============================================================================

/// JSON-RPC 2.0 error object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl JsonRpcError {
    /// Create a new error
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    /// Create a new error with data
    pub fn with_data(code: i32, message: impl Into<String>, data: Value) -> Self {
        Self {
            code,
            message: message.into(),
            data: Some(data),
        }
    }

    /// Create a parse error
    pub fn parse_error(msg: impl Into<String>) -> Self {
        Self::new(error_codes::PARSE_ERROR, msg)
    }

    /// Create an invalid request error
    pub fn invalid_request(msg: impl Into<String>) -> Self {
        Self::new(error_codes::INVALID_REQUEST, msg)
    }

    /// Create a method not found error
    pub fn method_not_found(method: &str) -> Self {
        Self::new(error_codes::METHOD_NOT_FOUND, format!("Method not found: {}", method))
    }

    /// Create an invalid params error
    pub fn invalid_params(msg: impl Into<String>) -> Self {
        Self::new(error_codes::INVALID_PARAMS, msg)
    }

    /// Create an internal error
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::new(error_codes::INTERNAL_ERROR, msg)
    }

    /// Create a tool execution error
    pub fn tool_error(msg: impl Into<String>) -> Self {
        Self::new(error_codes::TOOL_ERROR, msg)
    }
}

impl std::fmt::Display for JsonRpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "JSON-RPC Error {}: {}", self.code, self.message)
    }
}

impl std::error::Error for JsonRpcError {}

// ============================================================================
// UTILITY FUNCTIONS
// ============================================================================

/// Parse a JSON-RPC request from a JSON value
pub fn parse_request(value: Value) -> Result<JsonRpcRequest, JsonRpcResponse> {
    let id = value.get("id").cloned().unwrap_or(Value::null());
    simd_json::serde::from_owned_value(value).map_err(|e| {
        JsonRpcResponse::error_with_code(
            id,
            error_codes::INVALID_REQUEST,
            format!("Invalid request: {}", e),
        )
    })
}

/// Parse a JSON-RPC request from a string
pub fn parse_request_str(input: &str) -> Result<JsonRpcRequest, JsonRpcResponse> {
    let mut input_mut = input.to_string();
    let value: Value = unsafe { simd_json::from_str(&mut input_mut) }.map_err(|e| {
        JsonRpcResponse::error_with_code(
            Value::null(),
            error_codes::PARSE_ERROR,
            format!("Parse error: {}", e),
        )
    })?;
    parse_request(value)
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_serialization() {
        let req = JsonRpcRequest::new("test", simd_json::json!(["arg1", "arg2"]));
        let json = simd_json::to_string(&req).unwrap();
        assert!(json.contains("\"method\":\"test\""));
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
    }

    #[test]
    fn test_response_success() {
        let resp = JsonRpcResponse::success(Value::Number(1.into()), simd_json::json!({"ok": true}));
        let json = simd_json::to_string(&resp).unwrap();
        assert!(json.contains("\"result\""));
        assert!(!json.contains("\"error\""));
        assert!(resp.is_success());
    }

    #[test]
    fn test_response_error() {
        let resp = JsonRpcResponse::error_with_code(
            Value::Number(1.into()),
            error_codes::METHOD_NOT_FOUND,
            "Method not found",
        );
        assert!(resp.is_error());
        assert!(!resp.is_success());
    }

    #[test]
    fn test_parse_request() {
        let json = r#"{"jsonrpc":"2.0","method":"test","params":{},"id":1}"#;
        let req = parse_request_str(json).unwrap();
        assert_eq!(req.method, "test");
    }

    #[test]
    fn test_error_constructors() {
        let _ = JsonRpcError::parse_error("bad json");
        let _ = JsonRpcError::method_not_found("foo");
        let _ = JsonRpcError::internal("oops");
    }
}
