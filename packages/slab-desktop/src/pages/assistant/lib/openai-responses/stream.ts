/**
 * Streaming layer for the OpenAI Responses protocol.
 *
 * The slab server emits canonical `response.*` events on the wire (produced by
 * `envelope_to_events`), plus OpenAI-compatible error frames. This module
 * parses those frames and converts the canonical events into AI-SDK
 * `UIMessageChunk`s.
 *
 * IMPORTANT differences from the legacy slab-dialect parser:
 * - Reasoning streams as `response.reasoning_summary_text.delta/done` (NOT
 *   `response.reasoning_text.*`, which the server never emits on the wire).
 * - The terminal is `response.completed` / `response.failed` (NOT
 *   `response.output_text.done`, which fires once per message item mid-stream).
 * - `response.cancelled`, `agent.status`, `response.tool_call.*` are NOT
 *   emitted by the current adapter — do not handle them as live events.
 */

import type { UIMessageChunk } from "ai"

import type { ResponseOutputItem, ResponseStreamEvent } from "./types"

/** Parse one canonical `response.*` stream event or OpenAI-compatible error frame. */
export function parseStreamEvent(data: string): ResponseStreamEvent | null {
  let value: unknown
  try {
    value = JSON.parse(data)
  } catch {
    return null
  }
  if (
    typeof value !== "object" ||
    value === null ||
    typeof (value as { type?: unknown }).type !== "string"
  ) {
    return null
  }
  return value as ResponseStreamEvent
}

const TEXT_PART_ID = "assistant-text"
const REASONING_PART_ID = "assistant-reasoning"

/** Mutable per-response streaming state carried across `convertEvent` calls. */
export interface StreamState {
  finished: boolean
  reasoningOpen: boolean
  textOpen: boolean
}

export function createStreamState(): StreamState {
  return { finished: false, reasoningOpen: false, textOpen: false }
}

function openText(state: StreamState): UIMessageChunk[] {
  if (state.textOpen) return []
  state.textOpen = true
  return [{ id: TEXT_PART_ID, type: "text-start" }]
}

function closeText(state: StreamState): UIMessageChunk[] {
  if (!state.textOpen) return []
  state.textOpen = false
  return [{ id: TEXT_PART_ID, type: "text-end" }]
}

function openReasoning(state: StreamState): UIMessageChunk[] {
  if (state.reasoningOpen) return []
  state.reasoningOpen = true
  return [{ id: REASONING_PART_ID, type: "reasoning-start" }]
}

function closeReasoning(state: StreamState): UIMessageChunk[] {
  if (!state.reasoningOpen) return []
  state.reasoningOpen = false
  return [{ id: REASONING_PART_ID, type: "reasoning-end" }]
}

function finishChunks(state: StreamState, finishReason: "stop" | "error" = "stop"): UIMessageChunk[] {
  if (state.finished) return []
  const chunks: UIMessageChunk[] = []
  chunks.push(...closeReasoning(state))
  chunks.push(...closeText(state))
  chunks.push({ type: "finish-step" }, { finishReason, type: "finish" })
  state.finished = true
  return chunks
}

function toolInputFromItem(
  item: ResponseOutputItem
): { toolCallId: string; toolName: string; input: unknown } | null {
  if (item.type === "function_call") {
    return { toolCallId: item.call_id, toolName: item.name, input: safeParse(item.arguments) }
  }
  if (item.type === "custom_tool_call") {
    return { toolCallId: item.call_id, toolName: item.name, input: safeParse(item.input) }
  }
  return null
}

function safeParse(value: string): unknown {
  try {
    return JSON.parse(value)
  } catch {
    return value
  }
}

/**
 * Convert one canonical stream event into 0..N `UIMessageChunk`s, mutating
 * `state` to track open text/reasoning parts and the terminal finish.
 */
export function convertEvent(event: ResponseStreamEvent, state: StreamState): UIMessageChunk[] {
  switch (event.type) {
    // ── Assistant text ────────────────────────────────────────────────────
    case "response.output_item.added": {
      if (event.item.type === "message") return openText(state)
      if (event.item.type === "reasoning") return openReasoning(state)
      return []
    }
    case "response.output_text.delta":
      return openText(state).concat({ delta: event.delta, id: TEXT_PART_ID, type: "text-delta" })
    case "response.output_text.done":
      // `.done` carries the final full text, but the `.delta` events already
      // appended it incrementally; slab always streams deltas first, so this
      // is a no-op (the text part is closed by `output_item.done`).
      return []
    case "response.output_item.done": {
      if (event.item.type === "message") return closeText(state)
      if (event.item.type === "reasoning") return closeReasoning(state)
      const tool = toolInputFromItem(event.item)
      if (tool) {
        return [
          {
            input: tool.input,
            toolCallId: tool.toolCallId,
            toolName: tool.toolName,
            type: "tool-input-available",
          },
        ]
      }
      return []
    }

    // ── Reasoning ──────────────────────────────────────────────────────────
    case "response.reasoning_summary_part.added":
      return openReasoning(state)
    case "response.reasoning_summary_text.delta":
      return [
        { delta: event.delta, id: REASONING_PART_ID, type: "reasoning-delta" },
      ]
    case "response.reasoning_summary_text.done":
      // Final full reasoning text; the `.delta` events already appended it.
      return []
    case "response.reasoning_summary_part.done":
      return closeReasoning(state)

    // ── Function / custom-tool calls (delta streaming optional) ────────────
    case "response.function_call_arguments.delta":
    case "response.custom_tool_call_input.delta":
      // Tool args stream as raw text; the finalized call arrives via
      // `response.output_item.done`. No progressive tool-input chunking here.
      return []

    // ── Lifecycle / terminal ───────────────────────────────────────────────
    case "response.created":
    case "response.in_progress":
      return []
    case "response.completed":
      return finishChunks(state, "stop")
    case "response.failed": {
      const err = (event.response as { error?: { message?: string } } | undefined)?.error
      return [
        { errorText: err?.message ?? "response failed", type: "error" },
        ...finishChunks(state, "error"),
      ]
    }
    case "error":
      return [{ errorText: event.message ?? "response error", type: "error" }]
    default:
      // Unrecognized canonical event (e.g. MCP / shell / web-search details the
      // basic chat UI does not render yet). Surfaced via the transport's debug
      // log; silently ignored here.
      return []
  }
}
