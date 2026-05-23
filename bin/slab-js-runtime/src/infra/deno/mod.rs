use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, anyhow, bail};
use deno_ast::{
    DecoratorsTranspileOption, EmitOptions, ImportsNotUsedAsValues, MediaType, ParseParams,
    SourceMapOption, TranspileModuleOptions, TranspileOptions,
};
use deno_core::{
    Extension, JsRuntime, ModuleLoadOptions, ModuleLoadReferrer, ModuleLoadResponse, ModuleLoader,
    ModuleSource, ModuleSourceCode, ModuleSpecifier, ModuleType, OpState, ResolutionKind,
    RuntimeOptions, error::ModuleLoaderError, op2, resolve_import,
};
use deno_error::JsErrorBox;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use slab_types::{
    DESKTOP_API_HOST, DESKTOP_API_PORT, PluginApiRequest, PluginNetworkMode,
    PluginRuntimeApiHostRequest, PluginRuntimeCallRequest, PluginRuntimeCallResponse,
    PluginRuntimeFileAccess, PluginRuntimeFileGrant, PluginRuntimeUiEmitRequest,
};

use crate::application::PluginExecutor;
use crate::domain::RuntimeHost;

const EXECUTION_TIMEOUT: Duration = Duration::from_secs(30);
const FETCH_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_FETCH_RESPONSE_BYTES: usize = 1024 * 1024;
const BOOTSTRAP_JS: &str = include_str!("bootstrap.js");

pub struct DenoPluginExecutor {
    host: Arc<dyn RuntimeHost>,
    http: reqwest::Client,
}

impl DenoPluginExecutor {
    pub fn new(host: Arc<dyn RuntimeHost>) -> Self {
        let http = reqwest::Client::builder().timeout(FETCH_TIMEOUT).build().unwrap_or_default();
        Self { host, http }
    }
}

#[async_trait::async_trait]
impl PluginExecutor for DenoPluginExecutor {
    async fn execute(
        &self,
        request: PluginRuntimeCallRequest,
    ) -> Result<PluginRuntimeCallResponse, anyhow::Error> {
        let host = self.host.clone();
        let http = self.http.clone();
        let result = tokio::task::spawn_blocking(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .context("failed to create JS plugin runtime thread")?;
            runtime.block_on(async move {
                tokio::time::timeout(EXECUTION_TIMEOUT, execute_call(request, host, http))
                    .await
                    .map_err(|_| {
                        anyhow!("JS plugin execution timed out after {:?}", EXECUTION_TIMEOUT)
                    })?
            })
        })
        .await
        .context("JS plugin runtime worker failed")??;
        Ok(PluginRuntimeCallResponse { result })
    }
}

#[derive(Clone)]
struct ExecutionContext {
    call_id: String,
    plugin_id: String,
    permissions: slab_types::PluginPermissionsManifest,
    file_grants: Vec<PluginRuntimeFileGrant>,
    blocked_fetch_origins: Vec<String>,
}

#[derive(Clone)]
struct RuntimeState {
    context: ExecutionContext,
    host: Arc<dyn RuntimeHost>,
    http: reqwest::Client,
    result: Arc<Mutex<Option<Value>>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FetchRequest {
    url: String,
    #[serde(default = "default_fetch_method")]
    method: String,
    #[serde(default)]
    headers: HashMap<String, String>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    timeout_ms: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FetchResponse {
    status: u16,
    headers: HashMap<String, String>,
    body: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UiEmitRequest {
    topic: String,
    #[serde(default)]
    data: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WriteFileRequest {
    path: String,
    bytes: Vec<u8>,
}

async fn execute_call(
    request: PluginRuntimeCallRequest,
    host: Arc<dyn RuntimeHost>,
    http: reqwest::Client,
) -> Result<Value, anyhow::Error> {
    validate_entry_extension(&request.entry)?;
    let root_dir = PathBuf::from(&request.root_dir);
    let entry_path = root_dir.join(&request.entry);
    ensure_path_within_root(&root_dir, &entry_path)?;
    if !entry_path.is_file() {
        bail!("plugin entry does not exist at {}", entry_path.display());
    }

    let result = Arc::new(Mutex::new(None));
    let context = ExecutionContext {
        call_id: request.call_id.clone(),
        plugin_id: request.plugin_id.clone(),
        permissions: request.permissions.clone(),
        file_grants: request.file_grants.clone(),
        blocked_fetch_origins: request.blocked_fetch_origins.clone(),
    };
    let state = RuntimeState { context, host, http, result: result.clone() };

    let source_maps = Rc::new(RefCell::new(HashMap::new()));
    let mut runtime = JsRuntime::new(RuntimeOptions {
        module_loader: Some(Rc::new(SlabModuleLoader {
            root_dir: root_dir.clone(),
            source_maps: source_maps.clone(),
        })),
        extensions: vec![slab_extension()],
        ..Default::default()
    });
    runtime.op_state().borrow_mut().put(state);
    runtime.execute_script("slab:bootstrap", BOOTSTRAP_JS)?;

    let main_module = ModuleSpecifier::from_file_path(root_dir.join("__slab_plugin_call__.mjs"))
        .map_err(|_| anyhow!("failed to build plugin wrapper module URL"))?;
    let entry_specifier = ModuleSpecifier::from_file_path(&entry_path).map_err(|_| {
        anyhow!("failed to convert entry path to file URL: {}", entry_path.display())
    })?;
    let wrapper = build_wrapper_module(
        entry_specifier.as_str(),
        request.export_name.as_str(),
        &request.params,
    )?;

    let mod_id = runtime.load_main_es_module_from_code(&main_module, wrapper).await?;
    let evaluation = runtime.mod_evaluate(mod_id);
    runtime.run_event_loop(Default::default()).await?;
    evaluation.await?;

    let result = result
        .lock()
        .map_err(|_| anyhow!("failed to lock JS plugin result"))?
        .take()
        .unwrap_or(Value::Null);
    Ok(result)
}

fn slab_extension() -> Extension {
    Extension {
        name: "slab_plugin_runtime",
        ops: std::borrow::Cow::Owned(vec![
            op_slab_plugin_id(),
            op_slab_set_result(),
            op_slab_api_request(),
            op_slab_ui_emit(),
            op_slab_fetch(),
            op_slab_read_file(),
            op_slab_write_file(),
            op_slab_decode_utf8(),
            op_slab_encode_utf8(),
        ]),
        ..Default::default()
    }
}

fn build_wrapper_module(
    entry_specifier: &str,
    export_name: &str,
    params: &Value,
) -> Result<String, anyhow::Error> {
    let entry_json = serde_json::to_string(entry_specifier)?;
    let export_json = serde_json::to_string(export_name)?;
    let params_json = serde_json::to_string(params)?;
    Ok(format!(
        r#"
const module = await import({entry_json});
const exportName = {export_json};
const target = module[exportName];
if (typeof target !== "function") {{
  throw new Error(`Plugin does not export function: ${{exportName}}`);
}}
const result = await target({params_json});
Deno.core.ops.op_slab_set_result(result === undefined ? null : result);
"#
    ))
}

struct SlabModuleLoader {
    root_dir: PathBuf,
    source_maps: Rc<RefCell<HashMap<String, Vec<u8>>>>,
}

impl ModuleLoader for SlabModuleLoader {
    fn resolve(
        &self,
        specifier: &str,
        referrer: &str,
        kind: ResolutionKind,
    ) -> Result<ModuleSpecifier, ModuleLoaderError> {
        let resolved = resolve_import(specifier, referrer).map_err(JsErrorBox::from_err)?;
        if resolved.scheme() != "file" {
            return Err(JsErrorBox::generic("Only file:// module imports are supported.").into());
        }
        let path = resolved
            .to_file_path()
            .map_err(|_| JsErrorBox::generic("Invalid file module specifier."))?;
        ensure_path_within_root(&self.root_dir, &path)
            .map_err(|error| JsErrorBox::generic(error.to_string()))?;
        let _ = kind;
        Ok(resolved)
    }

    fn load(
        &self,
        module_specifier: &ModuleSpecifier,
        _maybe_referrer: Option<&ModuleLoadReferrer>,
        _options: ModuleLoadOptions,
    ) -> ModuleLoadResponse {
        let source_maps = self.source_maps.clone();
        let root_dir = self.root_dir.clone();
        let specifier = module_specifier.clone();
        ModuleLoadResponse::Sync(load_module(root_dir, source_maps, &specifier))
    }

    fn get_source_map(&self, specifier: &str) -> Option<Cow<'_, [u8]>> {
        self.source_maps.borrow().get(specifier).map(|value| value.clone().into())
    }
}

fn load_module(
    root_dir: PathBuf,
    source_maps: Rc<RefCell<HashMap<String, Vec<u8>>>>,
    module_specifier: &ModuleSpecifier,
) -> Result<ModuleSource, ModuleLoaderError> {
    let path = module_specifier
        .to_file_path()
        .map_err(|_| JsErrorBox::generic("Only file:// module imports are supported."))?;
    ensure_path_within_root(&root_dir, &path)
        .map_err(|error| JsErrorBox::generic(error.to_string()))?;

    let media_type = MediaType::from_path(&path);
    let (module_type, should_transpile) = match media_type {
        MediaType::JavaScript | MediaType::Mjs => (ModuleType::JavaScript, false),
        MediaType::Jsx | MediaType::TypeScript | MediaType::Tsx | MediaType::Mts => {
            (ModuleType::JavaScript, true)
        }
        MediaType::Json => (ModuleType::Json, false),
        _ => {
            return Err(JsErrorBox::generic(format!(
                "Unsupported plugin module extension {:?}",
                path.extension()
            ))
            .into());
        }
    };

    let code = std::fs::read_to_string(&path).map_err(JsErrorBox::from_err)?;
    let code = if should_transpile {
        let parsed = deno_ast::parse_module(ParseParams {
            specifier: module_specifier.clone(),
            text: code.into(),
            media_type,
            capture_tokens: false,
            scope_analysis: false,
            maybe_syntax: None,
        })
        .map_err(JsErrorBox::from_err)?;
        let emitted = parsed
            .transpile(
                &TranspileOptions {
                    imports_not_used_as_values: ImportsNotUsedAsValues::Remove,
                    decorators: DecoratorsTranspileOption::Ecma,
                    ..Default::default()
                },
                &TranspileModuleOptions { module_kind: None },
                &EmitOptions {
                    source_map: SourceMapOption::Separate,
                    inline_sources: true,
                    ..Default::default()
                },
            )
            .map_err(JsErrorBox::from_err)?
            .into_source();
        if let Some(source_map) = emitted.source_map {
            source_maps.borrow_mut().insert(module_specifier.to_string(), source_map.into_bytes());
        }
        emitted.text
    } else {
        code
    };

    Ok(ModuleSource::new(
        module_type,
        ModuleSourceCode::String(code.into()),
        module_specifier,
        None,
    ))
}

#[op2]
#[string]
fn op_slab_plugin_id(state: &mut OpState) -> String {
    state.borrow::<RuntimeState>().context.plugin_id.clone()
}

#[op2]
fn op_slab_set_result(
    state: &mut OpState,
    #[serde] value: serde_json::Value,
) -> Result<(), JsErrorBox> {
    let result = state.borrow::<RuntimeState>().result.clone();
    *result.lock().map_err(|_| JsErrorBox::generic("failed to lock result"))? = Some(value);
    Ok(())
}

#[op2]
#[serde]
async fn op_slab_api_request(
    state: Rc<RefCell<OpState>>,
    #[serde] request: PluginApiRequest,
) -> Result<serde_json::Value, JsErrorBox> {
    let state = {
        let state = state.borrow();
        state.borrow::<RuntimeState>().clone()
    };
    let payload = PluginRuntimeApiHostRequest {
        call_id: state.context.call_id,
        plugin_id: state.context.plugin_id,
        request,
    };
    let params = serde_json::to_value(payload).map_err(JsErrorBox::from_err)?;
    state.host.request("slab.api.request", params).await.map_err(JsErrorBox::generic)
}

#[op2]
#[serde]
async fn op_slab_ui_emit(
    state: Rc<RefCell<OpState>>,
    #[serde] request: UiEmitRequest,
) -> Result<serde_json::Value, JsErrorBox> {
    let state = {
        let state = state.borrow();
        state.borrow::<RuntimeState>().clone()
    };
    let payload = PluginRuntimeUiEmitRequest {
        call_id: state.context.call_id,
        plugin_id: state.context.plugin_id,
        topic: request.topic,
        data: request.data,
    };
    let params = serde_json::to_value(payload).map_err(JsErrorBox::from_err)?;
    state.host.request("slab.ui.emit", params).await.map_err(JsErrorBox::generic)
}

#[op2]
#[serde]
async fn op_slab_fetch(
    state: Rc<RefCell<OpState>>,
    #[serde] request: FetchRequest,
) -> Result<FetchResponse, JsErrorBox> {
    let state = {
        let state = state.borrow();
        state.borrow::<RuntimeState>().clone()
    };
    authorize_fetch(&state.context, &request.url).map_err(JsErrorBox::generic)?;

    let method = reqwest::Method::from_bytes(request.method.as_bytes())
        .map_err(|error| JsErrorBox::generic(format!("invalid fetch method: {error}")))?;
    let mut builder = state.http.request(method, request.url);
    for (name, value) in request.headers {
        if is_blocked_header(name.as_str()) {
            continue;
        }
        builder = builder.header(name, value);
    }
    if let Some(timeout_ms) = request.timeout_ms {
        builder = builder.timeout(Duration::from_millis(timeout_ms.min(60_000)));
    }
    if let Some(body) = request.body {
        builder = builder.body(body);
    }

    let response = builder
        .send()
        .await
        .map_err(|error| JsErrorBox::generic(format!("fetch failed: {error}")))?;
    let status = response.status().as_u16();
    let headers = collect_headers(response.headers());
    let bytes = response
        .bytes()
        .await
        .map_err(|error| JsErrorBox::generic(format!("failed to read fetch body: {error}")))?;
    if bytes.len() > MAX_FETCH_RESPONSE_BYTES {
        return Err(JsErrorBox::generic(format!(
            "fetch response exceeds {MAX_FETCH_RESPONSE_BYTES} byte limit"
        )));
    }

    Ok(FetchResponse { status, headers, body: String::from_utf8_lossy(&bytes).to_string() })
}

#[op2]
#[serde]
async fn op_slab_read_file(
    state: Rc<RefCell<OpState>>,
    #[string] path: String,
) -> Result<Vec<u8>, JsErrorBox> {
    let context = {
        let state = state.borrow();
        state.borrow::<RuntimeState>().context.clone()
    };
    authorize_file_access(&context, &path, PluginRuntimeFileAccess::Read)
        .map_err(JsErrorBox::generic)?;
    tokio::fs::read(path)
        .await
        .map_err(|error| JsErrorBox::generic(format!("failed to read file: {error}")))
}

#[op2]
async fn op_slab_write_file(
    state: Rc<RefCell<OpState>>,
    #[serde] request: WriteFileRequest,
) -> Result<(), JsErrorBox> {
    let context = {
        let state = state.borrow();
        state.borrow::<RuntimeState>().context.clone()
    };
    authorize_file_access(&context, &request.path, PluginRuntimeFileAccess::Write)
        .map_err(JsErrorBox::generic)?;
    tokio::fs::write(request.path, request.bytes)
        .await
        .map_err(|error| JsErrorBox::generic(format!("failed to write file: {error}")))
}

#[op2]
#[string]
fn op_slab_decode_utf8(#[serde] bytes: Vec<u8>) -> Result<String, JsErrorBox> {
    String::from_utf8(bytes).map_err(|error| JsErrorBox::generic(format!("invalid UTF-8: {error}")))
}

#[op2]
#[serde]
fn op_slab_encode_utf8(#[string] value: String) -> Vec<u8> {
    value.into_bytes()
}

fn authorize_fetch(context: &ExecutionContext, raw_url: &str) -> Result<(), String> {
    let url = Url::parse(raw_url).map_err(|error| format!("invalid fetch URL: {error}"))?;
    let scheme = url.scheme();
    if scheme != "http" && scheme != "https" {
        return Err("fetch only supports http:// and https:// URLs".to_owned());
    }
    let host = url.host_str().ok_or_else(|| "fetch URL is missing a host".to_owned())?;
    if is_blocked_slab_api_origin(context, &url) {
        return Err("local Slab API origins are blocked; use Slab.api.request instead".to_owned());
    }
    if context.permissions.network.mode != PluginNetworkMode::Allowlist {
        return Err("plugin network permission mode blocks fetch".to_owned());
    }
    if context.permissions.network.allow_hosts.iter().any(|allowed| host_matches(allowed, &url)) {
        return Ok(());
    }
    Err(format!("fetch host `{host}` is not declared in permissions.network.allowHosts"))
}

fn authorize_file_access(
    context: &ExecutionContext,
    raw_path: &str,
    access: PluginRuntimeFileAccess,
) -> Result<(), String> {
    let target = canonical_for_access(raw_path, access)?;
    for grant in &context.file_grants {
        if grant.access != access {
            continue;
        }
        let manifest_labels = match access {
            PluginRuntimeFileAccess::Read => &context.permissions.files.read,
            PluginRuntimeFileAccess::Write => &context.permissions.files.write,
        };
        if !manifest_labels.iter().any(|label| label == &grant.label) {
            continue;
        }
        let grant_path = canonical_for_access(&grant.path, access)?;
        if grant_path == target {
            return Ok(());
        }
    }

    Err(format!(
        "file {} access to `{raw_path}` requires a matching host-issued grant and manifest permission",
        match access {
            PluginRuntimeFileAccess::Read => "read",
            PluginRuntimeFileAccess::Write => "write",
        }
    ))
}

fn canonical_for_access(
    raw_path: &str,
    access: PluginRuntimeFileAccess,
) -> Result<PathBuf, String> {
    let path = PathBuf::from(raw_path);
    match access {
        PluginRuntimeFileAccess::Read => path
            .canonicalize()
            .map_err(|error| format!("failed to resolve `{}`: {error}", path.display())),
        PluginRuntimeFileAccess::Write => {
            if path.exists() {
                return path
                    .canonicalize()
                    .map_err(|error| format!("failed to resolve `{}`: {error}", path.display()));
            }
            let parent = path.parent().ok_or_else(|| {
                format!("write target `{}` does not have a parent directory", path.display())
            })?;
            let parent = parent.canonicalize().map_err(|error| {
                format!("failed to resolve parent `{}`: {error}", parent.display())
            })?;
            let file_name = path.file_name().ok_or_else(|| {
                format!("write target `{}` does not have a file name", path.display())
            })?;
            Ok(parent.join(file_name))
        }
    }
}

fn ensure_path_within_root(root: &Path, path: &Path) -> Result<(), anyhow::Error> {
    let root = root
        .canonicalize()
        .with_context(|| format!("failed to resolve plugin root {}", root.display()))?;
    let path = if path.exists() {
        path.canonicalize()
            .with_context(|| format!("failed to resolve plugin path {}", path.display()))?
    } else {
        let parent = path.parent().ok_or_else(|| {
            anyhow!("plugin path {} does not have a parent directory", path.display())
        })?;
        let parent = parent
            .canonicalize()
            .with_context(|| format!("failed to resolve plugin parent {}", parent.display()))?;
        let file_name = path
            .file_name()
            .ok_or_else(|| anyhow!("plugin path {} does not have a file name", path.display()))?;
        parent.join(file_name)
    };
    if path.starts_with(root) {
        Ok(())
    } else {
        bail!("plugin path {} escapes plugin root", path.display())
    }
}

fn validate_entry_extension(entry: &str) -> Result<(), anyhow::Error> {
    let extension = Path::new(entry)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if matches!(extension.as_str(), "ts" | "tsx" | "js" | "mjs") {
        return Ok(());
    }
    bail!("runtime.js.entry must use .ts, .tsx, .js, or .mjs")
}

fn host_matches(allowed: &str, url: &Url) -> bool {
    let allowed = allowed.trim();
    if allowed.is_empty() {
        return false;
    }
    if let Ok(allowed_url) = Url::parse(allowed) {
        return allowed_url.host_str() == url.host_str()
            && allowed_url.port_or_known_default() == url.port_or_known_default();
    }
    let host = url.host_str().unwrap_or_default();
    let host_port = url.port().map_or_else(|| host.to_owned(), |port| format!("{host}:{port}"));
    allowed == host || allowed == host_port
}

fn is_blocked_slab_api_origin(context: &ExecutionContext, url: &Url) -> bool {
    is_default_slab_api_origin(url)
        || context
            .blocked_fetch_origins
            .iter()
            .filter_map(|origin| Url::parse(origin).ok())
            .any(|origin| same_origin(&origin, url))
}

fn is_default_slab_api_origin(url: &Url) -> bool {
    let Some(host) = url.host_str() else {
        return false;
    };
    matches!(host, DESKTOP_API_HOST | "localhost")
        && url.port_or_known_default() == Some(DESKTOP_API_PORT)
}

fn same_origin(left: &Url, right: &Url) -> bool {
    left.scheme() == right.scheme()
        && left.host_str() == right.host_str()
        && left.port_or_known_default() == right.port_or_known_default()
}

fn collect_headers(headers: &reqwest::header::HeaderMap) -> HashMap<String, String> {
    let mut result = HashMap::new();
    for (name, value) in headers {
        if is_blocked_header(name.as_str()) {
            continue;
        }
        if let Ok(value) = value.to_str() {
            result.insert(name.to_string(), value.to_string());
        }
    }
    result
}

fn is_blocked_header(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "host" | "connection" | "content-length" | "transfer-encoding"
    )
}

fn default_fetch_method() -> String {
    "GET".to_owned()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use serde_json::json;
    use slab_types::{
        PluginNetworkManifest, PluginPermissionsManifest, PluginRuntimeCallRequest,
        PluginRuntimeFileAccess, PluginRuntimeFileGrant,
    };
    use tempfile::tempdir;
    use tokio::net::TcpListener;

    use super::{DenoPluginExecutor, PluginExecutor, RuntimeHost};

    struct TestHost;

    #[async_trait]
    impl RuntimeHost for TestHost {
        async fn request(
            &self,
            method: &str,
            params: serde_json::Value,
        ) -> Result<serde_json::Value, String> {
            match method {
                "slab.api.request" => {
                    Ok(json!({ "status": 200, "headers": {}, "body": "{\"ok\":true}" }))
                }
                "slab.ui.emit" => Ok(params),
                _ => Err(format!("unexpected host method `{method}`")),
            }
        }
    }

    fn call_request(
        root_dir: &std::path::Path,
        entry: &str,
        export_name: &str,
    ) -> PluginRuntimeCallRequest {
        PluginRuntimeCallRequest {
            call_id: "call-1".to_owned(),
            plugin_id: "test-plugin".to_owned(),
            root_dir: root_dir.to_string_lossy().into_owned(),
            entry: entry.to_owned(),
            export_name: export_name.to_owned(),
            params: json!({ "name": "Slab" }),
            permissions: PluginPermissionsManifest::default(),
            file_grants: Vec::new(),
            blocked_fetch_origins: Vec::new(),
        }
    }

    #[tokio::test]
    async fn runs_ts_esm_named_export_and_awaits_result() {
        let dir = tempdir().unwrap();
        std::fs::write(
            dir.path().join("plugin.ts"),
            "export async function greet(input: { name: string }) { return { message: `hello ${input.name}` }; }",
        )
        .unwrap();

        let executor = DenoPluginExecutor::new(Arc::new(TestHost));
        let response =
            executor.execute(call_request(dir.path(), "plugin.ts", "greet")).await.unwrap();

        assert_eq!(response.result, json!({ "message": "hello Slab" }));
    }

    #[tokio::test]
    async fn blocks_file_read_without_grant() {
        let dir = tempdir().unwrap();
        let secret = dir.path().join("secret.txt");
        std::fs::write(&secret, "secret").unwrap();
        std::fs::write(
            dir.path().join("plugin.ts"),
            format!(
                "export async function run() {{ await Deno.readTextFile({}); }}",
                serde_json::to_string(&secret.to_string_lossy()).unwrap()
            ),
        )
        .unwrap();

        let executor = DenoPluginExecutor::new(Arc::new(TestHost));
        let error =
            executor.execute(call_request(dir.path(), "plugin.ts", "run")).await.unwrap_err();

        assert!(error.to_string().contains("requires a matching host-issued grant"));
    }

    #[tokio::test]
    async fn allows_granted_file_read_with_matching_manifest_label() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("allowed.txt");
        std::fs::write(&file, "ok").unwrap();
        std::fs::write(
            dir.path().join("plugin.ts"),
            format!(
                "export async function run() {{ return await Deno.readTextFile({}); }}",
                serde_json::to_string(&file.to_string_lossy()).unwrap()
            ),
        )
        .unwrap();

        let mut request = call_request(dir.path(), "plugin.ts", "run");
        request.permissions.files.read.push("fixture".to_owned());
        request.file_grants.push(PluginRuntimeFileGrant {
            label: "fixture".to_owned(),
            path: file.to_string_lossy().into_owned(),
            access: PluginRuntimeFileAccess::Read,
        });
        let executor = DenoPluginExecutor::new(Arc::new(TestHost));
        let response = executor.execute(request).await.unwrap();

        assert_eq!(response.result, json!("ok"));
    }

    #[tokio::test]
    async fn blocks_fetch_without_allowlist() {
        let dir = tempdir().unwrap();
        std::fs::write(
            dir.path().join("plugin.ts"),
            "export async function run() { await fetch('https://example.com'); }",
        )
        .unwrap();

        let executor = DenoPluginExecutor::new(Arc::new(TestHost));
        let error =
            executor.execute(call_request(dir.path(), "plugin.ts", "run")).await.unwrap_err();

        assert!(error.to_string().contains("blocks fetch"));
    }

    #[tokio::test]
    async fn allows_allowlisted_fetch() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let mut buffer = [0u8; 1024];
            let _ = socket.read(&mut buffer).await.unwrap();
            socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok").await.unwrap();
        });

        let dir = tempdir().unwrap();
        std::fs::write(
            dir.path().join("plugin.ts"),
            format!(
                "export async function run() {{ const r = await fetch('http://{}'); return await r.text(); }}",
                addr
            ),
        )
        .unwrap();

        let mut request = call_request(dir.path(), "plugin.ts", "run");
        request.permissions.network = PluginNetworkManifest {
            mode: slab_types::PluginNetworkMode::Allowlist,
            allow_hosts: vec![addr.to_string()],
        };
        let executor = DenoPluginExecutor::new(Arc::new(TestHost));
        let response = executor.execute(request).await.unwrap();

        assert_eq!(response.result, json!("ok"));
    }

    #[tokio::test]
    async fn blocks_fetch_to_configured_slab_api_origin() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let dir = tempdir().unwrap();
        std::fs::write(
            dir.path().join("plugin.ts"),
            format!("export async function run() {{ await fetch('http://{}'); }}", addr),
        )
        .unwrap();

        let mut request = call_request(dir.path(), "plugin.ts", "run");
        request.permissions.network = PluginNetworkManifest {
            mode: slab_types::PluginNetworkMode::Allowlist,
            allow_hosts: vec![addr.to_string()],
        };
        request.blocked_fetch_origins.push(format!("http://{addr}"));

        let executor = DenoPluginExecutor::new(Arc::new(TestHost));
        let error = executor.execute(request).await.unwrap_err();

        assert!(error.to_string().contains("use Slab.api.request"));
    }

    #[tokio::test]
    async fn slab_api_request_goes_through_host() {
        let dir = tempdir().unwrap();
        std::fs::write(
            dir.path().join("plugin.ts"),
            "export async function run() { return await Slab.api.request({ method: 'GET', path: '/v1/models' }); }",
        )
        .unwrap();

        let executor = DenoPluginExecutor::new(Arc::new(TestHost));
        let response =
            executor.execute(call_request(dir.path(), "plugin.ts", "run")).await.unwrap();

        assert_eq!(response.result["status"], json!(200));
    }
}
