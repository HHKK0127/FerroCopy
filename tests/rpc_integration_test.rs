//! Integration test for JSON-RPC protocol.
//! Tests the full request/response cycle over TCP.

use serde_json::Value;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

/// Start a minimal RPC server on a random port, run the test closure, then shut down.
fn with_rpc_server<F>(test: F)
where
    F: FnOnce(u16) + Send + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind RPC server");
    let port = listener.local_addr().unwrap().port();

    let handle = thread::spawn(move || {
        for stream in listener.incoming().take(1) {
            match stream {
                Ok(mut stream) => {
                    let mut reader = BufReader::new(stream.try_clone().unwrap());
                    let mut line = String::new();
                    reader.read_line(&mut line).ok();

                    // Parse the JSON-RPC request
                    if let Ok(req) = serde_json::from_str::<Value>(&line) {
                        let id = req.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
                        let method = req
                            .get("method")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");

                        let response = match method {
                            "Ping" => serde_json::json!({
                                "jsonrpc": "2.0",
                                "id": id,
                                "result": "pong"
                            }),
                            "Echo" => {
                                let params = req.get("params");
                                serde_json::json!({
                                    "jsonrpc": "2.0",
                                    "id": id,
                                    "result": params
                                })
                            }
                            _ => serde_json::json!({
                                "jsonrpc": "2.0",
                                "id": id,
                                "error": {
                                    "code": -32601,
                                    "message": "Method not found"
                                }
                            }),
                        };

                        let resp_line =
                            serde_json::to_string(&response).unwrap() + "\n";
                        stream.write_all(resp_line.as_bytes()).ok();
                    }
                }
                Err(e) => eprintln!("RPC server error: {}", e),
            }
        }
    });

    test(port);
    handle.join().expect("RPC server thread panicked");
}

#[test]
fn test_rpc_ping_request() {
    with_rpc_server(|port| {
        let mut stream =
            TcpStream::connect(format!("127.0.0.1:{}", port)).expect("connect");
        let request = r#"{"jsonrpc":"2.0","id":1,"method":"Ping"}"#;
        stream.write_all(request.as_bytes()).ok();
        stream.write_all(b"\n").ok();
        stream.flush().ok();

        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).expect("read response");

        let resp: Value = serde_json::from_str(&line).expect("parse JSON");
        assert_eq!(resp["jsonrpc"], "2.0");
        assert_eq!(resp["id"], 1);
        assert_eq!(resp["result"], "pong");
    });
}

#[test]
fn test_rpc_echo_params() {
    with_rpc_server(|port| {
        let mut stream =
            TcpStream::connect(format!("127.0.0.1:{}", port)).expect("connect");
        let request =
            r#"{"jsonrpc":"2.0","id":2,"method":"Echo","params":{"key":"value"}}"#;
        stream.write_all(request.as_bytes()).ok();
        stream.write_all(b"\n").ok();
        stream.flush().ok();

        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).expect("read response");

        let resp: Value = serde_json::from_str(&line).expect("parse JSON");
        assert_eq!(resp["jsonrpc"], "2.0");
        assert_eq!(resp["id"], 2);
        assert_eq!(resp["result"]["key"], "value");
    });
}

#[test]
fn test_rpc_unknown_method_returns_error() {
    with_rpc_server(|port| {
        let mut stream =
            TcpStream::connect(format!("127.0.0.1:{}", port)).expect("connect");
        let request =
            r#"{"jsonrpc":"2.0","id":3,"method":"Unknown"}"#;
        stream.write_all(request.as_bytes()).ok();
        stream.write_all(b"\n").ok();
        stream.flush().ok();

        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).expect("read response");

        let resp: Value = serde_json::from_str(&line).expect("parse JSON");
        assert_eq!(resp["jsonrpc"], "2.0");
        assert_eq!(resp["id"], 3);
        assert_eq!(resp["error"]["code"], -32601);
        assert_eq!(resp["error"]["message"], "Method not found");
    });
}

#[test]
fn test_rpc_malformed_json_returns_no_response() {
    // Server should not crash on bad input, but may not respond
    // (the server reads until newline, so malformed JSON produces no reply)
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().unwrap().port();

    let server = thread::spawn(move || {
        for stream in listener.incoming().take(1) {
            if let Ok(mut stream) = stream {
                let mut buf = [0u8; 1024];
                stream.read(&mut buf).ok();
                // Server should not crash; just close silently
                // (no valid JSON → no response)
            }
        }
    });

    let mut stream = TcpStream::connect(format!("127.0.0.1:{}", port)).expect("connect");
    stream.write_all(b"not valid json\n").ok();
    stream.flush().ok();
    // No response expected — just ensure no panic on server side
    drop(stream);
    server.join().expect("server panicked on bad input");
}