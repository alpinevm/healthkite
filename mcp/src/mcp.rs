use crate::wirebody::{
    integer_arg, optional_string_arg, string_arg, WirebodyClient, WirebodyError,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use thiserror::Error;

const JSONRPC_VERSION: &str = "2.0";
const DEFAULT_PROTOCOL_VERSION: &str = "2024-11-05";
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_REQUEST: i64 = -32600;
const INVALID_PARAMS: i64 = -32602;
const PARSE_ERROR: i64 = -32700;

#[derive(Debug, Error)]
pub enum McpRuntimeError {
    #[error("stdin read failed: {0}")]
    Stdin(String),
    #[error("stdout write failed: {0}")]
    Stdout(String),
}

#[derive(Debug, Deserialize)]
struct JsonRpcMessage {
    id: Option<Value>,
    method: Option<String>,
    params: Option<Value>,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse<'a> {
    jsonrpc: &'a str,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i64,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

#[derive(Debug, Error)]
enum ToolCallError {
    #[error("{message}")]
    Mcp {
        code: i64,
        message: String,
        data: Option<Value>,
    },
    #[error(transparent)]
    Wirebody(#[from] WirebodyError),
}

pub fn run_stdio(client: WirebodyClient) -> Result<(), McpRuntimeError> {
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line.map_err(|error| McpRuntimeError::Stdin(error.to_string()))?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let response = handle_line(&client, trimmed);
        if let Some(response) = response {
            serde_json::to_writer(&mut stdout, &response)
                .map_err(|error| McpRuntimeError::Stdout(error.to_string()))?;
            stdout
                .write_all(b"\n")
                .map_err(|error| McpRuntimeError::Stdout(error.to_string()))?;
            stdout
                .flush()
                .map_err(|error| McpRuntimeError::Stdout(error.to_string()))?;
        }
    }
    Ok(())
}

fn handle_line(client: &WirebodyClient, line: &str) -> Option<JsonRpcResponse<'static>> {
    let message = match serde_json::from_str::<JsonRpcMessage>(line) {
        Ok(message) => message,
        Err(error) => {
            return Some(error_response(
                Value::Null,
                PARSE_ERROR,
                format!("Parse error: {error}"),
                None,
            ));
        }
    };

    let Some(method) = message.method.as_deref() else {
        return message.id.map(|id| {
            error_response(
                id,
                INVALID_REQUEST,
                "Invalid JSON-RPC request".to_string(),
                None,
            )
        });
    };

    if message.id.is_none() {
        return None;
    }

    let id = message.id.unwrap_or(Value::Null);
    match handle_request(client, method, message.params.as_ref()) {
        Ok(result) => Some(success_response(id, result)),
        Err(error) => Some(tool_error_response(id, error)),
    }
}

fn handle_request(
    client: &WirebodyClient,
    method: &str,
    params: Option<&Value>,
) -> Result<Value, ToolCallError> {
    match method {
        "initialize" => Ok(initialize_result(params)),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tools() })),
        "tools/call" => call_tool_request(client, params),
        _ => Err(ToolCallError::Mcp {
            code: METHOD_NOT_FOUND,
            message: format!("Unknown method: {method}"),
            data: None,
        }),
    }
}

fn initialize_result(params: Option<&Value>) -> Value {
    let protocol_version = params
        .and_then(|params| params.get("protocolVersion"))
        .and_then(Value::as_str)
        .unwrap_or(DEFAULT_PROTOCOL_VERSION);
    json!({
        "protocolVersion": protocol_version,
        "capabilities": { "tools": {} },
        "serverInfo": {
            "name": "wirebody-mcp",
            "version": env!("CARGO_PKG_VERSION")
        }
    })
}

fn call_tool_request(
    client: &WirebodyClient,
    params: Option<&Value>,
) -> Result<Value, ToolCallError> {
    let params = params
        .and_then(Value::as_object)
        .ok_or_else(|| ToolCallError::Mcp {
            code: INVALID_PARAMS,
            message: "tools/call requires object params".to_string(),
            data: None,
        })?;
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| ToolCallError::Mcp {
            code: INVALID_PARAMS,
            message: "tools/call requires a tool name".to_string(),
            data: None,
        })?;
    let args = params.get("arguments").and_then(Value::as_object);
    let text = call_tool(client, name, args)?;
    Ok(json!({ "content": [{ "type": "text", "text": text }] }))
}

fn call_tool(
    client: &WirebodyClient,
    name: &str,
    args: Option<&serde_json::Map<String, Value>>,
) -> Result<String, ToolCallError> {
    match name {
        "status" => Ok(client.status()?),
        "list_workouts" => {
            let limit = integer_arg(args.and_then(|args| args.get("limit")), 50, 1, Some(200));
            let offset = integer_arg(args.and_then(|args| args.get("offset")), 0, 0, None);
            Ok(client.list_workouts(limit, offset)?)
        }
        "get_workout" => {
            let uuid = string_arg(args.and_then(|args| args.get("uuid"))).ok_or_else(|| {
                ToolCallError::Mcp {
                    code: INVALID_PARAMS,
                    message: "get_workout requires a non-empty uuid string".to_string(),
                    data: None,
                }
            })?;
            Ok(client.get_workout(&uuid)?)
        }
        "list_quantity_types" => Ok(client.list_quantity_types()?),
        "get_quantity_series" => {
            let quantity_type =
                string_arg(args.and_then(|args| args.get("type"))).ok_or_else(|| {
                    ToolCallError::Mcp {
                        code: INVALID_PARAMS,
                        message: "get_quantity_series requires a non-empty type string".to_string(),
                        data: None,
                    }
                })?;
            let from = optional_string_arg(args.and_then(|args| args.get("from")));
            let to = optional_string_arg(args.and_then(|args| args.get("to")));
            let limit = integer_arg(
                args.and_then(|args| args.get("limit")),
                5000,
                1,
                Some(50000),
            );
            let offset = integer_arg(args.and_then(|args| args.get("offset")), 0, 0, None);
            Ok(client.get_quantity_series(
                &quantity_type,
                from.as_deref(),
                to.as_deref(),
                limit,
                offset,
            )?)
        }
        "list_sleep_sessions" => {
            let from = optional_string_arg(args.and_then(|args| args.get("from")));
            let to = optional_string_arg(args.and_then(|args| args.get("to")));
            let limit = integer_arg(args.and_then(|args| args.get("limit")), 30, 1, Some(365));
            let offset = integer_arg(args.and_then(|args| args.get("offset")), 0, 0, None);
            Ok(client.list_sleep_sessions(from.as_deref(), to.as_deref(), limit, offset)?)
        }
        "get_day_snapshot" => {
            let date = string_arg(args.and_then(|args| args.get("date"))).ok_or_else(|| {
                ToolCallError::Mcp {
                    code: INVALID_PARAMS,
                    message: "get_day_snapshot requires a date string in YYYY-MM-DD form"
                        .to_string(),
                    data: None,
                }
            })?;
            Ok(client.get_day_snapshot(&date)?)
        }
        _ => Err(ToolCallError::Mcp {
            code: METHOD_NOT_FOUND,
            message: format!("Unknown tool: {name}"),
            data: None,
        }),
    }
}

fn success_response(id: Value, result: Value) -> JsonRpcResponse<'static> {
    JsonRpcResponse {
        jsonrpc: JSONRPC_VERSION,
        id,
        result: Some(result),
        error: None,
    }
}

fn error_response(
    id: Value,
    code: i64,
    message: String,
    data: Option<Value>,
) -> JsonRpcResponse<'static> {
    JsonRpcResponse {
        jsonrpc: JSONRPC_VERSION,
        id,
        result: None,
        error: Some(JsonRpcError {
            code,
            message,
            data,
        }),
    }
}

fn tool_error_response(id: Value, error: ToolCallError) -> JsonRpcResponse<'static> {
    match error {
        ToolCallError::Mcp {
            code,
            message,
            data,
        } => error_response(id, code, message, data),
        ToolCallError::Wirebody(error) => error_response(
            id,
            INVALID_REQUEST,
            error.to_string(),
            Some(json!({ "code": error.code() })),
        ),
    }
}

pub fn tools() -> Value {
    json!([
        {
            "name": "status",
            "description": "Get a quick status snapshot of the Wirebody iOS app's LAN server: name, version, workout count, and sample-encoding format.",
            "inputSchema": { "type": "object", "properties": {}, "required": [] }
        },
        {
            "name": "list_workouts",
            "description": "List workouts from the Wirebody iOS app, sorted by start date descending. Returns a paginated envelope with workout summaries and pagination metadata. Use this to discover workout UUIDs that can be passed to get_workout.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "limit": { "type": "integer", "minimum": 1, "maximum": 200, "default": 50, "description": "Number of workouts per page (1-200)." },
                    "offset": { "type": "integer", "minimum": 0, "default": 0, "description": "Pagination offset, 0-indexed." }
                },
                "required": []
            }
        },
        {
            "name": "get_workout",
            "description": "Fetch the full columnar workout detail for a single workout by UUID. Returns the same JSON shape Wirebody emits via share/copy/push: sampleEncoding 'columnar-v1', hoisted device+sourceRevision in 'sources', per-stream parallel arrays (unit, t, v, dur?, src?). Typical size ~50-100 KB for a long workout.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "uuid": { "type": "string", "description": "Workout UUID, e.g., 06E9D6D6-7E37-406C-8465-1DAAD7C223A1. Get one from list_workouts." }
                },
                "required": ["uuid"]
            }
        },
        {
            "name": "list_quantity_types",
            "description": "List standalone HealthKit quantity types that Wirebody can export, including category, preferred unit, aggregation style, readable sample count, and first/last sample timestamps.",
            "inputSchema": { "type": "object", "properties": {}, "required": [] }
        },
        {
            "name": "get_quantity_series",
            "description": "Fetch a standalone HealthKit quantity series in Wirebody's columnar-v1 shape. Use list_quantity_types to discover identifiers. Supports optional ISO-8601 from/to range, limit, and offset.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "type": { "type": "string", "description": "HealthKit quantity type identifier, e.g. HKQuantityTypeIdentifierHeartRate or HKQuantityTypeIdentifierStepCount." },
                    "from": { "type": "string", "description": "Optional ISO-8601 UTC start timestamp. Defaults to 7 days before to." },
                    "to": { "type": "string", "description": "Optional ISO-8601 UTC end timestamp. Defaults to now." },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 50000, "default": 5000, "description": "Maximum number of samples to return (1-50000)." },
                    "offset": { "type": "integer", "minimum": 0, "default": 0, "description": "Pagination offset, 0-indexed." }
                },
                "required": ["type"]
            }
        },
        {
            "name": "list_sleep_sessions",
            "description": "List reconciled HealthKit sleep sessions over a date range. Sessions contain in-bed, asleep, awake durations and sparse phase interval columns keyed by Apple's sleep phase names.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "from": { "type": "string", "description": "Optional ISO-8601 UTC start timestamp. Defaults to 30 days before to." },
                    "to": { "type": "string", "description": "Optional ISO-8601 UTC end timestamp. Defaults to now." },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 365, "default": 30, "description": "Maximum number of sessions to return (1-365)." },
                    "offset": { "type": "integer", "minimum": 0, "default": 0, "description": "Pagination offset, 0-indexed." }
                },
                "required": []
            }
        },
        {
            "name": "get_day_snapshot",
            "description": "Fetch a single local-calendar day overview from Wirebody: sleep, workouts, activity totals, heart, body, and mobility. Returns the same numbers as the iOS Day view.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "date": { "type": "string", "description": "Local date in YYYY-MM-DD form, e.g. 2026-05-11." }
                },
                "required": ["date"]
            }
        }
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::{HttpError, HttpResponse, HttpTransport};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[test]
    fn initialize_echoes_protocol_and_capabilities() {
        let client = client_without_server();
        let response = handle_line(
            &client,
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"test","version":"0"}}}"#,
        )
        .unwrap();
        let value = serde_json::to_value(response).unwrap();
        assert_eq!(value["result"]["protocolVersion"], "2025-11-25");
        assert_eq!(value["result"]["capabilities"]["tools"], json!({}));
    }

    #[test]
    fn notifications_do_not_emit_responses() {
        let client = client_without_server();
        assert!(handle_line(
            &client,
            r#"{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}"#,
        )
        .is_none());
    }

    #[test]
    fn status_tool_returns_text_content_from_wirebody() {
        let body = r#"{"name":"Wirebody","sampleEncoding":"columnar-v1","version":"1.0","workoutCount":3}"#;
        let client = client_with_response(body);
        let rpc = handle_line(
            &client,
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"status","arguments":{}}}"#,
        )
        .unwrap();

        let value = serde_json::to_value(rpc).unwrap();
        assert_eq!(
            value["result"]["content"][0]["text"],
            "{\"name\":\"Wirebody\",\"sampleEncoding\":\"columnar-v1\",\"version\":\"1.0\",\"workoutCount\":3}"
        );
    }

    #[test]
    fn invalid_tool_params_return_invalid_params_error() {
        let client = client_without_server();
        let response = handle_line(
            &client,
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"get_workout","arguments":{}}}"#,
        )
        .unwrap();
        let value = serde_json::to_value(response).unwrap();
        assert_eq!(value["error"]["code"], INVALID_PARAMS);
        assert_eq!(
            value["error"]["message"],
            "get_workout requires a non-empty uuid string"
        );
    }

    #[test]
    fn initialize_and_tools_list_do_not_touch_wirebody_backend() {
        let client = WirebodyClient::new("unused backend", Arc::new(PanicTransport));

        let initialize = handle_line(
            &client,
            r#"{"jsonrpc":"2.0","id":4,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}"#,
        )
        .unwrap();
        let tools = handle_line(
            &client,
            r#"{"jsonrpc":"2.0","id":5,"method":"tools/list","params":{}}"#,
        )
        .unwrap();

        assert!(serde_json::to_value(initialize).unwrap()["result"]["serverInfo"]["name"]
            == "wirebody-mcp");
        assert!(serde_json::to_value(tools).unwrap()["result"]["tools"]
            .as_array()
            .is_some_and(|tools| !tools.is_empty()));
    }

    #[test]
    fn backend_failure_returns_tool_error_and_later_request_still_works() {
        let client = WirebodyClient::new(
            "Bonjour service fixture._wirebody._tcp.local.",
            Arc::new(FlakyTransport {
                calls: AtomicUsize::new(0),
                response: HttpResponse {
                    status: 200,
                    reason: "OK".to_string(),
                    body: r#"{"name":"Wirebody","sampleEncoding":"columnar-v1","version":"1.0","workoutCount":0}"#
                        .to_string(),
                },
            }),
        );

        let failed = handle_line(
            &client,
            r#"{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"status","arguments":{}}}"#,
        )
        .unwrap();
        let failed = serde_json::to_value(failed).unwrap();
        assert_eq!(failed["error"]["data"]["code"], "Unreachable");

        let recovered = handle_line(
            &client,
            r#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"status","arguments":{}}}"#,
        )
        .unwrap();
        let recovered = serde_json::to_value(recovered).unwrap();
        assert_eq!(
            recovered["result"]["content"][0]["text"],
            "{\"name\":\"Wirebody\",\"sampleEncoding\":\"columnar-v1\",\"version\":\"1.0\",\"workoutCount\":0}"
        );
    }

    struct PanicTransport;

    impl HttpTransport for PanicTransport {
        fn get(&self, _path_and_query: &str) -> Result<HttpResponse, HttpError> {
            panic!("backend should not be contacted for MCP lifecycle methods");
        }
    }

    struct FlakyTransport {
        calls: AtomicUsize,
        response: HttpResponse,
    }

    impl HttpTransport for FlakyTransport {
        fn get(&self, _path_and_query: &str) -> Result<HttpResponse, HttpError> {
            if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
                return Err(HttpError::Connect("fixture unavailable".to_string()));
            }

            Ok(self.response.clone())
        }
    }

    struct ConstantTransport {
        response: HttpResponse,
    }

    impl HttpTransport for ConstantTransport {
        fn get(&self, _path_and_query: &str) -> Result<HttpResponse, HttpError> {
            Ok(self.response.clone())
        }
    }

    fn client_without_server() -> WirebodyClient {
        client_with_response("{}")
    }

    fn client_with_response(body: &str) -> WirebodyClient {
        WirebodyClient::new(
            "https://wirebody.local:5606",
            Arc::new(ConstantTransport {
                response: HttpResponse {
                    status: 200,
                    reason: "OK".to_string(),
                    body: body.to_string(),
                },
            }),
        )
    }
}
