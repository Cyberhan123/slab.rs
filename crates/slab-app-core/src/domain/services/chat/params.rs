use serde_json::Value;

use crate::domain::models::{
    ChatCompletionCommand, StructuredOutput, TextCompletionCommand, TextGenerationResponse,
    TextGenerationUsage,
};
use crate::error::{AppCoreError, AppCoreErrorData};

const REASONING_CONTENT_METADATA_KEY: &str = "reasoning_content";

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

pub(super) fn finish_reason_from_token_budget(completion_tokens: u32, max_tokens: u32) -> String {
    if completion_tokens >= max_tokens && max_tokens > 0 {
        "length".to_owned()
    } else {
        "stop".to_owned()
    }
}

pub(super) fn build_estimated_usage(
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

pub(super) fn text_response_has_visible_output(response: &TextGenerationResponse) -> bool {
    let has_content = !response.text.trim_end_matches('\0').trim().is_empty();
    let has_reasoning = response
        .metadata
        .get(REASONING_CONTENT_METADATA_KEY)
        .and_then(Value::as_str)
        .is_some_and(|value| !value.trim().is_empty());

    has_content || has_reasoning || !response.tool_calls.is_empty()
}

pub(super) fn apply_stop_sequences(text: &str, stop: &[String]) -> (String, bool) {
    let Some((index, _)) = stop
        .iter()
        .filter(|value| !value.is_empty())
        .filter_map(|value| text.find(value).map(|index| (index, value)))
        .min_by_key(|(index, _)| *index)
    else {
        return (text.to_owned(), false);
    };

    (text[..index].to_owned(), true)
}

pub(super) fn merge_usage(
    total: &mut Option<TextGenerationUsage>,
    next: Option<TextGenerationUsage>,
) {
    let Some(next) = next else {
        return;
    };

    match total {
        Some(total) => {
            total.prompt_tokens = total.prompt_tokens.saturating_add(next.prompt_tokens);
            total.completion_tokens =
                total.completion_tokens.saturating_add(next.completion_tokens);
            total.total_tokens = total.total_tokens.saturating_add(next.total_tokens);
            total.prompt_tokens_details.cached_tokens = total
                .prompt_tokens_details
                .cached_tokens
                .saturating_add(next.prompt_tokens_details.cached_tokens);
            total.estimated |= next.estimated;
        }
        None => *total = Some(next),
    }
}

pub(super) fn validate_cloud_structured_output(
    structured_output: Option<&StructuredOutput>,
) -> Result<(), AppCoreError> {
    let Some(StructuredOutput::JsonSchema(schema)) = structured_output else {
        return Ok(());
    };

    if matches!(schema.strict, Some(false)) {
        return Err(unsupported_chat_parameter(
            "response_format.json_schema.strict",
            "cloud structured outputs currently require strict=true",
        ));
    }

    Ok(())
}

fn unsupported_chat_parameter(param: &str, message: impl Into<String>) -> AppCoreError {
    AppCoreError::BadRequestData {
        message: message.into(),
        data: Box::new(AppCoreErrorData::unsupported_chat_parameter(param)),
    }
}

pub(super) fn validate_chat_route_params(
    route_to_cloud: bool,
    command: &ChatCompletionCommand,
) -> Result<(), AppCoreError> {
    if route_to_cloud {
        if command.local.gbnf.is_some() {
            return Err(unsupported_chat_parameter(
                "gbnf",
                "cloud chat completions do not support raw gbnf constraints",
            ));
        }
        if command.common.top_k.is_some() {
            return Err(unsupported_chat_parameter(
                "top_k",
                "cloud chat completions do not support local top_k sampling controls",
            ));
        }
        if command.common.min_p.is_some() {
            return Err(unsupported_chat_parameter(
                "min_p",
                "cloud chat completions do not support local min_p sampling controls",
            ));
        }
        if command.common.presence_penalty.is_some() {
            return Err(unsupported_chat_parameter(
                "presence_penalty",
                "cloud chat completions do not support local presence penalty controls",
            ));
        }
        if command.common.repetition_penalty.is_some() {
            return Err(unsupported_chat_parameter(
                "repetition_penalty",
                "cloud chat completions do not support local repetition penalty controls",
            ));
        }
        validate_cloud_structured_output(command.cloud.structured_output.as_ref())?;
        return Ok(());
    }

    Ok(())
}

pub(super) fn validate_text_route_params(
    route_to_cloud: bool,
    command: &TextCompletionCommand,
) -> Result<(), AppCoreError> {
    if route_to_cloud {
        if command.local.gbnf.is_some() {
            return Err(unsupported_chat_parameter(
                "gbnf",
                "cloud text completions do not support raw gbnf constraints",
            ));
        }
        if command.common.top_k.is_some() {
            return Err(unsupported_chat_parameter(
                "top_k",
                "cloud text completions do not support local top_k sampling controls",
            ));
        }
        if command.common.min_p.is_some() {
            return Err(unsupported_chat_parameter(
                "min_p",
                "cloud text completions do not support local min_p sampling controls",
            ));
        }
        if command.common.presence_penalty.is_some() {
            return Err(unsupported_chat_parameter(
                "presence_penalty",
                "cloud text completions do not support local presence penalty controls",
            ));
        }
        if command.common.repetition_penalty.is_some() {
            return Err(unsupported_chat_parameter(
                "repetition_penalty",
                "cloud text completions do not support local repetition penalty controls",
            ));
        }
        validate_cloud_structured_output(command.cloud.structured_output.as_ref())?;
    }

    Ok(())
}
