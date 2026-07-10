import { describe, expect, it } from "vitest"

import { turnItemToUiParts, turnItemsToMessages, toolItemFields } from "../turn-items"
import type { TurnItem } from "../types"

const user = (id: string, text: string): TurnItem => ({
  type: "userMessage",
  id,
  content: [{ type: "text", text }],
})

const agent = (id: string, text: string): TurnItem => ({ type: "agentMessage", id, text })

describe("harness turnItemsToMessages", () => {
  it("returns no messages for an empty item list", () => {
    expect(turnItemsToMessages([])).toEqual([])
  })

  it("groups items into user / assistant messages by user-boundary", () => {
    const messages = turnItemsToMessages([
      user("u1", "hello"),
      agent("a1", "hi there"),
      user("u2", "again"),
      agent("a2", "yes"),
    ])
    expect(messages.map((m) => m.role)).toEqual(["user", "assistant", "user", "assistant"])
    expect(messages[0]).toMatchObject({ id: "u1", role: "user", parts: [{ type: "text", text: "hello" }] })
    expect(messages[1]).toMatchObject({ id: "a1", role: "assistant" })
  })

  it("folds consecutive assistant-side items into one assistant message in order", () => {
    const messages = turnItemsToMessages([
      user("u1", "run it"),
      {
        type: "reasoning",
        id: "r1",
        summary: "recap",
        content: "full trace",
      },
      agent("a1", "done"),
      {
        type: "commandExecution",
        id: "c1",
        command: "ls",
        cwd: "/tmp",
        status: "completed",
        aggregatedOutput: "out",
        exitCode: 0,
      },
    ])
    expect(messages).toHaveLength(2)
    const assistant = messages[1]
    expect(assistant.role).toBe("assistant")
    expect(assistant.parts.map((p) => p.type)).toEqual([
      "reasoning",
      "text",
      "tool-commandExecution",
    ])
    // Reasoning uses `content` (the full trace), not the summary recap.
    const reasoning = assistant.parts[0] as { type: string; text: string }
    expect(reasoning).toMatchObject({ type: "reasoning", text: "full trace" })
    // The command tool part carries its output + completed state.
    const tool = assistant.parts[2] as {
      type: string
      toolCallId: string
      state: string
      output: string
    }
    expect(tool).toMatchObject({
      type: "tool-commandExecution",
      toolCallId: "c1",
      state: "output-available",
      output: "out",
    })
  })

  it("does not emit an empty assistant message when assistant items produce no parts", () => {
    const messages = turnItemsToMessages([agent("a1", "")])
    expect(messages).toEqual([])
  })
})

describe("harness turnItemToUiParts / toolItemFields (ex-lossy cases)", () => {
  it("maps a failed commandExecution to an output-error tool part", () => {
    const parts = turnItemToUiParts({
      type: "commandExecution",
      id: "c1",
      command: "boom",
      cwd: "",
      status: "completed",
      aggregatedOutput: "trace",
      exitCode: 2,
    })
    expect(parts).toHaveLength(1)
    expect(parts[0]).toMatchObject({ type: "tool-commandExecution", state: "output-error", errorText: "trace" })
  })

  it("preserves mcpToolCall result/error and fileChange diff fields", () => {
    expect(
      toolItemFields({
        type: "mcpToolCall",
        id: "m1",
        server: "srv",
        tool: "search",
        arguments: { q: "x" },
        status: "completed",
        result: { hits: 3 },
      }),
    ).toMatchObject({ toolName: "search", output: { hits: 3 }, failed: false })

    expect(
      toolItemFields({
        type: "mcpToolCall",
        id: "m2",
        server: "srv",
        tool: "search",
        arguments: {},
        status: "completed",
        error: { message: "boom" },
      })?.errorText,
    ).toBe(JSON.stringify({ message: "boom" }, null, 2))

    expect(
      toolItemFields({
        type: "fileChange",
        id: "f1",
        changes: [{ path: "/a", type: "edit", diff: "@@" }],
        status: "completed",
      }),
    )?.toMatchObject({ toolName: "fileChange", output: { status: "completed" } })
  })
})
