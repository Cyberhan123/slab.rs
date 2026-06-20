use std::sync::Arc;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode, header};
use serde_json::{Value, json};
use slab_app_core::config::Config;
use slab_app_core::context::AppState;
use slab_app_core::domain::services::PmidService;
use slab_app_core::infra::db::AnyStore;
use slab_app_core::infra::rpc::gateway::GrpcGateway;
use slab_app_core::runtime_supervisor::RuntimeSupervisorStatus;
use slab_config::{
    LaunchProfile, PluginJsRuntimeTransport, PluginPythonRuntimeTransport, ProviderDefaultsConfig,
    ProviderFamily, ProviderRegistryEntry, ResolvedLaunchSpec, ResolvedRuntimeEndpoints,
    RuntimeTransportMode, SettingsDocument,
};
use slab_types::sqlite_url_for_path;
use tempfile::TempDir;
use tower::ServiceExt;

use crate::api;

pub(crate) const TEST_PROVIDER_ID: &str = "openai-main";

pub(crate) struct TestServer {
    _temp_dir: TempDir,
    pub(crate) state: Arc<AppState>,
    pub(crate) store: Arc<AnyStore>,
}

#[derive(Default)]
pub(crate) struct TestServerOptions {
    pub(crate) bind_address: Option<String>,
    pub(crate) admin_api_token: Option<String>,
    pub(crate) workspace_root: Option<std::path::PathBuf>,
}

pub(crate) struct TestResponse {
    pub(crate) status: StatusCode,
    pub(crate) body: Value,
}

impl TestServer {
    pub(crate) async fn new() -> Self {
        Self::new_with(TestServerOptions::default()).await
    }

    pub(crate) async fn new_with(options: TestServerOptions) -> Self {
        let temp_dir = tempfile::tempdir().expect("test server temp dir");
        let root = temp_dir.path();
        let settings_dir = root.join("config");
        let model_config_dir = settings_dir.join("models");
        let model_cache_dir = root.join("model-cache");
        let plugins_dir = root.join("plugins");
        let session_state_dir = root.join("sessions");
        let exec_rules_dir = root.join("rules");
        let runtime_log_dir = root.join("runtime-logs");

        for dir in [
            &settings_dir,
            &model_config_dir,
            &model_cache_dir,
            &plugins_dir,
            &session_state_dir,
            &exec_rules_dir,
            &runtime_log_dir,
        ] {
            std::fs::create_dir_all(dir).expect("create test server directory");
        }

        let settings_path = settings_dir.join("settings.json");
        let bind_address = options.bind_address.unwrap_or_else(|| "127.0.0.1:0".to_owned());
        write_test_settings(
            &settings_path,
            &model_cache_dir,
            &model_config_dir,
            &plugins_dir,
            &bind_address,
            options.admin_api_token.as_deref(),
        );

        let database_url = sqlite_url_for_path(&root.join("slab.db"));
        let config = Arc::new(Config {
            bind_address,
            database_url: database_url.clone(),
            log_level: "warn".to_owned(),
            log_json: false,
            log_file: None,
            cloud_http_trace: false,
            queue_capacity: 64,
            backend_capacity: 4,
            enable_swagger: false,
            cors_allowed_origins: None,
            admin_api_token: options.admin_api_token,
            transport_mode: "http".to_owned(),
            llama_grpc_endpoint: None,
            whisper_grpc_endpoint: None,
            diffusion_grpc_endpoint: None,
            candle_llama_grpc_endpoint: None,
            candle_whisper_grpc_endpoint: None,
            candle_diffusion_grpc_endpoint: None,
            lib_dir: None,
            session_state_dir: session_state_dir.to_string_lossy().into_owned(),
            settings_path,
            settings_overlay_path: None,
            workspace_root: options.workspace_root,
            model_config_dir,
            plugins_dir,
            exec_rules_dir,
            plugin_js_runtime_transport: PluginJsRuntimeTransport::default(),
            plugin_python_runtime_transport: PluginPythonRuntimeTransport::default(),
        });
        let pmid = Arc::new(
            PmidService::load_from_path(config.settings_path.clone())
                .await
                .expect("load test server settings"),
        );
        let store = Arc::new(AnyStore::connect(&database_url).await.expect("connect test DB"));
        let grpc = Arc::new(GrpcGateway::default());
        let launch_spec = ResolvedLaunchSpec {
            profile: LaunchProfile::Server,
            transport: RuntimeTransportMode::Http,
            runtime_log_dir,
            runtime_ipc_dir: None,
            extra_dirs: Vec::new(),
            children: Vec::new(),
            endpoints: ResolvedRuntimeEndpoints::default(),
            gateway: None,
        };
        let runtime_status = Arc::new(RuntimeSupervisorStatus::from_launch_spec(&launch_spec));
        let state =
            Arc::new(AppState::new(config, pmid, grpc, runtime_status, None, store.clone()));

        Self { _temp_dir: temp_dir, state, store }
    }

    pub(crate) fn app(&self) -> Router {
        api::build(Arc::clone(&self.state))
    }

    /// Path to the test server's plugin install directory. Tests can stage plugin
    /// manifests here to exercise the registry/dispatch paths end-to-end.
    pub(crate) fn plugins_dir(&self) -> std::path::PathBuf {
        self._temp_dir.path().join("plugins")
    }

    pub(crate) async fn get(&self, uri: &str) -> TestResponse {
        self.send_json(Method::GET, uri, None, None).await
    }

    pub(crate) async fn post_json(&self, uri: &str, body: Value) -> TestResponse {
        self.send_json(Method::POST, uri, Some(body), None).await
    }

    pub(crate) async fn put_json(&self, uri: &str, body: Value) -> TestResponse {
        self.send_json(Method::PUT, uri, Some(body), None).await
    }

    pub(crate) async fn get_with_token(&self, uri: &str, token: &str) -> TestResponse {
        self.send_json(Method::GET, uri, None, Some(token)).await
    }

    pub(crate) async fn put_json_with_token(
        &self,
        uri: &str,
        body: Value,
        token: &str,
    ) -> TestResponse {
        self.send_json(Method::PUT, uri, Some(body), Some(token)).await
    }

    pub(crate) async fn raw(&self, request: Request<Body>) -> axum::http::Response<Body> {
        self.app().oneshot(request).await.expect("route response")
    }

    async fn send_json(
        &self,
        method: Method,
        uri: &str,
        body: Option<Value>,
        bearer_token: Option<&str>,
    ) -> TestResponse {
        let mut builder = Request::builder().method(method).uri(uri);
        if body.is_some() {
            builder = builder.header(header::CONTENT_TYPE, "application/json");
        }
        if let Some(token) = bearer_token {
            builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        let body = body.map_or_else(Body::empty, |value| Body::from(value.to_string()));
        let response = self.raw(builder.body(body).expect("test request")).await;
        response_json(response).await
    }
}

pub(crate) async fn response_json(response: axum::http::Response<Body>) -> TestResponse {
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.expect("response body");
    let body = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or_else(|_| {
            json!({
                "raw": String::from_utf8_lossy(&bytes).into_owned()
            })
        })
    };
    TestResponse { status, body }
}

fn write_test_settings(
    settings_path: &std::path::Path,
    model_cache_dir: &std::path::Path,
    model_config_dir: &std::path::Path,
    plugins_dir: &std::path::Path,
    bind_address: &str,
    admin_api_token: Option<&str>,
) {
    let mut document = SettingsDocument::default();
    document.server.address = bind_address.to_owned();
    document.server.admin.token = admin_api_token.map(ToOwned::to_owned);
    document.models.cache_dir = Some(model_cache_dir.to_string_lossy().into_owned());
    document.models.config_dir = Some(model_config_dir.to_string_lossy().into_owned());
    document.plugin.install_dir = Some(plugins_dir.to_string_lossy().into_owned());
    document.providers.registry.push(ProviderRegistryEntry {
        id: TEST_PROVIDER_ID.to_owned(),
        family: ProviderFamily::OpenaiCompatible,
        display_name: "OpenAI Test".to_owned(),
        api_base: "https://api.openai.test/v1".to_owned(),
        auth: Default::default(),
        defaults: ProviderDefaultsConfig::default(),
    });

    let raw = serde_json::to_string_pretty(&document).expect("serialize test settings");
    std::fs::write(settings_path, format!("{raw}\n")).expect("write test settings");
}
