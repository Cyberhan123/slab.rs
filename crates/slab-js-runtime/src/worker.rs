//! JS plugin worker using rquickjs (QuickJS-based engine).
//!
//! Provides a sandboxed JavaScript execution environment for plugins.
//! The API surface (`globalThis.Slab`) is designed to be compatible with
//! Deno, so plugins can be developed/tested with `deno run` and then
//! executed by the embedded engine at runtime.

use std::path::PathBuf;

use anyhow::{Result, bail};
use rquickjs::{Context, Function, Object, Runtime};
use serde_json::Value;
use tracing::debug;

use crate::host_ops::{PluginApiRequest, create_event_payload, execute_api_request};
use crate::{JsCallRequest, JsCallResponse, JsPluginPermissions, JsRuntimeConfig};

/// A handle to a running JS plugin worker backed by QuickJS.
pub struct JsWorkerHandle {
    module_path: PathBuf,
    #[allow(dead_code)]
    permissions: JsPluginPermissions,
    config: JsRuntimeConfig,
}

impl JsWorkerHandle {
    pub fn new(
        module_path: PathBuf,
        permissions: JsPluginPermissions,
        config: JsRuntimeConfig,
    ) -> Result<Self> {
        if !module_path.is_file() {
            bail!("js module entry does not exist at {}", module_path.display());
        }

        debug!("created JS worker for module {}", module_path.display());

        Ok(Self { module_path, permissions, config })
    }

    pub async fn call(&self, request: JsCallRequest) -> Result<JsCallResponse> {
        if self.module_path != request.module_path {
            bail!(
                "js module path mismatch for plugin `{}`: expected {}, got {}",
                request.plugin_id,
                self.module_path.display(),
                request.module_path.display()
            );
        }

        let module_path = self.module_path.clone();
        let method = request.method.clone();
        let params = request.params.clone();
        let plugin_id = request.plugin_id.clone();
        let config = self.config.clone();
        let permissions = self.permissions.clone();

        // Execute on a blocking thread since QuickJS runtime is !Send
        let result = tokio::task::spawn_blocking(move || {
            execute_js_call(&module_path, &method, &params, &plugin_id, &config, &permissions)
        })
        .await??;

        Ok(JsCallResponse { result })
    }
}

/// Execute a JS function call in a fresh QuickJS context.
fn execute_js_call(
    module_path: &PathBuf,
    method: &str,
    params: &Value,
    plugin_id: &str,
    config: &JsRuntimeConfig,
    _permissions: &JsPluginPermissions,
) -> Result<Value> {
    let rt = Runtime::new()
        .map_err(|e| anyhow::anyhow!("failed to create QuickJS runtime: {e}"))?;
    let ctx = Context::full(&rt)
        .map_err(|e| anyhow::anyhow!("failed to create QuickJS context: {e}"))?;

    ctx.with(|ctx| {
        // Inject the Slab host bridge
        inject_slab_bridge(&ctx, plugin_id, config)?;

        // Read the plugin module source
        let source = std::fs::read_to_string(module_path)
            .map_err(|e| anyhow::anyhow!("failed to read plugin module: {e}"))?;

        // Serialize params for injection into JS
        let params_json = serde_json::to_string(params)?;
        // Safely serialize method name as a JSON string for use in bracket notation
        let method_json = serde_json::to_string(method)?;

        // Build a self-contained script that loads the module and calls the function.
        let call_script = format!(
            r#"(function() {{
    var exports = {{}};
    var module = {{ exports: exports }};
    {source}
    var mod = module.exports;
    var fnName = {method_json};
    var target = mod[fnName] || (mod.default && mod.default[fnName]);
    if (typeof target !== "function") {{
        throw new Error("Plugin does not export function: " + fnName);
    }}
    var params = {params_json};
    var result = target(params);
    return JSON.stringify(result !== undefined ? result : null);
}})()"#,
            source = source,
            method_json = method_json,
            params_json = params_json,
        );

        let result: String = ctx
            .eval(call_script)
            .map_err(|e| anyhow::anyhow!(
                "JS execution error in {}: {e}",
                module_path.display()
            ))?;

        let value: Value = match serde_json::from_str(&result) {
            Ok(v) => v,
            Err(_) => Value::String(result),
        };

        Ok(value)
    })
}

/// Inject `globalThis.Slab` with host API bridge functions.
///
/// Rust-side functions accept and return `String` (JSON). JS wrappers handle
/// serialization/deserialization. This avoids rquickjs lifetime issues with
/// Value<'js> in closures.
fn inject_slab_bridge(
    ctx: &rquickjs::Ctx<'_>,
    plugin_id: &str,
    config: &JsRuntimeConfig,
) -> Result<()> {
    let globals = ctx.globals();

    let slab = Object::new(ctx.clone())
        .map_err(|e| anyhow::anyhow!("failed to create Slab object: {e}"))?;

    // Slab.pluginId
    slab.set(
        "pluginId",
        rquickjs::String::from_str(ctx.clone(), plugin_id)
            .map_err(|e| anyhow::anyhow!("failed to create string: {e}"))?,
    )
    .map_err(|e| anyhow::anyhow!("failed to set pluginId: {e}"))?;

    // Slab.api object
    let api = Object::new(ctx.clone())
        .map_err(|e| anyhow::anyhow!("failed to create api object: {e}"))?;

    // __apiRequestImpl(optionsJson: String) -> String
    let api_base_url = config.api_base_url.clone().unwrap_or_default();
    let api_permissions = config.slab_api_permissions.clone();
    let request_impl = Function::new(
        ctx.clone(),
        move |options_json: String| -> rquickjs::Result<String> {
            let req: PluginApiRequest = serde_json::from_str(&options_json).map_err(|_| {
                rquickjs::Error::new_from_js("string", "PluginApiRequest")
            })?;

            if api_base_url.is_empty() {
                return Err(rquickjs::Error::new_from_js(
                    "undefined",
                    "api_base_url",
                ));
            }

            let response = execute_api_request(&api_base_url, &api_permissions, &req)
                .map_err(|_| rquickjs::Error::new_from_js("request", "response"))?;

            serde_json::to_string(&response)
                .map_err(|_| rquickjs::Error::new_from_js("response", "json"))
        },
    )
    .map_err(|e| anyhow::anyhow!("failed to create __apiRequestImpl: {e}"))?;

    api.set("__impl", request_impl)
        .map_err(|e| anyhow::anyhow!("failed to set api.__impl: {e}"))?;
    slab.set("api", api)
        .map_err(|e| anyhow::anyhow!("failed to set Slab.api: {e}"))?;

    // Slab.ui object
    let ui = Object::new(ctx.clone())
        .map_err(|e| anyhow::anyhow!("failed to create ui object: {e}"))?;

    // __emitImpl(requestJson: String) -> String
    let emit_plugin_id = plugin_id.to_string();
    let emit_impl = Function::new(
        ctx.clone(),
        move |request_json: String| -> rquickjs::Result<String> {
            let emit_req: crate::host_ops::PluginEmitRequest =
                serde_json::from_str(&request_json).map_err(|_| {
                    rquickjs::Error::new_from_js("string", "PluginEmitRequest")
                })?;

            let payload = create_event_payload(&emit_plugin_id, &emit_req);

            serde_json::to_string(&payload)
                .map_err(|_| rquickjs::Error::new_from_js("payload", "json"))
        },
    )
    .map_err(|e| anyhow::anyhow!("failed to create __emitImpl: {e}"))?;

    ui.set("__impl", emit_impl)
        .map_err(|e| anyhow::anyhow!("failed to set ui.__impl: {e}"))?;
    slab.set("ui", ui)
        .map_err(|e| anyhow::anyhow!("failed to set Slab.ui: {e}"))?;

    globals
        .set("Slab", slab)
        .map_err(|e| anyhow::anyhow!("failed to set globalThis.Slab: {e}"))?;

    // Install high-level JS wrappers that serialize/deserialize for the user
    let bridge_js = r#"(function() {
        var apiImpl = Slab.api.__impl;
        Slab.api.request = function(options) {
            var json = apiImpl(JSON.stringify(options));
            return JSON.parse(json);
        };
        delete Slab.api.__impl;

        var emitImpl = Slab.ui.__impl;
        Slab.ui.emit = function(topic, data) {
            var json = emitImpl(JSON.stringify({ topic: topic, data: data || null }));
            return JSON.parse(json);
        };
        delete Slab.ui.__impl;
    })()"#;

    ctx.eval::<rquickjs::Value, _>(bridge_js)
        .map_err(|e| anyhow::anyhow!("failed to evaluate bridge JS: {e}"))?;

    Ok(())
}
