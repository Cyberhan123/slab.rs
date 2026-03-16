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
use tracing::{debug, warn};
use uuid::Uuid;

use crate::context::ModelState;
use crate::domain::models::{
    ChatModelOption, ChatModelSource, ChatStreamChunk, CloudProviderSettingValue,
    ConversationMessage as DomainConversationMessage,
};
use crate::error::ServerError;

use super::GeneratedChatOutput;

type CloudProviderConfig = CloudProviderSettingValue;

#[derive(Debug, Clone)]
struct ResolvedCloudModel {
    provider_id: String,
    provider_name: String,
    api_base: String,
    api_key: String,
    remote_model: String,
}

#[derive(Debug)]
enum CloudDelta {
    Content(String),
    Reasoning(String),
}

type CloudTokenStream =
    std::pin::Pin<Box<dyn futures::Stream<Item = Result<CloudDelta, ServerError>> + Send>>;

pub(super) async fn list_chat_models(state: &ModelState) -> Vec<ChatModelOption> {
    let mut items = Vec::new();

    for provider in load_cloud_providers_lenient(state).await {
        for model in provider.models {
            items.push(ChatModelOption {
                id: cloud_option_id(&provider.id, &model.id),
                display_name: model.display_name,
                source: ChatModelSource::Cloud,
                provider_id: Some(provider.id.clone()),
                provider_name: Some(provider.name.clone()),
                backend_id: None,
                downloaded: true,
                pending: false,
            });
        }
    }

    items.sort_by(|left, right| left.display_name.cmp(&right.display_name));
    items
}

pub(super) fn is_cloud_model_option_id(model_id: &str) -> bool {
    model_id
        .strip_prefix(super::CLOUD_MODEL_ID_PREFIX)
        .is_some_and(|suffix| suffix.starts_with('/'))
}

pub(super) async fn create_chat_completion(
    state: &ModelState,
    requested_model: &str,
    messages: &[DomainConversationMessage],
    max_tokens: u32,
    temperature: f32,
    stream: bool,
) -> Result<GeneratedChatOutput, ServerError> {
    let target = resolve_cloud_model(state, requested_model).await?;

    if stream {
        let backend_stream = cloud_chat_stream(&target, messages, max_tokens, temperature).await?;
        let completion_id = format!("chatcmpl-{}", Uuid::new_v4());
        let created_ts = chrono::Utc::now().timestamp();
        let model_name = requested_model.to_owned();

        let token_stream = backend_stream.map(move |chunk| -> ChatStreamChunk {
            match chunk {
                Ok(CloudDelta::Content(token)) => ChatStreamChunk::Data(super::build_chunk(
                    &completion_id,
                    created_ts,
                    &model_name,
                    &token,
                )),
                Ok(CloudDelta::Reasoning(token)) => ChatStreamChunk::Data(
                    super::build_reasoning_chunk(&completion_id, created_ts, &model_name, &token),
                ),
                Err(error) => ChatStreamChunk::Comment(error.to_string()),
            }
        });

        let sse_stream = token_stream.chain(stream::once(async {
            ChatStreamChunk::Data("[DONE]".into())
        }));

        return Ok(GeneratedChatOutput::Stream(Box::pin(sse_stream)));
    }

    let generated = cloud_chat_completion(&target, messages, max_tokens, temperature).await?;
    Ok(GeneratedChatOutput::Text(generated))
}

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

async fn load_cloud_providers_lenient(state: &ModelState) -> Vec<CloudProviderConfig> {
    match load_cloud_providers_strict(state).await {
        Ok(providers) => providers,
        Err(error) => {
            warn!(error = %error, "invalid chat cloud provider settings; cloud models disabled");
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
        "cloud provider '{}' is missing api key (set settings api_key or api_key_env)",
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
        .map_err(|error| map_genai_error("chat", error))?;

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
        .map_err(|error| map_genai_error("chat_stream", error))?;

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
            Err(error) => Some(Err(map_genai_error("chat_stream", error))),
        };
        futures::future::ready(mapped)
    });

    Ok(Box::pin(stream))
}

#[cfg(test)]
mod test {
    use super::cloud_option_id;

    #[test]
    fn cloud_option_id_has_prefix() {
        assert_eq!(cloud_option_id("openai", "gpt-4.1"), "cloud/openai/gpt-4.1");
    }
}
