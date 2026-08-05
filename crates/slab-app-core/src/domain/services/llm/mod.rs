//! Unified LLM service: cloud/local routing + low-level call wrappers + token estimation.
//!
//! `chat` and `agent`/`response` share this module to avoid duplicating genai (cloud) and
//! slab-llama (local) calls, HTTP diagnostics, and token estimation across two layers.
//! Currently provides:
//! - routing decision [`should_route_to_cloud`];
//! - cloud catalog resolution and genai invocation ([`cloud`]);
//! - shared token estimation [`build_estimated_usage`] / [`finish_reason_from_token_budget`].
//!
//! The local backend is intended to migrate from `domain::services::chat::local` into [`local`]
//! (not yet migrated).

pub(crate) mod cloud;
pub(crate) mod local;

use crate::context::ModelState;
use crate::domain::models::{TextGenerationUsage, UnifiedModel};
use crate::error::AppCoreError;
use crate::infra::db::ModelStore;

/// Explicit cloud model id prefix (`cloud/<provider>/<model>`).
const CLOUD_MODEL_ID_PREFIX: &str = "cloud";

/// Whether the requested model id should route to the cloud: an explicit `cloud/...` id
/// or a cloud catalog model. Local models return `false`.
pub(crate) async fn should_route_to_cloud(
    state: &ModelState,
    requested_model: &str,
) -> Result<bool, AppCoreError> {
    if is_cloud_model_option_id(requested_model) {
        return Ok(true);
    }

    let Some(record) = state.store().get_model(requested_model).await? else {
        return Ok(false);
    };
    let model: UnifiedModel =
        record.try_into().map_err(|error: String| AppCoreError::Internal(error))?;
    Ok(cloud::is_cloud_catalog_model(&model))
}

fn is_cloud_model_option_id(model_id: &str) -> bool {
    model_id.strip_prefix(CLOUD_MODEL_ID_PREFIX).is_some_and(|suffix| suffix.starts_with('/'))
}

/// Rough token-count estimate for a piece of text (upper bound of byte / whitespace
/// groups). Cloud providers usually do not return precise usage, so it is estimated
/// client-side uniformly.
fn estimate_token_count(text: &str) -> u32 {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return 0;
    }

    let bytes = trimmed.len() as u32;
    let whitespace_groups = trimmed.split_whitespace().count() as u32;
    let byte_estimate = bytes.div_ceil(4);
    byte_estimate.max(whitespace_groups).max(1)
}

/// Infer the finish_reason from produced tokens vs. the token budget.
pub(crate) fn finish_reason_from_token_budget(completion_tokens: u32, max_tokens: u32) -> String {
    if completion_tokens >= max_tokens && max_tokens > 0 {
        "length".to_owned()
    } else {
        "stop".to_owned()
    }
}

/// Build an estimated usage. When `completion_tokens` is `None`, it is estimated from
/// the completion text.
pub(crate) fn build_estimated_usage(
    prompt_text: &str,
    completion_text: &str,
    completion_tokens: Option<u32>,
) -> TextGenerationUsage {
    let prompt_tokens = estimate_token_count(prompt_text);
    let completion_tokens =
        completion_tokens.unwrap_or_else(|| estimate_token_count(completion_text));

    TextGenerationUsage {
        prompt_tokens,
        completion_tokens,
        total_tokens: prompt_tokens.saturating_add(completion_tokens),
        prompt_tokens_details: Default::default(),
        estimated: true,
    }
}
