use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

static METRICS_ENABLED: AtomicBool = AtomicBool::new(false);

pub(crate) fn set_enabled(enabled: bool) {
    METRICS_ENABLED.store(enabled, Ordering::Relaxed);
}

pub fn enabled() -> bool {
    METRICS_ENABLED.load(Ordering::Relaxed)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenAiTokenType {
    Input,
    Output,
}

impl GenAiTokenType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::Output => "output",
        }
    }
}

pub fn record_gen_ai_operation_duration(
    provider_name: &str,
    model: &str,
    operation_name: &str,
    duration: Duration,
) {
    if !enabled() {
        return;
    }
    tracing::info!(
        target: "slab_otel::metrics",
        metric = "gen_ai.client.operation.duration",
        otel_attributes = %serde_json::json!({
            "gen_ai.provider.name": provider_name,
            "gen_ai.request.model": model,
            "gen_ai.operation.name": operation_name,
        }),
        duration_ms = duration.as_secs_f64() * 1000.0,
        "gen_ai client operation duration"
    );
}

pub fn record_gen_ai_token_usage(
    provider_name: &str,
    model: &str,
    token_type: GenAiTokenType,
    tokens: u64,
) {
    if !enabled() {
        return;
    }
    tracing::info!(
        target: "slab_otel::metrics",
        metric = "gen_ai.client.token.usage",
        otel_attributes = %serde_json::json!({
            "gen_ai.provider.name": provider_name,
            "gen_ai.request.model": model,
            "gen_ai.token.type": token_type.as_str(),
        }),
        tokens,
        "gen_ai client token usage"
    );
}

pub fn record_tool_execution(tool_name: &str, tool_type: &str, duration: Duration, success: bool) {
    if !enabled() {
        return;
    }
    tracing::info!(
        target: "slab_otel::metrics",
        metric = "slab.tool.execution",
        otel_attributes = %serde_json::json!({
            "gen_ai.tool.name": tool_name,
            "gen_ai.tool.type": tool_type,
        }),
        duration_ms = duration.as_secs_f64() * 1000.0,
        success,
        "tool execution metric"
    );
}

pub fn record_tool_count(tool_name: &str, tool_type: &str, count: u64) {
    if !enabled() {
        return;
    }
    tracing::info!(
        target: "slab_otel::metrics",
        metric = "slab.tool.execution.count",
        otel_attributes = %serde_json::json!({
            "gen_ai.tool.name": tool_name,
            "gen_ai.tool.type": tool_type,
        }),
        count,
        "tool execution count"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_helpers_are_noop_when_disabled() {
        set_enabled(false);

        record_gen_ai_operation_duration("local", "model", "chat", Duration::from_millis(1));
        record_gen_ai_token_usage("local", "model", GenAiTokenType::Input, 3);
        record_tool_execution("read_file", "function", Duration::from_millis(1), true);
        record_tool_count("read_file", "function", 1);
    }
}
