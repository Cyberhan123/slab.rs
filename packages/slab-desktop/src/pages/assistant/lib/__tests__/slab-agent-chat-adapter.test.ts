import type { UIMessage } from "ai"
import { describe, expect, it } from "vitest"

import { SlabAgentChatAdapter } from "../slab-agent-chat-adapter"

function userMessage(text: string): UIMessage {
  return {
    id: "user-message",
    parts: [{ text, type: "text" }],
    role: "user",
  }
}

describe("SlabAgentChatAdapter", () => {
  it("creates an agent response command for the first chat turn", () => {
    const adapter = new SlabAgentChatAdapter({
      model: "test-model",
      sessionId: "session-1",
    })

    expect(adapter.createCommand({ messages: [userMessage(" hello ")] })).toMatchObject({
      config: {
        max_turns: 8,
        model: "test-model",
      },
      messages: [{ content: " hello ", role: "user" }],
      session_id: "session-1",
      type: "agent.response.create",
    })
  })

  it("creates an agent input command after an ack supplies the thread id", () => {
    const adapter = new SlabAgentChatAdapter()

    adapter.handleServerMessage({
      accepted: true,
      action: "response_create",
      status: "pending",
      thread_id: "thread-1",
      type: "agent.ack",
    })

    expect(adapter.createCommand({ messages: [userMessage("next")] })).toMatchObject({
      content: "next",
      thread_id: "thread-1",
      type: "agent.input",
    })
  })

  it("continues from thread ids restored by the page-level session restore path", () => {
    const adapter = new SlabAgentChatAdapter({
      model: "test-model",
      sessionId: "session-restored",
    })

    adapter.handleServerMessage({
      messages: [],
      session_id: "session-restored",
      thread: {
        id: "thread-restored",
        session_id: "session-restored",
        status: "completed",
      },
      type: "agent.session.restored",
    })

    expect(adapter.createCommand({ messages: [userMessage("continue")] })).toMatchObject({
      content: "continue",
      thread_id: "thread-restored",
      type: "agent.input",
    })
  })

  it("uses constructor thread ids without leaking across adapter instances", () => {
    const restoredAdapter = new SlabAgentChatAdapter({
      sessionId: "session-a",
      threadId: "thread-a",
    })
    const freshAdapter = new SlabAgentChatAdapter({
      model: "model-b",
      sessionId: "session-b",
    })

    expect(restoredAdapter.createCommand({ messages: [userMessage("next")] })).toMatchObject({
      thread_id: "thread-a",
      type: "agent.input",
    })
    expect(freshAdapter.createCommand({ messages: [userMessage("first")] })).toMatchObject({
      config: {
        model: "model-b",
      },
      session_id: "session-b",
      type: "agent.response.create",
    })
  })

  it("converts text streaming events into AI SDK UI chunks", () => {
    const adapter = new SlabAgentChatAdapter()

    expect(
      adapter.transformPayload(
        '{"thread_id":"thread-1","sequence_number":1,"type":"response.output_text.delta","delta":"hel"}'
      )
    ).toEqual([
      { id: "assistant-text", type: "text-start" },
      { delta: "hel", id: "assistant-text", type: "text-delta" },
    ])

    expect(
      adapter.transformPayload(
        '{"thread_id":"thread-1","sequence_number":2,"type":"response.output_text.done","text":"hello"}'
      )
    ).toEqual([
      { id: "assistant-text", type: "text-end" },
      { type: "finish-step" },
      { finishReason: "stop", type: "finish" },
    ])
    expect(
      adapter.transformPayload(
        '{"thread_id":"thread-1","sequence_number":3,"type":"response.completed","response":{"id":"thread-1","status":"completed"}}'
      )
    ).toEqual([])
  })

  it("converts reasoning and tool events into AI SDK UI chunks", () => {
    const adapter = new SlabAgentChatAdapter()

    expect(
      adapter.transformPayload(
        '{"thread_id":"thread-1","sequence_number":1,"type":"response.reasoning_text.delta","delta":"think"}'
      )
    ).toEqual([
      { id: "assistant-reasoning", type: "reasoning-start" },
      { delta: "think", id: "assistant-reasoning", type: "reasoning-delta" },
    ])
    expect(
      adapter.transformPayload(
        '{"thread_id":"thread-1","sequence_number":2,"type":"response.reasoning_text.done","text":"thinking done"}'
      )
    ).toEqual([{ id: "assistant-reasoning", type: "reasoning-end" }])
    expect(
      adapter.transformPayload(
        '{"thread_id":"thread-1","sequence_number":3,"type":"response.function_call_arguments.done","name":"shell","call_id":"call-1","arguments":"{\\"command\\":\\"pwd\\"}"}'
      )
    ).toEqual([
      {
        input: { command: "pwd" },
        toolCallId: "call-1",
        toolName: "shell",
        type: "tool-input-available",
      },
    ])
  })
})
