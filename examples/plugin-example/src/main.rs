/// Zenith Reference Plugin — example-backend
///
/// This binary demonstrates the Zenith plugin protocol.
/// It reads JSON-RPC requests from stdin (one per line) and writes
/// JSON-RPC responses to stdout (one per line).
///
/// Test it manually:
///   echo '{"method":"name","params":null,"id":0}' | ./zenith-plugin-example
///   echo '{"method":"execute","params":{"cmd":"echo hello","env":{},"lab_id":"x","base_os":"local","target_arch":"x86_64","working_directory":null},"id":2}' | ./zenith-plugin-example

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{self, BufRead, Write};

#[derive(Deserialize)]
struct Request {
    method: String,
    params: Value,
    id:     u64,
}

#[derive(Serialize)]
struct Response {
    result: Option<Value>,
    error:  Option<String>,
    id:     u64,
}

fn main() {
    let stdin  = io::stdin();
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) if !l.trim().is_empty() => l,
            _ => continue,
        };

        let resp = match serde_json::from_str::<Request>(&line) {
            Err(e) => Response {
                result: None,
                error:  Some(format!("JSON parse error: {}", e)),
                id:     0,
            },
            Ok(req) => handle(req),
        };

        let _ = serde_json::to_writer(&mut out, &resp);
        let _ = out.write_all(b"\n");
        let _ = out.flush();
    }
}

fn handle(req: Request) -> Response {
    let id = req.id;
    match req.method.as_str() {
        // Return the plugin's self-reported name
        "name" => ok(id, json!("example-backend")),

        // Provision: nothing to set up for this simple backend
        "provision" => ok(id, Value::Null),

        // Execute: run the command locally
        "execute" => {
            let cmd = req.params["cmd"].as_str().unwrap_or("").to_string();
            let wd  = req.params["working_directory"].as_str().map(str::to_string);
            let env: HashMap<String, String> = req.params["env"]
                .as_object()
                .map(|m| m.iter().filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string()))).collect())
                .unwrap_or_default();

            let mut builder = if cfg!(target_os = "windows") {
                let mut b = std::process::Command::new("cmd");
                b.args(["/C", &cmd]);
                b
            } else {
                let mut b = std::process::Command::new("sh");
                b.args(["-c", &cmd]);
                b
            };

            if let Some(dir) = wd {
                builder.current_dir(dir);
            }
            for (k, v) in &env {
                builder.env(k, v);
            }

            // IMPORTANT: Pipe child stdout/stderr so they don't corrupt the
            // JSON response channel (plugin's own stdout is read by Zenith).
            // Forward captured output to the plugin's stderr (→ terminal).
            builder.stdout(std::process::Stdio::piped());
            builder.stderr(std::process::Stdio::piped());

            match builder.output() {
                Err(e) => err(id, format!("Failed to spawn command: {}", e)),
                Ok(out) => {
                    // Forward command output to terminal via the plugin's stderr
                    let stderr = io::stderr();
                    let mut se = stderr.lock();
                    if !out.stdout.is_empty() { let _ = se.write_all(&out.stdout); }
                    if !out.stderr.is_empty() { let _ = se.write_all(&out.stderr); }

                    if out.status.success() {
                        ok(id, Value::Null)
                    } else {
                        err(id, format!("Command exited with status {}", out.status))
                    }
                }
            }
        }

        // Teardown: nothing to clean up
        "teardown" => ok(id, Value::Null),

        other => err(id, format!("Unknown method: {}", other)),
    }
}

fn ok(id: u64, result: Value) -> Response {
    Response { result: Some(result), error: None, id }
}

fn err(id: u64, msg: String) -> Response {
    Response { result: None, error: Some(msg), id }
}
