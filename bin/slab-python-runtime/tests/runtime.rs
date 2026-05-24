use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use slab_python_runtime::{PythonRuntime, PythonRuntimeConfig, RuntimeHost};
use slab_types::{
    PluginApiResponse, PluginPermissionsManifest, PluginRuntimeApiHostRequest,
    PluginRuntimeCallRequest, PluginRuntimeUiEmitRequest,
};

#[derive(Default)]
struct RecordingHost {
    calls: Mutex<Vec<String>>,
}

#[async_trait::async_trait]
impl RuntimeHost for RecordingHost {
    async fn request(&self, method: &str, params: Value) -> Result<Value, String> {
        self.calls.lock().unwrap().push(method.to_owned());
        match method {
            "slab.api.request" => {
                let request: PluginRuntimeApiHostRequest =
                    serde_json::from_value(params).map_err(|error| error.to_string())?;
                Ok(serde_json::to_value(PluginApiResponse {
                    status: 200,
                    headers: Default::default(),
                    body: format!("{} {}", request.request.method, request.request.path),
                })
                .map_err(|error| error.to_string())?)
            }
            "slab.ui.emit" => {
                let request: PluginRuntimeUiEmitRequest =
                    serde_json::from_value(params).map_err(|error| error.to_string())?;
                Ok(json!({
                    "pluginId": request.plugin_id,
                    "topic": request.topic,
                    "data": request.data,
                    "ts": 1
                }))
            }
            _ => Err(format!("unexpected host method `{method}`")),
        }
    }
}

#[tokio::test]
async fn executes_python_plugins_with_json_vfs_callbacks_and_restrictions() {
    let temp = tempfile::tempdir().unwrap();
    let entry = temp.path().join("plugin.py");
    std::fs::write(
        &entry,
        r#"
import json
import slab
from vfstest.child import inc

def run(params):
    api = slab.api.request("GET", "/v1/models")
    event = slab.ui.emit("finished", {"value": params["value"]})
    return {
        "value": inc(params["value"]),
        "api": api["body"],
        "event": event["topic"],
        "plugin": slab.plugin_id,
    }

def legacy(params_json):
    params = json.loads(params_json)
    return json.dumps({"legacy": params["value"]})

def blocked_file(params):
    open("blocked.txt", "w")

def blocked_socket(params):
    import socket
    socket.socket()
"#,
    )
    .unwrap();

    let host = Arc::new(RecordingHost::default());
    let mut config = PythonRuntimeConfig { host: host.clone(), ..PythonRuntimeConfig::default() };
    config.embedded_stdlib.add_package("vfstest", b"VALUE = 41\n");
    config.embedded_stdlib.add_module("vfstest.child", b"def inc(value):\n    return value + 1\n");
    let runtime = PythonRuntime::with_config(config);

    let response = runtime.call(request(temp.path(), "run", json!({ "value": 2 }))).await.unwrap();
    assert_eq!(
        response.result,
        json!({
            "value": 3,
            "api": "GET /v1/models",
            "event": "finished",
            "plugin": "test.plugin"
        })
    );

    let response =
        runtime.call(request(temp.path(), "legacy", json!({ "value": "ok" }))).await.unwrap();
    assert_eq!(response.result, json!({ "legacy": "ok" }));

    let error = runtime
        .call(request(temp.path(), "blocked_file", Value::Null))
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains("PermissionError"));

    let error = runtime
        .call(request(temp.path(), "blocked_socket", Value::Null))
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains("PermissionError"));

    let calls = host.calls.lock().unwrap().clone();
    assert_eq!(calls, vec!["slab.api.request", "slab.ui.emit"]);

    assert_stdio_json_rpc(temp.path());
}

fn request(root: &std::path::Path, export_name: &str, params: Value) -> PluginRuntimeCallRequest {
    PluginRuntimeCallRequest {
        call_id: format!("call-{export_name}"),
        plugin_id: "test.plugin".to_owned(),
        root_dir: root.to_string_lossy().into_owned(),
        entry: "plugin.py".to_owned(),
        export_name: export_name.to_owned(),
        params,
        permissions: PluginPermissionsManifest::default(),
        file_grants: Vec::new(),
        blocked_fetch_origins: Vec::new(),
    }
}

fn assert_stdio_json_rpc(root: &std::path::Path) {
    std::fs::write(
        root.join("stdio_plugin.py"),
        r#"
def run(params):
    return {"value": params["value"] + 1}
"#,
    )
    .unwrap();

    let runtime_exe = std::env::var("CARGO_BIN_EXE_slab-python-runtime").unwrap();
    let mut child = Command::new(runtime_exe)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut stdout = BufReader::new(stdout);

    let mut ready = String::new();
    stdout.read_line(&mut ready).unwrap();
    let ready: Value = serde_json::from_str(ready.trim()).unwrap();
    assert_eq!(ready["method"], "runtime.ready");
    assert_eq!(ready["params"]["runtime"], "slab-python-runtime");

    let mut call = request(root, "run", json!({ "value": 2 }));
    call.call_id = "stdio-call".to_owned();
    call.entry = "stdio_plugin.py".to_owned();
    let line = json!({
        "jsonrpc": "2.0",
        "id": "1",
        "method": "plugin.call",
        "params": call
    });
    writeln!(stdin, "{line}").unwrap();
    stdin.flush().unwrap();

    let mut response = String::new();
    stdout.read_line(&mut response).unwrap();
    let response: Value = serde_json::from_str(response.trim()).unwrap();
    assert_eq!(response["id"], "1");
    assert_eq!(response["result"]["result"], json!({ "value": 3 }));

    let _ = child.kill();
    let _ = child.wait();
}
