/**
 * Restore projection: convert persisted OpenAI-Responses-canonical `Response`
 * objects (one per agent run) into the assistant page's
 * [`AssistantMessageRecord`] shape used to seed `useChat`'s `initialMessages`.
 *
 * Each `Response` is one assistant run → one assistant message record. Text
 * comes from `output[].message.content[].output_text`; reasoning from
 * `output[].reasoning.summary[]`; tool calls from `output[].function_call`.
 *
 * NOTE: a `Response` carries only the assistant's output for a run, not the
 * user prompt that triggered it (slab does not yet echo `input` on the stored
 * Response). The caller is responsible for interleaving user-side messages
 * (still available via the legacy per-message history during the migration).
 */

import type {
  AgentThreadMessageResponse,
  AssistantMessageRecord,
  AssistantMessageStatus,
  AssistantThought,
  AssistantUiMessage,
} from "../assistant-types"
import type { Response, ResponseOutputItem } from "./types"

function statusFor(responseStatus: string | undefined): AssistantMessageStatus {
  switch (responseStatus) {
    case "failed":
      return "error"
    case "cancelled":
      return "abort"
    case "in_progress":
    case "queued":
      return "loading"
    case "completed":
    case "incomplete":
    default:
      return "success"
  }
}

function reasoningText(item: Extract<ResponseOutputItem, { type: "reasoning" }>): string {
  const summary = item.summary
    ?.map((part) => (part.type === "summary_text" ? part.text : ""))
    .filter((text) => text.length > 0)
    .join("\n")
  if (summary && summary.length > 0) {
    return summary
  }
  // Fall back to reasoning content parts (slab bridges reasoning text here).
  const content = item.content
    ?.map((part) => part.text ?? "")
    .filter((text) => text.length > 0)
    .join("\n")
  return content ?? ""
}

function messageText(item: Extract<ResponseOutputItem, { type: "message" }>): string {
  return item.content
    .map((part) => (part.type === "output_text" ? part.text : ""))
    .filter((text) => text.length > 0)
    .join("\n")
}

function thoughtFromCall(
  item: Extract<ResponseOutputItem, { type: "function_call" | "custom_tool_call" }>,
  index: number
): AssistantThought {
  const callId = item.call_id || `${item.id ?? "call"}-${index}`
  const toolName = item.name || "tool_call"
  const detail = item.type === "function_call" ? item.arguments : item.input
  return {
    id: callId,
    callId,
    title: "tool_call",
    toolName,
    detail,
    summary: `tool_call id=${callId}: ${toolName}(${detail})`,
    status: "success",
  }
}

function recordFromResponse(response: Response, index: number): AssistantMessageRecord {
  let text = ""
  let reasoning = ""
  const thoughts: AssistantThought[] = []

  for (const item of response.output ?? []) {
    if (item.type === "message") {
      const part = messageText(item)
      if (part) {
        text = text ? `${text}\n\n${part}` : part
      }
    } else if (item.type === "reasoning") {
      const part = reasoningText(item)
      if (part) {
        reasoning = reasoning ? `${reasoning}\n${part}` : part
      }
    } else if (item.type === "function_call" || item.type === "custom_tool_call") {
      thoughts.push(thoughtFromCall(item, thoughts.length))
    }
  }

  // `output_text` is a server-convenience aggregate; use it only when the
  // structured message items did not yield text.
  if (!text && typeof response.output_text === "string") {
    text = response.output_text
  }

  const message: AssistantUiMessage = {
    role: "assistant",
    content: text,
  }
  if (reasoning) {
    message.reasoningContent = reasoning
  }
  if (thoughts.length > 0) {
    message.thoughts = thoughts
  }

  return {
    id: response.id || `assistant-${index}`,
    message,
    status: statusFor(response.status),
  }
}

/**
 * Project one-or-more stored `Response` objects into assistant message records,
 * oldest run first.
 */
export function projectResponses(responses: Response[] | undefined): AssistantMessageRecord[] {
  return (responses ?? []).map((response, index) => recordFromResponse(response, index))
}

// ── Full-session restore ────────────────────────────────────────────────────

interface Timestamped {
  ts: number
  record: AssistantMessageRecord
}

function toMs(value: unknown): number {
  if (typeof value === "number") {
    // Response.created_at is unix seconds.
    return value * 1000
  }
  if (typeof value === "string") {
    const parsed = Date.parse(value)
    if (!Number.isNaN(parsed)) {
      return parsed
    }
  }
  return 0
}

function thoughtsFromToolCalls(
  message: AgentThreadMessageResponse
): AssistantThought[] | undefined {
  const calls = message.tool_calls ?? []
  if (calls.length === 0) {
    return undefined
  }
  return calls.map((call, index) => {
    const callId = call.id?.trim() || `${message.id}-tool-${index}`
    return {
      id: callId,
      callId,
      title: "tool_call",
      toolName: call.function?.name?.trim() || "tool_call",
      detail: call.function?.arguments ?? "",
      summary: `tool_call id=${callId}: ${call.function?.name ?? ""}(${call.function?.arguments ?? ""})`,
      status: "success",
    }
  })
}

function recordFromMessage(message: AgentThreadMessageResponse): AssistantMessageRecord {
  const ui: AssistantUiMessage = { role: message.role, content: message.content }
  const thoughts = thoughtsFromToolCalls(message)
  if (thoughts && thoughts.length > 0) {
    ui.thoughts = thoughts
  }
  return { id: message.id, message: ui, status: "success" }
}

/**
 * Reconstruct the full conversation for session restore.
 *
 * Assistant turns come from the stored per-run `responses` (the canonical
 * OpenAI-Responses shape) when available; otherwise they fall back to the
 * legacy per-message `messages`. User turns always come from `messages` (a
 * stored `Response` carries only the assistant's output, not the user prompt).
 * Entries are merged and ordered by timestamp.
 */
export function projectRestoreSession(
  messages: AgentThreadMessageResponse[] | undefined,
  responses: unknown[] | undefined
): AssistantMessageRecord[] {
  // The wire contract carries `responses` as `unknown[]` (utoipa cannot model
  // the slab-proto Response tree); cast to the SDK-sourced `Response` shape.
  const typedResponses = (responses ?? []) as Response[]
  const useResponses = typedResponses.length > 0
  const entries: Timestamped[] = []

  for (const message of messages ?? []) {
    if (message.role === "user") {
      entries.push({ ts: toMs(message.created_at), record: recordFromMessage(message) })
    } else if (message.role === "assistant" && !useResponses) {
      entries.push({ ts: toMs(message.created_at), record: recordFromMessage(message) })
    }
    // tool-role rows are represented as `thoughts` on their assistant message;
    // they do not become standalone records here.
  }

  if (useResponses) {
    for (const response of typedResponses) {
      entries.push({
        ts: toMs(response.created_at),
        record: recordFromResponse(response, entries.length),
      })
    }
  }

  entries.sort((a, b) => a.ts - b.ts)
  return entries.map((entry) => entry.record)
}
