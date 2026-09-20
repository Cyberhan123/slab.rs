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

/// Fallback max-tokens when nothing else supplies one — the workspace-wide
/// contract constant in `slab-types` (also the runtime workers' fallback).
/// Re-exported here so the `chat::sampling` path keeps a single name.
pub(super) use slab_types::chat::DEFAULT_COMPLETION_MAX_TOKENS;

/// Visible-answer headroom added on top of the thinking budget when the effort
/// derives the generation cap (see [`resolve_sampling`]). The cap covers
/// thinking + answer, so the budget alone would leave no room to answer.
const ANSWER_ALLOWANCE_TOKENS: u32 = 2048;

/// Built-in sampling preset for an effort level, used when neither the request
/// nor the model's runtime preset supplies a given field. "High effort" biases
/// toward convergent sampling. Deliberately carries **no** max-tokens dial:
/// effort strength is expressed by the thinking budget (which derives its own
/// cap, budget + [`ANSWER_ALLOWANCE_TOKENS`]) — the old per-effort caps
/// self-clamped High's 16384 budget down to its own 4096 minus the answer
/// floor.
pub(super) fn built_in_for_effort(effort: Option<ChatReasoningEffort>) -> RuntimePresets {
    match effort {
        Some(ChatReasoningEffort::High) => RuntimePresets {
            temperature: Some(0.3f32),
            top_p: Some(0.98f32),
            top_k: Some(40i32),
            min_p: Some(0.05f32),
            ..Default::default()
        },
        Some(ChatReasoningEffort::Medium) => {
            RuntimePresets { temperature: Some(0.6f32), top_p: Some(0.95f32), ..Default::default() }
        }
        Some(ChatReasoningEffort::Low) | Some(ChatReasoningEffort::Minimal) => {
            RuntimePresets { temperature: Some(0.5f32), top_p: Some(0.9f32), ..Default::default() }
        }
        Some(ChatReasoningEffort::None) | None => {
            RuntimePresets { temperature: Some(0.7f32), ..Default::default() }
        }
    }
}

/// Built-in thinking-token budget per effort level, used when the model's
/// runtime presets don't override `efforts.<key>.thinking_budget` (or the flat
/// `thinking_budget`). Mirrors llama.cpp `--reasoning-budget` semantics: when
/// the `<think>` segment reaches this many generated tokens the decode loop
/// force-closes it by injecting the model's `</think>` tokens, so generation
/// continues with the visible answer. `None` (effort absent or the `None`
/// variant) means no enforcement — the `None` effort disables thinking via the
/// chat template's `enable_thinking=false` instead, and a budget there would
/// fight the pre-closed `<think></think>` the template emits.
pub(crate) fn built_in_thinking_budget_for_effort(
    effort: Option<ChatReasoningEffort>,
) -> Option<u32> {
    match effort {
        Some(ChatReasoningEffort::Minimal) => Some(512),
        Some(ChatReasoningEffort::Low) => Some(1024),
        Some(ChatReasoningEffort::Medium) => Some(4096),
        Some(ChatReasoningEffort::High) => Some(16384),
        Some(ChatReasoningEffort::None) | None => None,
    }
}

/// Clamp the thinking budget so a forced close always leaves room for a
/// visible answer: effective = min(raw, max_tokens - 256). When the ceiling
/// itself is tiny (< 64) enforcement is pointless — the injected close tag plus
/// a couple of answer tokens would immediately hit the length limit — so it is
/// skipped entirely.
pub(crate) fn clamp_thinking_budget(raw: Option<u32>, max_tokens: u32) -> Option<u32> {
    let raw = raw?;
    let ceiling = max_tokens.saturating_sub(256);
    (ceiling >= 64).then(|| raw.min(ceiling))
}

/// Resolved sampling values for one chat call.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ResolvedSampling {
    pub max_tokens: u32,
    /// The max-tokens value only when it was *deliberately* chosen — supplied
    /// by the request or authored in the model's runtime presets, never filled
    /// in by the built-in effort presets or the legacy fallback. Cloud callers
    /// send this on the wire instead of `max_tokens`: implicit small caps
    /// truncate cloud reasoning models mid-thought (reasoning tokens count
    /// against `max_tokens`) and surface as "empty assistant content".
    pub explicit_max_tokens: Option<u32>,
    /// Local-only thinking-token budget (model effort-preset > built-in table;
    /// never request-sourced — the wire API has no per-request budget field).
    /// Already clamped against the resolved `max_tokens`.
    pub thinking_budget: Option<u32>,
    pub temperature: f32,
    pub top_p: Option<f32>,
    pub top_k: Option<i32>,
    pub min_p: Option<f32>,
    pub presence_penalty: Option<f32>,
    pub repetition_penalty: Option<f32>,
}

/// Resolve sampling for a chat call. Precedence per field:
/// request (`common`) > model effort-preset > built-in effort preset.
/// `pub(crate)`: the chat completion paths and the `/responses` streaming path
/// both resolve here so their sampling semantics cannot drift.
pub(crate) fn resolve_sampling(
    common: &CommonChatParams,
    effort: Option<ChatReasoningEffort>,
    model_presets: Option<&RuntimePresets>,
) -> ResolvedSampling {
    let effort_preset =
        model_presets.map(|presets| presets.resolve_for_effort(effort)).unwrap_or_default();
    let built_in = built_in_for_effort(effort);
    let explicit_max_tokens = common.max_tokens.or(effort_preset.max_tokens);
    let raw_budget =
        effort_preset.thinking_budget.or_else(|| built_in_thinking_budget_for_effort(effort));
    // Effort owns the thinking budget; the generation cap follows from it
    // (budget + answer allowance) unless something explicit overrides. The
    // clamp below therefore never bites in the derived branch — it only
    // guards small *deliberate* caps.
    let thinking_on = matches!(effort, Some(e) if !matches!(e, ChatReasoningEffort::None));
    let max_tokens = match explicit_max_tokens {
        Some(explicit) => explicit,
        None if thinking_on && raw_budget.is_some() => {
            raw_budget.unwrap_or(0) + ANSWER_ALLOWANCE_TOKENS
        }
        None => DEFAULT_COMPLETION_MAX_TOKENS,
    };
    ResolvedSampling {
        max_tokens,
        explicit_max_tokens,
        // The clamp uses the FINAL resolved max-tokens (request caps included)
        // so a small deliberate cap can never leave a budget larger than the
        // answer floor.
        thinking_budget: clamp_thinking_budget(
            effort_preset.thinking_budget.or_else(|| built_in_thinking_budget_for_effort(effort)),
            max_tokens,
        ),
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
        assert_eq!(resolved.max_tokens, DEFAULT_COMPLETION_MAX_TOKENS);
        assert_eq!(resolved.explicit_max_tokens, None);
        assert!((resolved.temperature - 0.7).abs() < f32::EPSILON);
    }

    #[test]
    fn derived_effort_caps_are_not_explicit() {
        // Effort derives the cap from the budget (budget + answer allowance);
        // it is a fallback, never an explicit cap.
        let resolved = resolve_sampling(&common(None, None), Some(ChatReasoningEffort::High), None);
        assert_eq!(resolved.max_tokens, 16384 + 2048);
        assert_eq!(resolved.explicit_max_tokens, None);
    }

    #[test]
    fn effort_derives_cap_from_budget_plus_answer_allowance() {
        let budget_and_cap = |effort| {
            let resolved = resolve_sampling(&common(None, None), Some(effort), None);
            (resolved.thinking_budget, resolved.max_tokens)
        };
        assert_eq!(budget_and_cap(ChatReasoningEffort::Minimal), (Some(512), 512 + 2048));
        assert_eq!(budget_and_cap(ChatReasoningEffort::Low), (Some(1024), 1024 + 2048));
        assert_eq!(budget_and_cap(ChatReasoningEffort::Medium), (Some(4096), 4096 + 2048));
        assert_eq!(budget_and_cap(ChatReasoningEffort::High), (Some(16384), 16384 + 2048));
        // No thinking (disabled variant or absent effort): the workspace-wide
        // default, and no budget at all.
        assert_eq!(
            budget_and_cap(ChatReasoningEffort::None),
            (None, DEFAULT_COMPLETION_MAX_TOKENS)
        );
        let resolved = resolve_sampling(&common(None, None), None, None);
        assert_eq!(resolved.thinking_budget, None);
        assert_eq!(resolved.max_tokens, DEFAULT_COMPLETION_MAX_TOKENS);
    }

    #[test]
    fn explicit_cap_overrides_derivation_and_clamps_budget() {
        // A deliberate request cap wins over the budget-derived cap and clamps
        // the budget to the answer floor (cap - 256).
        let mut params = common(None, None);
        params.max_tokens = Some(4096);
        let resolved = resolve_sampling(&params, Some(ChatReasoningEffort::High), None);
        assert_eq!(resolved.max_tokens, 4096);
        assert_eq!(resolved.explicit_max_tokens, Some(4096));
        assert_eq!(resolved.thinking_budget, Some(3840));
    }

    #[test]
    fn request_and_model_preset_caps_are_explicit() {
        let mut params = common(None, None);
        params.max_tokens = Some(1200);
        assert_eq!(resolve_sampling(&params, None, None).explicit_max_tokens, Some(1200));

        let model = RuntimePresets { max_tokens: Some(2048), ..Default::default() };
        assert_eq!(
            resolve_sampling(&common(None, None), None, Some(&model)).explicit_max_tokens,
            Some(2048)
        );
    }

    #[test]
    fn request_overrides_model_preset() {
        let model =
            RuntimePresets { temperature: Some(0.6), top_p: Some(0.95), ..Default::default() };
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
        let mut model =
            RuntimePresets { temperature: Some(0.6), top_p: Some(0.95), ..Default::default() };
        model.efforts.insert(
            "high".to_owned(),
            RuntimePresets { temperature: Some(0.3), ..Default::default() },
        );
        let resolved =
            resolve_sampling(&common(None, None), Some(ChatReasoningEffort::High), Some(&model));
        // high override temperature (0.3) wins over the flat default (0.6).
        assert!((resolved.temperature - 0.3).abs() < f32::EPSILON);
        // top_p not set on the override, so the flat default (0.95) fills in.
        assert!((resolved.top_p.unwrap() - 0.95).abs() < f32::EPSILON);
    }

    #[test]
    fn built_in_thinking_budget_table() {
        use super::{built_in_thinking_budget_for_effort, clamp_thinking_budget};
        assert_eq!(
            built_in_thinking_budget_for_effort(Some(ChatReasoningEffort::Minimal)),
            Some(512)
        );
        assert_eq!(built_in_thinking_budget_for_effort(Some(ChatReasoningEffort::Low)), Some(1024));
        assert_eq!(
            built_in_thinking_budget_for_effort(Some(ChatReasoningEffort::Medium)),
            Some(4096)
        );
        assert_eq!(
            built_in_thinking_budget_for_effort(Some(ChatReasoningEffort::High)),
            Some(16384)
        );
        // No enforcement for the disable variant or an absent effort.
        assert_eq!(built_in_thinking_budget_for_effort(Some(ChatReasoningEffort::None)), None);
        assert_eq!(built_in_thinking_budget_for_effort(None), None);
        // Clamp: raw budget vs the answer floor (max_tokens - 256).
        assert_eq!(clamp_thinking_budget(Some(16384), 4096), Some(3840));
        assert_eq!(clamp_thinking_budget(Some(1024), 1024), Some(768));
        // A ceiling below 64 leaves no room for a forced close + answer.
        assert_eq!(clamp_thinking_budget(Some(1024), 300), None);
        assert_eq!(clamp_thinking_budget(None, 4096), None);
    }

    #[test]
    fn resolve_sampling_thinking_budget_uses_built_in_unclamped_when_derived() {
        // High effort without a request cap: the budget survives whole — the
        // derived cap (budget + allowance) always leaves answer room.
        let resolved = resolve_sampling(&common(None, None), Some(ChatReasoningEffort::High), None);
        assert_eq!(resolved.thinking_budget, Some(16384));
        // A small explicit request cap becomes the clamp base.
        let mut params = common(None, None);
        params.max_tokens = Some(512);
        let resolved = resolve_sampling(&params, Some(ChatReasoningEffort::Low), None);
        assert_eq!(resolved.thinking_budget, Some(256));
        // Absent effort keeps today's behaviour: no budget at all.
        let resolved = resolve_sampling(&common(None, None), None, None);
        assert_eq!(resolved.thinking_budget, None);
    }

    #[test]
    fn model_preset_thinking_budget_overrides_built_in() {
        // Flat model budget applies when the effort has no override…
        let model = RuntimePresets::default().with_thinking_budget(Some(64));
        let resolved =
            resolve_sampling(&common(None, None), Some(ChatReasoningEffort::High), Some(&model));
        assert_eq!(resolved.thinking_budget, Some(64));
        // …and the per-effort override wins over both flat and built-in.
        let mut model = RuntimePresets::default().with_thinking_budget(Some(64));
        model
            .efforts
            .insert("high".to_owned(), RuntimePresets::default().with_thinking_budget(Some(128)));
        let resolved =
            resolve_sampling(&common(None, None), Some(ChatReasoningEffort::High), Some(&model));
        assert_eq!(resolved.thinking_budget, Some(128));
    }
}
