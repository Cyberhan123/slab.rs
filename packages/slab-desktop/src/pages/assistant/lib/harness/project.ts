/**
 * Restore projection: convert a harness {@link Thread} (returned by
 * `thread/resume` with populated `turns`) into AI-SDK {@link UIMessage}s used to
 * seed `useChat`'s `initialMessages`.
 *
 * Items are walked in turn/item order. `userMessage` items start a user message;
 * consecutive non-user items (assistant text, reasoning, tool calls) are grouped
 * into one assistant message. Text and reasoning become typed UI parts; tool
 * items become `type: "tool-<name>"` parts (matching the live streaming path in
 * `stream.ts`) so restored tool calls render the same rich cards as live ones.
 */

import type { UIMessage } from "ai"

import type { ReasoningText, Thread, TurnItem } from "./types"

function reasoningToString(value: ReasoningText): string {
  return Array.isArray(value) ? value.join("\n") : value
}

/** Stringify a tool value of unknown shape for an error/output field. */
function stringifyToolValue(value: unknown): string {
  if (typeof value === "string") return value
  try {
    return JSON.stringify(value, null, 2)
  } catch {
    return String(value)
  }
}

type ToolUIPartLike = {
  type: `tool-${string}`
  toolCallId: string
  toolName: string
  input: unknown
  state: "input-available" | "output-available" | "output-error"
  output?: unknown
  errorText?: string
}

/** Build a finalized tool UI part (input + output/error) for a restored item. */
function toolPartFromItem(item: TurnItem): ToolUIPartLike | null {
  switch (item.type) {
    case "commandExecution": {
      const failed = item.exitCode !== undefined && item.exitCode !== 0
      return {
        input: { command: item.command, cwd: item.cwd },
        errorText: failed ? item.aggregatedOutput ?? `exit code ${item.exitCode}` : undefined,
        output: !failed && item.aggregatedOutput ? item.aggregatedOutput : undefined,
        state: failed ? "output-error" : "output-available",
        toolCallId: item.id,
        toolName: "commandExecution",
        type: "tool-commandExecution",
      }
    }
    case "mcpToolCall": {
      const failed = item.error !== undefined && item.error !== null
      return {
        errorText: failed ? stringifyToolValue(item.error) : undefined,
        input: item.arguments,
        output: !failed && item.result !== undefined && item.result !== null
          ? item.result
          : undefined,
        state: failed ? "output-error" : "output-available",
        toolCallId: item.id,
        toolName: item.tool,
        type: `tool-${item.tool}`,
      }
    }
    case "fileChange":
      return {
        input: { changes: item.changes },
        output: { status: item.status },
        state: "output-available",
        toolCallId: item.id,
        toolName: "fileChange",
        type: "tool-fileChange",
      }
    case "webSearch":
      return {
        input: { query: item.query },
        state: "output-available",
        toolCallId: item.id,
        toolName: "webSearch",
        type: "tool-webSearch",
      }
    default:
      return null
  }
}

/** Render a non-user item as a UI part (text, reasoning, or tool). */
function partFromItem(item: TurnItem): UIMessage["parts"][number] | null {
  switch (item.type) {
    case "agentMessage":
      return item.text ? { text: item.text, type: "text" } : null
    case "reasoning": {
      const text = reasoningToString(item.summary ?? item.content)
      // `state: "done"` so the restored reasoning part is not treated as
      // streaming and renders as an openable "Thought for a few seconds" block.
      return text ? { state: "done" as const, text, type: "reasoning" as const } : null
    }
    case "commandExecution":
    case "mcpToolCall":
    case "fileChange":
    case "webSearch":
      return toolPartFromItem(item) as UIMessage["parts"][number] | null
    case "imageView":
      return { text: "[image]", type: "text" }
    case "userMessage":
      return null
    default:
      return null
  }
}

/**
 * Project a resumed harness thread into `UIMessage`s, oldest turn first.
 * Empty turns/items produce no messages.
 */
export function projectThread(thread: Thread): UIMessage[] {
  const messages: UIMessage[] = []
  let pendingAssistantId: string | null = null
  let pendingParts: UIMessage["parts"] = []

  const flushAssistant = () => {
    if (pendingAssistantId !== null && pendingParts.length > 0) {
      messages.push({ id: pendingAssistantId, parts: pendingParts, role: "assistant" })
    }
    pendingAssistantId = null
    pendingParts = []
  }

  for (const turn of thread.turns) {
    for (const item of turn.items) {
      if (item.type === "userMessage") {
        // A user message terminates any in-flight assistant grouping.
        flushAssistant()
        const text = item.content
          .map((part) => (part.type === "text" ? part.text : ""))
          .join("")
          .trim()
        if (text) {
          messages.push({ id: item.id, parts: [{ text, type: "text" }], role: "user" })
        }
        continue
      }
      if (pendingAssistantId === null) pendingAssistantId = item.id
      const part = partFromItem(item)
      if (part) pendingParts.push(part)
    }
    flushAssistant()
  }

  return messages
}
