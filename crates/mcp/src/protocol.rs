//! MCP over stdio: newline-delimited JSON-RPC 2.0. Logs go to stderr only.
use crate::tools::Tools;
use serde_json::{json, Value};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt};

const VERSIONS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];
const MAX_LINE: usize = 4 * 1024 * 1024;

const INSTRUCTIONS: &str = "Galroon is a local catalog for visual novel (galgame) collections. \
Works are titles; resources are scanned folders/files that should be matched to works; editions group resources. \
Typical tidy-up: galroon_overview -> list_resources(status=unmatched) or list_issues -> search_vndb -> match_resource; \
then update_work for play status/favorites and lists for curation. \
Catalog edits are revision-checked: always read the current revision first and pass it back. \
This server never moves, deletes or quarantines files. preview_organize only drafts a plan that the user must review and approve inside Galroon.";

fn reply(id: &Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

fn failure(id: &Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

pub async fn handle(tools: &Tools, message: Value) -> Option<Value> {
    let id = message.get("id").cloned();
    let method = message["method"].as_str().unwrap_or("");
    // Notifications (no id) and responses to our own requests never get a reply.
    let id = id?;
    if message.get("method").is_none() {
        return None;
    }
    let params = &message["params"];
    Some(match method {
        "initialize" => {
            let requested = params["protocolVersion"].as_str().unwrap_or("");
            let version = VERSIONS.iter().find(|v| **v == requested).copied().unwrap_or(VERSIONS[0]);
            reply(&id, json!({
                "protocolVersion": version,
                "capabilities": {"tools": {"listChanged": false}},
                "serverInfo": {"name": "galroon", "title": "Galroon", "version": env!("CARGO_PKG_VERSION")},
                "instructions": INSTRUCTIONS,
            }))
        }
        "ping" => reply(&id, json!({})),
        "tools/list" => reply(&id, json!({"tools": crate::tools::definitions()})),
        "tools/call" => {
            let Some(name) = params["name"].as_str() else {
                return Some(failure(&id, -32602, "Tool name required"));
            };
            if !crate::tools::exists(name) {
                return Some(failure(&id, -32602, &format!("Unknown tool: {name}")));
            }
            let arguments = if params["arguments"].is_null() { json!({}) } else { params["arguments"].clone() };
            // Tool failures are results, so the model can read them and recover.
            let (text, is_error) = match tools.call(name, &arguments).await {
                Ok(value) => (crate::tools::render(&value), false),
                Err(error) => (error, true),
            };
            reply(&id, json!({"content": [{"type": "text", "text": text}], "isError": is_error}))
        }
        _ => failure(&id, -32601, &format!("Method not found: {method}")),
    })
}

pub async fn serve<In: AsyncBufRead + Unpin, Out: AsyncWrite + Unpin>(tools: Tools, input: In, mut output: Out) -> std::io::Result<()> {
    let mut lines = input.lines();
    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        let response = if line.len() > MAX_LINE {
            Some(failure(&Value::Null, -32600, "Message too large"))
        } else {
            match serde_json::from_str::<Value>(&line) {
                Ok(message @ Value::Object(_)) => handle(&tools, message).await,
                Ok(_) => Some(failure(&Value::Null, -32600, "Batch requests are not supported")),
                Err(_) => Some(failure(&Value::Null, -32700, "Parse error")),
            }
        };
        if let Some(response) = response {
            output.write_all(format!("{response}\n").as_bytes()).await?;
            output.flush().await?;
        }
    }
    Ok(())
}
