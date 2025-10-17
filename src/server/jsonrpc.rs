/// JSON-RPC 2.0 protocol implementation
/// Separate concern for protocol handling
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// JSON-RPC 2.0 request structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    pub params: Option<Value>,
}

/// JSON-RPC 2.0 response structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// JSON-RPC 2.0 error structure
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl JsonRpcResponse {
    /// Create a successful response
    pub fn success(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Create an error response
    pub fn error(id: Option<Value>, error: JsonRpcError) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(error),
        }
    }
}

impl JsonRpcError {
    /// Create a parse error (-32700)
    pub fn parse_error() -> Self {
        Self {
            code: -32700,
            message: "Parse error".to_string(),
            data: None,
        }
    }

    /// Create an invalid request error (-32600)
    pub fn invalid_request() -> Self {
        Self {
            code: -32600,
            message: "Invalid Request".to_string(),
            data: None,
        }
    }

    /// Create an invalid request error with custom message (-32600)
    pub fn invalid_request_with_message(message: &str) -> Self {
        Self {
            code: -32600,
            message: format!("Invalid Request: {}", message),
            data: None,
        }
    }

    /// Create a method not found error (-32601)
    pub fn method_not_found() -> Self {
        Self {
            code: -32601,
            message: "Method not found".to_string(),
            data: None,
        }
    }

    /// Create an invalid params error (-32602)
    pub fn invalid_params(message: &str) -> Self {
        Self {
            code: -32602,
            message: format!("Invalid params: {}", message),
            data: None,
        }
    }

    /// Create an internal error (-32603)
    pub fn internal_error(message: &str) -> Self {
        Self {
            code: -32603,
            message: format!("Internal error: {}", message),
            data: None,
        }
    }
}

/// Validate JSON-RPC 2.0 request format
pub fn validate_request(value: &Value) -> Result<JsonRpcRequest, JsonRpcError> {
    let request: JsonRpcRequest =
        serde_json::from_value(value.clone()).map_err(|_| JsonRpcError::invalid_request())?;

    if request.jsonrpc != "2.0" {
        return Err(JsonRpcError::invalid_request());
    }

    Ok(request)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_valid_jsonrpc_request() {
        let json = json!({
            "jsonrpc": "2.0",
            "method": "tools/list",
            "id": 1
        });

        let request = validate_request(&json).unwrap();
        assert_eq!(request.jsonrpc, "2.0");
        assert_eq!(request.method, "tools/list");
        assert_eq!(request.id, Some(json!(1)));
    }

    #[test]
    fn test_valid_jsonrpc_request_with_params() {
        let json = json!({
            "jsonrpc": "2.0",
            "method": "get_balance",
            "params": {"wallet_address": "0x123"},
            "id": "test-id"
        });

        let request = validate_request(&json).unwrap();
        assert_eq!(request.jsonrpc, "2.0");
        assert_eq!(request.method, "get_balance");
        assert_eq!(request.id, Some(json!("test-id")));
        assert!(request.params.is_some());
    }

    #[test]
    fn test_valid_jsonrpc_request_no_id() {
        let json = json!({
            "jsonrpc": "2.0",
            "method": "tools/list"
        });

        let request = validate_request(&json).unwrap();
        assert_eq!(request.jsonrpc, "2.0");
        assert_eq!(request.method, "tools/list");
        assert!(request.id.is_none());
    }

    #[test]
    fn test_invalid_jsonrpc_version() {
        let json = json!({
            "jsonrpc": "1.0",
            "method": "test",
            "id": 1
        });

        let result = validate_request(&json);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, -32600);
    }

    #[test]
    fn test_invalid_request_missing_method() {
        let json = json!({
            "jsonrpc": "2.0",
            "id": 1
        });

        let result = validate_request(&json);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, -32600);
    }

    #[test]
    fn test_invalid_request_malformed() {
        let json = json!({
            "invalid": "request"
        });

        let result = validate_request(&json);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, -32600);
    }

    #[test]
    fn test_success_response_creation() {
        let response = JsonRpcResponse::success(Some(json!(1)), json!({"result": "ok"}));
        assert_eq!(response.jsonrpc, "2.0");
        assert!(response.result.is_some());
        assert!(response.error.is_none());
        assert_eq!(response.id, Some(json!(1)));
    }

    #[test]
    fn test_success_response_no_id() {
        let response = JsonRpcResponse::success(None, json!({"data": "test"}));
        assert_eq!(response.jsonrpc, "2.0");
        assert!(response.result.is_some());
        assert!(response.error.is_none());
        assert!(response.id.is_none());
    }

    #[test]
    fn test_error_response_creation() {
        let error = JsonRpcError::method_not_found();
        let response = JsonRpcResponse::error(Some(json!(1)), error);
        assert_eq!(response.jsonrpc, "2.0");
        assert!(response.result.is_none());
        assert!(response.error.is_some());
    }

    #[test]
    fn test_parse_error() {
        let error = JsonRpcError::parse_error();
        assert_eq!(error.code, -32700);
        assert_eq!(error.message, "Parse error");
        assert!(error.data.is_none());
    }

    #[test]
    fn test_invalid_request_error() {
        let error = JsonRpcError::invalid_request();
        assert_eq!(error.code, -32600);
        assert_eq!(error.message, "Invalid Request");
        assert!(error.data.is_none());
    }

    #[test]
    fn test_method_not_found_error() {
        let error = JsonRpcError::method_not_found();
        assert_eq!(error.code, -32601);
        assert_eq!(error.message, "Method not found");
        assert!(error.data.is_none());
    }

    #[test]
    fn test_invalid_params_error() {
        let error = JsonRpcError::invalid_params("missing wallet_address");
        assert_eq!(error.code, -32602);
        assert!(error.message.contains("Invalid params"));
        assert!(error.message.contains("missing wallet_address"));
        assert!(error.data.is_none());
    }

    #[test]
    fn test_internal_error() {
        let error = JsonRpcError::internal_error("database connection failed");
        assert_eq!(error.code, -32603);
        assert!(error.message.contains("Internal error"));
        assert!(error.message.contains("database connection failed"));
        assert!(error.data.is_none());
    }

    #[test]
    fn test_jsonrpc_request_serialization() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(42)),
            method: "test_method".to_string(),
            params: Some(json!({"key": "value"})),
        };

        let serialized = serde_json::to_string(&request).unwrap();
        let deserialized: JsonRpcRequest = serde_json::from_str(&serialized).unwrap();

        assert_eq!(request.jsonrpc, deserialized.jsonrpc);
        assert_eq!(request.id, deserialized.id);
        assert_eq!(request.method, deserialized.method);
        assert_eq!(request.params, deserialized.params);
    }

    #[test]
    fn test_jsonrpc_response_serialization() {
        let response = JsonRpcResponse::success(Some(json!("test-id")), json!({"balance": "1.5"}));

        let serialized = serde_json::to_string(&response).unwrap();
        let deserialized: JsonRpcResponse = serde_json::from_str(&serialized).unwrap();

        assert_eq!(response.jsonrpc, deserialized.jsonrpc);
        assert_eq!(response.id, deserialized.id);
        assert_eq!(response.result, deserialized.result);
        assert_eq!(response.error, deserialized.error);
    }

    #[test]
    fn test_jsonrpc_error_serialization() {
        let error = JsonRpcError::invalid_params("test error");

        let serialized = serde_json::to_string(&error).unwrap();
        let deserialized: JsonRpcError = serde_json::from_str(&serialized).unwrap();

        assert_eq!(error.code, deserialized.code);
        assert_eq!(error.message, deserialized.message);
        assert_eq!(error.data, deserialized.data);
    }
}
