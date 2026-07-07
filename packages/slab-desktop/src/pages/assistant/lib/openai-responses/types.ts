/**
 * OpenAI Responses protocol types — sourced from the official `openai` SDK
 * (`OpenAI.Responses.*`), the single source of truth for an OpenAI-compatible
 * client. This also lets us validate the slab server against the real SDK in
 * tests (round-trip a stored `Response` through `client.responses.create`
 * typing).
 *
 * Only the top-level union/object types are re-exported; narrow sub-shapes are
 * derived via `Extract` / indexed access at use sites so this surface stays
 * minimal and tracks the SDK automatically.
 */

import type OpenAI from "openai"

/** A complete OpenAI Responses `Response` object (the persisted/stored shape). */
export type Response = OpenAI.Responses.Response

/** `output[]` element (discriminated by `type`). */
export type ResponseOutputItem = OpenAI.Responses.ResponseOutputItem

/** A canonical `response.*` stream event. */
export type ResponseStreamEvent = OpenAI.Responses.ResponseStreamEvent

// Narrow sub-shapes derived from the unions above.
export type ResponseOutputMessage = Extract<ResponseOutputItem, { type: "message" }>
export type ResponseReasoningItem = Extract<ResponseOutputItem, { type: "reasoning" }>
export type ResponseFunctionToolCall = Extract<ResponseOutputItem, { type: "function_call" }>
export type ResponseCustomToolCall = Extract<ResponseOutputItem, { type: "custom_tool_call" }>
export type ResponseOutputMessageContent = ResponseOutputMessage["content"][number]
export type ResponseOutputText = Extract<ResponseOutputMessageContent, { type: "output_text" }>
export type ResponseReasoningSummaryPart =
  ResponseReasoningItem["summary"][number]
