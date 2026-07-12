//! RPC — JSON-RPC proxy separation for command dispatch.
//!
//! Inspired by Lapce's RPC architecture. Allows external tools to
//! send commands to a running FerroCopy instance over a simple
//! JSON-RPC channel.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Possible JSON-RPC methods.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "params")]
pub enum RpcMethod {
    Ping,
    StartCopy {
        source: String,
        destination: String,
        recursive: bool,
        verify: bool,
    },
    Pause,
    Resume,
    Cancel,
    GetStatus,
}

/// A JSON-RPC request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcRequest {
    pub jsonrpc: String,
    pub id: u64,
    #[serde(flatten)]
    pub method: RpcMethod,
}

/// A JSON-RPC response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcResponse {
    pub jsonrpc: String,
    pub id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
}

/// Parse a JSON string into an RPC request.
pub fn parse_request(json: &str) -> Result<RpcRequest, String> {
    serde_json::from_str(json).map_err(|e| format!("JSON parse error: {}", e))
}

/// Create a success response.
pub fn success_response(id: u64, result: Value) -> RpcResponse {
    RpcResponse {
        jsonrpc: "2.0".into(),
        id,
        result: Some(result),
        error: None,
    }
}

/// Create an error response.
pub fn error_response(id: u64, code: i64, message: String) -> RpcResponse {
    RpcResponse {
        jsonrpc: "2.0".into(),
        id,
        result: None,
        error: Some(RpcError { code, message }),
    }
}

/// Handle an RPC method call and return a response value.
pub fn handle_method(req: &RpcRequest) -> RpcResponse {
    match &req.method {
        RpcMethod::Ping => {
            success_response(req.id, Value::String("pong".into()))
        }
        RpcMethod::GetStatus => {
            success_response(req.id, serde_json::json!({"status": "idle"}))
        }
        RpcMethod::StartCopy { .. } => {
            success_response(req.id, serde_json::json!({"accepted": true}))
        }
        RpcMethod::Pause | RpcMethod::Resume | RpcMethod::Cancel => {
            success_response(req.id, serde_json::json!({"ok": true}))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ping_request() {
        let json = r#"{"jsonrpc":"2.0","id":1,"method":"Ping"}"#;
        let req = parse_request(json).unwrap();
        assert_eq!(req.id, 1);
        let resp = handle_method(&req);
        assert_eq!(resp.result.unwrap(), "pong");
    }

    #[test]
    fn test_invalid_json() {
        let res = parse_request("not json");
        assert!(res.is_err());
    }

    #[test]
    fn test_error_response_format() {
        let resp = error_response(1, -32601, "Method not found".into());
        assert_eq!(resp.jsonrpc, "2.0");
        assert!(resp.result.is_none());
        assert_eq!(resp.error.unwrap().code, -32601);
    }
}