import { describe, expect, it } from "vitest"

import { turnItemToUiParts, turnItemsToMessages, toolItemFields } from "../turn-items"
import type { TurnItem } from "@slab/api/harness"

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

  it("strips legacy embedded <think> blocks from agentMessage text", () => {
    // Rollout files written before the server-side strip carry the
    // LLM-context form (reasoning wrapped in <think status="done">) in the
    // persisted item; history must render only the visible text.
    const messages = turnItemsToMessages([
      user("u1", "hello"),
      {
        type: "reasoning",
        id: "r1",
        summary: "trace",
        content: "the trace",
      },
      agent("a1", '<think status="done">\n\nthe trace\n\n</think>\n\nthe answer'),
    ])
    const assistant = messages[1]
    const text = assistant.parts.find((p) => p.type === "text")
    expect(text).toMatchObject({ type: "text", text: "the answer" })
  })

  it("drops an agentMessage whose text is only an embedded <think> block", () => {
    expect(
      turnItemToUiParts(agent("a1", '<think status="done">\n\nonly thinking\n\n</think>')),
    ).toEqual([])
  })

  it("keeps unterminated or lookalike think markup verbatim", () => {
    expect(turnItemToUiParts(agent("a1", "before<think>never closes"))).toEqual([
      { type: "text", text: "before<think>never closes" },
    ])
    expect(turnItemToUiParts(agent("a1", "<thinking>not a think tag</thinking>"))).toEqual([
      { type: "text", text: "<thinking>not a think tag</thinking>" },
    ])
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

  it("maps a generic toolCall to a tool-<name> part with its arguments", () => {
    const parts = turnItemToUiParts({
      type: "toolCall",
      id: "t1",
      tool: "read_file",
      arguments: { path: "src/main.rs" },
      status: "completed",
      result: "fn main() {}",
      durationMs: 4,
    })
    expect(parts).toHaveLength(1)
    expect(parts[0]).toMatchObject({
      type: "tool-read_file",
      toolCallId: "t1",
      toolName: "read_file",
      input: { path: "src/main.rs" },
      state: "output-available",
      output: "fn main() {}",
    })

    // Failed calls route the payload into errorText and mark output-error.
    expect(
      turnItemToUiParts({
        type: "toolCall",
        id: "t2",
        tool: "grep",
        arguments: { pattern: "todo" },
        status: "failed",
        error: "no such file",
      })[0],
    ).toMatchObject({ type: "tool-grep", state: "output-error", errorText: "no such file" })

    // Running calls (no result yet) stay input-shaped without failing.
    expect(
      toolItemFields({
        type: "toolCall",
        id: "t3",
        tool: "git_status",
        arguments: {},
        status: "running",
      }),
    ).toMatchObject({ toolName: "git_status", input: {}, failed: false })
  })

  it("maps a plan item to a tool-plan part carrying the full plan", () => {
    const plan = {
      plan_id: "plan-0",
      summary: "ship it",
      items: [{ step: "do", status: "pending" as const }],
      counts: { pending: 1, in_progress: 0, completed: 0, blocked: 0 },
    }
    const fields = toolItemFields({ type: "plan", id: "p1", plan })
    expect(fields).toMatchObject({ toolName: "plan", input: plan, output: plan, failed: false })

    const parts = turnItemToUiParts({ type: "plan", id: "p1", plan })
    expect(parts).toHaveLength(1)
    expect(parts[0]).toMatchObject({
      type: "tool-plan",
      toolCallId: "p1",
      state: "output-available",
      output: plan,
    })
  })
})

describe("harness turnItemToUiParts (multimodal image rendering)", () => {
  it("renders an imageView artifact path as an inline image file part", () => {
    const parts = turnItemToUiParts({
      type: "imageView",
      id: "iv1",
      path: "/v1/images/generations/op-1/artifacts/0",
    })
    expect(parts).toHaveLength(1)
    expect(parts[0]).toMatchObject({
      type: "file",
      mediaType: "image/png",
      url: expect.stringMatching(/\/v1\/images\/generations\/op-1\/artifacts\/0$/),
    })
  })

  it("renders a data-URL user image content inline", () => {
    const messages = turnItemsToMessages([
      {
        type: "userMessage",
        id: "u1",
        content: [
          { type: "image", image_url: "data:image/png;base64,iVBOR=", mime_type: "image/png" },
        ],
      },
    ])
    expect(messages).toHaveLength(1)
    expect(messages[0].parts[0]).toMatchObject({
      type: "file",
      mediaType: "image/png",
      url: "data:image/png;base64,iVBOR=",
    })
  })
})
