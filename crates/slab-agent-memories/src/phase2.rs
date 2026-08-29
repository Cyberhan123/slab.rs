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
    // Most-used-first (Codex parity): when the limit binds, the memories the
    // agent actually cites must survive — sorting ascending would starve
    // high-value inputs and preferentially consolidate never-used ones.
    inputs.sort_by(|left, right| {
        right
            .usage_count
            .cmp(&left.usage_count)
            .then_with(|| usage_sort_key(right).cmp(&usage_sort_key(left)))
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

    #[test]
    fn selection_orders_by_usage_count_then_usage_time_and_watermark_max() {
        let now = Utc.with_ymd_and_hms(2026, 6, 11, 0, 0, 0).unwrap();
        let claimed = now - Duration::hours(1);
        let newest_source = now + Duration::hours(1);

        let selection = select_phase2_inputs(
            vec![
                Phase2Input {
                    thread_id: "often".into(),
                    session_id: "s".into(),
                    raw_memory: "often".into(),
                    rollout_summary: "often".into(),
                    rollout_slug: None,
                    generated_at: now,
                    source_updated_at: now,
                    last_usage: Some(now - Duration::minutes(5)),
                    usage_count: 5,
                },
                Phase2Input {
                    thread_id: "older".into(),
                    session_id: "s".into(),
                    raw_memory: "older".into(),
                    rollout_summary: "older".into(),
                    rollout_slug: None,
                    generated_at: now,
                    source_updated_at: newest_source,
                    last_usage: Some(now - Duration::minutes(10)),
                    usage_count: 1,
                },
                Phase2Input {
                    thread_id: "newer".into(),
                    session_id: "s".into(),
                    raw_memory: "newer".into(),
                    rollout_summary: "newer".into(),
                    rollout_slug: None,
                    generated_at: now,
                    source_updated_at: now,
                    last_usage: Some(now - Duration::minutes(1)),
                    usage_count: 1,
                },
            ],
            Phase2SelectionConfig { limit: 3, max_unused_days: 30 },
            now,
            Some(claimed),
        );

        let ids = selection.inputs.iter().map(|input| input.thread_id.as_str()).collect::<Vec<_>>();
        assert_eq!(ids, vec!["often", "newer", "older"]);
        assert_eq!(selection.new_watermark, Some(newest_source));
    }

    #[test]
    fn selection_prefers_most_used_when_limit_binds() {
        let now = Utc.with_ymd_and_hms(2026, 6, 11, 0, 0, 0).unwrap();

        let input = |thread_id: &str, usage_count: u64| Phase2Input {
            thread_id: thread_id.to_owned(),
            session_id: "s".into(),
            raw_memory: thread_id.to_owned(),
            rollout_summary: thread_id.to_owned(),
            rollout_slug: None,
            generated_at: now,
            source_updated_at: now,
            last_usage: None,
            usage_count,
        };

        let selection = select_phase2_inputs(
            vec![input("never", 0), input("rare", 1), input("often", 5)],
            Phase2SelectionConfig { limit: 1, max_unused_days: 30 },
            now,
            None,
        );

        let ids = selection.inputs.iter().map(|input| input.thread_id.as_str()).collect::<Vec<_>>();
        assert_eq!(ids, vec!["often"]);
    }
}
