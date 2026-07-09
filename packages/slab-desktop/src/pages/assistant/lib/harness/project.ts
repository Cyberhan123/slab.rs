/**
 * Restore projection: convert a harness {@link Thread} (returned by
 * `thread/resume` with populated `turns`) into AI-SDK {@link UIMessage}s used to
 * seed `useChat`'s `initialMessages`.
 *
 * Items are walked in turn/item order. `userMessage` items start a user message;
 * consecutive non-user items (assistant text, reasoning, tool calls) are grouped
 * into one assistant message. Text and reasoning become typed UI parts; tool
 * items are rendered as text parts (the generic `ToolUIPart` is tool-set-typed,
 * so restore keeps to the type-safe text/reasoning vocabulary).
 */

import type { UIMessage } from "ai"

import type { ReasoningText, Thread, TurnItem } from "./types"

function reasoningToString(value: ReasoningText): string {
  return Array.isArray(value) ? value.join("\n") : value
}

/** Render a non-user item as a UI part (text or reasoning). */
function partFromItem(item: TurnItem): UIMessage["parts"][number] | null {
  switch (item.type) {
    case "agentMessage":
      return item.text ? { text: item.text, type: "text" } : null
    case "reasoning": {
      const text = reasoningToString(item.summary ?? item.content)
      return text ? { text, type: "reasoning" } : null
    }
    case "commandExecution": {
      const lines = [`[command] ${item.command || "(shell)"}`.trim()]
      if (item.aggregatedOutput) lines.push(item.aggregatedOutput)
      return { text: lines.join("\n"), type: "text" }
    }
    case "mcpToolCall": {
      const args = (() => {
        try {
          return JSON.stringify(item.arguments)
        } catch {
          return String(item.arguments)
        }
      })()
      return { text: `[tool] ${item.tool}(${args})`, type: "text" }
    }
    case "fileChange":
      return { text: `[file] ${item.changes.length} change(s)`, type: "text" }
    case "webSearch":
      return { text: `[search] ${item.query}`, type: "text" }
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
