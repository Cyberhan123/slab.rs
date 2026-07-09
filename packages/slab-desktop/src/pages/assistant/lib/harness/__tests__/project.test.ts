import { describe, expect, it } from "vitest"

import { projectThread } from "../project"
import type { Thread, TurnItem } from "../types"

function thread(turns: Array<{ id: string; items: TurnItem[]; status?: string }>): Thread {
  return {
    id: "hthread-1",
    preview: "",
    modelProvider: "",
    createdAt: 0,
    turns: turns.map((turn) => ({
      id: turn.id,
      items: turn.items,
      status: turn.status ?? "completed",
    })),
  }
}

const userMsg = (id: string, text: string): TurnItem => ({
  type: "userMessage",
  id,
  content: [{ type: "text", text }],
})

const agentMsg = (id: string, text: string): TurnItem => ({
  type: "agentMessage",
  id,
  text,
})

describe("harness projectThread", () => {
  it("projects user + assistant messages in turn order", () => {
    const messages = projectThread(
      thread([
        { id: "0", items: [userMsg("u1", "hello"), agentMsg("a1", "hi there")] },
        { id: "1", items: [userMsg("u2", "again"), agentMsg("a2", "yes")] },
      ]),
    )

    expect(messages.map((m) => m.role)).toEqual(["user", "assistant", "user", "assistant"])
    expect(messages[0]).toEqual({ id: "u1", role: "user", parts: [{ text: "hello", type: "text" }] })
    expect(messages[1].role).toBe("assistant")
    expect(messages[1].parts).toEqual([{ text: "hi there", type: "text" }])
  })

  it("groups consecutive assistant-side items into one assistant message", () => {
    const messages = projectThread(
      thread([
        {
          id: "0",
          items: [
            userMsg("u1", "run it"),
            { type: "reasoning", id: "r1", summary: "thinking", content: "thinking" },
            agentMsg("a1", "done"),
            {
              type: "commandExecution",
              id: "c1",
              command: "ls",
              cwd: "/tmp",
              status: "completed",
              aggregatedOutput: "a",
            },
          ],
        },
      ]),
    )

    expect(messages).toHaveLength(2)
    expect(messages[0].role).toBe("user")
    const assistant = messages[1]
    expect(assistant.role).toBe("assistant")
    expect(assistant.parts.map((p) => p.type)).toEqual(["reasoning", "text", "text"])
  })

  it("returns no messages for an empty thread", () => {
    expect(projectThread(thread([]))).toEqual([])
  })
})
