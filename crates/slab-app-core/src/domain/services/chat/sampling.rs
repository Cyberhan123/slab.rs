//! Effort-aware sampling resolution.
//!
//! Merges, per field: **request > model effort-preset > built-in effort
//! preset**. This is the sampling half of thinking-strength (the prompt half
//! lives in `slab-agent-context`'s `ReasoningEffortFragment`); "high effort"
//! pairs with convergent sampling (low temperature, high top-p, small min-p
//! floor) so stronger thinking goes with more deterministic decoding.
//!
//! The model effort-preset comes from the pack's `runtime_presets` (flat
//! default + optional `efforts` overrides), finally applied to the call here —
//! it was parsed and stored but ignored before this module existed.

use slab_types::ChatReasoningEffort;

use crate::domain::models::{CommonChatParams, RuntimePresets};

/// Fallback max-tokens when nothing else supplies one. Identical to the legacy
/// hardcoded fallback so behavior is unchanged for models without presets and
/// requests without overrides.
pub(super) const DEFAULT_COMPLETION_MAX_TOKENS: u32 = 512;

/// Built-in sampling preset for an effort level, used when neither the request
/// nor the model's runtime preset supplies a given field. "High effort" biases
/// toward convergent sampling.
pub(super) fn built_in_for_effort(effort: Option<ChatReasoningEffort>) -> RuntimePresets {
    match effort {
        Some(ChatReasoningEffort::High) => RuntimePresets::new(
            Some(4096u32),
            Some(0.3f32),
            Some(0.98f32),
            Some(40i32),
            Some(0.05f32),
            None,
            None,
        ),
        Some(ChatReasoningEffort::Medium) => {
            RuntimePresets::new(Some(2048u32), Some(0.6f32), Some(0.95f32), None, None, None, None)
        }
        Some(ChatReasoningEffort::Low) | Some(ChatReasoningEffort::Minimal) => {
            RuntimePresets::new(Some(1024u32), Some(0.5f32), Some(0.9f32), None, None, None, None)
        }
        Some(ChatReasoningEffort::None) | None => RuntimePresets::new(
            Some(DEFAULT_COMPLETION_MAX_TOKENS),
            Some(0.7f32),
            None,
            None,
            None,
            None,
            None,
        ),
    }
}

/// Resolved sampling values for one chat call.
#[derive(Debug, Clone, Copy)]
pub(super) struct ResolvedSampling {
    pub max_tokens: u32,
    /// The max-tokens value only when it was *deliberately* chosen — supplied
    /// by the request or authored in the model's runtime presets, never filled
    /// in by the built-in effort presets or the legacy fallback. Cloud callers
    /// send this on the wire instead of `max_tokens`: implicit small caps
    /// truncate cloud reasoning models mid-thought (reasoning tokens count
    /// against `max_tokens`) and surface as "empty assistant content".
    pub explicit_max_tokens: Option<u32>,
    pub temperature: f32,
    pub top_p: Option<f32>,
    pub top_k: Option<i32>,
    pub min_p: Option<f32>,
    pub presence_penalty: Option<f32>,
    pub repetition_penalty: Option<f32>,
}

/// Resolve sampling for a chat call. Precedence per field:
/// request (`common`) > model effort-preset > built-in effort preset.
pub(super) fn resolve_sampling(
    common: &CommonChatParams,
    effort: Option<ChatReasoningEffort>,
    model_presets: Option<&RuntimePresets>,
) -> ResolvedSampling {
    let effort_preset =
        model_presets.map(|presets| presets.resolve_for_effort(effort)).unwrap_or_default();
    let built_in = built_in_for_effort(effort);
    let explicit_max_tokens = common.max_tokens.or(effort_preset.max_tokens);
    ResolvedSampling {
        max_tokens: explicit_max_tokens
            .or(built_in.max_tokens)
            .unwrap_or(DEFAULT_COMPLETION_MAX_TOKENS),
        explicit_max_tokens,
        temperature: common
            .temperature
            .or(effort_preset.temperature)
            .or(built_in.temperature)
            .unwrap_or(0.7),
        top_p: common.top_p.or(effort_preset.top_p).or(built_in.top_p),
        top_k: common.top_k.or(effort_preset.top_k).or(built_in.top_k),
        min_p: common.min_p.or(effort_preset.min_p).or(built_in.min_p),
        presence_penalty: common
            .presence_penalty
            .or(effort_preset.presence_penalty)
            .or(built_in.presence_penalty),
        repetition_penalty: common
            .repetition_penalty
            .or(effort_preset.repetition_penalty)
            .or(built_in.repetition_penalty),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn common(temperature: Option<f32>, top_p: Option<f32>) -> CommonChatParams {
        CommonChatParams {
            max_tokens: None,
            temperature,
            top_p,
            top_k: None,
            min_p: None,
            presence_penalty: None,
            repetition_penalty: None,
            n: 1,
            stream: false,
            stop: Vec::new(),
            stream_options: crate::domain::models::ChatStreamOptions::default(),
        }
    }

    #[test]
    fn no_preset_no_effort_keeps_legacy_defaults() {
        let resolved = resolve_sampling(&common(None, None), None, None);
        assert_eq!(resolved.max_tokens, 512);
        assert_eq!(resolved.explicit_max_tokens, None);
        assert!((resolved.temperature - 0.7).abs() < f32::EPSILON);
    }

    #[test]
    fn built_in_effort_caps_are_not_explicit() {
        let resolved = resolve_sampling(&common(None, None), Some(ChatReasoningEffort::High), None);
        assert_eq!(resolved.max_tokens, 4096);
        assert_eq!(resolved.explicit_max_tokens, None);
    }

    #[test]
    fn request_and_model_preset_caps_are_explicit() {
        let mut params = common(None, None);
        params.max_tokens = Some(1200);
        assert_eq!(resolve_sampling(&params, None, None).explicit_max_tokens, Some(1200));

        let model = RuntimePresets::new(Some(2048), None, None, None, None, None, None);
        assert_eq!(
            resolve_sampling(&common(None, None), None, Some(&model)).explicit_max_tokens,
            Some(2048)
        );
    }

    #[test]
    fn request_overrides_model_preset() {
        let model = RuntimePresets::new(None, Some(0.6), Some(0.95), None, None, None, None);
        let resolved = resolve_sampling(&common(Some(0.2), None), None, Some(&model));
        assert!((resolved.temperature - 0.2).abs() < f32::EPSILON);
        assert!((resolved.top_p.unwrap() - 0.95).abs() < f32::EPSILON);
    }

    #[test]
    fn high_effort_without_preset_uses_convergent_built_in() {
        let resolved = resolve_sampling(&common(None, None), Some(ChatReasoningEffort::High), None);
        assert!((resolved.temperature - 0.3).abs() < f32::EPSILON);
        assert!((resolved.top_p.unwrap() - 0.98).abs() < f32::EPSILON);
        assert_eq!(resolved.top_k, Some(40));
    }

    #[test]
    fn effort_override_wins_over_model_default() {
        // Model ships a flat default temp 0.6 plus a `high` override temp 0.3.
        let mut model = RuntimePresets::new(None, Some(0.6), Some(0.95), None, None, None, None);
        model.efforts.insert(
            "high".to_owned(),
            RuntimePresets::new(None, Some(0.3), None, None, None, None, None),
        );
        let resolved =
            resolve_sampling(&common(None, None), Some(ChatReasoningEffort::High), Some(&model));
        // high override temperature (0.3) wins over the flat default (0.6).
        assert!((resolved.temperature - 0.3).abs() < f32::EPSILON);
        // top_p not set on the override, so the flat default (0.95) fills in.
        assert!((resolved.top_p.unwrap() - 0.95).abs() < f32::EPSILON);
    }
}
