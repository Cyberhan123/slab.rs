//! Typed request handlers — the "body" of each harness method.
//!
//! Each function takes the [`HarnessSession`] **by value** plus typed params
//! and returns a typed result (or `String` error). The host (see `host.rs`)
//! registers them into the [`Router`] as default typed handlers. Phase 5 will
//! wrap the `thread_op` / `establish_op` families in transformers to absorb
//! cross-cutting concerns (binding resolution, fan-out establishment); for now
//! every handler does that work inline, preserving the prior behavior exactly.

use chrono::Utc;
use slab_agent::port::ThreadListFilter;
use slab_agent::protocol::{Thread, Turn};
use slab_cloud_provider::default_models_for_provider;
use slab_proto::harness::messages::{
    ApprovalPolicy, ApprovalResolveParams, ApprovalResolveResult, SandboxPolicy, ShutdownParams,
    ShutdownResult, ThreadArchiveParams, ThreadArchiveResult, ThreadForkParams, ThreadForkResult,
    ThreadListParams, ThreadListResult, ThreadResumeParams, ThreadResumeResult,
    ThreadRollbackParams, ThreadRollbackResult, ThreadStartParams, ThreadStartResult,
    TurnInterruptParams, TurnInterruptResult, TurnStartParams, TurnStartResult,
    WorkspaceMigrateParams, WorkspaceMigrateResult,
};
use slab_proto::harness::{ModelInfo, ModelListParams, ModelListResult};

use super::session::HarnessSession;
use super::transform::Established;
use super::{
    join_user_text, messages_from_input, model_info_from_spec, thread_from_snapshot,
    thread_from_snapshot_with_id, thread_from_snapshot_with_turns,
};
use crate::api::v1::agent::schema::AgentConfigInput;

pub(crate) async fn thread_start(
    session: HarnessSession,
    params: ThreadStartParams,
) -> Result<ThreadStartResult, String> {
    let thread_id = session.mint_thread_id();
    let model_provider =
        params.model_provider.clone().or_else(|| params.model.clone()).unwrap_or_default();
    session.bind_empty(&thread_id);

    let cwd =
        params.cwd.as_ref().map(|path| path.to_string_lossy().into_owned()).unwrap_or_default();
    let thread = Thread {
        id: thread_id.clone(),
        preview: String::new(),
        model_provider: model_provider.clone(),
        created_at: Utc::now().timestamp_millis(),
        cwd: (!cwd.is_empty()).then_some(cwd.clone()),
        ..Default::default()
    };
    Ok(ThreadStartResult {
        thread,
        model: params.model.clone().unwrap_or_default(),
        model_provider,
        cwd,
        approval_policy: params.approval_policy.unwrap_or(ApprovalPolicy::OnRequest),
        sandbox: SandboxPolicy::default(),
        reasoning_effort: None,
    })
}

pub(crate) async fn turn_start(
    session: HarnessSession,
    params: TurnStartParams,
) -> Result<TurnStartResult, String> {
    match session.existing_real(&params.thread_id) {
        Some(real_id) => {
            let content = join_user_text(&params.input);
            session.service().send_input(&real_id, content).await.map_err(|e| e.to_string())?;
        }
        None => {
            // First turn materializes the slab thread (create + run).
            let config =
                AgentConfigInput { model: params.model.clone(), ..Default::default() }.into();
            let messages = messages_from_input(&params.input);
            let real_id = session
                .service()
                .spawn(session.session_id().to_owned(), config, messages)
                .await
                .map_err(|e| e.to_string())?;
            session.bind(&params.thread_id, real_id.clone());
            // ⚠️ Only the first turn establishes a fan-out task; subsequent
            // turns reuse it (a second task would double-deliver every event).
            session.spawn_event_fanout(real_id, params.thread_id.clone());
        }
    }

    // Apply the per-session permission mode (if any) to the real thread id.
    if let Some(mode) = params.permission_mode {
        let real_id = session.real_id_for(&params.thread_id);
        let runtime_mode =
            slab_app_core::infra::agent::exec_policy::permission_mode_from_proto(mode);
        session.service().set_thread_mode(&real_id, runtime_mode).await;
    }

    Ok(TurnStartResult {
        turn: Turn {
            id: "0".to_owned(),
            items: vec![],
            status: "inProgress".to_owned(),
            error: None,
        },
    })
}

pub(crate) async fn turn_interrupt(
    session: HarnessSession,
    real_id: String,
    params: TurnInterruptParams,
) -> Result<TurnInterruptResult, String> {
    let _ = &params;
    session.service().interrupt(&real_id).await.map_err(|e| e.to_string())?;
    Ok(TurnInterruptResult { status: Some("interrupting".to_owned()) })
}

pub(crate) async fn approval_resolve(
    session: HarnessSession,
    real_id: String,
    params: ApprovalResolveParams,
) -> Result<ApprovalResolveResult, String> {
    let scope = slab_app_core::infra::agent::exec_policy::approval_scope_from_proto(
        params.scope.unwrap_or(slab_proto::harness::ApprovalScope::RunOnce),
    );
    let delivered =
        session.service().approve_call(&real_id, &params.item_id, params.approved, scope);
    Ok(ApprovalResolveResult { delivered: Some(delivered), status: None })
}

pub(crate) async fn shutdown(
    session: HarnessSession,
    real_id: String,
    params: ShutdownParams,
) -> Result<ShutdownResult, String> {
    let _ = &params;
    session.service().shutdown(&real_id).await.map_err(|e| e.to_string())?;
    Ok(ShutdownResult { status: Some("shutdown".to_owned()) })
}

pub(crate) async fn thread_list(
    session: HarnessSession,
    params: ThreadListParams,
) -> Result<ThreadListResult, String> {
    let filter = ThreadListFilter {
        limit: params.limit,
        before_updated_at: params.cursor.clone(),
        // Archived threads (soft-deleted via `thread/archive`) are hidden from
        // the default list. Callers opt in via `include_archived`.
        include_archived: false,
    };
    let snapshots = session
        .service()
        .list_session_threads_filtered(session.session_id(), &filter)
        .await
        .map_err(|e| e.to_string())?;
    let next_cursor = match (params.limit, snapshots.last()) {
        (Some(limit), Some(last)) if (snapshots.len() as u32) >= limit => {
            Some(last.updated_at.clone())
        }
        _ => None,
    };
    let data: Vec<Thread> = snapshots.iter().map(thread_from_snapshot).collect();
    Ok(ThreadListResult { data, next_cursor })
}

pub(crate) async fn model_list(
    session: HarnessSession,
    params: ModelListParams,
) -> Result<ModelListResult, String> {
    // Curated catalog of *configured* providers only.
    let providers = &session.state().context.pmid.config().chat.providers;
    let data: Vec<ModelInfo> = providers
        .iter()
        .filter(|provider| match params.model_providers.as_ref() {
            Some(ids) => ids.iter().any(|id| id == &provider.id),
            None => true,
        })
        .flat_map(|provider| {
            let provider_id = provider.id.clone();
            default_models_for_provider(provider)
                .into_iter()
                .map(move |spec| model_info_from_spec(&provider_id, &spec))
        })
        .collect();
    Ok(ModelListResult { data, next_cursor: None })
}

pub(crate) async fn thread_fork(
    session: HarnessSession,
    params: ThreadForkParams,
) -> Result<Established<ThreadForkResult>, String> {
    // `sandbox_override` is accepted but not applied (see thread/start).
    let real_parent = session.real_id_for(&params.thread_id);
    let snapshot = session
        .service()
        .fork_thread(&real_parent, params.model_override.clone())
        .await
        .map_err(|e| e.to_string())?;
    let harness_id = session.mint_thread_id();
    // `bind` + `spawn_event_fanout` run centrally in the establish_op adapter.
    Ok(Established {
        real_id: snapshot.id.clone(),
        harness_id: harness_id.clone(),
        result: ThreadForkResult { thread: thread_from_snapshot_with_id(&harness_id, &snapshot) },
    })
}

pub(crate) async fn thread_rollback(
    session: HarnessSession,
    real_id: String,
    params: ThreadRollbackParams,
) -> Result<ThreadRollbackResult, String> {
    let to_turn_index: u32 = params
        .to_turn_id
        .parse()
        .map_err(|e| format!("invalid to_turn_id `{}`: {e}", params.to_turn_id))?;
    let snapshot = session
        .service()
        .rollback_thread(&real_id, to_turn_index)
        .await
        .map_err(|e| e.to_string())?;
    Ok(ThreadRollbackResult { thread: thread_from_snapshot(&snapshot) })
}

pub(crate) async fn thread_archive(
    session: HarnessSession,
    real_id: String,
    params: ThreadArchiveParams,
) -> Result<ThreadArchiveResult, String> {
    let _ = &params;
    let snapshot = session.service().archive_thread(&real_id).await.map_err(|e| e.to_string())?;
    Ok(ThreadArchiveResult { thread: thread_from_snapshot(&snapshot) })
}

pub(crate) async fn thread_resume(
    session: HarnessSession,
    params: ThreadResumeParams,
) -> Result<Established<ThreadResumeResult>, String> {
    // Resolve the target thread: an explicit id wins; otherwise fall back to
    // the session's most-recent root thread.
    let (harness_id, snapshot) = match params.thread_id.as_deref() {
        Some(id) => {
            let real_id = session.real_id_for(id);
            let snapshot = session
                .service()
                .thread_snapshot(&real_id)
                .await
                .map_err(|e| e.to_string())?
                .ok_or_else(|| format!("thread not found: {id}"))?;
            (id.to_owned(), snapshot)
        }
        None => {
            let restored = session
                .service()
                .restore_session(session.session_id())
                .await
                .map_err(|e| e.to_string())?;
            let snapshot =
                restored.thread.ok_or_else(|| "no thread to resume for session".to_owned())?;
            (session.mint_thread_id(), snapshot)
        }
    };
    // `bind` + `spawn_event_fanout` are run centrally by the establish_op
    // adapter once we return the Established thread.
    let messages =
        session.service().list_thread_messages(&snapshot.id).await.map_err(|e| e.to_string())?;
    let turn_states =
        session.service().list_turn_states(&snapshot.id).await.map_err(|e| e.to_string())?;
    let turn_items =
        session.service().list_turn_items(&snapshot.id).await.map_err(|e| e.to_string())?;
    Ok(Established {
        real_id: snapshot.id.clone(),
        harness_id: harness_id.clone(),
        result: ThreadResumeResult {
            thread: thread_from_snapshot_with_turns(
                &harness_id,
                &snapshot,
                &messages,
                &turn_states,
                &turn_items,
            ),
        },
    })
}

pub(crate) async fn workspace_migrate(
    session: HarnessSession,
    params: WorkspaceMigrateParams,
) -> Result<WorkspaceMigrateResult, String> {
    let config = &session.state().context.config;
    let workspace_root = params
        .workspace_root
        .or_else(|| {
            slab_app_core::domain::services::WorkspaceService::workspace_root_from_config(config)
        })
        .ok_or_else(|| "no active workspace to migrate".to_owned())?;
    let snapshot_dir = std::path::PathBuf::from(&config.session_state_dir);
    let outcome = session
        .service()
        .prepare_workspace_migration(&workspace_root, &snapshot_dir)
        .await
        .map_err(|e| e.to_string())?;
    Ok(WorkspaceMigrateResult {
        project_id: Some(outcome.project_id),
        suspended_count: outcome.suspended_count as u32,
    })
}
