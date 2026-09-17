use std::path::{Path, PathBuf};

use chrono::{DateTime, Local, Utc};
use uuid::Uuid;

use crate::context::ModelState;
use crate::domain::models::{
    CreateSessionCommand, DeleteSessionView, SessionMessageView, SessionView,
};
use crate::error::AppCoreError;
use crate::infra::db::{ChatSession, ChatStore, SessionStore};

/// Max character length of the sanitized session-name component inside a
/// session artifacts directory name.
const SESSION_NAME_MAX_CHARS: usize = 50;
/// Placeholder leaf component when a session name sanitizes to nothing.
const UNTITLED_SESSION_NAME: &str = "untitled";

#[derive(Clone)]
pub struct SessionService {
    state: ModelState,
    /// Test-only pinned artifacts root — unit tests must not touch the real
    /// Documents tree, and process-global env mutation would race parallel
    /// tests. Production derives the root per allocation via
    /// [`slab_utils::app_home::global_sessions_root`] (which honors the
    /// `SLAB_GLOBAL_SESSIONS_DIR` override the server test harnesses set).
    #[cfg(test)]
    test_artifacts_root: Option<PathBuf>,
}

impl SessionService {
    pub fn new(state: ModelState) -> Self {
        Self {
            state,
            #[cfg(test)]
            test_artifacts_root: None,
        }
    }

    /// Test-only construction with an explicit artifacts root.
    #[cfg(test)]
    pub(crate) fn new_with_artifacts_root(state: ModelState, artifacts_root: PathBuf) -> Self {
        Self { state, test_artifacts_root: Some(artifacts_root) }
    }

    /// Root of the per-session artifacts tree.
    fn artifacts_root(&self) -> PathBuf {
        #[cfg(test)]
        if let Some(root) = &self.test_artifacts_root {
            return root.clone();
        }
        slab_utils::app_home::global_sessions_root()
    }

    pub async fn create_session(
        &self,
        req: CreateSessionCommand,
    ) -> Result<SessionView, AppCoreError> {
        let now = Utc::now();
        let id = Uuid::new_v4().to_string();
        let state_path = self.allocate_global_state_path(&id, now, req.name.as_deref());
        let session = ChatSession {
            id,
            name: req.name.unwrap_or_default(),
            state_path,
            created_at: now,
            updated_at: now,
        };
        self.state.store().create_session(session.clone()).await?;
        Ok(SessionView::from(&session))
    }

    pub async fn list_sessions(&self) -> Result<Vec<SessionView>, AppCoreError> {
        let sessions = self.state.store().list_sessions().await?;
        Ok(sessions.into_iter().map(|session| SessionView::from(&session)).collect())
    }

    pub async fn update_session_name(
        &self,
        id: &str,
        name: String,
    ) -> Result<SessionView, AppCoreError> {
        let name = name.trim().to_owned();
        if name.is_empty() {
            return Err(AppCoreError::BadRequest("session name must not be empty".to_owned()));
        }

        let mut session = self
            .state
            .store()
            .update_session_name(id, &name, Utc::now())
            .await?
            .ok_or_else(|| AppCoreError::NotFound(format!("session {id} not found")))?;
        // Rename never migrates an existing artifacts dir (live threads root
        // at the recorded path); the allocation below only BACKFILLS sessions
        // that predate state_path population (or arrived through the
        // auto-session chat-completions path).
        if session.state_path.is_none() {
            let now = Utc::now();
            let state_path =
                self.allocate_global_state_path(&session.id, now, Some(session.name.as_str()));
            if let Some(state_path) = state_path {
                session = self
                    .state
                    .store()
                    .update_session_state_path(id, &state_path, now)
                    .await?
                    .unwrap_or(session);
            }
        }
        Ok(SessionView::from(&session))
    }

    pub async fn delete_session(&self, id: &str) -> Result<DeleteSessionView, AppCoreError> {
        // The on-disk artifacts dir (if any) is deliberately NOT removed:
        // session deletion already leaves rollouts/traces in place, and the
        // user-visible Documents tree must not lose files the model wrote.
        self.state.store().delete_session(id).await?;
        Ok(DeleteSessionView { deleted: true })
    }

    pub async fn list_session_messages(
        &self,
        id: &str,
    ) -> Result<Vec<SessionMessageView>, AppCoreError> {
        let messages = self.state.store().list_messages(id).await?;
        Ok(messages.into_iter().map(|message| SessionMessageView::from(&message)).collect())
    }

    /// The per-session artifacts directory for a GLOBAL (workspace-less)
    /// session: the `state_path` recorded on the session row. The harness
    /// roots such sessions' threads here instead of letting a rootless tool
    /// registration degrade relative paths to the process cwd. `None` for
    /// workspace sessions (no dir allocated) or unknown ids.
    pub async fn global_session_dir(&self, id: &str) -> Option<PathBuf> {
        let session = self.state.store().get_session(id).await.ok()??;
        session.state_path.map(PathBuf::from)
    }

    /// Allocate (and create) a session artifacts dir under the global
    /// artifacts root for workspace-less processes. `None` when a workspace is
    /// configured (workspace sessions get no dir) or the directory cannot be
    /// created — session creation must not hard-fail on an unwritable
    /// Documents folder; the session stays rootless and the harness keeps the
    /// historical cwd degradation. The workspace gate uses the EXPLICIT root
    /// only: the process must not adopt the checkout its cwd happens to sit
    /// in when deciding whether this is a global session.
    fn allocate_global_state_path(
        &self,
        id: &str,
        now: DateTime<Utc>,
        name: Option<&str>,
    ) -> Option<String> {
        if crate::domain::services::workspace_root_from_config_explicit(self.state.config())
            .is_some()
        {
            return None;
        }
        let dir = global_session_dir_path(
            &self.artifacts_root(),
            now.with_timezone(&Local),
            name.unwrap_or_default(),
            id,
        );
        match std::fs::create_dir_all(&dir) {
            Ok(()) => Some(dir.to_string_lossy().into_owned()),
            Err(error) => {
                tracing::warn!(
                    session_dir = %dir.display(),
                    %error,
                    "failed to create global session artifacts dir; session stays rootless"
                );
                None
            }
        }
    }
}

/// Build the session artifacts dir path:
/// `{root}/{YYYY}-{MM}-{DD}/{hh}-{mm}-{sanitize(name)}-{session id prefix 8}`.
/// The session-id suffix keeps two sessions created in the same minute with
/// the same title on distinct directories.
fn global_session_dir_path(
    root: &Path,
    at: DateTime<Local>,
    name: &str,
    session_id: &str,
) -> PathBuf {
    let id_prefix: String = session_id.chars().take(8).collect();
    root.join(at.format("%Y-%m-%d").to_string()).join(format!(
        "{}-{}-{}",
        at.format("%H-%M"),
        sanitize_session_dir_component(name),
        id_prefix
    ))
}

/// Sanitize a session name into a single filesystem path component: strip the
/// characters illegal in Windows/Linux names (`/\:*?"<>|` plus control
/// characters), collapse whitespace runs into `-`, cap at
/// [`SESSION_NAME_MAX_CHARS`] characters, and fall back to
/// [`UNTITLED_SESSION_NAME`] when nothing survives.
fn sanitize_session_dir_component(name: &str) -> String {
    let mut sanitized = String::with_capacity(name.len());
    let mut pending_dash = false;
    for ch in name.chars() {
        // Whitespace check FIRST: tab/newline are control characters too, and
        // the whitespace rule (collapse to `-`) is the more specific contract.
        if ch.is_whitespace() {
            pending_dash = true;
            continue;
        }
        if ch.is_control() || "\\/:*?\"<>|".contains(ch) {
            continue;
        }
        if pending_dash && !sanitized.is_empty() {
            sanitized.push('-');
        }
        pending_dash = false;
        sanitized.push(ch);
    }
    let sanitized: String = sanitized.chars().take(SESSION_NAME_MAX_CHARS).collect();
    // Windows dirnames cannot END in a dot or space; truncation may also land
    // on a trailing dash, which is merely ugly.
    let sanitized = sanitized.trim_end_matches(['.', ' ', '-']);
    if sanitized.is_empty() { UNTITLED_SESSION_NAME.to_owned() } else { sanitized.to_owned() }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use chrono::{Local, TimeZone, Utc};

    use super::*;
    use crate::test_support::TestAppCore;

    #[test]
    fn sanitize_strips_illegal_characters_and_collapses_whitespace() {
        assert_eq!(sanitize_session_dir_component(r#"a/b\c:d*e?f"g<h>i|j"#), "abcdefghij");
        assert_eq!(
            sanitize_session_dir_component("  multiple   spaces\tand\ntabs "),
            "multiple-spaces-and-tabs"
        );
        // Control characters vanish (not even a dash).
        assert_eq!(sanitize_session_dir_component("a\u{0}\u{1f}b"), "ab");
    }

    #[test]
    fn sanitize_truncates_to_fifty_chars_and_trims_windows_illegal_tail() {
        let long = "x".repeat(80);
        assert_eq!(sanitize_session_dir_component(&long).chars().count(), 50);

        // Truncation landing on a dot/space tail must not produce a Windows
        // un-creatable dirname.
        let tail = format!("{}.", "y".repeat(60));
        let sanitized = sanitize_session_dir_component(&tail);
        assert!(!sanitized.ends_with('.'), "{sanitized}");
        assert!(sanitized.chars().count() <= 50);
        // CJK truncation stays on char boundaries.
        let cjk = "会话".repeat(40);
        assert_eq!(sanitize_session_dir_component(&cjk).chars().count(), 50);
    }

    #[test]
    fn sanitize_falls_back_to_untitled_for_empty_or_fully_illegal_names() {
        assert_eq!(sanitize_session_dir_component(""), UNTITLED_SESSION_NAME);
        assert_eq!(sanitize_session_dir_component("   "), UNTITLED_SESSION_NAME);
        assert_eq!(sanitize_session_dir_component(r#"/*?:"<>|"#), UNTITLED_SESSION_NAME);
    }

    #[test]
    fn session_dir_path_shape_and_same_minute_collision_freedom() {
        let root = Path::new(r"C:\Users\example\Documents\slab");
        let at = Local.with_ymd_and_hms(2026, 9, 17, 10, 30, 0).unwrap();
        let first = global_session_dir_path(root, at, "My Chat: 2026", "a1b2c3d4-0000");
        let second = global_session_dir_path(root, at, "My Chat: 2026", "ffff0000-0000");

        let expected_first = root.join("2026-09-17").join("10-30-My-Chat-2026-a1b2c3d4");
        assert_eq!(first, expected_first);
        // Same minute, same title, different session ids → distinct dirs.
        assert_ne!(first, second);
    }

    /// Lifecycle: a global session gets `state_path` filled at creation (dir
    /// exists on disk), a rename does NOT migrate the dir (only backfills an
    /// empty one), and deletion keeps the dir (rollouts/traces semantics).
    #[tokio::test]
    async fn global_session_state_path_lifecycle() {
        let app = TestAppCore::new().await;
        let documents = tempfile::tempdir().expect("documents tempdir");
        let artifacts_root = documents.path().join("slab");
        let service = super::SessionService::new_with_artifacts_root(
            app.model_state.clone(),
            artifacts_root.clone(),
        );

        let created = service
            .create_session(crate::domain::models::CreateSessionCommand {
                name: Some("Artifact Chat".to_owned()),
            })
            .await
            .expect("create session");
        let recorded = created.state_path.expect("global session records a state_path");
        let dir = Path::new(&recorded);
        assert!(
            dir.starts_with(&artifacts_root),
            "recorded dir {recorded} must sit under the artifacts root"
        );
        assert!(dir.is_dir(), "artifacts dir exists on disk");

        // Rename keeps the recorded dir (no migration).
        let renamed = service
            .update_session_name(&created.id, "Different Title".to_owned())
            .await
            .expect("rename session");
        assert_eq!(renamed.state_path.as_deref(), Some(recorded.as_str()));

        // A session without state_path (legacy / auto-created) is backfilled
        // on rename — and the backfill is stable across further renames.
        let seeded = ChatSession {
            id: "legacy-session-0001".to_owned(),
            name: "Legacy".to_owned(),
            state_path: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        app.store.create_session(seeded).await.expect("seed legacy session");
        let backfilled = service
            .update_session_name("legacy-session-0001", "Renamed Legacy".to_owned())
            .await
            .expect("rename legacy session");
        let backfilled_path = backfilled.state_path.expect("rename backfills empty state_path");
        assert!(Path::new(&backfilled_path).starts_with(&artifacts_root));
        assert!(Path::new(&backfilled_path).is_dir(), "backfilled dir exists on disk");
        let renamed_again = service
            .update_session_name("legacy-session-0001", "Renamed Again".to_owned())
            .await
            .expect("rename again");
        assert_eq!(
            renamed_again.state_path.as_deref(),
            Some(backfilled_path.as_str()),
            "subsequent renames must not migrate the recorded dir"
        );

        // The harness lookup resolves the recorded dir while the row exists.
        assert_eq!(service.global_session_dir(&created.id).await, Some(PathBuf::from(&recorded)));

        // Deletion drops the row but keeps the dir.
        service.delete_session("legacy-session-0001").await.expect("delete session");
        assert_eq!(service.global_session_dir("legacy-session-0001").await, None);
        assert!(Path::new(&backfilled_path).is_dir(), "artifacts dir survives deletion");
    }

    /// The workspace gate: with an explicitly configured workspace root no
    /// dir is allocated (workspace sessions carry no artifacts dir).
    #[tokio::test]
    async fn workspace_session_gets_no_state_path() {
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let app = TestAppCore::new_with_workspace_root(Some(workspace.path().to_path_buf())).await;
        let documents = tempfile::tempdir().expect("documents tempdir");
        let service = super::SessionService::new_with_artifacts_root(
            app.model_state.clone(),
            documents.path().join("slab"),
        );

        let created = service
            .create_session(crate::domain::models::CreateSessionCommand {
                name: Some("Workspace Chat".to_owned()),
            })
            .await
            .expect("create session");

        assert_eq!(created.state_path, None, "workspace sessions allocate no artifacts dir");
        assert!(
            !documents.path().join("slab").exists(),
            "no artifacts tree is created for workspace sessions"
        );
    }
}
