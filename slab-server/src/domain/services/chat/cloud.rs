use futures::{StreamExt, stream};
use genai::adapter::AdapterKind;
use genai::chat::{
    ChatMessage as GenaiChatMessage, ChatOptions as GenaiChatOptions,
    ChatRequest as GenaiChatRequest, ChatResponseFormat as GenaiChatResponseFormat,
    ChatStreamEvent as GenaiChatStreamEvent, JsonSpec as GenaiJsonSpec,
    ReasoningEffort as GenaiReasoningEffort, Verbosity as GenaiVerbosity,
};
use genai::resolver::{AuthData, Endpoint, ServiceTargetResolver};
use genai::{
    Client as GenaiClient, ModelIden as GenaiModelIden, ServiceTarget as GenaiServiceTarget,
};
use serde_json::{Value, json};
use slab_types::inference::TextGenerationResponse;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::context::ModelState;
use crate::domain::models::{
    ChatModelOption, ChatModelSource, ChatReasoningEffort, ChatStreamChunk, ChatVerbosity,
    ConversationMessage as DomainConversationMessage, StructuredOutput, UnifiedModel,
    UnifiedModelStatus,
};
use crate::error::ServerError;
use crate::infra::db::ModelStore;

use super::GeneratedChatOutput;

type CloudProviderConfig = slab_types::settings::CloudProviderConfig;

#[derive(Debug, Clone)]
struct ResolvedCloudModel {
    provider_id: String,
    provider_name: String,
    api_base: String,
    api_key: String,
    remote_model: String,
}

#[derive(Debug, Clone)]
pub(super) struct CloudChatRequestConfig {
    pub(super) max_tokens: u32,
    pub(super) temperature: f32,
    pub(super) top_p: Option<f32>,
    pub(super) structured_output: Option<StructuredOutput>,
    pub(super) reasoning_effort: Option<ChatReasoningEffort>,
    pub(super) verbosity: Option<ChatVerbosity>,
    pub(super) stream: bool,
    pub(super) include_usage: bool,
}

#[derive(Debug)]
enum CloudDelta {
    Content(String),
    Reasoning(String),
}

type CloudTokenStream =
    std::pin::Pin<Box<dyn futures::Stream<Item = Result<CloudDelta, ServerError>> + Send>>;

#[derive(Debug, Clone)]
struct CloudHttpTraceContext {
    request_id: String,
    request_url: String,
    request_headers: String,
    request_body: String,
}

pub(super) fn is_cloud_model_option_id(model_id: &str) -> bool {
    model_id
        .strip_prefix(super::CLOUD_MODEL_ID_PREFIX)
        .is_some_and(|suffix| suffix.starts_with('/'))
}

pub(super) async fn should_route_to_cloud(
    state: &ModelState,
    requested_model: &str,
) -> Result<bool, ServerError> {
    if is_cloud_model_option_id(requested_model) {
        return Ok(true);
    }

    let Some(record) = state.store().get_model(requested_model).await? else {
        return Ok(false);
    };
    let model: UnifiedModel =
        record.try_into().map_err(|error: String| ServerError::Internal(error))?;
    Ok(is_cloud_catalog_model(&model))
}

pub(super) async fn list_chat_models(
    state: &ModelState,
) -> Result<Vec<ChatModelOption>, ServerError> {
    let providers = load_cloud_provider_map(state).await?;
    let records = state.store().list_models().await?;
    let mut items = Vec::new();

    for record in records {
        let model: UnifiedModel = match record.try_into() {
            Ok(model) => model,
            Err(error) => {
                warn!(error = %error, "failed to deserialize chat model record; skipping");
                continue;
            }
        };

        if let Some(item) = build_local_chat_model_option(&model) {
            items.push(item);
            continue;
        }

        if let Some(item) = build_cloud_chat_model_option(&providers, &model) {
            items.push(item);
        }
    }

    items.sort_by(|left, right| {
        left.display_name
            .to_ascii_lowercase()
            .cmp(&right.display_name.to_ascii_lowercase())
            .then_with(|| left.id.cmp(&right.id))
    });

    Ok(items)
}

pub(super) async fn create_chat_completion(
    state: &ModelState,
    requested_model: &str,
    messages: &[DomainConversationMessage],
    config: CloudChatRequestConfig,
) -> Result<GeneratedChatOutput, ServerError> {
    let target = resolve_cloud_model(state, requested_model).await?;
    let trace_http = state.config().cloud_http_trace;

    if config.stream {
        let max_tokens = config.max_tokens;
        let include_usage = config.include_usage;
        let backend_stream = cloud_chat_stream(&target, messages, config, trace_http).await?;
        let completion_id = format!("chatcmpl-{}", Uuid::new_v4());
        let created_ts = chrono::Utc::now().timestamp();
        let model_name = requested_model.to_owned();
        let completion_id_for_tokens = completion_id.clone();
        let model_name_for_tokens = model_name.clone();
        let completion_id_for_role = completion_id.clone();
        let model_name_for_role = model_name.clone();
        let completion_id_for_finish = completion_id.clone();
        let model_name_for_finish = model_name.clone();
        let completion_id_for_usage = completion_id.clone();
        let model_name_for_usage = model_name.clone();
        let prompt_for_usage = render_messages_for_usage(messages);

        let error_flag = Arc::new(AtomicBool::new(false));
        let completion_tokens = Arc::new(AtomicU32::new(0));

        let role_chunk = stream::once(async move {
            ChatStreamChunk::Data(super::build_role_chunk(
                &completion_id_for_role,
                created_ts,
                &model_name_for_role,
            ))
        });

        let token_stream_error_flag = Arc::clone(&error_flag);
        let token_stream_completion_tokens = Arc::clone(&completion_tokens);
        let token_stream = backend_stream.map(move |chunk| -> ChatStreamChunk {
            match chunk {
                Ok(CloudDelta::Content(token)) => {
                    token_stream_completion_tokens.fetch_add(1, Ordering::SeqCst);
                    ChatStreamChunk::Data(super::build_chunk(
                        &completion_id_for_tokens,
                        created_ts,
                        &model_name_for_tokens,
                        &token,
                    ))
                }
                Ok(CloudDelta::Reasoning(token)) => {
                    ChatStreamChunk::Data(super::build_reasoning_chunk(
                        &completion_id_for_tokens,
                        created_ts,
                        &model_name_for_tokens,
                        &token,
                    ))
                }
                Err(error) => {
                    token_stream_error_flag.store(true, Ordering::SeqCst);
                    ChatStreamChunk::Data(super::build_error_chunk(&error.to_string()))
                }
            }
        });

        let finish_chunk_error_flag = Arc::clone(&error_flag);
        let finish_chunk_completion_tokens = Arc::clone(&completion_tokens);
        let finish_chunk = stream::once(async move {
            if finish_chunk_error_flag.load(Ordering::SeqCst) {
                None
            } else {
                let finish_reason = super::finish_reason_from_token_budget(
                    finish_chunk_completion_tokens.load(Ordering::SeqCst),
                    max_tokens,
                );
                Some(ChatStreamChunk::Data(super::build_finish_chunk(
                    &completion_id_for_finish,
                    created_ts,
                    &model_name_for_finish,
                    &finish_reason,
                )))
            }
        });

        let usage_chunk_error_flag = Arc::clone(&error_flag);
        let usage_chunk_completion_tokens = Arc::clone(&completion_tokens);
        let usage_chunk = stream::once(async move {
            if !include_usage || usage_chunk_error_flag.load(Ordering::SeqCst) {
                None
            } else {
                let usage = super::build_estimated_usage(
                    &prompt_for_usage,
                    "",
                    Some(usage_chunk_completion_tokens.load(Ordering::SeqCst)),
                );
                Some(ChatStreamChunk::Data(super::build_usage_chunk(
                    &completion_id_for_usage,
                    created_ts,
                    &model_name_for_usage,
                    &usage,
                )))
            }
        });

        let sse_stream = role_chunk
            .chain(token_stream)
            .chain(finish_chunk.filter_map(futures::future::ready))
            .chain(usage_chunk.filter_map(futures::future::ready))
            .chain(stream::once(async { ChatStreamChunk::Data("[DONE]".into()) }));

        return Ok(GeneratedChatOutput::Stream(Box::pin(sse_stream)));
    }

    let generated = cloud_chat_completion(&target, messages, config, trace_http).await?;
    Ok(GeneratedChatOutput::Text(generated))
}

pub(super) async fn create_text_completion(
    state: &ModelState,
    requested_model: &str,
    prompt: &str,
    config: CloudChatRequestConfig,
) -> Result<TextGenerationResponse, ServerError> {
    let target = resolve_cloud_model(state, requested_model).await?;
    let messages = vec![DomainConversationMessage {
        role: "user".to_owned(),
        content: slab_types::chat::ConversationMessageContent::Text(prompt.to_owned()),
        name: None,
        tool_call_id: None,
        tool_calls: Vec::new(),
    }];

    cloud_chat_completion(&target, &messages, config, state.config().cloud_http_trace).await
}

#[cfg(test)]
pub(super) fn cloud_option_id(provider_id: &str, model_id: &str) -> String {
    format!("{}/{provider_id}/{model_id}", super::CLOUD_MODEL_ID_PREFIX)
}

fn looks_like_env_var_name(value: &str) -> bool {
    let mut chars = value.chars();
    match chars.next() {
        Some(ch) if ch == '_' || ch.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

async fn load_cloud_providers_strict(
    state: &ModelState,
) -> Result<Vec<CloudProviderConfig>, ServerError> {
    Ok(state.pmid().config().chat.providers)
}

async fn load_cloud_provider_map(
    state: &ModelState,
) -> Result<BTreeMap<String, CloudProviderConfig>, ServerError> {
    Ok(load_cloud_providers_strict(state)
        .await?
        .into_iter()
        .map(|provider| (provider.id.clone(), provider))
        .collect())
}

fn resolve_provider_api_key(provider: &CloudProviderConfig) -> Result<String, ServerError> {
    if let Some(key) = provider.api_key.as_deref() {
        return Ok(key.to_owned());
    }

    if let Some(env_key) = provider.api_key_env.as_deref() {
        let env_key = env_key.trim();
        if let Ok(value) = std::env::var(env_key) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Ok(trimmed.to_owned());
            }
        }

        // Be tolerant to common misconfiguration: users paste a literal API key into `api_key_env`.
        if !env_key.is_empty() && !looks_like_env_var_name(env_key) {
            warn!(
                provider_id = %provider.id,
                "api_key_env does not look like an env var name; treating it as a literal api key"
            );
            return Ok(env_key.to_owned());
        }
    }

    Err(ServerError::BackendNotReady(format!(
        "cloud provider '{}' is missing api key (set settings api_key or api_key_env)",
        provider.id
    )))
}

async fn resolve_cloud_model(
    state: &ModelState,
    requested_model: &str,
) -> Result<ResolvedCloudModel, ServerError> {
    let providers = load_cloud_provider_map(state).await?;
    let Some(model) = find_cloud_catalog_model(state, requested_model).await? else {
        return Err(ServerError::BadRequest(format!("unknown cloud model '{}'", requested_model)));
    };

    resolve_cloud_catalog_model(&providers, &model)
}

fn is_cloud_catalog_model(model: &UnifiedModel) -> bool {
    model.provider.starts_with("cloud.")
}

fn is_local_chat_model(model: &UnifiedModel) -> bool {
    model.provider == format!("local.{}", super::LLAMA_BACKEND_ID)
}

fn provider_id_from_provider_string(provider: &str) -> Option<String> {
    provider
        .strip_prefix("cloud.")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn referenced_provider_id(model: &UnifiedModel) -> Option<String> {
    model
        .spec
        .provider_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .or_else(|| provider_id_from_provider_string(&model.provider))
}

fn local_model_downloaded(model: &UnifiedModel) -> bool {
    matches!(model.status, UnifiedModelStatus::Ready) && model.spec.local_path.is_some()
}

fn local_model_pending(model: &UnifiedModel) -> bool {
    matches!(model.status, UnifiedModelStatus::Downloading)
}

fn build_local_chat_model_option(model: &UnifiedModel) -> Option<ChatModelOption> {
    if !is_local_chat_model(model) {
        return None;
    }

    Some(ChatModelOption {
        id: model.id.clone(),
        display_name: model.display_name.clone(),
        source: ChatModelSource::Local,
        downloaded: local_model_downloaded(model),
        pending: local_model_pending(model),
        backend_id: Some(super::LLAMA_BACKEND_ID.to_owned()),
        provider_id: None,
        provider_name: None,
    })
}

fn build_cloud_chat_model_option(
    providers: &BTreeMap<String, CloudProviderConfig>,
    model: &UnifiedModel,
) -> Option<ChatModelOption> {
    if !is_cloud_catalog_model(model) {
        return None;
    }

    let provider_id = referenced_provider_id(model)?;
    let remote_model_id =
        model.spec.remote_model_id.as_deref().map(str::trim).filter(|value| !value.is_empty());
    if remote_model_id.is_none() {
        warn!(
            model_id = %model.id,
            provider_id = %provider_id,
            "cloud model is missing remote_model_id; hiding from chat picker"
        );
        return None;
    }
    let Some(provider) = providers.get(&provider_id) else {
        warn!(
            model_id = %model.id,
            provider_id = %provider_id,
            "cloud model references unknown provider; hiding from chat picker"
        );
        return None;
    };

    Some(ChatModelOption {
        id: model.id.clone(),
        display_name: model.display_name.clone(),
        source: ChatModelSource::Cloud,
        downloaded: true,
        pending: false,
        backend_id: None,
        provider_id: Some(provider_id),
        provider_name: Some(provider.name.clone()),
    })
}

fn resolve_cloud_catalog_model(
    providers: &BTreeMap<String, CloudProviderConfig>,
    model: &UnifiedModel,
) -> Result<ResolvedCloudModel, ServerError> {
    let provider_id = referenced_provider_id(model).ok_or_else(|| {
        ServerError::BadRequest(format!("cloud model '{}' is missing provider reference", model.id))
    })?;
    let provider = providers.get(&provider_id).ok_or_else(|| {
        ServerError::BadRequest(format!(
            "cloud model '{}' references unknown provider '{}'",
            model.id, provider_id
        ))
    })?;
    let api_key = resolve_provider_api_key(provider)?;
    let remote_model = model
        .spec
        .remote_model_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ServerError::BadRequest(format!(
                "cloud model '{}' is missing remote_model_id",
                model.id
            ))
        })?
        .to_owned();

    Ok(ResolvedCloudModel {
        provider_id: provider_id.clone(),
        provider_name: provider.name.clone(),
        api_base: provider.api_base.clone(),
        api_key,
        remote_model,
    })
}

async fn find_cloud_catalog_model(
    state: &ModelState,
    requested_model: &str,
) -> Result<Option<UnifiedModel>, ServerError> {
    if let Some((provider_id, legacy_model_id)) = parse_legacy_cloud_option_id(requested_model) {
        let records = state.store().list_models().await?;
        for record in records {
            let model: UnifiedModel = match record.try_into() {
                Ok(model) => model,
                Err(error) => {
                    warn!(error = %error, "failed to deserialize cloud model record; skipping");
                    continue;
                }
            };
            if model_matches_legacy_cloud_option(&model, provider_id, legacy_model_id) {
                return Ok(Some(model));
            }
        }
        return Ok(None);
    }

    let Some(record) = state.store().get_model(requested_model).await? else {
        return Ok(None);
    };
    let model: UnifiedModel =
        record.try_into().map_err(|error: String| ServerError::Internal(error))?;
    if is_cloud_catalog_model(&model) { Ok(Some(model)) } else { Ok(None) }
}

fn parse_legacy_cloud_option_id(model_id: &str) -> Option<(&str, &str)> {
    let suffix = model_id.strip_prefix("cloud/")?;
    let (provider_id, legacy_model_id) = suffix.split_once('/')?;
    let provider_id = provider_id.trim();
    let legacy_model_id = legacy_model_id.trim();
    if provider_id.is_empty() || legacy_model_id.is_empty() {
        return None;
    }
    Some((provider_id, legacy_model_id))
}

fn model_matches_legacy_cloud_option(
    model: &UnifiedModel,
    provider_id: &str,
    legacy_model_id: &str,
) -> bool {
    if !is_cloud_catalog_model(model) {
        return false;
    }

    let Some(model_provider_id) = referenced_provider_id(model) else {
        return false;
    };
    if model_provider_id != provider_id {
        return false;
    }

    model.id == legacy_model_id
        || model
            .spec
            .remote_model_id
            .as_deref()
            .map(str::trim)
            .is_some_and(|remote_model_id| remote_model_id == legacy_model_id)
}

fn map_genai_error(action: &str, err: genai::Error) -> ServerError {
    log_genai_error(action, &err);

    let detail = err.to_string();
    let lower = detail.to_ascii_lowercase();
    if lower.contains("400")
        || lower.contains("bad request")
        || lower.contains("invalid")
        || lower.contains("not found")
    {
        return ServerError::BadRequest(format!("cloud {action} failed: {detail}"));
    }
    ServerError::BackendNotReady(format!("cloud {action} failed: {detail}"))
}

fn build_genai_client_for_target(target: &ResolvedCloudModel) -> GenaiClient {
    let endpoint = ensure_genai_endpoint_base(&target.api_base);
    let api_key = target.api_key.clone();
    let remote_model = target.remote_model.clone();

    let resolver = ServiceTargetResolver::from_resolver_fn(
        move |_service_target: GenaiServiceTarget| -> Result<GenaiServiceTarget, genai::resolver::Error> {
            Ok(GenaiServiceTarget {
                endpoint: Endpoint::from_owned(endpoint.clone()),
                auth: AuthData::from_single(api_key.clone()),
                model: GenaiModelIden::new(AdapterKind::OpenAI, remote_model.clone()),
            })
        },
    );

    GenaiClient::builder().with_service_target_resolver(resolver).build()
}

fn build_genai_chat_request(messages: &[DomainConversationMessage]) -> GenaiChatRequest {
    let mapped: Vec<GenaiChatMessage> =
        messages.iter().map(conversation_message_to_genai).collect();
    GenaiChatRequest::new(mapped)
}

fn conversation_message_to_genai(message: &DomainConversationMessage) -> GenaiChatMessage {
    let rendered = message.rendered_text();
    match message.role.as_str() {
        "system" | "developer" => GenaiChatMessage::system(rendered),
        "assistant" => GenaiChatMessage::assistant(rendered),
        _ => GenaiChatMessage::user(rendered),
    }
}

fn render_messages_for_usage(messages: &[DomainConversationMessage]) -> String {
    messages
        .iter()
        .map(|message| {
            format!("{}: {}", normalize_openai_role(&message.role), message.rendered_text())
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn build_genai_chat_options(
    config: &CloudChatRequestConfig,
    capture_raw_body: bool,
) -> GenaiChatOptions {
    let mut options = GenaiChatOptions::default()
        .with_max_tokens(config.max_tokens)
        .with_temperature(f64::from(config.temperature));

    if let Some(top_p) = config.top_p {
        options = options.with_top_p(f64::from(top_p));
    }
    if let Some(reasoning_effort) = config.reasoning_effort {
        options = options.with_reasoning_effort(map_reasoning_effort(reasoning_effort));
    }
    if let Some(verbosity) = config.verbosity {
        options = options.with_verbosity(map_verbosity(verbosity));
    }
    if let Some(response_format) =
        structured_output_to_genai_response_format(config.structured_output.as_ref())
    {
        options = options.with_response_format(response_format);
    }

    if capture_raw_body { options.with_capture_raw_body(true) } else { options }
}

fn map_reasoning_effort(value: ChatReasoningEffort) -> GenaiReasoningEffort {
    match value {
        ChatReasoningEffort::None => GenaiReasoningEffort::None,
        ChatReasoningEffort::Low => GenaiReasoningEffort::Low,
        ChatReasoningEffort::Medium => GenaiReasoningEffort::Medium,
        ChatReasoningEffort::High => GenaiReasoningEffort::High,
        ChatReasoningEffort::Minimal => GenaiReasoningEffort::Minimal,
    }
}

fn map_verbosity(value: ChatVerbosity) -> GenaiVerbosity {
    match value {
        ChatVerbosity::Low => GenaiVerbosity::Low,
        ChatVerbosity::Medium => GenaiVerbosity::Medium,
        ChatVerbosity::High => GenaiVerbosity::High,
    }
}

async fn cloud_chat_completion(
    target: &ResolvedCloudModel,
    messages: &[DomainConversationMessage],
    config: CloudChatRequestConfig,
    trace_http: bool,
) -> Result<TextGenerationResponse, ServerError> {
    debug!(
        provider_id = %target.provider_id,
        provider_name = %target.provider_name,
        remote_model = %target.remote_model,
        api_base = %target.api_base,
        "sending cloud chat completion request via genai"
    );

    let trace = trace_http.then(|| build_cloud_http_trace_context(target, messages, &config));
    if let Some(trace) = trace.as_ref() {
        log_cloud_http_request(target, trace, false);
    }

    let client = build_genai_client_for_target(target);
    let request = build_genai_chat_request(messages);
    let options = build_genai_chat_options(&config, trace_http);

    let response =
        client.exec_chat(&target.remote_model, request, Some(&options)).await.map_err(|error| {
            if let Some(trace) = trace.as_ref() {
                log_cloud_http_response_error(target, trace, &error);
            }
            map_genai_error("chat", error)
        })?;

    if let Some(trace) = trace.as_ref() {
        log_cloud_http_response_success(target, trace, response.captured_raw_body.as_ref());
    }

    let text = response.first_text().map(str::to_owned).ok_or_else(|| {
        ServerError::Internal("cloud response has empty assistant content".to_owned())
    })?;
    let usage = super::build_estimated_usage(&render_messages_for_usage(messages), &text, None);
    let finish_reason =
        super::finish_reason_from_token_budget(usage.completion_tokens, config.max_tokens);
    let mut metadata = slab_types::inference::JsonOptions::default();
    if let Some(reasoning) =
        extract_reasoning_content_from_raw_body(response.captured_raw_body.as_ref())
    {
        metadata.insert("reasoning_content".into(), Value::String(reasoning));
    }

    Ok(TextGenerationResponse {
        text,
        finish_reason: Some(finish_reason),
        tokens_used: Some(usage.completion_tokens),
        usage: Some(usage),
        metadata,
    })
}

async fn cloud_chat_stream(
    target: &ResolvedCloudModel,
    messages: &[DomainConversationMessage],
    config: CloudChatRequestConfig,
    trace_http: bool,
) -> Result<CloudTokenStream, ServerError> {
    info!(
        provider_id = %target.provider_id,
        provider_name = %target.provider_name,
        remote_model = %target.remote_model,
        api_base = %target.api_base,
        "opening cloud chat stream via genai"
    );

    let trace = trace_http.then(|| build_cloud_http_trace_context(target, messages, &config));
    if let Some(trace) = trace.as_ref() {
        log_cloud_http_request(target, trace, true);
    }

    let client = build_genai_client_for_target(target);
    let request = build_genai_chat_request(messages);
    let options = build_genai_chat_options(&config, false);
    let response = client
        .exec_chat_stream(&target.remote_model, request, Some(&options))
        .await
        .map_err(|error| {
            if let Some(trace) = trace.as_ref() {
                log_cloud_http_response_error(target, trace, &error);
            }
            map_genai_error("chat_stream", error)
        })?;

    let trace_target = target.clone();
    let stream = response.stream.filter_map(move |item| {
        let mapped = match item {
            Ok(GenaiChatStreamEvent::Chunk(chunk)) => {
                let token = chunk.content;
                if token.is_empty() { None } else { Some(Ok(CloudDelta::Content(token))) }
            }
            Ok(GenaiChatStreamEvent::ReasoningChunk(chunk)) => {
                let token = chunk.content;
                if token.is_empty() { None } else { Some(Ok(CloudDelta::Reasoning(token))) }
            }
            Ok(GenaiChatStreamEvent::ToolCallChunk(_))
            | Ok(GenaiChatStreamEvent::ThoughtSignatureChunk(_))
            | Ok(GenaiChatStreamEvent::Start)
            | Ok(GenaiChatStreamEvent::End(_)) => None,
            Err(error) => {
                if let Some(trace) = trace.as_ref() {
                    log_cloud_http_response_error(&trace_target, trace, &error);
                }
                Some(Err(map_genai_error("chat_stream", error)))
            }
        };
        futures::future::ready(mapped)
    });

    Ok(Box::pin(stream))
}

fn ensure_genai_endpoint_base(api_base: &str) -> String {
    match api_base.trim().split_once('?') {
        Some((base, query)) => format!("{}/?{query}", base.trim_end_matches('/')),
        None => format!("{}/", api_base.trim().trim_end_matches('/')),
    }
}

fn build_openai_chat_completions_url(api_base: &str) -> String {
    match api_base.trim().split_once('?') {
        Some((base, query)) => format!("{}/chat/completions?{query}", base.trim_end_matches('/')),
        None => format!("{}/chat/completions", api_base.trim().trim_end_matches('/')),
    }
}

fn extract_reasoning_content_from_raw_body(raw_body: Option<&Value>) -> Option<String> {
    let payload = raw_body?;
    payload
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("reasoning_content"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn build_cloud_http_trace_context(
    target: &ResolvedCloudModel,
    messages: &[DomainConversationMessage],
    config: &CloudChatRequestConfig,
) -> CloudHttpTraceContext {
    let request_headers = redact_headers(build_cloud_http_request_headers(target));
    let request_body =
        serde_json::to_string_pretty(&build_cloud_http_request_body(target, messages, config))
            .unwrap_or_else(|_| "<failed to serialize request body>".to_owned());

    CloudHttpTraceContext {
        request_id: Uuid::new_v4().to_string(),
        request_url: build_openai_chat_completions_url(&target.api_base),
        request_headers: serde_json::to_string_pretty(&request_headers)
            .unwrap_or_else(|_| "<failed to serialize request headers>".to_owned()),
        request_body,
    }
}

fn build_cloud_http_request_headers(target: &ResolvedCloudModel) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("authorization".to_owned(), format!("Bearer {}", target.api_key)),
        ("content-type".to_owned(), "application/json".to_owned()),
    ])
}

fn build_cloud_http_request_body(
    target: &ResolvedCloudModel,
    messages: &[DomainConversationMessage],
    config: &CloudChatRequestConfig,
) -> Value {
    let mut payload = json!({
        "model": target.remote_model,
        "messages": messages
            .iter()
            .map(|message| {
                json!({
                    "role": normalize_openai_role(&message.role),
                    "content": message.rendered_text(),
                })
            })
            .collect::<Vec<_>>(),
        "stream": config.stream,
        "max_tokens": config.max_tokens,
        "temperature": f64::from(config.temperature),
    });

    if let Some(top_p) = config.top_p {
        payload["top_p"] = json!(f64::from(top_p));
    }
    if let Some(reasoning_effort) = config.reasoning_effort {
        payload["reasoning_effort"] = json!(reasoning_effort.as_str());
    }
    if let Some(verbosity) = config.verbosity {
        payload["verbosity"] = json!(verbosity.as_str());
    }
    if let Some(response_format) =
        structured_output_to_openai_response_format(config.structured_output.as_ref())
    {
        payload["response_format"] = response_format;
    }

    payload
}

fn structured_output_to_genai_response_format(
    value: Option<&StructuredOutput>,
) -> Option<GenaiChatResponseFormat> {
    match value {
        Some(StructuredOutput::JsonObject) => Some(GenaiChatResponseFormat::JsonMode),
        Some(StructuredOutput::JsonSchema(schema)) => {
            let mut spec = GenaiJsonSpec::new(schema.name.clone(), schema.schema.clone());
            if let Some(description) = schema.description.as_deref() {
                spec = spec.with_description(description);
            }
            Some(spec.into())
        }
        None => None,
    }
}

fn structured_output_to_openai_response_format(value: Option<&StructuredOutput>) -> Option<Value> {
    match value {
        Some(StructuredOutput::JsonObject) => Some(json!({ "type": "json_object" })),
        Some(StructuredOutput::JsonSchema(schema)) => Some(json!({
            "type": "json_schema",
            "json_schema": {
                "name": schema.name,
                "strict": true,
                "schema": enforce_closed_object_schema(&schema.schema),
            }
        })),
        None => None,
    }
}

fn enforce_closed_object_schema(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut normalized = map
                .iter()
                .map(|(key, value)| (key.clone(), enforce_closed_object_schema(value)))
                .collect::<serde_json::Map<_, _>>();
            if normalized.get("type").and_then(Value::as_str) == Some("object") {
                normalized.insert("additionalProperties".to_owned(), Value::Bool(false));
            }
            Value::Object(normalized)
        }
        Value::Array(values) => {
            Value::Array(values.iter().map(enforce_closed_object_schema).collect())
        }
        _ => value.clone(),
    }
}

fn normalize_openai_role(role: &str) -> &str {
    match role {
        "system" | "assistant" | "user" | "tool" | "developer" => role,
        _ => "user",
    }
}

fn log_cloud_http_request(
    target: &ResolvedCloudModel,
    trace: &CloudHttpTraceContext,
    stream: bool,
) {
    info!(
        cloud_http_trace = true,
        request_id = %trace.request_id,
        provider_id = %target.provider_id,
        provider_name = %target.provider_name,
        remote_model = %target.remote_model,
        request_method = "POST",
        request_url = %trace.request_url,
        request_stream = stream,
        request_headers = %trace.request_headers,
        request_body = %trace.request_body,
        "cloud provider request prepared"
    );
}

fn log_cloud_http_response_success(
    target: &ResolvedCloudModel,
    trace: &CloudHttpTraceContext,
    response_body: Option<&Value>,
) {
    let response_body = response_body
        .map(redact_json_value)
        .and_then(|body| serde_json::to_string_pretty(&body).ok())
        .unwrap_or_else(|| "<raw response body not captured by genai>".to_owned());

    info!(
        cloud_http_trace = true,
        request_id = %trace.request_id,
        provider_id = %target.provider_id,
        provider_name = %target.provider_name,
        remote_model = %target.remote_model,
        response_body = %response_body,
        "cloud provider response received"
    );
}

fn log_cloud_http_response_error(
    target: &ResolvedCloudModel,
    trace: &CloudHttpTraceContext,
    err: &genai::Error,
) {
    match err {
        genai::Error::WebModelCall { webc_error, .. }
        | genai::Error::WebAdapterCall { webc_error, .. } => match webc_error {
            genai::webc::Error::ResponseFailedStatus { status, body, headers } => {
                let headers = redact_header_map(headers.as_ref());
                let body = redact_text_body(body);
                error!(
                    cloud_http_trace = true,
                    request_id = %trace.request_id,
                    provider_id = %target.provider_id,
                    provider_name = %target.provider_name,
                    remote_model = %target.remote_model,
                    response_status = status.as_u16(),
                    response_headers = %headers,
                    response_body = %body,
                    "cloud provider request failed"
                );
            }
            other => {
                error!(
                    cloud_http_trace = true,
                    request_id = %trace.request_id,
                    provider_id = %target.provider_id,
                    provider_name = %target.provider_name,
                    remote_model = %target.remote_model,
                    error = %other,
                    "cloud provider request failed before a structured HTTP response was available"
                );
            }
        },
        genai::Error::HttpError { status, canonical_reason, body } => {
            error!(
                cloud_http_trace = true,
                request_id = %trace.request_id,
                provider_id = %target.provider_id,
                provider_name = %target.provider_name,
                remote_model = %target.remote_model,
                response_status = status.as_u16(),
                response_reason = %canonical_reason,
                response_body = %redact_text_body(body),
                "cloud provider stream request failed"
            );
        }
        other => {
            error!(
                cloud_http_trace = true,
                request_id = %trace.request_id,
                provider_id = %target.provider_id,
                provider_name = %target.provider_name,
                remote_model = %target.remote_model,
                error = %other,
                "cloud provider request failed"
            );
        }
    }
}

fn log_genai_error(action: &str, err: &genai::Error) {
    match err {
        genai::Error::WebModelCall { model_iden, webc_error } => {
            warn!(
                action,
                model = %model_iden,
                error = %webc_error,
                "genai web model call failed"
            );
        }
        genai::Error::WebAdapterCall { adapter_kind, webc_error } => {
            warn!(
                action,
                adapter = ?adapter_kind,
                error = %webc_error,
                "genai web adapter call failed"
            );
        }
        genai::Error::HttpError { status, canonical_reason, body } => {
            warn!(
                action,
                response_status = status.as_u16(),
                response_reason = %canonical_reason,
                response_body = %redact_text_body(body),
                "genai HTTP error"
            );
        }
        _ => {}
    }
}

fn redact_headers(headers: BTreeMap<String, String>) -> BTreeMap<String, String> {
    headers
        .into_iter()
        .map(|(name, value)| {
            let redacted = redact_header_value(&name, &value);
            (name, redacted)
        })
        .collect()
}

fn redact_header_map(headers: &reqwest::header::HeaderMap) -> String {
    let redacted = headers
        .iter()
        .map(|(name, value)| {
            let value = value.to_str().unwrap_or("<non-utf8>");
            (name.as_str().to_owned(), redact_header_value(name.as_str(), value))
        })
        .collect::<BTreeMap<_, _>>();

    serde_json::to_string_pretty(&redacted)
        .unwrap_or_else(|_| "<failed to serialize response headers>".to_owned())
}

fn redact_header_value(name: &str, value: &str) -> String {
    if !header_is_sensitive(name) {
        return value.to_owned();
    }

    if name.eq_ignore_ascii_case("authorization") {
        return value
            .strip_prefix("Bearer ")
            .map(|secret| format!("Bearer {}", redact_secret(secret)))
            .unwrap_or_else(|| redact_secret(value));
    }

    redact_secret(value)
}

fn header_is_sensitive(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower == "authorization"
        || lower == "proxy-authorization"
        || lower == "cookie"
        || lower == "set-cookie"
        || lower.contains("api-key")
        || lower.contains("token")
        || lower.contains("secret")
}

fn redact_secret(value: &str) -> String {
    if value.is_empty() {
        return "<redacted>".to_owned();
    }

    let len = value.chars().count();
    let prefix: String = value.chars().take(4).collect();
    let suffix: String = value.chars().rev().take(2).collect::<String>().chars().rev().collect();

    format!("{prefix}...{suffix} (redacted,len={len})")
}

fn redact_text_body(body: &str) -> String {
    serde_json::from_str::<Value>(body)
        .map(|value| redact_json_value(&value))
        .and_then(|value| serde_json::to_string_pretty(&value))
        .unwrap_or_else(|_| body.to_owned())
}

fn redact_json_value(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let redacted = map
                .iter()
                .map(|(key, value)| {
                    if json_key_is_sensitive(key) {
                        (key.clone(), Value::String(redact_secret_json(value)))
                    } else {
                        (key.clone(), redact_json_value(value))
                    }
                })
                .collect();
            Value::Object(redacted)
        }
        Value::Array(items) => Value::Array(items.iter().map(redact_json_value).collect()),
        _ => value.clone(),
    }
}

fn json_key_is_sensitive(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    lower.contains("api_key")
        || lower.contains("apikey")
        || lower.contains("token")
        || lower.contains("secret")
        || lower == "authorization"
}

fn redact_secret_json(value: &Value) -> String {
    match value {
        Value::String(text) => redact_secret(text),
        Value::Null => "<redacted>".to_owned(),
        other => format!("<redacted:{other}>"),
    }
}

#[cfg(test)]
mod test {
    use super::{
        CloudChatRequestConfig, GenaiChatResponseFormat, ResolvedCloudModel,
        build_cloud_http_request_body, build_openai_chat_completions_url, cloud_option_id,
        ensure_genai_endpoint_base, redact_header_value,
        structured_output_to_genai_response_format,
    };
    use crate::domain::models::{
        ConversationMessage as DomainConversationMessage, ConversationMessageContent,
        StructuredOutput, StructuredOutputJsonSchema,
    };
    use serde_json::json;

    #[test]
    fn cloud_option_id_has_prefix() {
        assert_eq!(cloud_option_id("openai", "gpt-4.1"), "cloud/openai/gpt-4.1");
    }

    #[test]
    fn ensure_genai_endpoint_base_keeps_v1_path() {
        assert_eq!(
            ensure_genai_endpoint_base("https://api.openai.com/v1"),
            "https://api.openai.com/v1/"
        );
    }

    #[test]
    fn build_openai_chat_completions_url_keeps_v1_path() {
        assert_eq!(
            build_openai_chat_completions_url("https://api.openai.com/v1"),
            "https://api.openai.com/v1/chat/completions"
        );
    }

    #[test]
    fn redact_authorization_header() {
        assert_eq!(
            redact_header_value("authorization", "Bearer secret-token-value"),
            "Bearer secr...ue (redacted,len=18)"
        );
    }

    #[test]
    fn structured_output_json_object_maps_to_genai_json_mode() {
        let mapped =
            structured_output_to_genai_response_format(Some(&StructuredOutput::JsonObject));

        assert!(matches!(mapped, Some(GenaiChatResponseFormat::JsonMode)));
    }

    #[test]
    fn cloud_http_request_body_includes_json_schema_response_format() {
        let payload = build_cloud_http_request_body(
            &make_target(),
            &[make_message("user", "hello")],
            &CloudChatRequestConfig {
                max_tokens: 64,
                temperature: 0.7,
                top_p: None,
                structured_output: Some(StructuredOutput::JsonSchema(StructuredOutputJsonSchema {
                    name: "example_schema".to_owned(),
                    description: Some("example".to_owned()),
                    strict: Some(true),
                    schema: json!({
                        "type": "object",
                        "properties": {
                            "answer": { "type": "string" }
                        }
                    }),
                })),
                reasoning_effort: None,
                verbosity: None,
                stream: false,
                include_usage: false,
            },
        );

        assert_eq!(payload["response_format"]["type"], "json_schema");
        assert_eq!(payload["response_format"]["json_schema"]["name"], "example_schema");
        assert_eq!(payload["response_format"]["json_schema"]["strict"], true);
        assert_eq!(
            payload["response_format"]["json_schema"]["schema"]["additionalProperties"],
            false
        );
    }

    fn make_target() -> ResolvedCloudModel {
        ResolvedCloudModel {
            provider_id: "openai".to_owned(),
            provider_name: "OpenAI".to_owned(),
            api_base: "https://api.openai.com/v1".to_owned(),
            api_key: "secret".to_owned(),
            remote_model: "gpt-4.1-mini".to_owned(),
        }
    }

    fn make_message(role: &str, text: &str) -> DomainConversationMessage {
        DomainConversationMessage {
            role: role.to_owned(),
            content: ConversationMessageContent::Text(text.to_owned()),
            name: None,
            tool_call_id: None,
            tool_calls: Vec::new(),
        }
    }
}
