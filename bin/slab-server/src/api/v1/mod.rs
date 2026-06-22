pub mod agent;
pub mod audio;
pub mod backend;
pub mod chat;
#[path = "settings/mod.rs"]
pub mod configuration_routes;
pub mod ffmpeg;
pub mod images;
pub mod models;
mod path;
pub mod plugins;
pub mod session;
pub mod setup;
pub mod subtitles;
pub mod system;
pub mod tasks;
pub mod ui_state;
pub mod video;
pub mod workspace;
pub mod workspace_lsp;

use std::sync::Arc;

use axum::Router;
use utoipa::OpenApi;

use slab_app_core::context::AppState;

#[derive(OpenApi)]
#[openapi()]
pub struct V1Api;

/// Routes nested under `/v1`.
pub fn router(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .merge(agent::router())
        .merge(chat::router())
        .merge(models::router())
        .merge(plugins::router())
        .merge(session::router())
        .merge(audio::router())
        .merge(images::router())
        .merge(video::router())
        .merge(ffmpeg::router())
        .merge(system::router())
        .merge(tasks::router())
        .merge(configuration_routes::router(state.clone()))
        .merge(subtitles::router())
        .merge(ui_state::router())
        .merge(setup::router())
        .merge(workspace::router(state.clone()))
        .merge(workspace_lsp::router())
        .merge(backend::router(state))
}

pub fn api_docs() -> utoipa::openapi::OpenApi {
    let mut spec = V1Api::openapi();
    spec.merge(agent::AgentApi::openapi());
    spec.merge(chat::ChatApi::openapi());
    spec.merge(models::ModelsApi::openapi());
    spec.merge(plugins::PluginApi::openapi());
    spec.merge(session::SessionApi::openapi());
    spec.merge(audio::AudioApi::openapi());
    spec.merge(images::ImagesApi::openapi());
    spec.merge(video::VideoApi::openapi());
    spec.merge(ffmpeg::FfmpegApi::openapi());
    spec.merge(system::SystemApi::openapi());
    spec.merge(tasks::TasksApi::openapi());
    spec.merge(configuration_routes::SettingsApi::openapi());
    spec.merge(subtitles::SubtitleApi::openapi());
    spec.merge(ui_state::UiStateApi::openapi());
    spec.merge(setup::SetupApi::openapi());
    spec.merge(workspace::WorkspaceApi::openapi());
    spec.merge(backend::BackendApi::openapi());

    spec
}

#[cfg(test)]
mod tests {
    use super::api_docs;

    const DOCUMENTED_METHODS: &[&str] = &["delete", "get", "patch", "post", "put"];
    const EXPECTED_OPERATIONS: &[(&str, &str)] = &[
        ("/v1/agents/responses", "get"),
        ("/v1/agents/responses", "post"),
        ("/v1/audio/transcriptions", "get"),
        ("/v1/audio/transcriptions", "post"),
        ("/v1/audio/transcriptions/{id}", "get"),
        ("/v1/backends", "get"),
        ("/v1/backends/status", "get"),
        ("/v1/chat/completions", "post"),
        ("/v1/chat/models", "get"),
        ("/v1/completions", "post"),
        ("/v1/ffmpeg/convert", "post"),
        ("/v1/images/generations", "get"),
        ("/v1/images/generations", "post"),
        ("/v1/images/generations/{id}", "get"),
        ("/v1/images/generations/{id}/artifacts/{index}", "get"),
        ("/v1/images/generations/{id}/reference", "get"),
        ("/v1/models", "get"),
        ("/v1/models", "post"),
        ("/v1/models/{id}", "delete"),
        ("/v1/models/{id}", "get"),
        ("/v1/models/{id}", "put"),
        ("/v1/models/{id}/config-document", "get"),
        ("/v1/models/{id}/config-selection", "put"),
        ("/v1/models/available", "get"),
        ("/v1/models/download", "post"),
        ("/v1/models/import-pack", "post"),
        ("/v1/models/load", "post"),
        ("/v1/models/switch", "post"),
        ("/v1/models/unload", "post"),
        ("/v1/plugins", "get"),
        ("/v1/plugins/{id}", "delete"),
        ("/v1/plugins/{id}", "get"),
        ("/v1/plugins/{id}/api-request", "post"),
        ("/v1/plugins/{id}/disable", "post"),
        ("/v1/plugins/{id}/enable", "post"),
        ("/v1/plugins/{id}/start", "post"),
        ("/v1/plugins/{id}/stop", "post"),
        ("/v1/plugins/events", "get"),
        ("/v1/plugins/import-pack", "post"),
        ("/v1/plugins/install", "post"),
        ("/v1/plugins/rpc", "get"),
        ("/v1/sessions", "get"),
        ("/v1/sessions", "post"),
        ("/v1/sessions/{id}", "delete"),
        ("/v1/sessions/{id}", "put"),
        ("/v1/sessions/{id}/messages", "get"),
        ("/v1/settings", "get"),
        ("/v1/settings/{pmid}", "get"),
        ("/v1/settings/{pmid}", "put"),
        ("/v1/setup/complete", "post"),
        ("/v1/setup/provision", "post"),
        ("/v1/setup/status", "get"),
        ("/v1/subtitles/render", "post"),
        ("/v1/system/diagnostics", "get"),
        ("/v1/system/gpu", "get"),
        ("/v1/tasks", "get"),
        ("/v1/tasks/{id}", "get"),
        ("/v1/tasks/{id}/cancel", "post"),
        ("/v1/tasks/{id}/restart", "post"),
        ("/v1/tasks/{id}/result", "get"),
        ("/v1/ui-state/{key}", "delete"),
        ("/v1/ui-state/{key}", "get"),
        ("/v1/ui-state/{key}", "put"),
        ("/v1/video/generations", "get"),
        ("/v1/video/generations", "post"),
        ("/v1/video/generations/{id}", "get"),
        ("/v1/video/generations/{id}/artifact", "get"),
        ("/v1/video/generations/{id}/reference", "get"),
        ("/v1/workspace", "get"),
        ("/v1/workspace/close", "post"),
        ("/v1/workspace/console/run", "post"),
        ("/v1/workspace/directories", "post"),
        ("/v1/workspace/directory", "get"),
        ("/v1/workspace/files", "get"),
        ("/v1/workspace/files", "post"),
        ("/v1/workspace/files", "put"),
        ("/v1/workspace/git/commit", "post"),
        ("/v1/workspace/git/diff", "post"),
        ("/v1/workspace/git/discard", "post"),
        ("/v1/workspace/git/stage", "post"),
        ("/v1/workspace/git/status", "get"),
        ("/v1/workspace/git/unstage", "post"),
        ("/v1/workspace/open", "post"),
        ("/v1/workspace/path", "delete"),
        ("/v1/workspace/path", "patch"),
        ("/v1/workspace/path/stat", "get"),
        ("/v1/workspace/plugins/{plugin_id}/preference", "put"),
        ("/v1/workspace/watch", "get"),
        ("/v1/workspace/search", "get"),
        ("/v1/workspace/search/text", "get"),
    ];

    #[test]
    fn v1_api_docs_publish_current_operation_surface() {
        let openapi = serde_json::to_value(api_docs()).expect("serialize v1 openapi");
        let paths = openapi["paths"].as_object().expect("paths");
        let mut actual = Vec::new();

        for (path, operations) in paths {
            let operations = operations.as_object().expect("path operations");
            for method in DOCUMENTED_METHODS {
                if operations.contains_key(*method) {
                    actual.push((path.as_str(), *method));
                }
            }
        }

        let mut expected = EXPECTED_OPERATIONS.to_vec();
        actual.sort_unstable();
        expected.sort_unstable();

        assert_eq!(actual, expected);
    }

    #[test]
    fn v1_api_docs_publish_cross_module_schema_components() {
        let openapi = serde_json::to_value(api_docs()).expect("serialize v1 openapi");
        let schemas = openapi["components"]["schemas"].as_object().expect("schema components");

        for schema in [
            "AgentResponsesClientMessage",
            "AudioTranscriptionRequest",
            "BackendStatusResponse",
            "ChatCompletionRequest",
            "CompleteSetupRequest",
            "ConvertRequest",
            "CreateModelRequest",
            "CreateSessionRequest",
            "ImageGenerationRequest",
            "OpenAiErrorResponse",
            "PluginResponse",
            "RenderSubtitleRequest",
            "SystemDiagnosticsResponse",
            "TaskResponse",
            "UiStateValueResponse",
            "VideoGenerationRequest",
            "WorkspaceStateResponse",
        ] {
            assert!(schemas.contains_key(schema), "missing schema component {schema}");
        }
    }
}
