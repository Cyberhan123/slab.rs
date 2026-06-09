//! OpenAI-compatible chat completion routes.

mod cloud;
mod gbnf;
mod local;
mod params;
mod session;
mod streaming;
mod template;

use chrono::Utc;
use futures::stream::BoxStream;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::context::ModelState;
use crate::domain::models::{
    ChatCompletionCommand, ChatCompletionOutput, ChatCompletionResult, ChatResultChoice,
    ChatStreamChunk, ConversationMessage as DomainConversationMessage, ConversationMessageContent,
    TextCompletionCommand, TextCompletionOutput, TextCompletionResult, TextGenerationResponse,
    TextResultChoice, assistant_message_from_text_response,
};
use crate::error::AppCoreError;
#[cfg(test)]
use params::validate_cloud_structured_output;
use params::{
    apply_stop_sequences, build_estimated_usage, finish_reason_from_token_budget, merge_usage,
    text_response_has_visible_output, validate_chat_route_params, validate_text_route_params,
};
use session::{build_messages, persist_session_message};
use streaming::{
    build_chunk, build_error_chunk, build_finish_chunk, build_reasoning_chunk, build_role_chunk,
    build_usage_chunk, into_text_completion_stream, with_stream_session_persistence,
};

const CLOUD_MODEL_ID_PREFIX: &str = "cloud";
const DEFAULT_COMPLETION_MAX_TOKENS: u32 = 512;
const SYSTEM_FINGERPRINT: &str = "b-slab";

enum GeneratedChatOutput {
    Text(TextGenerationResponse),
    Stream(BoxStream<'static, ChatStreamChunk>),
}

#[derive(Clone)]
pub struct ChatService {
    state: ModelState,
}

impl ChatService {
    pub fn new(state: ModelState) -> Self {
        Self { state }
    }

    pub async fn create_chat_completion(
        &self,
        command: ChatCompletionCommand,
    ) -> Result<ChatCompletionOutput, AppCoreError> {
        create_chat_completion_with_state(self.state.clone(), command).await
    }

    pub async fn create_text_completion(
        &self,
        command: TextCompletionCommand,
    ) -> Result<TextCompletionOutput, AppCoreError> {
        create_text_completion_with_state(self.state.clone(), command).await
    }
}

async fn resolve_requested_model(
    state: &ModelState,
    requested_model: &str,
) -> Result<String, AppCoreError> {
    let trimmed = requested_model.trim();
    if !trimmed.is_empty() {
        return Ok(trimmed.to_owned());
    }

    let options = crate::domain::services::model::list_chat_models_from_state(state).await?;
    let preferred = options
        .iter()
        .find(|item| item.downloaded || item.provider_id.is_some())
        .or_else(|| options.first());

    preferred
        .map(|item| item.id.clone())
        .ok_or_else(|| AppCoreError::BadRequest("no chat-compatible models are configured".into()))
}

async fn create_chat_completion_with_state(
    state: ModelState,
    command: ChatCompletionCommand,
) -> Result<ChatCompletionOutput, AppCoreError> {
    if command.common.stream && command.common.n > 1 {
        return Err(AppCoreError::NotImplemented("streaming with n > 1 is not supported".into()));
    }

    let resolved_model = resolve_requested_model(&state, &command.model).await?;
    let continue_generation = command.continue_generation;
    let user_content = command
        .messages
        .iter()
        .rev()
        .find(|message| message.role == "user")
        .map(DomainConversationMessage::rendered_text)
        .unwrap_or_default();

    let max_tokens = command.common.max_tokens.unwrap_or(DEFAULT_COMPLETION_MAX_TOKENS);
    let temperature = command.common.temperature.unwrap_or(0.7);
    let route_to_cloud = cloud::should_route_to_cloud(&state, &resolved_model).await?;
    if command.common.stream && route_to_cloud && !command.common.stop.is_empty() {
        return Err(AppCoreError::NotImplemented(
            "streaming with stop is not supported for cloud chat completions".into(),
        ));
    }
    validate_chat_route_params(route_to_cloud, &command)?;

    debug!(
        model = %resolved_model,
        prompt_len = user_content.len(),
        stream = command.common.stream,
        continue_generation,
        session_id = ?command.id,
        "chat completion request"
    );

    let resolved_messages =
        build_messages(&state, command.id.as_deref(), &command.messages).await?;
    let latest_user_message = command
        .messages
        .iter()
        .rev()
        .find(|message| message.role == "user" && message.has_meaningful_content())
        .cloned();

    if let Some(session_id) = command.id.as_deref().filter(|_| !continue_generation) {
        if let Some(message) = latest_user_message.as_ref() {
            persist_session_message(&state, session_id, message).await;
        } else if !user_content.trim().is_empty() {
            persist_session_message(
                &state,
                session_id,
                &DomainConversationMessage {
                    role: "user".into(),
                    content: ConversationMessageContent::Text(user_content.clone()),
                    name: None,
                    tool_call_id: None,
                    tool_calls: Vec::new(),
                },
            )
            .await;
        }
    }

    if command.common.stream {
        let generated = if route_to_cloud {
            cloud::create_chat_completion(
                &state,
                &resolved_model,
                &resolved_messages,
                cloud::CloudChatRequestConfig {
                    max_tokens,
                    temperature,
                    top_p: command.common.top_p,
                    structured_output: command.cloud.structured_output.clone(),
                    reasoning_effort: command.cloud.reasoning_effort,
                    verbosity: command.cloud.verbosity,
                    tools: command.tools.clone(),
                    stream: true,
                    include_usage: command.common.stream_options.include_usage,
                },
            )
            .await?
        } else {
            local::create_chat_completion(
                &state,
                &resolved_model,
                &resolved_messages,
                local::LocalChatRequestConfig {
                    session_id: command.id.clone(),
                    max_tokens,
                    temperature,
                    top_p: command.common.top_p,
                    top_k: command.common.top_k,
                    min_p: command.common.min_p,
                    presence_penalty: command.common.presence_penalty,
                    repetition_penalty: command.common.repetition_penalty,
                    reasoning_effort: command.cloud.reasoning_effort,
                    verbosity: command.cloud.verbosity,
                    gbnf: command.local.gbnf.clone(),
                    structured_output: command.local.structured_output.clone(),
                    tools: command.tools.clone(),
                    stop: command.common.stop.clone(),
                    agent_trace: command.agent_trace.clone(),
                    stream: true,
                    include_usage: command.common.stream_options.include_usage,
                },
            )
            .await?
        };

        return match generated {
            GeneratedChatOutput::Text(text) => {
                let assistant_message = assistant_message_from_text_response(&text);
                if let Some(session_id) = command.id.as_deref() {
                    persist_session_message(&state, session_id, &assistant_message).await;
                }

                let response = ChatCompletionResult {
                    id: format!("chatcmpl-{}", Uuid::new_v4()),
                    object: "chat.completion".into(),
                    created: Utc::now().timestamp(),
                    model: resolved_model,
                    system_fingerprint: SYSTEM_FINGERPRINT.into(),
                    choices: vec![ChatResultChoice {
                        index: 0,
                        message: assistant_message,
                        finish_reason: text.finish_reason.or(Some("stop".into())),
                    }],
                    usage: text.usage,
                };
                Ok(ChatCompletionOutput::Json(response))
            }
            GeneratedChatOutput::Stream(stream) => {
                let stream = match command.id.clone() {
                    Some(session_id) => {
                        with_stream_session_persistence(stream, state.clone(), session_id)
                    }
                    None => stream,
                };
                Ok(ChatCompletionOutput::Stream(stream))
            }
        };
    }

    let mut choices = Vec::new();
    let mut usage = None;
    for index in 0..command.common.n {
        let mut generated = if route_to_cloud {
            generate_cloud_chat_text(
                &state,
                &resolved_model,
                &resolved_messages,
                cloud::CloudChatRequestConfig {
                    max_tokens,
                    temperature,
                    top_p: command.common.top_p,
                    structured_output: command.cloud.structured_output.clone(),
                    reasoning_effort: command.cloud.reasoning_effort,
                    verbosity: command.cloud.verbosity,
                    tools: command.tools.clone(),
                    stream: false,
                    include_usage: false,
                },
            )
            .await?
        } else {
            generate_local_chat_text(
                &state,
                &resolved_model,
                &resolved_messages,
                local::LocalChatRequestConfig {
                    session_id: command.id.clone(),
                    max_tokens,
                    temperature,
                    top_p: command.common.top_p,
                    top_k: command.common.top_k,
                    min_p: command.common.min_p,
                    presence_penalty: command.common.presence_penalty,
                    repetition_penalty: command.common.repetition_penalty,
                    reasoning_effort: command.cloud.reasoning_effort,
                    verbosity: command.cloud.verbosity,
                    gbnf: command.local.gbnf.clone(),
                    structured_output: command.local.structured_output.clone(),
                    tools: command.tools.clone(),
                    stop: command.common.stop.clone(),
                    agent_trace: command.agent_trace.clone(),
                    stream: false,
                    include_usage: false,
                },
            )
            .await?
        };

        if route_to_cloud {
            let (trimmed_text, stop_matched) =
                apply_stop_sequences(&generated.text, &command.common.stop);
            if stop_matched {
                generated.text = trimmed_text;
                generated.finish_reason = Some("stop".into());
            }
        }

        merge_usage(&mut usage, generated.usage.clone());
        if !text_response_has_visible_output(&generated) {
            warn!(
                model = %resolved_model,
                route = if route_to_cloud { "cloud" } else { "local" },
                finish_reason = generated.finish_reason.as_deref().unwrap_or("unknown"),
                prompt_tokens = generated.usage.as_ref().map(|value| value.prompt_tokens).unwrap_or(0),
                completion_tokens = generated.tokens_used.unwrap_or(0),
                total_tokens = generated.usage.as_ref().map(|value| value.total_tokens).unwrap_or(0),
                usage_estimated = generated.usage.as_ref().map(|value| value.estimated).unwrap_or(true),
                message_count = resolved_messages.len(),
                "chat completion returned without visible assistant output"
            );
        }
        let assistant_message = assistant_message_from_text_response(&generated);
        choices.push(ChatResultChoice {
            index,
            message: assistant_message,
            finish_reason: generated.finish_reason.or(Some("stop".into())),
        });
    }

    info!(
        model = %resolved_model,
        output_len = choices
            .first()
            .map(|choice| choice.message.rendered_text().len())
            .unwrap_or_default(),
        "chat completion done"
    );

    if let Some(session_id) = command.id.as_deref()
        && let Some(first_choice) = choices.first()
    {
        persist_session_message(&state, session_id, &first_choice.message).await;
    }

    let response = ChatCompletionResult {
        id: format!("chatcmpl-{}", Uuid::new_v4()),
        object: "chat.completion".into(),
        created: Utc::now().timestamp(),
        model: resolved_model,
        system_fingerprint: SYSTEM_FINGERPRINT.into(),
        choices,
        usage,
    };

    Ok(ChatCompletionOutput::Json(response))
}

async fn create_text_completion_with_state(
    state: ModelState,
    command: TextCompletionCommand,
) -> Result<TextCompletionOutput, AppCoreError> {
    if command.common.stream && command.common.n > 1 {
        return Err(AppCoreError::NotImplemented("streaming with n > 1 is not supported".into()));
    }

    let resolved_model = resolve_requested_model(&state, &command.model).await?;
    let max_tokens = command.common.max_tokens.unwrap_or(DEFAULT_COMPLETION_MAX_TOKENS);
    let temperature = command.common.temperature.unwrap_or(0.7);
    let route_to_cloud = cloud::should_route_to_cloud(&state, &resolved_model).await?;
    validate_text_route_params(route_to_cloud, &command)?;

    debug!(
        model = %resolved_model,
        prompt_len = command.prompt.len(),
        stream = command.common.stream,
        "text completion request"
    );

    let mut choices = Vec::new();
    let mut usage = None;
    for index in 0..command.common.n {
        let mut generated = if route_to_cloud {
            cloud::create_text_completion(
                &state,
                &resolved_model,
                &command.prompt,
                cloud::CloudChatRequestConfig {
                    max_tokens,
                    temperature,
                    top_p: command.common.top_p,
                    structured_output: command.cloud.structured_output.clone(),
                    reasoning_effort: None,
                    verbosity: None,
                    tools: Vec::new(),
                    stream: false,
                    include_usage: false,
                },
            )
            .await?
        } else {
            local::create_text_completion(
                &state,
                &resolved_model,
                &command.prompt,
                local::LocalTextRequestConfig {
                    max_tokens,
                    temperature,
                    top_p: command.common.top_p,
                    top_k: command.common.top_k,
                    min_p: command.common.min_p,
                    presence_penalty: command.common.presence_penalty,
                    repetition_penalty: command.common.repetition_penalty,
                    reasoning_effort: command.cloud.reasoning_effort,
                    verbosity: command.cloud.verbosity,
                    gbnf: command.local.gbnf.clone(),
                    structured_output: command.local.structured_output.clone(),
                },
            )
            .await?
        };

        let (trimmed_text, stop_matched) =
            apply_stop_sequences(&generated.text, &command.common.stop);
        if stop_matched {
            generated.text = trimmed_text;
            generated.finish_reason = Some("stop".into());
        }

        merge_usage(&mut usage, generated.usage.clone());
        choices.push(TextResultChoice {
            index,
            text: generated.text,
            finish_reason: generated.finish_reason.or(Some("stop".into())),
        });
    }

    let response = TextCompletionResult {
        id: format!("cmpl-{}", Uuid::new_v4()),
        object: "text_completion".into(),
        created: Utc::now().timestamp(),
        model: resolved_model.clone(),
        system_fingerprint: SYSTEM_FINGERPRINT.into(),
        choices,
        usage,
    };

    if command.common.stream {
        let first_choice =
            response.choices.first().cloned().ok_or_else(|| {
                AppCoreError::Internal("text completion produced no choices".into())
            })?;
        return Ok(into_text_completion_stream(
            response.id,
            response.created,
            resolved_model,
            first_choice.text,
            first_choice.finish_reason.unwrap_or_else(|| "stop".into()),
        ));
    }

    Ok(TextCompletionOutput::Json(response))
}

async fn generate_cloud_chat_text(
    state: &ModelState,
    model: &str,
    messages: &[DomainConversationMessage],
    config: cloud::CloudChatRequestConfig,
) -> Result<TextGenerationResponse, AppCoreError> {
    match cloud::create_chat_completion(state, model, messages, config).await? {
        GeneratedChatOutput::Text(text) => Ok(text),
        GeneratedChatOutput::Stream(_) => Err(AppCoreError::Internal(
            "cloud chat completion unexpectedly returned a stream".into(),
        )),
    }
}

async fn generate_local_chat_text(
    state: &ModelState,
    model: &str,
    messages: &[DomainConversationMessage],
    config: local::LocalChatRequestConfig,
) -> Result<TextGenerationResponse, AppCoreError> {
    match local::create_chat_completion(state, model, messages, config).await? {
        GeneratedChatOutput::Text(text) => Ok(text),
        GeneratedChatOutput::Stream(_) => Err(AppCoreError::Internal(
            "local chat completion unexpectedly returned a stream".into(),
        )),
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::domain::models::{
        ChatReasoningEffort, ChatVerbosity, StructuredOutput, TextCompletionCommand,
    };

    fn make_command(role: &str, content: &str) -> ChatCompletionCommand {
        ChatCompletionCommand {
            model: "test".into(),
            messages: vec![DomainConversationMessage {
                role: role.into(),
                content: ConversationMessageContent::Text(content.into()),
                name: None,
                tool_call_id: None,
                tool_calls: Vec::new(),
            }],
            tools: Vec::new(),
            agent_trace: None,
            continue_generation: false,
            common: crate::domain::models::CommonChatParams {
                max_tokens: None,
                temperature: None,
                top_p: None,
                top_k: None,
                min_p: None,
                presence_penalty: None,
                repetition_penalty: None,
                n: 1,
                stream: false,
                stop: Vec::new(),
                stream_options: Default::default(),
            },
            local: crate::domain::models::LocalChatParams { gbnf: None, structured_output: None },
            cloud: crate::domain::models::CloudChatParams {
                reasoning_effort: None,
                verbosity: None,
                structured_output: None,
            },
            id: None,
        }
    }

    fn make_text_command(prompt: &str) -> TextCompletionCommand {
        TextCompletionCommand {
            model: "test".into(),
            prompt: prompt.into(),
            common: crate::domain::models::CommonChatParams {
                max_tokens: None,
                temperature: None,
                top_p: None,
                top_k: None,
                min_p: None,
                presence_penalty: None,
                repetition_penalty: None,
                n: 1,
                stream: false,
                stop: Vec::new(),
                stream_options: Default::default(),
            },
            local: crate::domain::models::LocalChatParams { gbnf: None, structured_output: None },
            cloud: crate::domain::models::CloudChatParams {
                reasoning_effort: None,
                verbosity: None,
                structured_output: None,
            },
        }
    }

    #[test]
    fn validate_max_tokens_zero() {
        let mut req = make_command("user", "hello");
        req.common.max_tokens = Some(0);
        assert_eq!(req.common.max_tokens, Some(0));
        let max_tokens = req.common.max_tokens.unwrap_or(DEFAULT_COMPLETION_MAX_TOKENS);
        assert_eq!(max_tokens, 0, "zero should stay invalid");
    }

    #[test]
    fn validate_large_max_tokens_is_preserved() {
        let mut req = make_command("user", "hello");
        req.common.max_tokens = Some(81_920);
        let max_tokens = req.common.max_tokens.unwrap_or(DEFAULT_COMPLETION_MAX_TOKENS);
        assert_eq!(max_tokens, 81_920);
    }

    #[test]
    fn validate_temperature_out_of_range() {
        let temperature = 3.0_f32;
        assert!(!(0.0..=2.0).contains(&temperature), "should be out of range");
    }

    #[test]
    fn no_user_message_returns_error() {
        let req = make_command("system", "you are a bot");
        let found = req.messages.iter().rev().find(|message| message.role == "user");
        assert!(found.is_none());
    }

    #[test]
    fn validate_chat_route_params_allows_local_reasoning_controls() {
        let mut req = make_command("user", "hello");
        req.cloud.reasoning_effort = Some(ChatReasoningEffort::High);
        req.cloud.verbosity = Some(ChatVerbosity::Low);

        assert!(validate_chat_route_params(false, &req).is_ok());
    }

    #[test]
    fn validate_text_route_params_allows_local_reasoning_controls() {
        let mut req = make_text_command("hello");
        req.cloud.reasoning_effort = Some(ChatReasoningEffort::Minimal);
        req.cloud.verbosity = Some(ChatVerbosity::High);

        assert!(validate_text_route_params(false, &req).is_ok());
    }

    #[test]
    fn apply_stop_sequences_truncates_at_first_match() {
        let (trimmed, matched) = apply_stop_sequences("hello STOP world", &["STOP".into()]);

        assert!(matched);
        assert_eq!(trimmed, "hello ");
    }

    #[test]
    fn cloud_structured_output_rejects_strict_false() {
        let result = validate_cloud_structured_output(Some(&StructuredOutput::JsonSchema(
            crate::domain::models::StructuredOutputJsonSchema {
                name: "example".into(),
                description: None,
                strict: Some(false),
                schema: serde_json::json!({ "type": "object" }),
            },
        )));

        assert!(matches!(result, Err(AppCoreError::BadRequestData { .. })));
    }

    #[test]
    fn local_route_allows_reasoning_controls() {
        let mut command = make_command("user", "hello");
        command.cloud.reasoning_effort = Some(crate::domain::models::ChatReasoningEffort::Low);
        command.cloud.verbosity = Some(crate::domain::models::ChatVerbosity::Medium);

        let result = validate_chat_route_params(false, &command);

        assert!(result.is_ok());
    }

    #[test]
    fn cloud_route_rejects_raw_gbnf() {
        let mut command = make_command("user", "hello");
        command.local.gbnf = Some("root ::= \"ok\"".into());

        let result = validate_chat_route_params(true, &command);

        assert!(matches!(result, Err(AppCoreError::BadRequestData { .. })));
    }
}
