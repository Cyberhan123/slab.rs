import type { UIMessage, UIMessageChunk } from "ai"

import type {
  AgentResponsesClientMessage,
  AgentResponsesServerMessage,
} from "../assistant-context"
import {
  parseAssistantAgentServerMessage,
  parseAssistantAgentStreamEvent,
} from "./assistant-agent-events"
import { nextId } from "./assistant-agent-state"

type SlabAgentChatAdapterOptions = {
  model?: string
  sessionId?: string
}

type SlabAgentChatRequest = {
  body?: {
    messages?: UIMessage[]
  }
  messages?: UIMessage[]
}

type SlabAgentChatState = {
  finished: boolean
  reasoningOpen: boolean
  textOpen: boolean
  threadId: string | null
}

const DEFAULT_ASSISTANT_MODEL = "slab-llama"
const DEFAULT_SESSION_ID = "assistant-default"
const TEXT_PART_ID = "assistant-text"
const REASONING_PART_ID = "assistant-reasoning"

function getMessageText(message: Pick<UIMessage, "parts">) {
  return message.parts
    .filter((part) => part.type === "text")
    .map((part) => part.text)
    .join("")
}

function getLastUserMessage(messages: UIMessage[]) {
  return messages.toReversed().find((message) => message.role === "user") ?? null
}

function toAgentMessages(messages: UIMessage[]) {
  return messages
    .filter((message) => message.role === "user" || message.role === "assistant")
    .map((message) => ({
      content: getMessageText(message),
      role: message.role,
    }))
    .filter((message) => message.content.trim().length > 0)
}

function createFinishChunks(state: SlabAgentChatState): UIMessageChunk[] {
  if (state.finished) {
    return []
  }

  const chunks: UIMessageChunk[] = []

  if (state.reasoningOpen) {
    chunks.push({ id: REASONING_PART_ID, type: "reasoning-end" })
    state.reasoningOpen = false
  }

  if (state.textOpen) {
    chunks.push({ id: TEXT_PART_ID, type: "text-end" })
    state.textOpen = false
  }

  chunks.push({ type: "finish-step" }, { finishReason: "stop", type: "finish" })
  state.finished = true

  return chunks
}

function parseToolInput(value: string) {
  try {
    return JSON.parse(value)
  } catch {
    return value
  }
}

export class SlabAgentChatAdapter {
  private readonly model: string
  private readonly sessionId: string
  private state: SlabAgentChatState = {
    finished: false,
    reasoningOpen: false,
    textOpen: false,
    threadId: null,
  }

  constructor(options: SlabAgentChatAdapterOptions = {}) {
    this.model = options.model ?? DEFAULT_ASSISTANT_MODEL
    this.sessionId = options.sessionId ?? DEFAULT_SESSION_ID
  }

  createCommand(request: SlabAgentChatRequest): AgentResponsesClientMessage {
    this.state.finished = false
    this.state.reasoningOpen = false
    this.state.textOpen = false

    const messages = request.messages ?? request.body?.messages ?? []
    const lastUserMessage = getLastUserMessage(messages)
    const content = lastUserMessage ? getMessageText(lastUserMessage).trim() : ""

    if (this.state.threadId) {
      return {
        content,
        request_id: nextId("request"),
        thread_id: this.state.threadId,
        type: "agent.input",
      }
    }

    return {
      config: {
        max_turns: 8,
        model: this.model,
      },
      messages: toAgentMessages(messages),
      request_id: nextId("request"),
      session_id: this.sessionId,
      type: "agent.response.create",
    }
  }

  handleServerMessage(message: AgentResponsesServerMessage) {
    if (message.type === "agent.ack" && message.thread_id) {
      this.state.threadId = message.thread_id
    }

    if (message.type === "agent.error") {
      return [{ errorText: message.message, type: "error" } satisfies UIMessageChunk]
    }

    return []
  }

  transformPayload(data: string): UIMessageChunk[] {
    const serverMessage = parseAssistantAgentServerMessage(data)
    if (serverMessage) {
      return this.handleServerMessage(serverMessage)
    }

    const event = parseAssistantAgentStreamEvent(data)
    if (!event) {
      return []
    }

    switch (event.type) {
      case "assistant_delta": {
        const chunks: UIMessageChunk[] = []

        if (!this.state.textOpen) {
          chunks.push({ id: TEXT_PART_ID, type: "text-start" })
          this.state.textOpen = true
        }

        chunks.push({ delta: event.text, id: TEXT_PART_ID, type: "text-delta" })
        return chunks
      }
      case "assistant_reasoning_delta": {
        const chunks: UIMessageChunk[] = []

        if (!this.state.reasoningOpen) {
          chunks.push({ id: REASONING_PART_ID, type: "reasoning-start" })
          this.state.reasoningOpen = true
        }

        chunks.push({
          delta: event.text,
          id: REASONING_PART_ID,
          type: "reasoning-delta",
        })
        return chunks
      }
      case "assistant_reasoning_done": {
        const chunks: UIMessageChunk[] = []

        if (!this.state.reasoningOpen) {
          chunks.push({ id: REASONING_PART_ID, type: "reasoning-start" })
          if (event.text) {
            chunks.push({
              delta: event.text,
              id: REASONING_PART_ID,
              type: "reasoning-delta",
            })
          }
        }

        chunks.push({ id: REASONING_PART_ID, type: "reasoning-end" })
        this.state.reasoningOpen = false
        return chunks
      }
      case "tool_call_started":
        return [
          {
            input: parseToolInput(event.arguments),
            toolCallId: event.call_id,
            toolName: event.tool_name,
            type: "tool-input-available",
          },
        ]
      case "tool_call_output":
        return [
          {
            output: event.output,
            toolCallId: event.call_id,
            type: "tool-output-available",
          },
        ]
      case "turn_completed": {
        const chunks: UIMessageChunk[] = []

        if (!this.state.textOpen && event.text) {
          chunks.push(
            { id: TEXT_PART_ID, type: "text-start" },
            { delta: event.text, id: TEXT_PART_ID, type: "text-delta" }
          )
          this.state.textOpen = true
        }

        chunks.push(...createFinishChunks(this.state))
        return chunks
      }
      case "turn_finished":
        return createFinishChunks(this.state)
      case "turn_failed":
        this.state.finished = true
        return [
          { errorText: event.error, type: "error" },
          { finishReason: "error", type: "finish" },
        ]
      case "turn_cancelled":
        return [{ reason: event.reason, type: "abort" }]
      case "agent_status":
      case "approval_required":
      case "lagged":
        return []
      default:
        return []
    }
  }
}
