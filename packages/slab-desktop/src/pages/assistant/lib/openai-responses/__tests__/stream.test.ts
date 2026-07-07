import { describe, expect, it } from "vitest"

import { convertEvent, createStreamState, parseControlMessage, parseStreamEvent } from "../stream"
import type { ResponseOutputItem, ResponseStreamEvent } from "../types"

// Test fixtures are intentionally minimal; cast through `unknown` to the
// SDK-sourced canonical types (the SDK is the source of truth — its stricter
// types are the point; fixtures only exercise `convertEvent`'s branching).
const messageItem = { type: "message", role: "assistant", content: [] } as unknown as ResponseOutputItem
const reasoningItem = { type: "reasoning", summary: [] } as unknown as ResponseOutputItem

function ev(e: unknown): ResponseStreamEvent {
  return e as ResponseStreamEvent
}

describe("openai-responses stream", () => {
  it("parses agent.ack as a control message", () => {
    const msg = parseControlMessage(
      JSON.stringify({ type: "agent.ack", thread_id: "thread-1", accepted: true }),
    )
    expect(msg).toMatchObject({ type: "agent.ack", thread_id: "thread-1" })
  })

  it("parses canonical response events", () => {
    const event = parseStreamEvent(
      JSON.stringify({ type: "response.output_text.delta", delta: "hi", output_index: 0, content_index: 0 }),
    )
    expect(event).toMatchObject({ type: "response.output_text.delta", delta: "hi" })
  })

  it("opens text on output_item.added(message) and streams deltas", () => {
    const state = createStreamState()
    expect(
      convertEvent(ev({ type: "response.output_item.added", output_index: 0, item: messageItem }), state),
    ).toEqual([{ id: "assistant-text", type: "text-start" }])
    expect(
      convertEvent(
        ev({ type: "response.output_text.delta", output_index: 0, content_index: 0, delta: "hi", item_id: "i", sequence_number: 1 }),
        state,
      ),
    ).toEqual([{ delta: "hi", id: "assistant-text", type: "text-delta" }])
  })

  it("treats output_text.done as a no-op (deltas already carry the text)", () => {
    const state = createStreamState()
    convertEvent(ev({ type: "response.output_item.added", output_index: 0, item: messageItem }), state)
    convertEvent(
      ev({ type: "response.output_text.delta", output_index: 0, content_index: 0, delta: "hi", item_id: "i", sequence_number: 1 }),
      state,
    )
    expect(
      convertEvent(
        ev({ type: "response.output_text.done", output_index: 0, content_index: 0, text: "hi", item_id: "i", sequence_number: 2 }),
        state,
      ),
    ).toEqual([])
  })

  it("streams reasoning via reasoning_summary_text.delta (canonical, not reasoning_text)", () => {
    const state = createStreamState()
    convertEvent(ev({ type: "response.output_item.added", output_index: 0, item: reasoningItem }), state)
    expect(
      convertEvent(
        ev({ type: "response.reasoning_summary_text.delta", output_index: 0, delta: "thinking", item_id: "i", sequence_number: 1 }),
        state,
      ),
    ).toEqual([{ delta: "thinking", id: "assistant-reasoning", type: "reasoning-delta" }])
  })

  it("finishes the response only on response.completed", () => {
    const state = createStreamState()
    convertEvent(ev({ type: "response.output_item.added", output_index: 0, item: messageItem }), state)
    convertEvent(
      ev({ type: "response.output_text.delta", output_index: 0, content_index: 0, delta: "hi", item_id: "i", sequence_number: 1 }),
      state,
    )
    // output_item.done closes the text part but does NOT finish the response.
    expect(
      convertEvent(
        ev({
          type: "response.output_item.done",
          output_index: 0,
          item: { type: "message", role: "assistant", content: [{ type: "output_text", text: "hi", annotations: [] }] } as unknown as ResponseOutputItem,
          sequence_number: 2,
        }),
        state,
      ),
    ).toEqual([{ id: "assistant-text", type: "text-end" }])
    expect(state.finished).toBe(false)
    // The terminal is response.completed.
    expect(
      convertEvent(
        ev({
          type: "response.completed",
          sequence_number: 3,
          response: { id: "thread-1", object: "response", created_at: 0, status: "completed", output: [] } as never,
        }),
        state,
      ),
    ).toEqual([{ type: "finish-step" }, { finishReason: "stop", type: "finish" }])
    expect(state.finished).toBe(true)
  })

  it("emits an error chunk and finishes on response.failed", () => {
    const state = createStreamState()
    const chunks = convertEvent(
      ev({
        type: "response.failed",
        sequence_number: 1,
        response: { id: "thread-1", object: "response", created_at: 0, status: "failed", output: [], error: { code: "x", message: "boom" } } as never,
      }),
      state,
    )
    expect(chunks[0]).toMatchObject({ type: "error", errorText: "boom" })
    expect(chunks.at(-1)).toMatchObject({ type: "finish", finishReason: "error" })
  })
})
