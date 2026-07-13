//! Response service — OpenAI Responses wire host for `/responses`.
//!
//! Slice C2 turns `/responses` into a standalone single-shot Model: one LLM
//! call via `llm-service` (through `ChatService`), with the canonical OpenAI
//! Responses wire produced by feeding *synthesized* [`AgentEventKind`] envelopes
//! into the existing pure projections ([`projection::build_response`] /
//! [`stream::envelope_to_events`]). The non-streaming assembler lives in
//! [`projection`], the streaming state machine in [`stream`], and the
//! single-shot orchestration (resolve model → route → call → persist →
//! synthesize) in [`single_shot`].
//!
//! `/responses` no longer drives the slab-agent turn loop and no longer
//! subscribes to the `AgentEventHub` `AgentEventKind` channel — that channel
//! now has zero consumers, which lets slice C3 retire `AgentEventKind` from
//! slab-agent entirely.

pub mod projection;
pub mod single_shot;
pub mod stream;

pub use projection::{
    AdapterInput, build_response, parse_mcp_status, parse_phase, parse_shell_output_content,
};
pub use stream::{StreamCtx, build_terminal_event, envelope_to_events};

use std::pin::Pin;

use futures::Stream;
use slab_proto::openai::Response;

use super::AgentCore;
use crate::context::ModelState;
use crate::domain::models::AgentSessionSnapshot;
use crate::domain::services::agent::response::single_shot::{
    StreamFrame, run_create_response, run_get_response, run_stream_response,
};
use crate::error::AppCoreError;
use crate::schemas::agent::OpenAICreateRequest;

type StreamFrameStream = Pin<Box<dyn Stream<Item = StreamFrame> + Send>>;

/// Response-side agent service: owns the OpenAI Responses wire surface
/// consumed by the `/responses` HTTP / SSE / WebSocket transport.
///
/// Holds a cheap clone of the shared [`AgentCore`] (for the thread store) and
/// the [`ModelState`] (for `llm-service` routing via `ChatService`).
#[derive(Clone)]
pub struct ResponseService {
    core: AgentCore,
    state: ModelState,
}

impl ResponseService {
    pub(crate) fn new(core: AgentCore, state: ModelState) -> Self {
        Self { core, state }
    }

    /// Non-streaming single-shot: one LLM call → canonical OpenAI [`Response`].
    pub async fn create_response(
        &self,
        req: OpenAICreateRequest,
        session_id: String,
    ) -> Result<Response, AppCoreError> {
        run_create_response(&self.core, &self.state, &req, &session_id).await
    }

    /// Streaming single-shot: returns the response id plus the synthesized
    /// frame stream (lifecycle + output-item envelopes + terminal). C2 emits
    /// the full sequence as a burst; true token streaming is a follow-up.
    pub async fn stream_response(
        &self,
        req: OpenAICreateRequest,
        session_id: String,
    ) -> Result<(String, StreamFrameStream), AppCoreError> {
        let (response_id, frames) =
            run_stream_response(&self.core, &self.state, &req, &session_id).await?;
        Ok((response_id, Box::pin(futures::stream::iter(frames))))
    }

    /// Reconstruct a [`Response`] for an already-completed run (GET SSE resume).
    pub async fn get_response(&self, response_id: &str) -> Result<Response, AppCoreError> {
        run_get_response(&self.core, response_id).await
    }

    pub async fn restore_session_snapshot(
        &self,
        session_id: &str,
    ) -> Result<AgentSessionSnapshot, AppCoreError> {
        let restored = self.core.restore_session(session_id).await?;
        Ok(AgentSessionSnapshot {
            session_id: session_id.to_owned(),
            thread: restored.thread,
            messages: restored.messages,
            responses: restored.responses,
        })
    }
}
