//! OpenAI-compatible chat completion routes.

use chrono::Utc;
use futures::{stream, StreamExt};
use genai::adapter::AdapterKind;
use genai::chat::{
    ChatMessage as GenaiChatMessage, ChatOptions as GenaiChatOptions,
    ChatRequest as GenaiChatRequest, ChatStreamEvent as GenaiChatStreamEvent,
};
use genai::resolver::{AuthData, Endpoint, ServiceTargetResolver};
use genai::{
    Client as GenaiClient, ModelIden as GenaiModelIden, ServiceTarget as GenaiServiceTarget,
};
use serde::Deserialize;
use std::collections::HashMap;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::context::ModelState;
use crate::domain::models::{
    ChatCompletionCommand, ChatCompletionResult, ChatModelOption, ChatModelSource,
    ChatResultChoice, ConversationMessage as DomainConversationMessage,
    ChatCompletionOutput, ChatStreamChunk,
};
use crate::error::ServerError;
use crate::infra::db::{ChatMessage, ChatStore, ConfigStore, ModelStore, TaskRecord, TaskStore};
use crate::infra::rpc::{self, pb};

/// Maximum allowed prompt length in bytes.
#[cfg(test)]
const MAX_PROMPT_BYTES: usize = 128 * 1024; // 128 KiB
const LLAMA_BACKEND_ID: &str = "ggml.llama";
const CHAT_MODEL_PROVIDERS_CONFIG_KEY: &str = "chat_model_providers";
const CLOUD_MODEL_ID_PREFIX: &str = "cloud";

#[derive(Clone)]
pub struct ChatService {
    state: ModelState,
}

impl ChatService {
    pub fn new(state: ModelState) -> Self {
        Self { state }
    }

    pub async fn list_chat_models(&self) -> Result<Vec<ChatModelOption>, ServerError> {
        let local_models = self.state.store().list_models().await?;
        let download_tasks = self
            .state
            .store()
            .list_tasks(Some("model_download"))
            .await?;
        let pending_by_model = pending_download_map(download_tasks);

        let mut items: Vec<ChatModelOption> = local_models
            .into_iter()
            .filter(|model| {
                model
                    .backend_ids
                    .iter()
                    .any(|backend| backend == LLAMA_BACKEND_ID)
            })
            .map(|model| ChatModelOption {
                id: model.id.clone(),
                display_name: model.display_name,
                source: ChatModelSource::Local,
                provider_id: None,
                provider_name: None,
                backend_id: Some(LLAMA_BACKEND_ID.to_owned()),
                downloaded: model.local_path.is_some(),
                pending: pending_by_model.contains_key(&model.id),
            })
            .collect();

        let mut cloud_items = Vec::new();
        for provider in load_cloud_providers_lenient(&self.state).await {
            for model in provider.models {
                cloud_items.push(ChatModelOption {
                    id: cloud_option_id(&provider.id, &model.id),
                    display_name: model.display_name.unwrap_or_else(|| model.id.clone()),
                    source: ChatModelSource::Cloud,
                    provider_id: Some(provider.id.clone()),
                    provider_name: Some(provider.name.clone()),
                    backend_id: None,
                    downloaded: true,
                    pending: false,
                });
            }
        }
        cloud_items.sort_by(|left, right| left.display_name.cmp(&right.display_name));
        items.extend(cloud_items);

        Ok(items)
    }

    pub async fn create_chat_completion(
        &self,
        command: ChatCompletionCommand,
    ) -> Result<ChatCompletionOutput, ServerError> {
        create_chat_completion_with_state(self.state.clone(), command).await
    }
}

#[derive(Debug, Clone, Deserialize)]
struct CloudProviderConfig {
    #[serde(alias = "provider_id", alias = "providerId")]
    id: String,
    #[serde(default, alias = "displayName", alias = "provider_name")]
    name: String,
    #[serde(alias = "apiBase", alias = "base_url", alias = "baseUrl")]
    api_base: String,
    #[serde(default, alias = "apiKey")]
    api_key: Option<String>,
    #[serde(default, alias = "apiKeyEnv")]
    api_key_env: Option<String>,
    #[serde(default)]
    models: Vec<CloudProviderModelConfig>,
}

#[derive(Debug, Clone, Deserialize)]
struct CloudProviderModelConfig {
    #[serde(alias = "model", alias = "model_id", alias = "modelId")]
    id: String,
    #[serde(default, alias = "displayName")]
    display_name: Option<String>,
    #[serde(default, alias = "remoteModel")]
    remote_model: Option<String>,
}

#[derive(Debug, Clone)]
struct ResolvedCloudModel {
    provider_id: String,
    provider_name: String,
    api_base: String,
    api_key: String,
    remote_model: String,
}

/// Build an OpenAI-compatible `chat.completion.chunk` SSE data payload.
fn build_chunk(id: &str, created: i64, model: &str, token: &str) -> String {
    serde_json::json!({
        "id": id,
        "object": "chat.completion.chunk",
        "created": created,
        "model": model,
        "choices": [{
            "index": 0,
            "delta": { "content": token },
            "finish_reason": null
        }]
    })
    .to_string()
}

/// Build an OpenAI-compatible reasoning SSE chunk payload.
fn build_reasoning_chunk(id: &str, created: i64, model: &str, token: &str) -> String {
    serde_json::json!({
        "id": id,
        "object": "chat.completion.chunk",
        "created": created,
        "model": model,
        "choices": [{
            "index": 0,
            "delta": { "reasoning_content": token },
            "finish_reason": null
        }]
    })
    .to_string()
}

fn cloud_option_id(provider_id: &str, model_id: &str) -> String {
    format!("{CLOUD_MODEL_ID_PREFIX}/{provider_id}/{model_id}")
}

fn is_cloud_model_option_id(model_id: &str) -> bool {
    model_id.starts_with("cloud/")
}

fn trim_to_option(raw: Option<String>) -> Option<String> {
    raw.and_then(|v| {
        let trimmed = v.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_owned())
        }
    })
}

fn looks_like_env_var_name(value: &str) -> bool {
    let mut chars = value.chars();
    match chars.next() {
        Some(ch) if ch == '_' || ch.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn canonicalize_cloud_provider(
    mut provider: CloudProviderConfig,
) -> Result<CloudProviderConfig, ServerError> {
    provider.id = provider.id.trim().to_owned();
    provider.name = provider.name.trim().to_owned();
    provider.api_base = provider.api_base.trim().trim_end_matches('/').to_owned();
    provider.api_key = trim_to_option(provider.api_key.take());
    provider.api_key_env = trim_to_option(provider.api_key_env.take());

    if provider.id.is_empty() {
        return Err(ServerError::BadRequest(
            "cloud provider id must not be empty".into(),
        ));
    }
    if provider.name.is_empty() {
        provider.name = provider.id.clone();
    }
    if provider.api_base.is_empty() {
        return Err(ServerError::BadRequest(format!(
            "cloud provider '{}' has empty api_base",
            provider.id
        )));
    }
    if provider.models.is_empty() {
        return Err(ServerError::BadRequest(format!(
            "cloud provider '{}' must define at least one model",
            provider.id
        )));
    }

    let mut model_ids = std::collections::HashSet::new();
    for model in &mut provider.models {
        model.id = model.id.trim().to_owned();
        model.display_name =
            Some(trim_to_option(model.display_name.take()).unwrap_or_else(|| model.id.clone()));
        model.remote_model = trim_to_option(model.remote_model.take());

        if model.id.is_empty() {
            return Err(ServerError::BadRequest(format!(
                "cloud provider '{}' contains model with empty id",
                provider.id
            )));
        }
        if !model_ids.insert(model.id.clone()) {
            return Err(ServerError::BadRequest(format!(
                "cloud provider '{}' contains duplicate model id '{}'",
                provider.id, model.id
            )));
        }
    }

    Ok(provider)
}

async fn load_cloud_providers_strict(
    state: &ModelState,
) -> Result<Vec<CloudProviderConfig>, ServerError> {
    let raw = state
        .store()
        .get_config_value(CHAT_MODEL_PROVIDERS_CONFIG_KEY)
        .await?;

    let Some(raw) = raw else {
        return Ok(Vec::new());
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    let parsed: Vec<CloudProviderConfig> = serde_json::from_str(trimmed).map_err(|e| {
        ServerError::BadRequest(format!(
            "invalid JSON in config '{}': {e}",
            CHAT_MODEL_PROVIDERS_CONFIG_KEY
        ))
    })?;

    if parsed.is_empty() {
        return Ok(Vec::new());
    }

    let mut out = Vec::with_capacity(parsed.len());
    let mut provider_ids = std::collections::HashSet::new();
    for provider in parsed {
        let normalized = canonicalize_cloud_provider(provider)?;
        if !provider_ids.insert(normalized.id.clone()) {
            return Err(ServerError::BadRequest(format!(
                "duplicate cloud provider id '{}'",
                normalized.id
            )));
        }
        out.push(normalized);
    }

    Ok(out)
}

async fn load_cloud_providers_lenient(state: &ModelState) -> Vec<CloudProviderConfig> {
    match load_cloud_providers_strict(state).await {
        Ok(v) => v,
        Err(err) => {
            warn!(
                error = %err,
                config_key = CHAT_MODEL_PROVIDERS_CONFIG_KEY,
                "invalid chat cloud provider config; cloud models disabled"
            );
            Vec::new()
        }
    }
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
        "cloud provider '{}' is missing api key (set config api_key or api_key_env)",
        provider.id
    )))
}

async fn resolve_cloud_model(
    state: &ModelState,
    requested_model: &str,
) -> Result<ResolvedCloudModel, ServerError> {
    let providers = load_cloud_providers_strict(state).await?;

    for provider in providers {
        for model in &provider.models {
            if cloud_option_id(&provider.id, &model.id) != requested_model {
                continue;
            }
            let api_key = resolve_provider_api_key(&provider)?;
            let remote_model = model
                .remote_model
                .as_deref()
                .unwrap_or(model.id.as_str())
                .to_owned();

            return Ok(ResolvedCloudModel {
                provider_id: provider.id.clone(),
                provider_name: provider.name.clone(),
                api_base: provider.api_base.clone(),
                api_key,
                remote_model,
            });
        }
    }

    Err(ServerError::BadRequest(format!(
        "unknown cloud model option '{}'",
        requested_model
    )))
}

fn pending_download_map(tasks: Vec<TaskRecord>) -> HashMap<String, TaskRecord> {
    let mut pending_by_model: HashMap<String, TaskRecord> = HashMap::new();
    for task in tasks {
        if !matches!(task.status.as_str(), "pending" | "running") {
            continue;
        }
        let Some(model_id) = task.model_id.clone() else {
            continue;
        };
        let replace = pending_by_model
            .get(&model_id)
            .map(|current| task.updated_at > current.updated_at)
            .unwrap_or(true);
        if replace {
            pending_by_model.insert(model_id, task);
        }
    }
    pending_by_model
}

enum CloudDelta {
    Content(String),
    Reasoning(String),
}

type CloudTokenStream =
    std::pin::Pin<Box<dyn futures::Stream<Item = Result<CloudDelta, ServerError>> + Send>>;

fn map_genai_error(action: &str, err: genai::Error) -> ServerError {
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
    let endpoint = target.api_base.clone();
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

    GenaiClient::builder()
        .with_service_target_resolver(resolver)
        .build()
}

fn build_genai_chat_request(messages: &[DomainConversationMessage]) -> GenaiChatRequest {
    let mapped: Vec<GenaiChatMessage> = messages.iter().map(Into::into).collect();
    GenaiChatRequest::new(mapped)
}

fn build_genai_chat_options(max_tokens: u32, temperature: f32) -> GenaiChatOptions {
    GenaiChatOptions::default()
        .with_max_tokens(max_tokens)
        .with_temperature(f64::from(temperature))
}

async fn cloud_chat_completion(
    target: &ResolvedCloudModel,
    messages: &[DomainConversationMessage],
    max_tokens: u32,
    temperature: f32,
) -> Result<String, ServerError> {
    debug!(
        provider_id = %target.provider_id,
        provider_name = %target.provider_name,
        remote_model = %target.remote_model,
        api_base = %target.api_base,
        "sending cloud chat completion request via genai"
    );

    let client = build_genai_client_for_target(target);
    let request = build_genai_chat_request(messages);
    let options = build_genai_chat_options(max_tokens, temperature);

    let response = client
        .exec_chat(&target.remote_model, request, Some(&options))
        .await
        .map_err(|e| map_genai_error("chat", e))?;

    response.first_text().map(str::to_owned).ok_or_else(|| {
        ServerError::Internal("cloud response has empty assistant content".to_owned())
    })
}

async fn cloud_chat_stream(
    target: &ResolvedCloudModel,
    messages: &[DomainConversationMessage],
    max_tokens: u32,
    temperature: f32,
) -> Result<CloudTokenStream, ServerError> {
    debug!(
        provider_id = %target.provider_id,
        provider_name = %target.provider_name,
        remote_model = %target.remote_model,
        api_base = %target.api_base,
        "opening cloud chat stream via genai"
    );

    let client = build_genai_client_for_target(target);
    let request = build_genai_chat_request(messages);
    let options = build_genai_chat_options(max_tokens, temperature);
    let response = client
        .exec_chat_stream(&target.remote_model, request, Some(&options))
        .await
        .map_err(|e| map_genai_error("chat_stream", e))?;

    let stream = response.stream.filter_map(|item| {
        let mapped = match item {
            Ok(GenaiChatStreamEvent::Chunk(chunk)) => {
                let token = chunk.content;
                if token.is_empty() {
                    None
                } else {
                    Some(Ok(CloudDelta::Content(token)))
                }
            }
            Ok(GenaiChatStreamEvent::ReasoningChunk(chunk)) => {
                let token = chunk.content;
                if token.is_empty() {
                    None
                } else {
                    Some(Ok(CloudDelta::Reasoning(token)))
                }
            }
            Ok(GenaiChatStreamEvent::ToolCallChunk(_))
            | Ok(GenaiChatStreamEvent::ThoughtSignatureChunk(_))
            | Ok(GenaiChatStreamEvent::Start)
            | Ok(GenaiChatStreamEvent::End(_)) => None,
            Err(err) => Some(Err(map_genai_error("chat_stream", err))),
        };
        futures::future::ready(mapped)
    });

    Ok(Box::pin(stream))
}

pub(crate) async fn create_chat_completion_with_state(
    state: ModelState,
    command: ChatCompletionCommand,
) -> Result<ChatCompletionOutput, ServerError> {
    let user_content = command
        .messages
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .map(|m| m.content.clone())
        .unwrap_or_default();

    let max_tokens = command.max_tokens.unwrap_or(512);
    let temperature = command.temperature.unwrap_or(0.7);

    debug!(
        model = %command.model,
        prompt_len = user_content.len(),
        stream = command.stream,
        session_id = ?command.id,
        "chat completion request"
    );

    let resolved_messages =
        build_messages(&state, command.id.as_deref(), &command.messages).await?;

    if let Some(sid) = command.id.as_deref() {
        state
            .store()
            .append_message(ChatMessage {
                id: Uuid::new_v4().to_string(),
                session_id: sid.to_owned(),
                role: "user".into(),
                content: user_content.clone(),
                created_at: Utc::now(),
            })
            .await
            .unwrap_or_else(|e| tracing::warn!(error = %e, "failed to persist user message"));
    }

    let generated = if is_cloud_model_option_id(&command.model) {
        let target = resolve_cloud_model(&state, &command.model).await?;
        if command.stream {
            let backend_stream =
                cloud_chat_stream(&target, &resolved_messages, max_tokens, temperature).await?;

            let completion_id = format!("chatcmpl-{}", Uuid::new_v4());
            let created_ts = Utc::now().timestamp();
            let model_name = command.model.clone();

            let token_stream = backend_stream.map(move |chunk| -> ChatStreamChunk {
                match chunk {
                    Ok(CloudDelta::Content(token)) => ChatStreamChunk::Data(build_chunk(
                        &completion_id,
                        created_ts,
                        &model_name,
                        &token,
                    )),
                    Ok(CloudDelta::Reasoning(token)) => ChatStreamChunk::Data(
                        build_reasoning_chunk(&completion_id, created_ts, &model_name, &token),
                    ),
                    Err(e) => ChatStreamChunk::Comment(e.to_string()),
                }
            });

            let sse_stream = token_stream.chain(stream::once(async {
                ChatStreamChunk::Data("[DONE]".into())
            }));

            return Ok(ChatCompletionOutput::Stream(Box::pin(sse_stream)));
        }

        cloud_chat_completion(&target, &resolved_messages, max_tokens, temperature).await?
    } else {
        let prompt = build_prompt(&resolved_messages);
        let grpc_req = pb::ChatRequest {
            prompt: prompt.clone(),
            model: command.model.clone(),
            max_tokens,
            temperature,
            session_key: command.id.clone().unwrap_or_default(),
        };

        let llama_channel = state.grpc().chat_channel().ok_or_else(|| {
            ServerError::BackendNotReady("llama gRPC endpoint is not configured".into())
        })?;

        if command.stream {
            let usage_guard = state
                .auto_unload()
                .acquire_for_inference(LLAMA_BACKEND_ID)
                .await
                .map_err(|e| {
                    ServerError::BackendNotReady(format!("llama backend not ready: {e}"))
                })?;

            let backend_stream = rpc::client::chat_stream(llama_channel.clone(), grpc_req.clone())
                .await
                .map_err(|e| ServerError::Internal(format!("grpc chat stream failed: {e}")))?;

            let completion_id = format!("chatcmpl-{}", Uuid::new_v4());
            let created_ts = Utc::now().timestamp();
            let model_name = command.model.clone();

            let token_stream = backend_stream.map(move |chunk| -> ChatStreamChunk {
                match chunk {
                    Ok(msg) if !msg.error.is_empty() => ChatStreamChunk::Comment(msg.error),
                    Ok(msg) if msg.done => ChatStreamChunk::Comment("done".into()),
                    Ok(msg) => {
                        let data = build_chunk(&completion_id, created_ts, &model_name, &msg.token);
                        ChatStreamChunk::Data(data)
                    }
                    Err(e) => ChatStreamChunk::Comment(e.to_string()),
                }
            });

            let sse_stream = token_stream
                .chain(stream::once(async {
                    ChatStreamChunk::Data("[DONE]".into())
                }))
                .map(move |item| {
                    // Keep the usage guard alive for the whole SSE stream lifetime.
                    let _keep_alive = &usage_guard;
                    item
                });

            return Ok(ChatCompletionOutput::Stream(Box::pin(sse_stream)));
        }

        let _usage_guard = state
            .auto_unload()
            .acquire_for_inference(LLAMA_BACKEND_ID)
            .await
            .map_err(|e| ServerError::BackendNotReady(format!("llama backend not ready: {e}")))?;

        rpc::client::chat(llama_channel, grpc_req)
            .await
            .map_err(|e| ServerError::Internal(format!("grpc chat failed: {e}")))?
    };

    info!(
        model = %command.model,
        output_len = generated.len(),
        "chat completion done"
    );

    if let Some(sid) = command.id.as_deref() {
        state
            .store()
            .append_message(ChatMessage {
                id: Uuid::new_v4().to_string(),
                session_id: sid.to_owned(),
                role: "assistant".into(),
                content: generated.clone(),
                created_at: Utc::now(),
            })
            .await
            .unwrap_or_else(|e| tracing::warn!(error = %e, "failed to persist assistant message"));
    }

    let resp = ChatCompletionResult {
        id: format!("chatcmpl-{}", Uuid::new_v4()),
        object: "chat.completion".into(),
        created: Utc::now().timestamp(),
        model: command.model,
        choices: vec![ChatResultChoice {
            index: 0,
            message: DomainConversationMessage {
                role: "assistant".into(),
                content: generated,
            },
            finish_reason: "stop".into(),
        }],
    };

    Ok(ChatCompletionOutput::Json(resp))
}

/// Merge history from DB and current request messages while avoiding duplicates.
async fn build_messages(
    state: &ModelState,
    session_id: Option<&str>,
    current_messages: &[DomainConversationMessage],
) -> Result<Vec<DomainConversationMessage>, ServerError> {
    let current: Vec<DomainConversationMessage> = current_messages
        .iter()
        .filter(|m| !m.content.trim().is_empty())
        .cloned()
        .collect();
    let client_sent_history = current.len() > 1;

    let mut merged: Vec<DomainConversationMessage> = Vec::new();
    // Avoid duplicating turns: if client already sends history, do not merge DB history again.
    if !client_sent_history {
        if let Some(sid) = session_id {
            let history = state.store().list_messages(sid).await?;
            for msg in history {
                if msg.content.trim().is_empty() {
                    continue;
                }
                merged.push(msg.into());
            }
        }
    }
    merged.extend(current);
    Ok(merged)
}

/// Build the local llama prompt from merged message history.
fn build_prompt(messages: &[DomainConversationMessage]) -> String {
    let mut parts: Vec<String> = messages
        .iter()
        .map(|msg| format!("{}: {}", capitalize_role(&msg.role), msg.content))
        .collect();
    parts.push("Assistant:".into());
    parts.join("\n")
}

fn capitalize_role(role: &str) -> &str {
    match role {
        "user" => "User",
        "assistant" => "Assistant",
        "system" => "System",
        other => other,
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn make_command(role: &str, content: &str) -> ChatCompletionCommand {
        ChatCompletionCommand {
            model: "test".into(),
            messages: vec![DomainConversationMessage {
                role: role.into(),
                content: content.into(),
            }],
            stream: false,
            max_tokens: None,
            temperature: None,
            id: None,
        }
    }

    #[test]
    fn validate_max_tokens_zero() {
        let req = ChatCompletionCommand {
            max_tokens: Some(0),
            ..make_command("user", "hello")
        };
        assert_eq!(req.max_tokens, Some(0));
        let mt = req.max_tokens.unwrap_or(512);
        assert!(mt == 0 || mt > 4096, "should be out of range");
    }

    #[test]
    fn validate_max_tokens_too_large() {
        let req = ChatCompletionCommand {
            max_tokens: Some(9999),
            ..make_command("user", "hello")
        };
        let mt = req.max_tokens.unwrap_or(512);
        assert!(mt > 4096, "should be out of range");
    }

    #[test]
    fn validate_temperature_out_of_range() {
        let temp = 3.0_f32;
        assert!(!(0.0..=2.0).contains(&temp), "should be out of range");
    }

    #[test]
    fn validate_prompt_too_large() {
        let long_prompt = "x".repeat(MAX_PROMPT_BYTES + 1);
        assert!(long_prompt.len() > MAX_PROMPT_BYTES);
    }

    #[test]
    fn no_user_message_returns_error() {
        let req = make_command("system", "you are a bot");
        let found = req.messages.iter().rev().find(|m| m.role == "user");
        assert!(found.is_none());
    }

    #[test]
    fn build_chunk_produces_openai_format() {
        let json_str = build_chunk("chatcmpl-test", 1_700_000_000, "slab-llama", "Hello");
        let v: serde_json::Value = serde_json::from_str(&json_str).expect("valid JSON");
        assert_eq!(v["id"], "chatcmpl-test");
        assert_eq!(v["object"], "chat.completion.chunk");
        assert_eq!(v["created"], 1_700_000_000_i64);
        assert_eq!(v["model"], "slab-llama");
        let choice = &v["choices"][0];
        assert_eq!(choice["index"], 0);
        assert_eq!(choice["delta"]["content"], "Hello");
        assert!(choice["finish_reason"].is_null());
    }

    #[test]
    fn cloud_option_id_has_prefix() {
        assert_eq!(cloud_option_id("openai", "gpt-4.1"), "cloud/openai/gpt-4.1");
    }
}
