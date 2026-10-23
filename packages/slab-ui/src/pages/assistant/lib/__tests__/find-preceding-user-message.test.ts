import { describe, expect, it } from "vitest"

import { findPrecedingUserMessage } from "../find-preceding-user-message"

type Msg = {
  id: string
  role: string
  parts?: ReadonlyArray<{ type: string; text?: string }>
}

function msg(id: string, role: string, parts: Msg["parts"] = []): Msg {
  return { id, role, parts }
}

describe("findPrecedingUserMessage", () => {
  it("returns the nearest earlier user message with its concatenated text", () => {
    const messages = [
      msg("u1", "user", [{ type: "text", text: "hello " }, { type: "text", text: "world" }]),
      msg("a1", "assistant", [{ type: "text", text: "hi" }]),
      msg("u2", "user", [{ type: "text", text: "again" }]),
      msg("a2", "assistant", [{ type: "text", text: "sure" }]),
    ]

    expect(findPrecedingUserMessage(messages, "a2")).toEqual({ id: "u2", text: "again" })
    expect(findPrecedingUserMessage(messages, "a1")).toEqual({ id: "u1", text: "hello world" })
  })

  it("walks back over interleaved tool and reasoning parts", () => {
    const messages = [
      msg("u1", "user", [{ type: "text", text: "run it" }]),
      msg("t1", "assistant", [{ type: "tool-call" }]),
      msg("r1", "assistant", [{ type: "reasoning" }]),
      msg("a1", "assistant", [{ type: "text", text: "done" }]),
    ]

    expect(findPrecedingUserMessage(messages, "a1")).toEqual({ id: "u1", text: "run it" })
  })

  it("returns null when the id is unknown, first, or has no earlier user message", () => {
    const messages = [msg("a1", "assistant", [{ type: "text", text: "first" }])]

    expect(findPrecedingUserMessage(messages, "missing")).toBeNull()
    expect(findPrecedingUserMessage(messages, "a1")).toBeNull()
    expect(findPrecedingUserMessage([], "a1")).toBeNull()
  })

  it("trims the joined text so whitespace-only prompts report as empty", () => {
    const messages = [
      msg("u1", "user", [{ type: "text", text: "  " }]),
      msg("a1", "assistant", [{ type: "text", text: "hmm" }]),
    ]

    expect(findPrecedingUserMessage(messages, "a1")).toEqual({ id: "u1", text: "" })
  })
})
