use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

use base64::Engine as _;
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
                let body = if request.call_id == "call-generated_client" {
                    "{\"items\":[\"model-a\"]}".to_owned()
                } else {
                    format!("{} {}", request.request.method, request.request.path)
                };
                Ok(serde_json::to_value(PluginApiResponse {
                    status: 200,
                    headers: Default::default(),
                    body,
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

#[tokio::test]
async fn executes_slabpy_bundle_with_embedded_modules() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(temp.path().join("python")).unwrap();
    write_bundle(
        &temp.path().join("python/backend.slabpy"),
        json!({
            "format": "slab.python.bundle.v1",
            "entryModule": "plugin",
            "nativeExtensions": [],
            "modules": [
                {
                    "name": "plugin",
                    "isPackage": false,
                    "sourceBase64": encode_source(
                        "import slab\nfrom helper import inc\n\ndef run(params):\n    api = slab.api.request('GET', '/v1/models')\n    slab.ui.emit('bundled.done', {'value': params['value']})\n    return {'value': inc(params['value']), 'api': api['body']}\n"
                    )
                },
                {
                    "name": "helper",
                    "isPackage": false,
                    "sourceBase64": encode_source("def inc(value):\n    return value + 1\n")
                }
            ]
        }),
    );

    let host = Arc::new(RecordingHost::default());
    let runtime =
        PythonRuntime::with_config(PythonRuntimeConfig { host, ..PythonRuntimeConfig::default() });
    let mut request = request(temp.path(), "run", json!({ "value": 4 }));
    request.entry = "python/plugin.py".to_owned();
    request.bundle = Some("python/backend.slabpy".to_owned());

    let response = runtime.call(request).await.unwrap();

    assert_eq!(response.result, json!({ "value": 5, "api": "GET /v1/models" }));
}

#[tokio::test]
async fn exposes_generated_python_api_client_through_slab_bridge() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(temp.path().join("python")).unwrap();
    write_bundle(
        &temp.path().join("python/backend.slabpy"),
        json!({
            "format": "slab.python.bundle.v1",
            "entryModule": "plugin",
            "nativeExtensions": [],
            "modules": [
                {
                    "name": "plugin",
                    "isPackage": false,
                    "sourceBase64": encode_source(
                        "import slab\nfrom slab_api_client.api.models import list_models\n\ndef run(params):\n    client = slab.api.client()\n    return list_models.sync(client=client)\n"
                    )
                },
                {
                    "name": "slab_api_client",
                    "isPackage": true,
                    "sourceBase64": encode_source("")
                },
                {
                    "name": "slab_api_client.api",
                    "isPackage": true,
                    "sourceBase64": encode_source("")
                },
                {
                    "name": "slab_api_client.api.models",
                    "isPackage": true,
                    "sourceBase64": encode_source("")
                },
                {
                    "name": "slab_api_client.api.models.list_models",
                    "isPackage": false,
                    "sourceBase64": encode_source(
                        "def sync(*, client):\n    response = client.get_httpx_client().request(method='get', url='/v1/models')\n    return response.json()\n"
                    )
                },
                {
                    "name": "slab_api_client.bridge",
                    "isPackage": false,
                    "sourceBase64": encode_source(include_str!(
                        "../../../python/slab-python-sdk/src/slab_api_client/bridge.py"
                    ))
                },
                {
                    "name": "slab_api_client.client",
                    "isPackage": false,
                    "sourceBase64": encode_source(
                        "class Client:\n    def __init__(self, base_url, headers=None, raise_on_unexpected_status=False):\n        self.raise_on_unexpected_status = raise_on_unexpected_status\n        self._client = None\n        self._async_client = None\n    def set_httpx_client(self, client):\n        self._client = client\n        return self\n    def set_async_httpx_client(self, client):\n        self._async_client = client\n        return self\n    def get_httpx_client(self):\n        return self._client\n"
                    )
                },
                {
                    "name": "httpx",
                    "isPackage": false,
                    "sourceBase64": encode_source(
                        "import json\n\nclass BaseTransport:\n    pass\n\nclass AsyncBaseTransport:\n    pass\n\nclass URL:\n    def __init__(self, value):\n        self.raw_path = value.encode('ascii')\n\nclass Request:\n    def __init__(self, method, url, headers=None, content=b''):\n        self.method = method\n        self.url = URL(url)\n        self.headers = headers or {}\n        self.content = content\n\nclass Response:\n    def __init__(self, status_code, headers=None, content=b'', request=None):\n        self.status_code = status_code\n        self.headers = headers or {}\n        self.content = content\n        self.request = request\n    def json(self):\n        return json.loads(self.content.decode('utf-8'))\n\nclass Client:\n    def __init__(self, base_url='', headers=None, transport=None, **kwargs):\n        self.headers = headers or {}\n        self.transport = transport\n    def request(self, method, url, **kwargs):\n        content = kwargs.get('content') or b''\n        request = Request(method.upper(), url, kwargs.get('headers') or self.headers, content)\n        return self.transport.handle_request(request)\n\nclass AsyncClient(Client):\n    async def request(self, method, url, **kwargs):\n        content = kwargs.get('content') or b''\n        request = Request(method.upper(), url, kwargs.get('headers') or self.headers, content)\n        return await self.transport.handle_async_request(request)\n"
                    )
                }
            ]
        }),
    );

    let host = Arc::new(RecordingHost::default());
    let runtime =
        PythonRuntime::with_config(PythonRuntimeConfig { host, ..PythonRuntimeConfig::default() });
    let mut request = request(temp.path(), "run", Value::Null);
    request.call_id = "call-generated_client".to_owned();
    request.entry = "python/plugin.py".to_owned();
    request.bundle = Some("python/backend.slabpy".to_owned());

    let response = runtime.call(request).await.unwrap();

    assert_eq!(response.result, json!({ "items": ["model-a"] }));
}

#[tokio::test]
async fn rejects_python_bundle_with_native_extensions() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(temp.path().join("python")).unwrap();
    write_bundle(
        &temp.path().join("python/backend.slabpy"),
        json!({
            "format": "slab.python.bundle.v1",
            "entryModule": "plugin",
            "nativeExtensions": ["native.pyd"],
            "modules": []
        }),
    );

    let runtime = PythonRuntime::new();
    let mut request = request(temp.path(), "run", Value::Null);
    request.entry = "python/plugin.py".to_owned();
    request.bundle = Some("python/backend.slabpy".to_owned());
    let error = runtime.call(request).await.unwrap_err().to_string();

    assert!(error.contains("unsupported native extensions"));
}

fn request(root: &std::path::Path, export_name: &str, params: Value) -> PluginRuntimeCallRequest {
    PluginRuntimeCallRequest {
        call_id: format!("call-{export_name}"),
        plugin_id: "test.plugin".to_owned(),
        root_dir: root.to_string_lossy().into_owned(),
        entry: "plugin.py".to_owned(),
        bundle: None,
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

fn encode_source(source: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(source.as_bytes())
}

fn write_bundle(path: &std::path::Path, bundle: Value) {
    std::fs::write(path, serde_json::to_string(&bundle).unwrap()).unwrap();
}
