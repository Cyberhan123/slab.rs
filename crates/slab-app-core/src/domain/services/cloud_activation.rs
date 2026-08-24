//! Keeps the cloud portion of the `models` catalog in sync with configured providers.
//!
//! Two sources drive the desired state:
//! - **Curated catalogs** (`slab-cloud-provider` static data) — reconciled inline on every
//!   trigger: missing or drifted auto rows are upserted, rows for removed providers (or pruned
//!   catalog entries) are deleted. Synchronized state costs zero DB writes.
//! - **Live `/models` discovery** for `OpenaiCompatible` providers — a bounded web refresh.
//!   Success upserts the advertised ids and prunes auto rows the endpoint no longer lists;
//!   failure keeps the previously discovered rows (a dead endpoint must not clear the catalog).
//!
//! Triggers: settings PUT on `providers.registry` (curated inline + live inline), service
//! bootstrap (curated inline + live spawned), and `ModelService` list reads (curated inline +
//! TTL-gated live spawned). All operations are best-effort: a failure for one model is logged
//! and does not abort the caller.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures::future::{BoxFuture, join_all};
use slab_config::CloudProviderConfig;
use tracing::{info, warn};

use crate::context::ModelState;
use crate::domain::models::{UnifiedModel, UnifiedModelKind};
use crate::infra::db::ModelStore;

use super::model::ModelService;

/// Prefix + separator scheme for auto-activated cloud model ids: `cloud:<provider_id>:<remote_model_id>`.
/// The colon separator avoids colliding with the legacy `cloud/<provider>/<model>` option-id form.
const AUTO_CLOUD_ID_PREFIX: &str = "cloud:";

/// How long a successful live discovery stays fresh before reads may trigger another refresh.
const LIVE_REFRESH_SUCCESS_TTL: Duration = Duration::from_secs(5 * 60);
/// Backoff after a failed live discovery so dead endpoints are not probed on every read.
const LIVE_REFRESH_FAILURE_BACKOFF: Duration = Duration::from_secs(60);
/// Bound for a single live discovery call (endpoint down must not stall the caller).
const LIVE_REFRESH_TIMEOUT: Duration = Duration::from_secs(8);

// ============ id scheme ============

/// Build the deterministic catalog id for a provider's remote model.
fn auto_cloud_model_id(provider_id: &str, remote_model_id: &str) -> String {
    format!("{AUTO_CLOUD_ID_PREFIX}{provider_id}:{remote_model_id}")
}

/// Parse an auto-activated cloud model id back into `(provider_id, remote_model_id)`.
/// Returns `None` for ids that do not match the `cloud:<provider>:<remote>` scheme (e.g. user-
/// created models with arbitrary ids), so cleanup never touches user-managed models.
fn parse_auto_cloud_model_id(id: &str) -> Option<(String, String)> {
    let rest = id.strip_prefix(AUTO_CLOUD_ID_PREFIX)?;
    let (provider_id, remote_model_id) = rest.split_once(':')?;
    if provider_id.is_empty() || remote_model_id.is_empty() {
        return None;
    }
    Some((provider_id.to_owned(), remote_model_id.to_owned()))
}

// ============ live-discovery seam & refresh bookkeeping ============

/// Live discovery seam. Production delegates to genai via `slab-cloud-provider`; tests inject
/// canned responses without touching the network.
pub(crate) trait RemoteModelLister: Send + Sync {
    fn list<'a>(
        &'a self,
        provider: &'a CloudProviderConfig,
    ) -> BoxFuture<'a, Result<Vec<String>, slab_cloud_provider::CloudError>>;
}

/// Default [`RemoteModelLister`]: genai `all_model_names` against the provider's api_base.
pub(crate) struct GenaiRemoteModelLister;

impl RemoteModelLister for GenaiRemoteModelLister {
    fn list<'a>(
        &'a self,
        provider: &'a CloudProviderConfig,
    ) -> BoxFuture<'a, Result<Vec<String>, slab_cloud_provider::CloudError>> {
        Box::pin(slab_cloud_provider::list_remote_model_ids(provider))
    }
}

/// Refresh bookkeeping shared between the read path and background refresh tasks.
#[derive(Default)]
pub(crate) struct CloudCatalogRefreshState {
    /// provider_id -> (last completed refresh, whether it succeeded).
    last_refresh: HashMap<String, (Instant, bool)>,
    /// provider ids with a refresh task currently running.
    in_flight: BTreeSet<String>,
}

/// Cloud-catalog refresh context carried by [`ModelService`].
pub(crate) struct CloudCatalogContext {
    pub(crate) lister: Arc<dyn RemoteModelLister>,
    pub(crate) refresh: Mutex<CloudCatalogRefreshState>,
}

impl Default for CloudCatalogContext {
    fn default() -> Self {
        Self {
            lister: Arc::new(GenaiRemoteModelLister),
            refresh: Mutex::new(CloudCatalogRefreshState::default()),
        }
    }
}

/// Atomically claim a provider refresh. `respect_ttl` additionally requires the last refresh to
/// be stale (success TTL or failure backoff). Returns false when a refresh is in flight.
fn claim_refresh(
    state: &Mutex<CloudCatalogRefreshState>,
    provider_id: &str,
    respect_ttl: bool,
) -> bool {
    let mut guard = state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if guard.in_flight.contains(provider_id) {
        return false;
    }
    if respect_ttl && let Some((at, succeeded)) = guard.last_refresh.get(provider_id) {
        let ttl = if *succeeded { LIVE_REFRESH_SUCCESS_TTL } else { LIVE_REFRESH_FAILURE_BACKOFF };
        if at.elapsed() < ttl {
            return false;
        }
    }
    guard.in_flight.insert(provider_id.to_owned());
    true
}

/// Record the outcome of a claimed refresh and release the claim.
fn complete_refresh(state: &Mutex<CloudCatalogRefreshState>, provider_id: &str, succeeded: bool) {
    let mut guard = state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.in_flight.remove(provider_id);
    guard.last_refresh.insert(provider_id.to_owned(), (Instant::now(), succeeded));
}

// ============ entry points ============

/// Settings-PUT semantics: reconcile curated catalogs inline, then refresh live providers
/// inline (concurrently). TTL is ignored — an explicit save is an explicit refresh intent.
pub async fn sync_provider_models(model: &ModelService, state: &ModelState) {
    let providers = state.pmid().config().chat.providers;
    let (activated, removed) = sync_curated_catalogs(model, state, &providers).await;

    let live_providers: Vec<CloudProviderConfig> = providers
        .iter()
        .filter(|provider| slab_cloud_provider::supports_live_discovery(provider.family))
        .cloned()
        .collect();
    let mut discovered = 0usize;
    if !live_providers.is_empty() {
        let refreshes = live_providers.iter().map(|provider| {
            let catalog = model.cloud_catalog();
            async move {
                if !claim_refresh(&catalog.refresh, &provider.id, /* respect_ttl */ false) {
                    return false;
                }
                refresh_claimed_provider(model, state, &catalog, provider).await
            }
        });
        for succeeded in join_all(refreshes).await {
            discovered += usize::from(succeeded);
        }
    }

    if activated > 0 || removed > 0 || discovered > 0 {
        info!(activated, removed, discovered, "synced cloud provider model catalog");
    }
}

/// Bootstrap semantics (server startup): curated reconcile inline; live refreshes spawn in the
/// background so a dead endpoint cannot delay startup.
pub async fn bootstrap_cloud_catalogs(model: &ModelService, state: &ModelState) {
    let providers = state.pmid().config().chat.providers;
    sync_curated_catalogs(model, state, &providers).await;
    for provider in providers {
        if slab_cloud_provider::supports_live_discovery(provider.family) {
            spawn_live_refresh(model.clone(), state.clone(), provider);
        }
    }
}

/// Read-path self-heal: curated reconcile inline (cheap diff over one SELECT), then spawn a
/// TTL-gated background refresh per live provider. Never blocks the read on the network.
pub async fn reconcile_catalogs_for_read(model: &ModelService, state: &ModelState) {
    let providers = state.pmid().config().chat.providers;
    sync_curated_catalogs(model, state, &providers).await;
    for provider in providers {
        if slab_cloud_provider::supports_live_discovery(provider.family) {
            spawn_live_refresh(model.clone(), state.clone(), provider);
        }
    }
}

fn spawn_live_refresh(model: ModelService, state: ModelState, provider: CloudProviderConfig) {
    let catalog = model.cloud_catalog();
    tokio::spawn(async move {
        if !claim_refresh(&catalog.refresh, &provider.id, /* respect_ttl */ true) {
            return;
        }
        refresh_claimed_provider(&model, &state, &catalog, &provider).await;
    });
}

/// Run one live discovery for an already-claimed provider and apply the result.
/// Success replaces the provider's auto rows with the advertised ids; failure keeps them.
async fn refresh_claimed_provider(
    model: &ModelService,
    state: &ModelState,
    catalog: &CloudCatalogContext,
    provider: &CloudProviderConfig,
) -> bool {
    let outcome =
        match tokio::time::timeout(LIVE_REFRESH_TIMEOUT, catalog.lister.list(provider)).await {
            Ok(Ok(remote_ids)) => {
                apply_live_discovery(model, state, provider, &remote_ids).await;
                info!(
                    provider_id = %provider.id,
                    count = remote_ids.len(),
                    "live model discovery refreshed cloud catalog"
                );
                true
            }
            Ok(Err(error)) => {
                warn!(
                    provider_id = %provider.id,
                    error = %error,
                    "live model discovery failed; keeping known models"
                );
                false
            }
            Err(_) => {
                warn!(
                    provider_id = %provider.id,
                    timeout_secs = LIVE_REFRESH_TIMEOUT.as_secs(),
                    "live model discovery timed out; keeping known models"
                );
                false
            }
        };
    complete_refresh(&catalog.refresh, &provider.id, outcome);
    outcome
}

// ============ reconcile core ============

/// One desired auto-activated cloud row derived from a curated catalog entry.
struct DesiredCloudRow {
    id: String,
    display_name: String,
    provider_id: String,
    remote_model_id: String,
}

/// Reconcile curated catalog rows against the store: upsert missing/drifted rows, delete auto
/// rows whose provider was removed or whose catalog entry was pruned. Returns (upserted, deleted).
async fn sync_curated_catalogs(
    model: &ModelService,
    state: &ModelState,
    providers: &[CloudProviderConfig],
) -> (usize, usize) {
    let records = match state.store().list_models().await {
        Ok(records) => records,
        Err(error) => {
            warn!(error = %error, "failed to list models for cloud catalog reconcile");
            return (0, 0);
        }
    };
    let existing: Vec<UnifiedModel> =
        records.into_iter().filter_map(|record| record.try_into().ok()).collect();

    let mut desired: BTreeMap<String, DesiredCloudRow> = BTreeMap::new();
    let mut curated_provider_ids: BTreeSet<String> = BTreeSet::new();
    for provider in providers {
        let specs = slab_cloud_provider::default_models_for_provider(provider);
        if specs.is_empty() {
            continue;
        }
        curated_provider_ids.insert(provider.id.clone());
        for spec in specs {
            let id = auto_cloud_model_id(&provider.id, &spec.remote_model_id);
            desired.insert(
                id.clone(),
                DesiredCloudRow {
                    id,
                    display_name: spec.display_name,
                    provider_id: provider.id.clone(),
                    remote_model_id: spec.remote_model_id,
                },
            );
        }
    }

    let mut upserted = 0usize;
    for row in desired.values() {
        let drifted = existing.iter().find(|m| m.id == row.id).is_none_or(|m| {
            m.kind != UnifiedModelKind::Cloud
                || m.display_name != row.display_name
                || m.spec.provider_id.as_deref() != Some(row.provider_id.as_str())
                || m.spec.remote_model_id.as_deref() != Some(row.remote_model_id.as_str())
        });
        if !drifted {
            continue;
        }
        match model
            .upsert_cloud_model(&row.id, &row.display_name, &row.provider_id, &row.remote_model_id)
            .await
        {
            Ok(()) => upserted += 1,
            Err(error) => warn!(
                provider_id = %row.provider_id,
                model_id = %row.id,
                error = %error,
                "failed to activate cloud catalog model; skipping",
            ),
        }
    }

    let active_provider_ids: BTreeSet<&str> = providers.iter().map(|p| p.id.as_str()).collect();
    let mut deleted = 0usize;
    for m in &existing {
        if m.kind != UnifiedModelKind::Cloud {
            continue;
        }
        let Some((provider_id, _)) = parse_auto_cloud_model_id(&m.id) else {
            continue; // not an auto-activated model; leave user-managed models alone
        };
        let orphaned = !active_provider_ids.contains(provider_id.as_str());
        // Only curated providers prune by desired set — live providers prune on successful
        // discovery only, so a failed probe never clears their catalog.
        let pruned = !orphaned
            && curated_provider_ids.contains(provider_id.as_str())
            && !desired.contains_key(&m.id);
        if !orphaned && !pruned {
            continue;
        }
        match state.store().delete_model(&m.id).await {
            Ok(()) => {
                deleted += 1;
                info!(model_id = %m.id, "removed auto-activated cloud catalog model");
            }
            Err(error) => {
                warn!(model_id = %m.id, error = %error, "failed to delete cloud catalog model")
            }
        }
    }

    (upserted, deleted)
}

/// Apply a successful live discovery: the provider's auto rows become exactly the advertised
/// remote ids (display name = remote id — discovered rows carry no friendly labels).
async fn apply_live_discovery(
    model: &ModelService,
    state: &ModelState,
    provider: &CloudProviderConfig,
    remote_ids: &[String],
) {
    let desired: BTreeMap<String, &String> = remote_ids
        .iter()
        .map(|remote| (auto_cloud_model_id(&provider.id, remote), remote))
        .collect();

    let records = match state.store().list_models().await {
        Ok(records) => records,
        Err(error) => {
            warn!(error = %error, "failed to list models for live discovery apply");
            return;
        }
    };
    let existing_auto: Vec<UnifiedModel> = records
        .into_iter()
        .filter_map(|record| record.try_into().ok())
        .filter(|m: &UnifiedModel| {
            m.kind == UnifiedModelKind::Cloud
                && parse_auto_cloud_model_id(&m.id).is_some_and(|(id, _)| id == provider.id)
        })
        .collect();

    let mut changed = 0usize;
    for (id, remote) in &desired {
        let drifted = existing_auto.iter().find(|m| &m.id == id).is_none_or(|m| {
            m.display_name != **remote || m.spec.remote_model_id.as_deref() != Some(remote.as_str())
        });
        if !drifted {
            continue;
        }
        match model.upsert_cloud_model(id, remote, &provider.id, remote).await {
            Ok(()) => changed += 1,
            Err(error) => warn!(
                provider_id = %provider.id,
                model_id = %id,
                error = %error,
                "failed to activate discovered cloud model; skipping",
            ),
        }
    }
    for m in &existing_auto {
        if desired.contains_key(&m.id) {
            continue;
        }
        match state.store().delete_model(&m.id).await {
            Ok(()) => changed += 1,
            Err(error) => {
                warn!(model_id = %m.id, error = %error, "failed to prune cloud catalog model")
            }
        }
    }

    if changed > 0 {
        info!(provider_id = %provider.id, changed, "applied live model discovery to cloud catalog");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_cloud_id_round_trips() {
        let id = auto_cloud_model_id("openai-main", "gpt-4o");
        assert_eq!(id, "cloud:openai-main:gpt-4o");
        assert_eq!(
            parse_auto_cloud_model_id(&id),
            Some(("openai-main".to_owned(), "gpt-4o".to_owned()))
        );
    }

    #[test]
    fn auto_cloud_id_handles_slashed_remote_models() {
        // OpenRouter/Together use namespaced remote ids with slashes, not colons.
        let id = auto_cloud_model_id("openrouter", "openai/gpt-4o");
        assert_eq!(
            parse_auto_cloud_model_id(&id),
            Some(("openrouter".to_owned(), "openai/gpt-4o".to_owned()))
        );
    }

    #[test]
    fn parse_rejects_non_auto_ids() {
        assert!(parse_auto_cloud_model_id("gpt-4o").is_none());
        // Legacy option-id form uses a slash, not the auto-activation colon scheme.
        assert!(parse_auto_cloud_model_id("cloud/openai-main/gpt-4o").is_none());
        assert!(parse_auto_cloud_model_id("cloud::gpt-4o").is_none());
        assert!(parse_auto_cloud_model_id("cloud:openai:").is_none());
    }

    #[test]
    fn claim_refresh_respects_ttl_and_dedups_in_flight() {
        let catalog = CloudCatalogContext::default();
        assert!(claim_refresh(&catalog.refresh, "p1", false));

        // In-flight claim blocks a second claim even when TTL is ignored.
        assert!(!claim_refresh(&catalog.refresh, "p1", false));

        complete_refresh(&catalog.refresh, "p1", true);
        // Fresh success blocks TTL-respecting claims but not explicit (PUT-style) ones.
        assert!(!claim_refresh(&catalog.refresh, "p1", true));
        assert!(claim_refresh(&catalog.refresh, "p1", false));
        complete_refresh(&catalog.refresh, "p1", false);
        // A fresh failure also blocks TTL-respecting claims.
        assert!(!claim_refresh(&catalog.refresh, "p1", true));
    }

    // ============ service-level reconcile tests (scripted lister — no network) ============

    use serde_json::{Value, json};
    use slab_config::{UpdateSettingCommand, UpdateSettingOperation};
    use slab_types::Capability;

    use crate::domain::models::ListModelsFilter;
    use crate::test_support::{TestAppCore, cloud_chat_model_command};

    /// Scripted discovery lister: `Some(ids)` succeeds with those ids, `None` fails.
    #[derive(Clone, Default)]
    struct FakeLister {
        ids: Arc<Mutex<Option<Vec<String>>>>,
    }

    impl FakeLister {
        fn succeed_with(ids: &[&str]) -> Self {
            Self {
                ids: Arc::new(Mutex::new(Some(ids.iter().map(|id| (*id).to_owned()).collect()))),
            }
        }

        fn fail(&self) {
            *self.ids.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
        }

        fn succeed(&self, ids: &[&str]) {
            *self.ids.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) =
                Some(ids.iter().map(|id| (*id).to_owned()).collect());
        }
    }

    impl RemoteModelLister for FakeLister {
        fn list<'a>(
            &'a self,
            _provider: &'a CloudProviderConfig,
        ) -> BoxFuture<'a, Result<Vec<String>, slab_cloud_provider::CloudError>> {
            let ids = self.ids.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone();
            Box::pin(async move {
                ids.ok_or_else(|| {
                    slab_cloud_provider::CloudError::BackendNotReady("scripted failure".to_owned())
                })
            })
        }
    }

    fn registry_entry(id: &str, family: &str, api_base: &str) -> Value {
        json!({
            "id": id,
            "family": family,
            "display_name": id,
            "api_base": api_base,
            "auth": {},
        })
    }

    /// Rewrite `providers.registry` through the pmid service (the storage half of the settings
    /// PUT) without invoking the cloud-activation hook.
    async fn set_registry_providers(app: &TestAppCore, entries: Vec<Value>) {
        let command = UpdateSettingCommand {
            op: UpdateSettingOperation::Set,
            value: Some(serde_json::from_value(Value::Array(entries)).expect("setting value")),
        };
        app.pmid
            .update_setting("providers.registry", command)
            .await
            .expect("update providers.registry");
    }

    async fn chat_model_ids(app: &TestAppCore) -> Vec<String> {
        app.model
            .list_models(ListModelsFilter { capability: Some(Capability::ChatGeneration) })
            .await
            .expect("list chat models")
            .into_iter()
            .map(|model| model.id)
            .collect()
    }

    #[tokio::test]
    async fn settings_sync_activates_curated_catalog_rows() {
        let app = TestAppCore::new().await;
        set_registry_providers(
            &app,
            vec![registry_entry(
                "glm-main",
                "big_model",
                "https://open.bigmodel.cn/api/coding/paas/v4",
            )],
        )
        .await;

        sync_provider_models(&app.model, &app.model_state).await;

        let ids = chat_model_ids(&app).await;
        for expected in [
            "cloud:glm-main:glm-4.6",
            "cloud:glm-main:glm-4.5",
            "cloud:glm-main:glm-4.5-air",
            "cloud:glm-main:glm-4-flash",
        ] {
            assert!(ids.iter().any(|id| id == expected), "missing {expected} in {ids:?}");
        }
    }

    #[tokio::test]
    async fn list_models_self_heals_curated_catalog_after_external_settings_change() {
        let app = TestAppCore::new().await;
        // Config refreshed through pmid only (as if settings.json were edited externally);
        // no settings PUT hook ran, so activation must come from the read path.
        set_registry_providers(
            &app,
            vec![registry_entry("zai-main", "zai", "https://api.z.ai/api/coding/v4")],
        )
        .await;

        let ids = chat_model_ids(&app).await;

        assert!(
            ids.iter().any(|id| id == "cloud:zai-main:glm-4.6"),
            "read-path reconcile should activate curated rows: {ids:?}"
        );
    }

    #[tokio::test]
    async fn removing_provider_cleans_auto_rows_but_keeps_user_models() {
        let app = TestAppCore::new().await;
        set_registry_providers(
            &app,
            vec![registry_entry("zai-main", "zai", "https://api.z.ai/api/coding/v4")],
        )
        .await;
        sync_provider_models(&app.model, &app.model_state).await;

        // User-created cloud model with a non-auto id must survive provider removal.
        app.model
            .create_model(cloud_chat_model_command("user-cloud", "zai-main"))
            .await
            .expect("create user cloud model");

        set_registry_providers(&app, vec![]).await;
        sync_provider_models(&app.model, &app.model_state).await;

        let ids = chat_model_ids(&app).await;
        assert!(
            !ids.iter().any(|id| id.starts_with("cloud:zai-main:")),
            "auto rows for removed provider cleaned: {ids:?}"
        );
        assert!(ids.iter().any(|id| id == "user-cloud"), "user model kept: {ids:?}");
    }

    #[tokio::test]
    async fn live_discovery_applies_prunes_and_keeps_rows_on_failure() {
        let lister = FakeLister::succeed_with(&["mock-a", "mock-b"]);
        let mut app = TestAppCore::new().await;
        app.model.override_cloud_catalog_for_tests(CloudCatalogContext {
            lister: Arc::new(lister.clone()),
            refresh: Mutex::new(CloudCatalogRefreshState::default()),
        });
        set_registry_providers(
            &app,
            vec![registry_entry("custom", "openai_compatible", "http://127.0.0.1:9/v1")],
        )
        .await;

        sync_provider_models(&app.model, &app.model_state).await;
        let ids = chat_model_ids(&app).await;
        assert!(ids.iter().any(|id| id == "cloud:custom:mock-a"), "ids: {ids:?}");
        assert!(ids.iter().any(|id| id == "cloud:custom:mock-b"), "ids: {ids:?}");

        // A failed discovery keeps the previously discovered rows.
        lister.fail();
        sync_provider_models(&app.model, &app.model_state).await;
        let ids = chat_model_ids(&app).await;
        assert!(
            ids.iter().any(|id| id == "cloud:custom:mock-b"),
            "failure must keep rows: {ids:?}"
        );

        // A successful discovery with a shrunk list prunes the disappeared model.
        lister.succeed(&["mock-a"]);
        sync_provider_models(&app.model, &app.model_state).await;
        let ids = chat_model_ids(&app).await;
        assert!(ids.iter().any(|id| id == "cloud:custom:mock-a"), "ids: {ids:?}");
        assert!(
            !ids.iter().any(|id| id == "cloud:custom:mock-b"),
            "pruned after successful discovery: {ids:?}"
        );
    }
}
