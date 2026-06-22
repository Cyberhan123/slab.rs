use std::convert::Infallible;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Path as AxumPath, Query, State};
use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
use axum::routing::{delete, get, post, put};
use axum::{Json, Router, middleware};
use notify::event::{ModifyKind, RenameMode};
use notify::{Event as NotifyEvent, EventKind as NotifyEventKind, RecursiveMode, Watcher};
use serde::Deserialize;
use slab_app_core::context::AppState;
use slab_app_core::domain::services::WorkspaceService;
use slab_utils::path::absolute::canonicalize_existing_preserving_symlinks;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::ReceiverStream;
use utoipa::OpenApi;

use super::terminal::upgrade_workspace_terminal;
use crate::api::middleware::auth;
use crate::api::v1::workspace::schema::{
    RecentWorkspaceResponse, WorkspaceConfigResponse, WorkspaceConsoleOutput,
    WorkspaceConsoleRunCommand, WorkspaceCreateDirectoryCommand, WorkspaceCreateFileCommand,
    WorkspaceDeletePathCommand, WorkspaceDirectoryView, WorkspaceFileContent, WorkspaceFileEntry,
    WorkspaceFileKind, WorkspaceFileSearchView, WorkspaceGitCommitCommand, WorkspaceGitDiffCommand,
    WorkspaceGitDiffView, WorkspaceGitFileStatus, WorkspaceGitOperationView,
    WorkspaceGitPathCommand, WorkspaceGitStatusEntry, WorkspaceGitStatusSummary,
    WorkspaceGitStatusView, WorkspaceInfoResponse, WorkspaceOpenCommand, WorkspacePathMetadata,
    WorkspacePathView, WorkspacePluginConfig, WorkspacePluginPreferenceUpdate,
    WorkspaceRenamePathCommand, WorkspaceStateResponse, WorkspaceTextSearchFileMatch,
    WorkspaceTextSearchLineMatch, WorkspaceTextSearchView, WorkspaceWatchEntryKind,
    WorkspaceWatchEvent, WorkspaceWatchEventType, WorkspaceWriteFileCommand,
    WorkspaceWriteFileView,
};
use crate::api::validation::ValidatedJson;
use crate::error::ServerError;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceDirectoryQuery {
    relative_path: Option<String>,
    include_ignored: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceRelativePathQuery {
    relative_path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceSearchQuery {
    query: String,
}

#[derive(OpenApi)]
#[openapi(
    paths(
        workspace_state,
        open_workspace,
        close_workspace,
        read_directory,
        read_file,
        stat_path,
        watch_workspace,
        search_files,
        search_text,
        write_file,
        create_file,
        create_directory,
        rename_path,
        delete_path,
        git_status,
        git_stage,
        git_unstage,
        git_discard,
        git_commit,
        git_diff,
        console_run,
        update_plugin_preference
    ),
    components(schemas(
        WorkspaceStateResponse,
        WorkspaceInfoResponse,
        RecentWorkspaceResponse,
        WorkspaceConfigResponse,
        WorkspacePluginConfig,
        WorkspaceDirectoryView,
        WorkspaceFileEntry,
        WorkspaceFileKind,
        WorkspaceFileContent,
        WorkspaceFileSearchView,
        WorkspaceTextSearchView,
        WorkspaceTextSearchFileMatch,
        WorkspaceTextSearchLineMatch,
        WorkspacePathMetadata,
        WorkspaceWatchEvent,
        WorkspaceWatchEventType,
        WorkspaceWatchEntryKind,
        WorkspaceOpenCommand,
        WorkspaceWriteFileCommand,
        WorkspaceWriteFileView,
        WorkspaceCreateFileCommand,
        WorkspaceCreateDirectoryCommand,
        WorkspaceRenamePathCommand,
        WorkspaceDeletePathCommand,
        WorkspacePathView,
        WorkspaceGitPathCommand,
        WorkspaceGitCommitCommand,
        WorkspaceGitDiffCommand,
        WorkspaceGitStatusView,
        WorkspaceGitStatusSummary,
        WorkspaceGitStatusEntry,
        WorkspaceGitFileStatus,
        WorkspaceGitOperationView,
        WorkspaceGitDiffView,
        WorkspacePluginPreferenceUpdate,
        WorkspaceConsoleRunCommand,
        WorkspaceConsoleOutput
    ))
)]
pub struct WorkspaceApi;

pub fn router(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/workspace", get(workspace_state))
        .route("/workspace/open", post(open_workspace))
        .route("/workspace/close", post(close_workspace))
        .route("/workspace/directory", get(read_directory))
        .route("/workspace/files", get(read_file).post(create_file).put(write_file))
        .route("/workspace/directories", post(create_directory))
        .route("/workspace/path", delete(delete_path).patch(rename_path))
        .route("/workspace/path/stat", get(stat_path))
        .route("/workspace/watch", get(watch_workspace))
        .route("/workspace/terminal", get(upgrade_workspace_terminal))
        .route("/workspace/search", get(search_files))
        .route("/workspace/search/text", get(search_text))
        .route("/workspace/git/status", get(git_status))
        .route("/workspace/git/stage", post(git_stage))
        .route("/workspace/git/unstage", post(git_unstage))
        .route("/workspace/git/discard", post(git_discard))
        .route("/workspace/git/commit", post(git_commit))
        .route("/workspace/git/diff", post(git_diff))
        .route("/workspace/console/run", post(console_run))
        .route("/workspace/plugins/{plugin_id}/preference", put(update_plugin_preference))
        .route_layer(middleware::from_fn_with_state(state.clone(), auth::auth_middleware))
        .with_state(state)
}

#[utoipa::path(
    get,
    path = "/v1/workspace",
    tag = "workspace",
    responses(
        (status = 200, description = "Configured workspace state", body = WorkspaceStateResponse),
        (status = 400, description = "Configured workspace root is invalid"),
    )
)]
async fn workspace_state(
    State(state): State<Arc<AppState>>,
) -> Result<Json<WorkspaceStateResponse>, ServerError> {
    let Some(root) = state.workspace_root() else {
        return Ok(Json(workspace_state_response(None, None)));
    };
    let root = canonical_workspace_root(root)?;
    Ok(Json(workspace_state_response_for_root(state.as_ref(), &root)?))
}

#[utoipa::path(
    post,
    path = "/v1/workspace/open",
    tag = "workspace",
    request_body = WorkspaceOpenCommand,
    responses(
        (status = 200, description = "Workspace opened", body = WorkspaceStateResponse),
        (status = 400, description = "Bad request"),
    )
)]
async fn open_workspace(
    State(state): State<Arc<AppState>>,
    ValidatedJson(command): ValidatedJson<WorkspaceOpenCommand>,
) -> Result<Json<WorkspaceStateResponse>, ServerError> {
    let root = canonical_workspace_root(PathBuf::from(command.root_path))?;
    WorkspaceService::ensure_workspace_settings(&root)?;
    state.set_workspace_root(Some(root.clone())).map_err(ServerError::Internal)?;
    Ok(Json(workspace_state_response_for_root(state.as_ref(), &root)?))
}

#[utoipa::path(
    post,
    path = "/v1/workspace/close",
    tag = "workspace",
    responses(
        (status = 200, description = "Workspace closed", body = WorkspaceStateResponse)
    )
)]
async fn close_workspace(
    State(state): State<Arc<AppState>>,
) -> Result<Json<WorkspaceStateResponse>, ServerError> {
    state.set_workspace_root(None).map_err(ServerError::Internal)?;
    Ok(Json(workspace_state_response(None, None)))
}

#[utoipa::path(
    get,
    path = "/v1/workspace/directory",
    tag = "workspace",
    params(
        ("relativePath" = Option<String>, Query, description = "Workspace-relative directory path. Empty or omitted reads the root."),
        ("includeIgnored" = Option<bool>, Query, description = "Whether to include ignored and hidden workspace directories.")
    ),
    responses(
        (status = 200, description = "Workspace directory listing", body = WorkspaceDirectoryView),
        (status = 400, description = "Bad request"),
    )
)]
async fn read_directory(
    State(state): State<Arc<AppState>>,
    Query(query): Query<WorkspaceDirectoryQuery>,
) -> Result<Json<WorkspaceDirectoryView>, ServerError> {
    let root = active_workspace_root(state.as_ref())?;
    Ok(Json(WorkspaceService::read_directory(
        root,
        query.relative_path.as_deref(),
        query.include_ignored.unwrap_or(false),
    )?))
}

#[utoipa::path(
    get,
    path = "/v1/workspace/files",
    tag = "workspace",
    params(
        ("relativePath" = String, Query, description = "Workspace-relative file path.")
    ),
    responses(
        (status = 200, description = "Workspace file content", body = WorkspaceFileContent),
        (status = 400, description = "Bad request"),
    )
)]
async fn read_file(
    State(state): State<Arc<AppState>>,
    Query(query): Query<WorkspaceRelativePathQuery>,
) -> Result<Json<WorkspaceFileContent>, ServerError> {
    let root = active_workspace_root(state.as_ref())?;
    Ok(Json(WorkspaceService::read_file(root, &query.relative_path)?))
}

#[utoipa::path(
    get,
    path = "/v1/workspace/path/stat",
    tag = "workspace",
    params(
        ("relativePath" = String, Query, description = "Workspace-relative file or directory path.")
    ),
    responses(
        (status = 200, description = "Workspace path metadata", body = WorkspacePathMetadata),
        (status = 400, description = "Bad request"),
    )
)]
async fn stat_path(
    State(state): State<Arc<AppState>>,
    Query(query): Query<WorkspaceRelativePathQuery>,
) -> Result<Json<WorkspacePathMetadata>, ServerError> {
    let root = active_workspace_root(state.as_ref())?;
    Ok(Json(WorkspaceService::stat_path(root, &query.relative_path)?))
}

#[utoipa::path(
    get,
    path = "/v1/workspace/watch",
    tag = "workspace",
    responses(
        (status = 200, description = "Workspace file-system watch stream", body = WorkspaceWatchEvent),
        (status = 400, description = "Bad request"),
    )
)]
async fn watch_workspace(
    State(state): State<Arc<AppState>>,
) -> Result<Sse<impl futures::Stream<Item = Result<SseEvent, Infallible>>>, ServerError> {
    let root = active_workspace_root(state.as_ref())?;
    let (tx, rx) = tokio::sync::mpsc::channel::<WorkspaceWatchEvent>(128);
    spawn_workspace_watcher(root, tx)?;

    let stream = ReceiverStream::new(rx).map(|event| {
        let data = serde_json::to_string(&event).unwrap_or_else(|_| "{}".to_owned());
        Ok(SseEvent::default().event("workspace.watch").data(data))
    });

    Ok(Sse::new(stream)
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)).text("workspace-watch")))
}

#[utoipa::path(
    get,
    path = "/v1/workspace/search",
    tag = "workspace",
    params(
        ("query" = String, Query, description = "File-name search query.")
    ),
    responses(
        (status = 200, description = "Workspace file search results", body = WorkspaceFileSearchView),
        (status = 400, description = "Bad request"),
    )
)]
async fn search_files(
    State(state): State<Arc<AppState>>,
    Query(query): Query<WorkspaceSearchQuery>,
) -> Result<Json<WorkspaceFileSearchView>, ServerError> {
    let root = active_workspace_root(state.as_ref())?;
    Ok(Json(WorkspaceService::search_files(root, &query.query)?))
}

#[utoipa::path(
    get,
    path = "/v1/workspace/search/text",
    tag = "workspace",
    params(
        ("query" = String, Query, description = "Text search query.")
    ),
    responses(
        (status = 200, description = "Workspace text search results", body = WorkspaceTextSearchView),
        (status = 400, description = "Bad request"),
    )
)]
async fn search_text(
    State(state): State<Arc<AppState>>,
    Query(query): Query<WorkspaceSearchQuery>,
) -> Result<Json<WorkspaceTextSearchView>, ServerError> {
    let root = active_workspace_root(state.as_ref())?;
    Ok(Json(WorkspaceService::search_text(root, &query.query)?))
}

#[utoipa::path(
    put,
    path = "/v1/workspace/files",
    tag = "workspace",
    request_body = WorkspaceWriteFileCommand,
    responses(
        (status = 200, description = "Workspace file written", body = WorkspaceWriteFileView),
        (status = 400, description = "Bad request"),
    )
)]
async fn write_file(
    State(state): State<Arc<AppState>>,
    Json(command): Json<WorkspaceWriteFileCommand>,
) -> Result<Json<WorkspaceWriteFileView>, ServerError> {
    let root = active_workspace_root(state.as_ref())?;
    Ok(Json(WorkspaceService::write_file(root, command)?))
}

#[utoipa::path(
    post,
    path = "/v1/workspace/files",
    tag = "workspace",
    request_body = WorkspaceCreateFileCommand,
    responses(
        (status = 200, description = "Workspace file created", body = WorkspacePathView),
        (status = 400, description = "Bad request"),
    )
)]
async fn create_file(
    State(state): State<Arc<AppState>>,
    Json(command): Json<WorkspaceCreateFileCommand>,
) -> Result<Json<WorkspacePathView>, ServerError> {
    let root = active_workspace_root(state.as_ref())?;
    Ok(Json(WorkspaceService::create_file(root, command)?))
}

#[utoipa::path(
    post,
    path = "/v1/workspace/directories",
    tag = "workspace",
    request_body = WorkspaceCreateDirectoryCommand,
    responses(
        (status = 200, description = "Workspace directory created", body = WorkspacePathView),
        (status = 400, description = "Bad request"),
    )
)]
async fn create_directory(
    State(state): State<Arc<AppState>>,
    Json(command): Json<WorkspaceCreateDirectoryCommand>,
) -> Result<Json<WorkspacePathView>, ServerError> {
    let root = active_workspace_root(state.as_ref())?;
    Ok(Json(WorkspaceService::create_directory(root, command)?))
}

#[utoipa::path(
    patch,
    path = "/v1/workspace/path",
    tag = "workspace",
    request_body = WorkspaceRenamePathCommand,
    responses(
        (status = 200, description = "Workspace path renamed", body = WorkspacePathView),
        (status = 400, description = "Bad request"),
    )
)]
async fn rename_path(
    State(state): State<Arc<AppState>>,
    Json(command): Json<WorkspaceRenamePathCommand>,
) -> Result<Json<WorkspacePathView>, ServerError> {
    let root = active_workspace_root(state.as_ref())?;
    Ok(Json(WorkspaceService::rename_path(root, command)?))
}

#[utoipa::path(
    delete,
    path = "/v1/workspace/path",
    tag = "workspace",
    request_body = WorkspaceDeletePathCommand,
    responses(
        (status = 200, description = "Workspace path deleted", body = WorkspacePathView),
        (status = 400, description = "Bad request"),
    )
)]
async fn delete_path(
    State(state): State<Arc<AppState>>,
    Json(command): Json<WorkspaceDeletePathCommand>,
) -> Result<Json<WorkspacePathView>, ServerError> {
    let root = active_workspace_root(state.as_ref())?;
    Ok(Json(WorkspaceService::delete_path(root, command)?))
}

#[utoipa::path(
    get,
    path = "/v1/workspace/git/status",
    tag = "workspace",
    responses(
        (status = 200, description = "Workspace Git status", body = WorkspaceGitStatusView),
        (status = 400, description = "Bad request"),
    )
)]
async fn git_status(
    State(state): State<Arc<AppState>>,
) -> Result<Json<WorkspaceGitStatusView>, ServerError> {
    let root = active_workspace_root(state.as_ref())?;
    Ok(Json(WorkspaceService::git_status(root)?.into()))
}

#[utoipa::path(
    post,
    path = "/v1/workspace/git/stage",
    tag = "workspace",
    request_body = WorkspaceGitPathCommand,
    responses(
        (status = 200, description = "Workspace Git path staged", body = WorkspaceGitOperationView),
        (status = 400, description = "Bad request"),
    )
)]
async fn git_stage(
    State(state): State<Arc<AppState>>,
    ValidatedJson(command): ValidatedJson<WorkspaceGitPathCommand>,
) -> Result<Json<WorkspaceGitOperationView>, ServerError> {
    let root = active_workspace_root(state.as_ref())?;
    Ok(Json(WorkspaceService::git_stage(root, &command.path)?.into()))
}

#[utoipa::path(
    post,
    path = "/v1/workspace/git/unstage",
    tag = "workspace",
    request_body = WorkspaceGitPathCommand,
    responses(
        (status = 200, description = "Workspace Git path unstaged", body = WorkspaceGitOperationView),
        (status = 400, description = "Bad request"),
    )
)]
async fn git_unstage(
    State(state): State<Arc<AppState>>,
    ValidatedJson(command): ValidatedJson<WorkspaceGitPathCommand>,
) -> Result<Json<WorkspaceGitOperationView>, ServerError> {
    let root = active_workspace_root(state.as_ref())?;
    Ok(Json(WorkspaceService::git_unstage(root, &command.path)?.into()))
}

#[utoipa::path(
    post,
    path = "/v1/workspace/git/discard",
    tag = "workspace",
    request_body = WorkspaceGitPathCommand,
    responses(
        (status = 200, description = "Workspace Git path discarded", body = WorkspaceGitOperationView),
        (status = 400, description = "Bad request"),
    )
)]
async fn git_discard(
    State(state): State<Arc<AppState>>,
    ValidatedJson(command): ValidatedJson<WorkspaceGitPathCommand>,
) -> Result<Json<WorkspaceGitOperationView>, ServerError> {
    let root = active_workspace_root(state.as_ref())?;
    Ok(Json(WorkspaceService::git_discard(root, &command.path)?.into()))
}

#[utoipa::path(
    post,
    path = "/v1/workspace/git/commit",
    tag = "workspace",
    request_body = WorkspaceGitCommitCommand,
    responses(
        (status = 200, description = "Workspace Git commit created", body = WorkspaceGitOperationView),
        (status = 400, description = "Bad request"),
    )
)]
async fn git_commit(
    State(state): State<Arc<AppState>>,
    ValidatedJson(command): ValidatedJson<WorkspaceGitCommitCommand>,
) -> Result<Json<WorkspaceGitOperationView>, ServerError> {
    let root = active_workspace_root(state.as_ref())?;
    Ok(Json(WorkspaceService::git_commit(root, &command.message)?.into()))
}

#[utoipa::path(
    post,
    path = "/v1/workspace/git/diff",
    tag = "workspace",
    request_body = WorkspaceGitDiffCommand,
    responses(
        (status = 200, description = "Workspace Git path diff", body = WorkspaceGitDiffView),
        (status = 400, description = "Bad request"),
    )
)]
async fn git_diff(
    State(state): State<Arc<AppState>>,
    ValidatedJson(command): ValidatedJson<WorkspaceGitDiffCommand>,
) -> Result<Json<WorkspaceGitDiffView>, ServerError> {
    let root = active_workspace_root(state.as_ref())?;
    Ok(Json(WorkspaceService::git_diff(root, &command.path, command.staged)?.into()))
}

#[utoipa::path(
    post,
    path = "/v1/workspace/console/run",
    tag = "workspace",
    request_body = WorkspaceConsoleRunCommand,
    responses(
        (status = 200, description = "Workspace console command output", body = WorkspaceConsoleOutput),
        (status = 400, description = "Bad request"),
    )
)]
async fn console_run(
    State(state): State<Arc<AppState>>,
    ValidatedJson(command): ValidatedJson<WorkspaceConsoleRunCommand>,
) -> Result<Json<WorkspaceConsoleOutput>, ServerError> {
    let root = active_workspace_root(state.as_ref())?;
    Ok(Json(WorkspaceService::run_console_command(root, &command.command).await?))
}

#[utoipa::path(
    put,
    path = "/v1/workspace/plugins/{plugin_id}/preference",
    tag = "workspace",
    params(
        ("plugin_id" = String, Path, description = "Plugin manifest id.")
    ),
    request_body = WorkspacePluginPreferenceUpdate,
    responses(
        (status = 200, description = "Workspace plugin preference updated", body = WorkspaceStateResponse),
        (status = 400, description = "Bad request"),
    )
)]
async fn update_plugin_preference(
    State(state): State<Arc<AppState>>,
    AxumPath(plugin_id): AxumPath<String>,
    Json(update): Json<WorkspacePluginPreferenceUpdate>,
) -> Result<Json<WorkspaceStateResponse>, ServerError> {
    let root = active_workspace_root(state.as_ref())?;
    let config = WorkspaceService::update_workspace_plugin_preference(&root, &plugin_id, update)?;
    Ok(Json(workspace_state_response(Some(workspace_info(state.as_ref(), &root)), Some(config))))
}

fn spawn_workspace_watcher(
    root: PathBuf,
    tx: tokio::sync::mpsc::Sender<WorkspaceWatchEvent>,
) -> Result<(), ServerError> {
    let (notify_tx, notify_rx) = std::sync::mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |event| {
        let _ = notify_tx.send(event);
    })
    .map_err(|error| {
        ServerError::Internal(format!("failed to create workspace watcher: {error}"))
    })?;

    watcher
        .watch(&root, RecursiveMode::Recursive)
        .map_err(|error| ServerError::Internal(format!("failed to watch workspace: {error}")))?;

    tokio::task::spawn_blocking(move || {
        let _watcher = watcher;
        let mut sequence_number = 0_u64;

        while let Ok(event) = notify_rx.recv() {
            if tx.is_closed() {
                break;
            }

            match event {
                Ok(event) => {
                    for event in workspace_watch_events(&root, event, &mut sequence_number) {
                        if tx.blocking_send(event).is_err() {
                            return;
                        }
                    }
                }
                Err(error) => {
                    tracing::debug!(%error, "workspace watcher event failed");
                }
            }
        }
    });

    Ok(())
}

fn workspace_watch_events(
    root: &Path,
    event: NotifyEvent,
    sequence_number: &mut u64,
) -> Vec<WorkspaceWatchEvent> {
    match event.kind {
        NotifyEventKind::Create(_) => workspace_watch_events_for_paths(
            root,
            event.paths,
            WorkspaceWatchEventType::Created,
            sequence_number,
        ),
        NotifyEventKind::Remove(_) => workspace_watch_events_for_paths(
            root,
            event.paths,
            WorkspaceWatchEventType::Deleted,
            sequence_number,
        ),
        NotifyEventKind::Modify(ModifyKind::Name(RenameMode::Both)) if event.paths.len() >= 2 => {
            let mut events = Vec::new();
            if let Some(old_path) = event.paths.first() {
                if let Some(event) = workspace_watch_event(
                    root,
                    old_path,
                    WorkspaceWatchEventType::Deleted,
                    sequence_number,
                ) {
                    events.push(event);
                }
            }
            if let Some(new_path) = event.paths.get(1) {
                if let Some(event) = workspace_watch_event(
                    root,
                    new_path,
                    WorkspaceWatchEventType::Created,
                    sequence_number,
                ) {
                    events.push(event);
                }
            }
            events
        }
        NotifyEventKind::Modify(_) => workspace_watch_events_for_paths(
            root,
            event.paths,
            WorkspaceWatchEventType::Changed,
            sequence_number,
        ),
        _ => Vec::new(),
    }
}

fn workspace_watch_events_for_paths(
    root: &Path,
    paths: Vec<PathBuf>,
    event_type: WorkspaceWatchEventType,
    sequence_number: &mut u64,
) -> Vec<WorkspaceWatchEvent> {
    paths
        .iter()
        .filter_map(|path| workspace_watch_event(root, path, event_type, sequence_number))
        .collect()
}

fn workspace_watch_event(
    root: &Path,
    path: &Path,
    event_type: WorkspaceWatchEventType,
    sequence_number: &mut u64,
) -> Option<WorkspaceWatchEvent> {
    let relative_path = path.strip_prefix(root).ok()?;
    let relative_path = relative_path.to_string_lossy().replace('\\', "/");
    if relative_path.is_empty() {
        return None;
    }

    *sequence_number += 1;
    Some(WorkspaceWatchEvent {
        sequence_number: *sequence_number,
        event_type,
        relative_path,
        kind: workspace_watch_entry_kind(path),
    })
}

fn workspace_watch_entry_kind(path: &Path) -> WorkspaceWatchEntryKind {
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_dir() => WorkspaceWatchEntryKind::Directory,
        Ok(metadata) if metadata.is_file() => WorkspaceWatchEntryKind::File,
        Ok(_) | Err(_) => WorkspaceWatchEntryKind::Unknown,
    }
}

pub(super) fn active_workspace_root(state: &AppState) -> Result<PathBuf, ServerError> {
    let root = state
        .workspace_root()
        .or_else(|| WorkspaceService::workspace_root_from_config(&state.context.config))
        .ok_or_else(|| ServerError::BadRequest("no workspace is currently open".to_owned()))?;
    canonical_workspace_root(root)
}

fn canonical_workspace_root(root: PathBuf) -> Result<PathBuf, ServerError> {
    let canonical = canonicalize_existing_preserving_symlinks(&root).map_err(|error| {
        ServerError::BadRequest(format!(
            "failed to resolve workspace root {}: {error}",
            root.display()
        ))
    })?;
    if !canonical.is_dir() {
        return Err(ServerError::BadRequest(format!(
            "workspace root {} is not a directory",
            canonical.display()
        )));
    }
    Ok(canonical)
}

fn workspace_info(state: &AppState, root: &Path) -> WorkspaceInfoResponse {
    let config = &state.context.config;
    let workspace_settings_path = root.join(".slab").join("settings.json");
    let configured_root = config.workspace_root.as_ref().and_then(|root| {
        canonicalize_existing_preserving_symlinks(root)
            .inspect_err(|error| {
                tracing::warn!(
                    workspace_root = %root.display(),
                    %error,
                    "failed to canonicalize configured workspace root"
                );
            })
            .ok()
    });
    let settings_path = config
        .settings_overlay_path
        .as_ref()
        .filter(|_| configured_root.as_deref() == Some(root))
        .unwrap_or(&workspace_settings_path);

    WorkspaceInfoResponse {
        root_path: path_string(root),
        name: root.file_name().and_then(|name| name.to_str()).unwrap_or("Workspace").to_owned(),
        slab_dir: path_string(&root.join(".slab")),
        settings_path: path_string(settings_path),
        settings_overlay_path: config.settings_overlay_path.as_deref().map(path_string),
        model_config_dir: path_string(&config.model_config_dir),
        session_state_dir: config.session_state_dir.clone(),
    }
}

fn workspace_state_response_for_root(
    state: &AppState,
    root: &Path,
) -> Result<WorkspaceStateResponse, ServerError> {
    let config = WorkspaceService::workspace_config(root)?;
    Ok(workspace_state_response(Some(workspace_info(state, root)), Some(config)))
}

fn workspace_state_response(
    current: Option<WorkspaceInfoResponse>,
    config: Option<WorkspaceConfigResponse>,
) -> WorkspaceStateResponse {
    WorkspaceStateResponse { current, recent: Vec::new(), config }
}

fn path_string(path: &Path) -> String {
    let raw = path.to_string_lossy();
    if let Some(path) = raw.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{path}");
    }
    if let Some(path) = raw.strip_prefix(r"\\?\") {
        return path.to_owned();
    }
    raw.into_owned()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::Value;
    use utoipa::OpenApi;

    use slab_utils::path::absolute::canonicalize_existing_preserving_symlinks;

    use super::{WorkspaceApi, canonical_workspace_root, path_string};
    use crate::error::ServerError;

    #[test]
    fn workspace_routes_publish_rest_slice_in_openapi() {
        let openapi =
            serde_json::to_value(WorkspaceApi::openapi()).expect("serialize workspace openapi");

        for (path, method) in [
            ("/v1/workspace", "get"),
            ("/v1/workspace/open", "post"),
            ("/v1/workspace/close", "post"),
            ("/v1/workspace/directory", "get"),
            ("/v1/workspace/files", "get"),
            ("/v1/workspace/files", "put"),
            ("/v1/workspace/files", "post"),
            ("/v1/workspace/path", "patch"),
            ("/v1/workspace/path", "delete"),
            ("/v1/workspace/watch", "get"),
            ("/v1/workspace/search/text", "get"),
            ("/v1/workspace/git/status", "get"),
            ("/v1/workspace/git/stage", "post"),
            ("/v1/workspace/git/unstage", "post"),
            ("/v1/workspace/git/discard", "post"),
            ("/v1/workspace/git/commit", "post"),
            ("/v1/workspace/git/diff", "post"),
            ("/v1/workspace/console/run", "post"),
            ("/v1/workspace/plugins/{plugin_id}/preference", "put"),
        ] {
            assert!(openapi["paths"][path][method].is_object(), "missing {method} {path}");
        }
    }

    #[test]
    fn workspace_file_query_uses_camel_case_path_parameter() {
        let openapi =
            serde_json::to_value(WorkspaceApi::openapi()).expect("serialize workspace openapi");
        let parameters = openapi["paths"]["/v1/workspace/files"]["get"]["parameters"]
            .as_array()
            .expect("parameters");

        assert!(parameters.iter().any(|parameter| {
            parameter["name"] == Value::String("relativePath".to_owned())
                && parameter["in"] == Value::String("query".to_owned())
        }));
    }

    #[test]
    fn canonical_workspace_root_accepts_existing_directories() {
        let temp = tempfile::tempdir().expect("tempdir");
        let nested = temp.path().join("workspace");
        fs::create_dir(&nested).expect("workspace dir");

        let canonical = canonical_workspace_root(nested.clone()).expect("canonical workspace");

        assert_eq!(
            canonical,
            canonicalize_existing_preserving_symlinks(&nested).expect("expected canonical path")
        );
    }

    #[test]
    fn canonical_workspace_root_rejects_missing_paths_and_files() {
        let temp = tempfile::tempdir().expect("tempdir");
        let missing = temp.path().join("missing");
        let missing_error = canonical_workspace_root(missing).expect_err("missing path rejected");
        assert!(matches!(missing_error, ServerError::BadRequest(_)));

        let file = temp.path().join("not-a-directory.txt");
        fs::write(&file, "content").expect("write file");
        let file_error = canonical_workspace_root(file).expect_err("file path rejected");
        assert!(matches!(file_error, ServerError::BadRequest(_)));
    }

    #[test]
    fn path_string_removes_windows_verbatim_prefixes() {
        assert_eq!(
            path_string(std::path::Path::new(r"\\?\C:\Users\example\repo")),
            r"C:\Users\example\repo"
        );
        assert_eq!(
            path_string(std::path::Path::new(r"\\?\UNC\server\share\repo")),
            r"\\server\share\repo"
        );
    }
}

#[cfg(test)]
mod route_tests {
    use std::fs;

    use axum::http::StatusCode;

    use slab_utils::path::absolute::canonicalize_existing_preserving_symlinks;

    use super::path_string;
    use crate::api::test_support::{TestServer, TestServerOptions};

    #[tokio::test]
    async fn workspace_state_reports_configured_root() {
        let workspace_root = tempfile::tempdir().expect("workspace root");
        let server = TestServer::new_with(TestServerOptions {
            workspace_root: Some(workspace_root.path().to_path_buf()),
            ..Default::default()
        })
        .await;

        let response = server.get("/v1/workspace").await;

        assert_eq!(response.status, StatusCode::OK);
        assert_eq!(
            response.body["current"]["rootPath"],
            path_string(
                &canonicalize_existing_preserving_symlinks(workspace_root.path())
                    .expect("canonical root")
            )
        );
        assert_eq!(response.body["config"]["plugins"], serde_json::json!({}));
    }

    #[tokio::test]
    async fn workspace_directory_route_rejects_missing_active_workspace() {
        let server = TestServer::new().await;

        let response = server.get("/v1/workspace/directory?relativePath=src").await;

        assert_eq!(response.status, StatusCode::BAD_REQUEST);
        assert!(
            response.body["message"]
                .as_str()
                .unwrap_or_default()
                .contains("no workspace is currently open")
        );
    }

    #[tokio::test]
    async fn workspace_open_rejects_blank_root_path() {
        let server = TestServer::new().await;

        let response =
            server.post_json("/v1/workspace/open", serde_json::json!({ "rootPath": " " })).await;

        assert_eq!(response.status, StatusCode::BAD_REQUEST);
        assert!(response.body["message"].as_str().unwrap_or_default().contains("root path"));
    }

    #[tokio::test]
    async fn workspace_file_routes_use_active_workspace_root() {
        let workspace_root = tempfile::tempdir().expect("workspace root");
        fs::create_dir_all(workspace_root.path().join("src")).expect("create workspace dir");
        fs::write(workspace_root.path().join("src/main.rs"), "fn main() {}").expect("seed file");

        let server = TestServer::new_with(TestServerOptions {
            workspace_root: Some(workspace_root.path().to_path_buf()),
            ..Default::default()
        })
        .await;

        let response = server.get("/v1/workspace/files?relativePath=src/main.rs").await;

        assert_eq!(response.status, StatusCode::OK);
        assert_eq!(response.body["content"], "fn main() {}");
    }

    #[tokio::test]
    async fn workspace_plugin_preference_route_updates_workspace_settings() {
        let workspace_root = tempfile::tempdir().expect("workspace root");
        let server = TestServer::new_with(TestServerOptions {
            workspace_root: Some(workspace_root.path().to_path_buf()),
            ..Default::default()
        })
        .await;

        let response = server
            .put_json(
                "/v1/workspace/plugins/video-subtitle_translator/preference",
                serde_json::json!({ "enabled": false }),
            )
            .await;

        assert_eq!(response.status, StatusCode::OK);
        assert_eq!(
            response.body["config"]["plugins"]["video-subtitle_translator"]["enabled"],
            false
        );
        let settings =
            fs::read_to_string(workspace_root.path().join(".slab").join("settings.json"))
                .expect("settings");
        assert!(settings.contains("video-subtitle_translator"));

        let response = server
            .put_json(
                "/v1/workspace/plugins/video-subtitle_translator/preference",
                serde_json::json!({ "enabled": true }),
            )
            .await;

        assert_eq!(response.status, StatusCode::OK);
        assert_eq!(response.body["config"]["plugins"], serde_json::json!({}));
    }
}
