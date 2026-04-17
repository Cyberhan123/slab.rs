use futures::StreamExt;
use serde_json::Value;
use slab_types::inference::{TextGenerationUsage, TextPromptTokensDetails};
use slab_types::{JsonOptions, TextGenerationChunk, TextGenerationResponse};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};
use tracing::{Instrument, debug, error, info, instrument, warn};

use slab_proto::{convert, slab::ipc::v1 as pb};

use super::{BackendKind, GrpcServiceImpl, extract_request_id, proto_to_status, runtime_to_status};

const REASONING_CONTENT_METADATA_KEY: &str = "reasoning_content";
const THINK_OPEN_MARKER: &str = "<think";
const THINK_CLOSE_TAG: &str = "</think>";

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedThinkingOutput {
    content: String,
    reasoning: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ThinkingDelta {
    Content(String),
    Reasoning(String),
}

#[derive(Debug, Default)]
struct ThinkingStreamState {
    raw_output: String,
    emitted_content_len: usize,
    emitted_reasoning_len: usize,
    prompt_prefills_open_think: bool,
}

fn trailing_partial_marker_len(raw: &str, marker: &str) -> usize {
    let max = raw.len().min(marker.len().saturating_sub(1));
    (1..=max).rev().find(|len| raw.ends_with(&marker[..*len])).unwrap_or(0)
}

fn normalize_thinking_content_prefix(prefix: &str) -> &str {
    if prefix.trim().is_empty() { "" } else { prefix }
}

fn parse_thinking_output(raw: &str, complete: bool) -> ParsedThinkingOutput {
    let Some(open_start) = raw.find(THINK_OPEN_MARKER) else {
        let stable_end = if complete {
            raw.len()
        } else {
            raw.len().saturating_sub(trailing_partial_marker_len(raw, THINK_OPEN_MARKER))
        };
        // No <think found — treat all text as content.
        let stable_content = &raw[..stable_end];
        let stable_content = if complete || !stable_content.trim().is_empty() {
            stable_content
        } else {
            ""
        };
        return ParsedThinkingOutput {
            content: stable_content.to_owned(),
            reasoning: String::new(),
        };
    };

    let content_prefix = normalize_thinking_content_prefix(&raw[..open_start]).to_owned();
    let after_open_marker = &raw[open_start..];
    let Some(open_end_rel) = after_open_marker.find('>') else {
        return ParsedThinkingOutput {
            content: if complete { raw.to_owned() } else { content_prefix },
            reasoning: String::new(),
        };
    };

    let reasoning_start = open_start + open_end_rel + 1;
    let after_open = &raw[reasoning_start..];
    if let Some(close_rel) = after_open.find(THINK_CLOSE_TAG) {
        let close_start = reasoning_start + close_rel;
        let close_end = close_start + THINK_CLOSE_TAG.len();
        let mut content = content_prefix;
        content.push_str(&raw[close_end..]);
        return ParsedThinkingOutput {
            content,
            reasoning: raw[reasoning_start..close_start].to_owned(),
        };
    }

    let stable_reasoning_end = if complete {
        raw.len()
    } else {
        raw.len().saturating_sub(trailing_partial_marker_len(raw, THINK_CLOSE_TAG))
    };

    ParsedThinkingOutput {
        content: content_prefix,
        reasoning: raw[reasoning_start..stable_reasoning_end].to_owned(),
    }
}

fn prompt_prefills_open_think(prompt: &str) -> bool {
    match (prompt.rfind(THINK_OPEN_MARKER), prompt.rfind(THINK_CLOSE_TAG)) {
        (Some(open), Some(close)) => open > close,
        (Some(_), None) => true,
        _ => false,
    }
}

fn parse_thinking_output_with_context(
    raw: &str,
    complete: bool,
    prompt_prefills_open_think: bool,
) -> ParsedThinkingOutput {
    if prompt_prefills_open_think && raw.find(THINK_OPEN_MARKER).is_none() {
        if let Some(close_start) = raw.find(THINK_CLOSE_TAG) {
            let close_end = close_start + THINK_CLOSE_TAG.len();
            return ParsedThinkingOutput {
                content: raw[close_end..].to_owned(),
                reasoning: raw[..close_start].to_owned(),
            };
        }

        let stable_reasoning_end = if complete {
            raw.len()
        } else {
            raw.len().saturating_sub(trailing_partial_marker_len(raw, THINK_CLOSE_TAG))
        };

        return ParsedThinkingOutput {
            content: String::new(),
            reasoning: raw[..stable_reasoning_end].to_owned(),
        };
    }

    parse_thinking_output(raw, complete)
}

impl ThinkingStreamState {
    fn new(prompt_prefills_open_think: bool) -> Self {
        Self { prompt_prefills_open_think, ..Default::default() }
    }

    fn ingest(&mut self, delta: &str) -> Vec<ThinkingDelta> {
        self.raw_output.push_str(delta);
        self.emit(false)
    }

    fn finish(&mut self) -> Vec<ThinkingDelta> {
        self.emit(true)
    }

    fn emit(&mut self, complete: bool) -> Vec<ThinkingDelta> {
        let parsed = parse_thinking_output_with_context(
            &self.raw_output,
            complete,
            self.prompt_prefills_open_think,
        );
        let mut deltas = Vec::new();

        if parsed.reasoning.len() > self.emitted_reasoning_len {
            deltas.push(ThinkingDelta::Reasoning(
                parsed.reasoning[self.emitted_reasoning_len..].to_owned(),
            ));
            self.emitted_reasoning_len = parsed.reasoning.len();
        }

        if parsed.content.len() > self.emitted_content_len {
            deltas.push(ThinkingDelta::Content(
                parsed.content[self.emitted_content_len..].to_owned(),
            ));
            self.emitted_content_len = parsed.content.len();
        }

        deltas
    }
}

fn attach_reasoning_metadata(
    response: &mut TextGenerationResponse,
    prompt_prefills_open_think: bool,
) {
    let parsed =
        parse_thinking_output_with_context(&response.text, true, prompt_prefills_open_think);
    let reasoning = parsed.reasoning.trim();
    if reasoning.is_empty() {
        return;
    }

    response.text = parsed.content;
    response
        .metadata
        .insert(REASONING_CONTENT_METADATA_KEY.into(), Value::String(reasoning.to_owned()));
}

fn text_chunk(delta: String) -> TextGenerationChunk {
    TextGenerationChunk {
        delta,
        done: false,
        finish_reason: None,
        usage: None,
        metadata: Default::default(),
    }
}

fn reasoning_chunk(delta: String) -> TextGenerationChunk {
    let mut metadata = JsonOptions::default();
    metadata.insert(REASONING_CONTENT_METADATA_KEY.into(), Value::String(delta));
    TextGenerationChunk {
        delta: String::new(),
        done: false,
        finish_reason: None,
        usage: None,
        metadata,
    }
}

#[tonic::async_trait]
impl pb::llama_service_server::LlamaService for GrpcServiceImpl {
    #[instrument(skip_all, fields(request_id, backend = "ggml.llama"))]
    async fn chat(
        &self,
        request: Request<pb::ChatRequest>,
    ) -> Result<Response<pb::ChatResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);

        let req = request.into_inner();
        let prompt_prefills_thinking = prompt_prefills_open_think(&req.prompt);
        debug!(
            prompt_len = req.prompt.len(),
            max_tokens = req.max_tokens,
            "llama chat request received"
        );

        let session = self.session_for_backend(BackendKind::Llama).await?;
        let request = convert::decode_chat_request(&req, false).map_err(proto_to_status)?;
        let mut response = session.run_text_generation(request).await.map_err(|error| {
            error!(error = %error, "llama text generation failed");
            runtime_to_status(error)
        })?;
        attach_reasoning_metadata(&mut response, prompt_prefills_thinking);

        info!(output_len = response.text.len(), "llama chat completed");
        Ok(Response::new(convert::encode_chat_response(&response)))
    }

    type ChatStreamStream = ReceiverStream<Result<pb::ChatStreamChunk, Status>>;

    #[instrument(skip_all, fields(request_id, backend = "ggml.llama"))]
    async fn chat_stream(
        &self,
        request: Request<pb::ChatRequest>,
    ) -> Result<Response<Self::ChatStreamStream>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);

        let req = request.into_inner();
        let prompt_prefills_thinking = prompt_prefills_open_think(&req.prompt);
        debug!(
            prompt_len = req.prompt.len(),
            max_tokens = req.max_tokens,
            "llama chat_stream request received"
        );

        let session = self.session_for_backend(BackendKind::Llama).await?;
        let max_tokens = req.max_tokens;
        let prompt_for_usage = req.prompt.clone();
        let request = convert::decode_chat_request(&req, true).map_err(proto_to_status)?;
        let stream_handle = session.submit_text_generation(request).await.map_err(|error| {
            error!(error = %error, "llama text generation stream setup failed");
            runtime_to_status(error)
        })?;
        let backend_stream = match stream_handle.take_stream().await {
            Ok(stream) => stream,
            Err(error) => {
                stream_handle.cancel_and_purge().await;
                error!(error = %error, "llama text generation stream handle failed");
                return Err(runtime_to_status(error));
            }
        };

        let (tx, rx) = mpsc::channel::<Result<pb::ChatStreamChunk, Status>>(32);
        tokio::spawn(
            async move {
                tokio::pin!(backend_stream);
                let mut token_count = 0usize;
                let mut terminal_usage: Option<TextGenerationUsage> = None;
                let mut terminal_finish_reason: Option<String> = None;
                let mut thinking_state = ThinkingStreamState::new(prompt_prefills_thinking);
                while let Some(chunk) = backend_stream.next().await {
                    let messages: Vec<pb::ChatStreamChunk> = match chunk {
                        Ok(chunk) => {
                            if !chunk.delta.is_empty() {
                                token_count += 1;
                            }
                            if let Some(finish_reason) = chunk.finish_reason.clone() {
                                terminal_finish_reason = Some(finish_reason);
                            }
                            if let Some(usage) = chunk.usage.clone() {
                                terminal_usage = Some(usage);
                            }
                            thinking_state
                                .ingest(&chunk.delta)
                                .into_iter()
                                .map(|delta| match delta {
                                    ThinkingDelta::Content(delta) => text_chunk(delta),
                                    ThinkingDelta::Reasoning(delta) => reasoning_chunk(delta),
                                })
                                .map(|chunk| convert::encode_chat_stream_chunk(&chunk))
                                .collect()
                        }
                        Err(error) => {
                            warn!(error = %error, "error in llama stream chunk");
                            vec![pb::ChatStreamChunk {
                                token: String::new(),
                                error: error.to_string(),
                                done: false,
                                finish_reason: String::new(),
                                usage: None,
                                reasoning_content: String::new(),
                            }]
                        }
                    };

                    for message in messages {
                        if tx.send(Ok(message)).await.is_err() {
                            debug!("llama stream receiver dropped; cancelling runtime task");
                            stream_handle.cancel_and_purge().await;
                            return;
                        }
                    }
                }

                for message in thinking_state
                    .finish()
                    .into_iter()
                    .map(|delta| match delta {
                        ThinkingDelta::Content(delta) => text_chunk(delta),
                        ThinkingDelta::Reasoning(delta) => reasoning_chunk(delta),
                    })
                    .map(|chunk| convert::encode_chat_stream_chunk(&chunk))
                {
                    if tx.send(Ok(message)).await.is_err() {
                        debug!("llama stream receiver dropped before final reasoning flush");
                        stream_handle.cancel_and_purge().await;
                        return;
                    }
                }

                debug!(token_count, "llama chat_stream relay finished");
                let completion_tokens = u32::try_from(token_count).unwrap_or(u32::MAX);
                let finish_reason =
                    resolve_finish_reason(terminal_finish_reason, completion_tokens, max_tokens);
                let usage = terminal_usage
                    .unwrap_or_else(|| build_estimated_usage(&prompt_for_usage, completion_tokens));
                let _ = tx
                    .send(Ok(convert::encode_chat_stream_chunk(&TextGenerationChunk {
                        delta: String::new(),
                        done: true,
                        finish_reason: Some(finish_reason),
                        usage: Some(usage),
                        metadata: Default::default(),
                    })))
                    .await;
                stream_handle.purge().await;
            }
            .instrument(tracing::Span::current()),
        );

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    #[instrument(skip_all, fields(request_id, backend = "ggml.llama"))]
    async fn load_model(
        &self,
        request: Request<pb::ModelLoadRequest>,
    ) -> Result<Response<pb::ModelStatusResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);

        debug!("llama load_model request received");
        let status = self.load_model_for_backend(BackendKind::Llama, request.into_inner()).await?;
        Ok(Response::new(status))
    }

    #[instrument(skip_all, fields(request_id, backend = "ggml.llama"))]
    async fn unload_model(
        &self,
        request: Request<pb::ModelUnloadRequest>,
    ) -> Result<Response<pb::ModelStatusResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);

        debug!("llama unload_model request received");
        let _ = request.into_inner();
        let status = self.unload_model_for_backend(BackendKind::Llama).await?;
        Ok(Response::new(status))
    }
}

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

fn finish_reason_from_token_budget(completion_tokens: u32, max_tokens: u32) -> String {
    if completion_tokens >= max_tokens && max_tokens > 0 {
        "length".to_owned()
    } else {
        "stop".to_owned()
    }
}

fn resolve_finish_reason(
    runtime_finish_reason: Option<String>,
    completion_tokens: u32,
    max_tokens: u32,
) -> String {
    runtime_finish_reason
        .unwrap_or_else(|| finish_reason_from_token_budget(completion_tokens, max_tokens))
}

fn build_estimated_usage(prompt: &str, completion_tokens: u32) -> TextGenerationUsage {
    let prompt_tokens = estimate_token_count(prompt);

    TextGenerationUsage {
        prompt_tokens,
        completion_tokens,
        total_tokens: prompt_tokens.saturating_add(completion_tokens),
        prompt_tokens_details: TextPromptTokensDetails::default(),
        estimated: true,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ParsedThinkingOutput, ThinkingDelta, ThinkingStreamState, attach_reasoning_metadata,
        parse_thinking_output, parse_thinking_output_with_context, prompt_prefills_open_think,
        resolve_finish_reason,
    };
    use serde_json::json;
    use slab_types::TextGenerationResponse;

    #[test]
    fn parse_thinking_output_extracts_reasoning_block() {
        let parsed = parse_thinking_output("<think>step one</think>\n\nfinal answer", true);
        assert_eq!(
            parsed,
            ParsedThinkingOutput {
                content: "\n\nfinal answer".to_owned(),
                reasoning: "step one".to_owned(),
            }
        );
    }

    #[test]
    fn parse_thinking_output_no_think_tag_passes_through() {
        let parsed = parse_thinking_output("just plain content", false);
        assert_eq!(
            parsed,
            ParsedThinkingOutput {
                content: "just plain content".to_owned(),
                reasoning: String::new(),
            }
        );
    }

    #[test]
    fn parse_thinking_output_holds_partial_open_tag_until_complete() {
        let parsed = parse_thinking_output("answer<th", false);
        assert_eq!(
            parsed,
            ParsedThinkingOutput { content: "answer".to_owned(), reasoning: String::new() }
        );

        let completed = parse_thinking_output("answer<th", true);
        assert_eq!(
            completed,
            ParsedThinkingOutput { content: "answer<th".to_owned(), reasoning: String::new() }
        );
    }

    #[test]
    fn thinking_stream_state_splits_reasoning_and_content_deltas() {
        let mut state = ThinkingStreamState::default();
        assert_eq!(
            state.ingest("<think>first thought"),
            vec![ThinkingDelta::Reasoning("first thought".to_owned())]
        );
        assert_eq!(
            state.ingest("</think>\n\nfinal answer"),
            vec![ThinkingDelta::Content("\n\nfinal answer".to_owned())]
        );
        assert!(state.finish().is_empty());
    }

    #[test]
    fn thinking_stream_state_holds_partial_open_tag_until_completed() {
        let mut state = ThinkingStreamState::default();
        assert_eq!(state.ingest("answer<th"), vec![ThinkingDelta::Content("answer".to_owned())]);
        assert_eq!(
            state.ingest("ink>first thought"),
            vec![ThinkingDelta::Reasoning("first thought".to_owned())]
        );
        assert_eq!(
            state.ingest("</think>\n\nfinal answer"),
            vec![ThinkingDelta::Content("\n\nfinal answer".to_owned())]
        );
        assert!(state.finish().is_empty());
    }

    #[test]
    fn thinking_stream_state_suppresses_whitespace_prefix_before_think_block() {
        let mut state = ThinkingStreamState::default();
        assert!(state.ingest(" ").is_empty());
        assert_eq!(
            state.ingest("<think>\nstep by step"),
            vec![ThinkingDelta::Reasoning("\nstep by step".to_owned())]
        );
        assert_eq!(
            state.ingest("</think>\n\nfinal answer"),
            vec![ThinkingDelta::Content("\n\nfinal answer".to_owned())]
        );
        assert!(state.finish().is_empty());
    }

    #[test]
    fn attach_reasoning_metadata_moves_reasoning_out_of_text() {
        let mut response = TextGenerationResponse {
            text: "<think>step by step</think>\n\nanswer".to_owned(),
            metadata: Default::default(),
            ..Default::default()
        };

        attach_reasoning_metadata(&mut response, false);

        assert_eq!(response.text, "\n\nanswer");
        assert_eq!(response.metadata.get("reasoning_content"), Some(&json!("step by step")));
    }

    #[test]
    fn attach_reasoning_metadata_ignores_whitespace_prefix_before_think() {
        let mut response = TextGenerationResponse {
            text: " <think>step by step</think>\n\nanswer".to_owned(),
            metadata: Default::default(),
            ..Default::default()
        };

        attach_reasoning_metadata(&mut response, false);

        assert_eq!(response.text, "\n\nanswer");
        assert_eq!(response.metadata.get("reasoning_content"), Some(&json!("step by step")));
    }

    #[test]
    fn thinking_stream_state_no_think_tag_passes_content_through() {
        let mut state = ThinkingStreamState::new(false);
        assert_eq!(
            state.ingest("hello world"),
            vec![ThinkingDelta::Content("hello world".to_owned())]
        );
        assert_eq!(
            state.ingest(" more text"),
            vec![ThinkingDelta::Content(" more text".to_owned())]
        );
        assert!(state.finish().is_empty());
    }

    #[test]
    fn prompt_prefills_open_think_detects_unclosed_suffix() {
        assert!(prompt_prefills_open_think("<|im_start|>assistant\n<think>\n"));
        assert!(!prompt_prefills_open_think("<think>done</think>\nanswer"));
        assert!(!prompt_prefills_open_think("plain content"));
    }

    #[test]
    fn parse_thinking_output_with_context_handles_prefilled_open_think() {
        let parsed =
            parse_thinking_output_with_context("step one</think>\n\nfinal answer", true, true);
        assert_eq!(
            parsed,
            ParsedThinkingOutput {
                content: "\n\nfinal answer".to_owned(),
                reasoning: "step one".to_owned(),
            }
        );
    }

    #[test]
    fn thinking_stream_state_handles_prefilled_open_think() {
        let mut state = ThinkingStreamState::new(true);
        assert_eq!(
            state.ingest("first thought"),
            vec![ThinkingDelta::Reasoning("first thought".to_owned())]
        );
        assert_eq!(
            state.ingest("</think>\n\nfinal answer"),
            vec![ThinkingDelta::Content("\n\nfinal answer".to_owned())]
        );
        assert!(state.finish().is_empty());
    }

    #[test]
    fn attach_reasoning_metadata_handles_prefilled_open_think() {
        let mut response = TextGenerationResponse {
            text: "step by step</think>\n\nanswer".to_owned(),
            metadata: Default::default(),
            ..Default::default()
        };

        attach_reasoning_metadata(&mut response, true);

        assert_eq!(response.text, "\n\nanswer");
        assert_eq!(response.metadata.get("reasoning_content"), Some(&json!("step by step")));
    }

    #[test]
    fn resolve_finish_reason_prefers_runtime_terminal_reason() {
        assert_eq!(resolve_finish_reason(Some("stop".to_owned()), 128, 16), "stop");
    }

    #[test]
    fn resolve_finish_reason_falls_back_to_token_budget() {
        assert_eq!(resolve_finish_reason(None, 16, 16), "length");
        assert_eq!(resolve_finish_reason(None, 8, 16), "stop");
    }
}
