import { beforeAll, describe, expect, inject, it } from "vitest"

import type { E2eRuntimeEndpoints } from "./support/e2e-global-setup"
import { createSession, eventually, requestJson, restoreSession } from "./support/e2e-runtime"
import { isMessageAppend, isSessionMeta, readRolloutLines } from "./support/rollout-files"

/**
 * Case ④ — single-shot Responses API (`POST /v1/agents/responses`).
 *
 * Unlike the harness turn loop, single-shot persists out-of-band via
 * `RolloutConversationStore::append_message` directly (no `EventMsg` observer,
 * no `await_durable`). This verifies that path end-to-end through the rollout
 * file: the input + reply must each land as a `TurnContext.MessageAppend` line,
 * readable by a subsequent history read.
 */
type ResponsesPayload = Record<string, unknown>

let env: E2eRuntimeEndpoints | undefined

describe("single-shot Responses API rollout persistence", () => {
  beforeAll(() => {
    env = inject("e2e-runtime")
  })

  it("persists input + reply out-of-band into the rollout file", async () => {
    const testEnv = requireEnv()
    const session = await createSession(testEnv.serverBaseUrl, `responses-e2e-${Date.now()}`)
    const marker = `SLAB_RESPONSES_E2E_${Date.now()}`
    const input = `Reply with only the token ${marker} and nothing else.`

    // Bearer = slab session id; the handler resolves/creates the thread in it.
    const reply = await requestJson<ResponsesPayload>(testEnv.serverBaseUrl, "/v1/agents/responses", {
      method: "POST",
      headers: { authorization: `Bearer ${session.id}` },
      json: { model: testEnv.modelId, input, stream: false },
    })
    expect(typeof (reply as { id?: unknown }).id).toBe("string")

    // The history read path flushes the recorder, so by the time this returns
    // the rollout writes are durable on disk.
    const restored = await restoreSession(testEnv.serverBaseUrl, session.id)
    expect(restored.thread?.id).toBeTruthy()
    const threadId = restored.thread?.id ?? ""
    expect(restored.messages.some((m) => m.role === "user" && m.content === input)).toBe(true)
    expect(
      restored.messages.some((m) => m.role === "assistant" && m.content.trim().length > 0),
    ).toBe(true)

    // The rollout file itself carries the out-of-band writes (SessionMeta first,
    // then a MessageAppend per appended message — no observer involved).
    await eventually("rollout file records the single-shot writes", async () => {
      const lines = readRolloutLines(testEnv.sessionStateDir, threadId)
      if (!lines.length) return false
      expect(isSessionMeta(lines[0]!)).toBe(true)
      const appends = lines.filter(isMessageAppend)
      expect(appends.length).toBeGreaterThanOrEqual(2) // user input + assistant reply
      return true
    })
  }, 900_000)
})

function requireEnv(): E2eRuntimeEndpoints {
  if (!env) {
    throw new Error("e2e shared runtime endpoints were not provided.")
  }
  return env
}
