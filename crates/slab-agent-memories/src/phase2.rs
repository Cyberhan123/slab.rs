use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Phase2Input {
    pub thread_id: String,
    pub session_id: String,
    pub raw_memory: String,
    pub rollout_summary: String,
    pub rollout_slug: Option<String>,
    pub generated_at: DateTime<Utc>,
    pub source_updated_at: DateTime<Utc>,
    pub last_usage: Option<DateTime<Utc>>,
    pub usage_count: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Phase2SelectionConfig {
    pub limit: usize,
    pub max_unused_days: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Phase2Selection {
    pub inputs: Vec<Phase2Input>,
    pub new_watermark: Option<DateTime<Utc>>,
}

pub fn select_phase2_inputs(
    mut inputs: Vec<Phase2Input>,
    config: Phase2SelectionConfig,
    now: DateTime<Utc>,
    claimed_watermark: Option<DateTime<Utc>>,
) -> Phase2Selection {
    let oldest_allowed = now - Duration::days(config.max_unused_days.max(0));
    inputs.retain(|input| input.last_usage.unwrap_or(input.generated_at) >= oldest_allowed);
    inputs.sort_by(|left, right| {
        left.usage_count
            .cmp(&right.usage_count)
            .then_with(|| usage_sort_key(left).cmp(&usage_sort_key(right)))
            .then_with(|| left.thread_id.cmp(&right.thread_id))
    });
    inputs.truncate(config.limit);

    let input_watermark = inputs.iter().map(|input| input.source_updated_at).max();
    let new_watermark = [claimed_watermark, input_watermark].into_iter().flatten().max();
    Phase2Selection { inputs, new_watermark }
}

fn usage_sort_key(input: &Phase2Input) -> DateTime<Utc> {
    input.last_usage.unwrap_or(input.generated_at)
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone as _;

    use super::*;

    #[test]
    fn selection_uses_generated_at_for_never_used_inputs() {
        let now = Utc.with_ymd_and_hms(2026, 6, 11, 0, 0, 0).unwrap();
        let old = now - Duration::days(40);
        let fresh = now - Duration::days(1);

        let selection = select_phase2_inputs(
            vec![
                Phase2Input {
                    thread_id: "old".into(),
                    session_id: "s".into(),
                    raw_memory: "old".into(),
                    rollout_summary: "old".into(),
                    rollout_slug: None,
                    generated_at: old,
                    source_updated_at: old,
                    last_usage: None,
                    usage_count: 0,
                },
                Phase2Input {
                    thread_id: "fresh".into(),
                    session_id: "s".into(),
                    raw_memory: "fresh".into(),
                    rollout_summary: "fresh".into(),
                    rollout_slug: None,
                    generated_at: fresh,
                    source_updated_at: fresh,
                    last_usage: None,
                    usage_count: 0,
                },
            ],
            Phase2SelectionConfig { limit: 10, max_unused_days: 30 },
            now,
            None,
        );

        assert_eq!(selection.inputs.len(), 1);
        assert_eq!(selection.inputs[0].thread_id, "fresh");
        assert_eq!(selection.new_watermark, Some(fresh));
    }
}
