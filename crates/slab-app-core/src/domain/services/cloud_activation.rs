//! Activates cloud catalog models when the `providers.registry` setting changes.
//!
//! After settings apply, for each configured provider we upsert one `UnifiedModel` per entry in
//! the provider's curated default catalog (sourced from `slab-cloud-provider`), so configuring a
//! provider immediately makes its models usable. Cloud models auto-activated for a provider that
//! has since been removed are cleaned up. All operations are best-effort: a failure for one model
//! is logged and does not abort the settings save.

use std::collections::{BTreeMap, BTreeSet};

use tracing::{info, warn};

use crate::context::ModelState;
use crate::domain::models::{UnifiedModel, UnifiedModelKind};
use crate::infra::db::ModelStore;

use super::model::ModelService;

/// Prefix + separator scheme for auto-activated cloud model ids: `cloud:<provider_id>:<remote_model_id>`.
/// The colon separator avoids colliding with the legacy `cloud/<provider>/<model>` option-id form.
const AUTO_CLOUD_ID_PREFIX: &str = "cloud:";

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

/// Ensure the cloud model catalog reflects the currently configured providers.
///
/// Reads `chat.providers` from the settings snapshot and upserts each provider's default catalog
/// models, then deletes auto-activated cloud models whose provider no longer exists. Idempotent;
/// errors are logged and do not propagate (the settings save must not fail because of activation).
pub async fn sync_provider_models(model: &ModelService, state: &ModelState) {
    let providers = state.pmid().config().chat.providers;
    let active_provider_ids: BTreeSet<String> = providers.iter().map(|p| p.id.clone()).collect();

    let mut activated = 0usize;
    for provider in &providers {
        let specs = slab_cloud_provider::default_models_for_provider(provider);
        for spec in specs {
            let id = auto_cloud_model_id(&provider.id, &spec.remote_model_id);
            if let Err(error) = model
                .upsert_cloud_model(&id, &spec.display_name, &provider.id, &spec.remote_model_id)
                .await
            {
                warn!(
                    provider_id = %provider.id,
                    model_id = %id,
                    error = %error,
                    "failed to activate cloud catalog model; skipping",
                );
            } else {
                activated += 1;
            }
        }
    }

    let removed = cleanup_orphaned_cloud_models(state, &active_provider_ids).await;

    if activated > 0 || removed > 0 {
        info!(activated, removed, "synced cloud provider model catalog");
    }
}

/// Delete auto-activated cloud models whose provider is no longer configured.
async fn cleanup_orphaned_cloud_models(
    state: &ModelState,
    active_provider_ids: &BTreeSet<String>,
) -> usize {
    let records = match state.store().list_models().await {
        Ok(records) => records,
        Err(error) => {
            warn!(error = %error, "failed to list models for cloud catalog cleanup");
            return 0;
        }
    };

    // Group orphaned ids by their provider so logs are actionable.
    let mut orphaned_by_provider: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for record in records {
        let model: UnifiedModel = match record.try_into() {
            Ok(model) => model,
            Err(_) => continue,
        };
        let UnifiedModel { id, kind, .. } = model;
        if kind != UnifiedModelKind::Cloud {
            continue;
        }
        let Some((provider_id, _)) = parse_auto_cloud_model_id(&id) else {
            continue; // not an auto-activated model; leave user-managed models alone
        };
        if !active_provider_ids.contains(&provider_id) {
            orphaned_by_provider.entry(provider_id).or_default().push(id);
        }
    }

    let mut removed = 0usize;
    for (provider_id, ids) in orphaned_by_provider {
        for id in ids {
            match state.store().delete_model(&id).await {
                Ok(()) => removed += 1,
                Err(error) => {
                    warn!(model_id = %id, error = %error, "failed to delete orphaned cloud model")
                }
            }
        }
        info!(provider_id = %provider_id, "removed cloud catalog models for deleted provider");
    }
    removed
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
}
