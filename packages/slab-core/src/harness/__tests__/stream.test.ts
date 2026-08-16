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
    // An explicit item/completed(reasoning) closes the part.
    expect(
      convertNotification({ method: "item/completed", params: { item, threadId: THREAD, turnId: TURN } }, state),
    ).toEqual([{ id: "r1", type: "reasoning-end" }])
  })

  it("closes an open reasoning part as soon as the agent message starts", () => {
    // Regression: some backends jump from reasoning deltas straight to
    // item/started(agentMessage) without an item/completed(reasoning). The
    // reasoning part must still close so its "Thinking..." indicator stops.
    const state = createStreamState()
    const r1: TurnItem = { type: "reasoning", id: "r1", summary: "", content: "" }
    convertNotification({ method: "item/started", params: { item: r1, threadId: THREAD, turnId: TURN } }, state)
    expect(state.openReasoning.has("r1")).toBe(true)

    const chunks = convertNotification(agentMessageStarted("i1"), state)
    expect(chunks).toEqual([
      { id: "r1", type: "reasoning-end" },
      { id: "i1", type: "text-start" },
    ])
    expect(state.openReasoning.has("r1")).toBe(false)

    // turn/completed must not re-emit a reasoning-end for the already-closed part.
    const finish = convertNotification(
      { method: "turn/completed", params: { threadId: THREAD, turn: { id: TURN, items: [], status: "completed" } } },
      state,
    )
    expect(finish).toEqual([
      { id: "i1", type: "text-end" },
      { type: "finish-step" },
      { finishReason: "stop", type: "finish" },
    ])
  })

  it("finalizes a commandExecution with input + output chunks", () => {
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
    expect(chunks).toEqual([
      {
        input: { command: "ls", cwd: "/tmp" },
        toolCallId: "c1",
        toolName: "commandExecution",
        type: "tool-input-available",
      },
      { output: "a\nb", toolCallId: "c1", type: "tool-output-available" },
    ])
  })

  it("emits a tool-output-error chunk for a failed commandExecution", () => {
    const state = createStreamState()
    const item: TurnItem = {
      type: "commandExecution",
      id: "c2",
      command: "false",
      cwd: "/tmp",
      status: "completed",
      exitCode: 1,
      aggregatedOutput: "boom",
    }
    const chunks = convertNotification(
      { method: "item/completed", params: { item, threadId: THREAD, turnId: TURN } },
      state,
    )
    expect(chunks).toEqual([
      {
        input: { command: "false", cwd: "/tmp" },
        toolCallId: "c2",
        toolName: "commandExecution",
        type: "tool-input-available",
      },
      { errorText: "boom", toolCallId: "c2", type: "tool-output-error" },
    ])
  })

  it("creates the Running card on item/started(commandExecution)", () => {
    const state = createStreamState()
    const item: TurnItem = {
      type: "commandExecution",
      id: "c9",
      command: "ls",
      cwd: "/tmp",
      status: "running",
    }
    const chunks = convertNotification(
      { method: "item/started", params: { item, threadId: THREAD, turnId: TURN } },
      state,
    )
    expect(chunks).toEqual([
      {
        input: { command: "ls", cwd: "/tmp" },
        toolCallId: "c9",
        toolName: "commandExecution",
        type: "tool-input-available",
      },
    ])
  })

  it("creates the Running card on item/started(fileChange) with the change list", () => {
    const state = createStreamState()
    const item: TurnItem = {
      type: "fileChange",
      id: "f1",
      changes: [{ path: "a.txt", type: "edit", diff: "+hello" }],
      status: "running",
    }
    const chunks = convertNotification(
      { method: "item/started", params: { item, threadId: THREAD, turnId: TURN } },
      state,
    )
    expect(chunks).toEqual([
      {
        input: { changes: [{ path: "a.txt", type: "edit", diff: "+hello" }] },
        toolCallId: "f1",
        toolName: "fileChange",
        type: "tool-input-available",
      },
    ])
  })

  it("keeps one toolCallId across started → approval → completed (single card)", () => {
    // Regression lock for the live "Running + Completed split card" bug: the
    // approval notification, item/started and item/completed must all carry the
    // same id so the AI SDK merges them into one tool part.
    const state = createStreamState()
    const started = convertNotification(
      {
        method: "item/started",
        params: {
          item: { type: "commandExecution", id: "c1", command: "whoami", cwd: "/tmp", status: "running" },
          threadId: THREAD,
          turnId: TURN,
        },
      },
      state,
    )
    const approval = convertNotification(
      {
        method: "item/commandExecution/requestApproval",
        params: { threadId: THREAD, turnId: TURN, itemId: "c1", command: "whoami", cwd: "/tmp", allowedScopes: [] },
      },
      state,
    )
    const completed = convertNotification(
      {
        method: "item/completed",
        params: {
          item: {
            type: "commandExecution",
            id: "c1",
            command: "whoami",
            cwd: "/tmp",
            status: "completed",
            aggregatedOutput: "cyberhan",
          },
          threadId: THREAD,
          turnId: TURN,
        },
      },
      state,
    )
    const all = [...started, ...approval, ...completed]
    const ids = all
      .filter((chunk): chunk is Extract<(typeof all)[number], { toolCallId: string }> =>
        "toolCallId" in chunk,
      )
      .map((chunk) => chunk.toolCallId)
    expect(ids).toEqual(["c1", "c1", "c1", "c1"])
  })

  it("emits a tool-output-error chunk for a failed mcpToolCall", () => {
    const state = createStreamState()
    const item: TurnItem = {
      type: "mcpToolCall",
      id: "m1",
      server: "srv",
      tool: "ping",
      arguments: { host: "x" },
      status: "completed",
      error: { message: "offline" },
    }
    const chunks = convertNotification(
      { method: "item/completed", params: { item, threadId: THREAD, turnId: TURN } },
      state,
    )
    expect(chunks.at(-1)).toMatchObject({
      errorText: JSON.stringify({ message: "offline" }, null, 2),
      toolCallId: "m1",
      type: "tool-output-error",
    })
  })

  it("renders an approval request as a tool-input-available chunk", () => {
    const state = createStreamState()
    const chunks = convertNotification(
      {
        method: "item/commandExecution/requestApproval",
        params: { threadId: THREAD, turnId: TURN, itemId: "c3", command: "rm -rf", cwd: "/", allowedScopes: [] },
      },
      state,
    )
    expect(chunks).toEqual([
      {
        input: { command: "rm -rf", cwd: "/" },
        toolCallId: "c3",
        toolName: "commandExecution",
        type: "tool-input-available",
      },
    ])
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

  it("treats context-compaction lifecycle as out-of-band (no message parts)", () => {
    const state = createStreamState()
    expect(
      convertNotification({ method: "context/compacting", params: { threadId: THREAD } }, state),
    ).toEqual([])
    expect(
      convertNotification(
        {
          method: "context/compacted",
          params: { threadId: THREAD, status: "compacted", removedMessages: 4, outputTokens: 200 },
        },
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
