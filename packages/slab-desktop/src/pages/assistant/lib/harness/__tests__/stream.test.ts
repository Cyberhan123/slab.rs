import { describe, expect, it } from "vitest"

import { convertNotification, createStreamState, isTerminalNotification } from "../stream"
import type { ServerNotification, TurnItem } from "../types"

const THREAD = "hthread-1"
const TURN = "0"

function agentMessageStarted(itemId: string): ServerNotification {
  const item: TurnItem = { type: "agentMessage", id: itemId, text: "" }
  return { method: "item/started", params: { item, threadId: THREAD, turnId: TURN } }
}

function agentMessageCompleted(itemId: string, text: string): ServerNotification {
  const item: TurnItem = { type: "agentMessage", id: itemId, text }
  return { method: "item/completed", params: { item, threadId: THREAD, turnId: TURN } }
}

describe("harness stream convertNotification", () => {
  it("opens text on item/started(agentMessage) and streams deltas", () => {
    const state = createStreamState()
    expect(convertNotification(agentMessageStarted("i1"), state)).toEqual([
      { id: "i1", type: "text-start" },
    ])
    expect(
      convertNotification(
        { method: "item/agentMessage/delta", params: { threadId: THREAD, turnId: TURN, itemId: "i1", delta: "hi" } },
        state,
      ),
    ).toEqual([{ delta: "hi", id: "i1", type: "text-delta" }])
    expect(convertNotification(agentMessageCompleted("i1", "hi"), state)).toEqual([
      { id: "i1", type: "text-end" },
    ])
  })

  it("streams reasoning parts", () => {
    const state = createStreamState()
    const item: TurnItem = { type: "reasoning", id: "r1", summary: "", content: "" }
    expect(convertNotification({ method: "item/started", params: { item, threadId: THREAD, turnId: TURN } }, state)).toEqual(
      [{ id: "r1", type: "reasoning-start" }],
    )
    expect(
      convertNotification(
        { method: "item/reasoning/textDelta", params: { threadId: THREAD, turnId: TURN, itemId: "r1", contentIndex: 0, delta: "think" } },
        state,
      ),
    ).toEqual([{ delta: "think", id: "r1", type: "reasoning-delta" }])
  })

  it("finalizes a tool call on item/completed(commandExecution)", () => {
    const state = createStreamState()
    const item: TurnItem = {
      type: "commandExecution",
      id: "c1",
      command: "ls",
      cwd: "/tmp",
      status: "completed",
      aggregatedOutput: "a\nb",
    }
    const chunks = convertNotification(
      { method: "item/completed", params: { item, threadId: THREAD, turnId: TURN } },
      state,
    )
    expect(chunks).toHaveLength(1)
    expect(chunks[0]).toMatchObject({
      toolCallId: "c1",
      toolName: "commandExecution",
      type: "tool-input-available",
    })
  })

  it("emits finish chunks on turn/completed and stops after", () => {
    const state = createStreamState()
    convertNotification(agentMessageStarted("i1"), state)
    const chunks = convertNotification(
      { method: "turn/completed", params: { threadId: THREAD, turn: { id: TURN, items: [], status: "completed" } } },
      state,
    )
    expect(chunks).toEqual([
      { id: "i1", type: "text-end" },
      { type: "finish-step" },
      { finishReason: "stop", type: "finish" },
    ])
    expect(state.finished).toBe(true)
  })

  it("surfaces error notifications and treats failed turns as errors", () => {
    const state = createStreamState()
    const errorChunks = convertNotification(
      { method: "error", params: { code: "turn_failed", message: "boom" } },
      state,
    )
    expect(errorChunks).toEqual([{ errorText: "boom", type: "error" }])

    const failed = convertNotification(
      { method: "turn/completed", params: { threadId: THREAD, turn: { id: TURN, items: [], status: "failed" } } },
      createStreamState(),
    )
    expect(failed.at(-1)).toEqual({ finishReason: "error", type: "finish" })
  })

  it("ignores lifecycle no-ops", () => {
    const state = createStreamState()
    expect(
      convertNotification(
        { method: "turn/started", params: { threadId: THREAD, turn: { id: TURN, items: [], status: "inProgress" } } },
        state,
      ),
    ).toEqual([])
  })
})

describe("harness isTerminalNotification", () => {
  it("marks turn/completed and error as terminal", () => {
    expect(
      isTerminalNotification({
        method: "turn/completed",
        params: { threadId: THREAD, turn: { id: TURN, items: [], status: "completed" } },
      }),
    ).toBe(true)
    expect(isTerminalNotification({ method: "error", params: { code: "x", message: "y" } })).toBe(true)
  })

  it("does not mark item deltas as terminal", () => {
    expect(
      isTerminalNotification({
        method: "item/agentMessage/delta",
        params: { threadId: THREAD, turnId: TURN, itemId: "i1", delta: "x" },
      }),
    ).toBe(false)
  })
})
