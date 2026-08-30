import { afterAll, beforeAll, describe, expect, inject, it } from "vitest"
import { chromium, type Browser, type BrowserContext, type Page } from "playwright"

import type { E2eRuntimeEndpoints } from "./support/e2e-global-setup"
import {
  createSession,
  listSessions,
  restoreSession,
  type AgentThreadMessageResponse,
  type SessionResponse,
} from "./support/e2e-runtime"
import {
  expectAssistantPageText,
  openAssistant,
  sendAssistantMessage,
  waitForCompletedAssistantReply,
  waitForComposerReady,
  waitForCurrentAssistantSession,
} from "./support/assistant-ui"

let env: E2eRuntimeEndpoints | undefined

describe("assistant e2e", () => {
  let browser: Browser | undefined
  let context: BrowserContext | undefined
  let page: Page
  let session: SessionResponse

  beforeAll(async () => {
    env = inject("e2e-runtime")
    session = await createSession(env.serverBaseUrl, `assistant-e2e-${Date.now()}`)

    browser = await chromium.launch({ headless: true })
    context = await browser.newContext({
      viewport: { width: 1440, height: 960 },
    })
    await context.addInitScript(() => {
      window.localStorage.setItem("slab.ui.language", "en-US")
    })
    page = await context.newPage()
    // Deep-link the conversation detail — `/` is the new-chat landing now.
    await openAssistant(page, env.uiBaseUrl, session.id)
  })

  afterAll(async () => {
    await context?.close().catch(() => {})
    await browser?.close().catch(() => {})
  })

  it("drives UI assistant inference through /v1/agents/responses and restores the session", async () => {
    const testEnv = requireEnv()
    const runId = `assistant-${Date.now()}`
    const prompt = `Assistant E2E ${runId}. Reply with one short sentence that includes ${runId}.`

    await sendAssistantMessage(page, prompt)
    // Wait for the RUN ID (not the prompt): the prompt instructs the model to
    // include it in the reply, so this deterministically verifies the streamed
    // assistant bubble rendered. Waiting for the full prompt text would depend
    // on the model quoting the request verbatim in its reasoning/reply.
    await expectAssistantPageText(page, runId)

    const reply = await waitForCompletedAssistantReply(testEnv.serverBaseUrl, session.id, prompt)
    expect(reply.restore.thread?.status).toBe("completed")
    expect(reply.text.trim().length).toBeGreaterThan(0)
    await expectAssistantPageText(page, reply.text)

    const restored = await restoreSession(testEnv.serverBaseUrl, session.id)
    expect(restored.messages.some((message: AgentThreadMessageResponse) => message.role === "user" && message.content === prompt)).toBe(true)
    expect(restored.messages.some((message: AgentThreadMessageResponse) => message.role === "assistant" && message.content.trim().length > 0)).toBe(true)

    // The header control leaves for the new-chat landing — the homepage —
    // without creating a session (composing from it does).
    await page.getByTestId("header-new-session-control").click()
    await page.getByTestId("assistant-new-chat-landing").waitFor({ state: "visible", timeout: 90_000 })

    const secondRunId = `assistant-second-${Date.now()}`
    await sendAssistantMessage(page, `Assistant E2E ${secondRunId}. Reply with one short sentence.`)
    // Submitting from the landing creates + selects a NEW conversation and
    // navigates into its detail; the landing is gone.
    const secondSessionId = await waitForCurrentAssistantSession(
      testEnv.serverBaseUrl,
      (sessionId) => sessionId !== session.id
    )
    await page.getByTestId("assistant-new-chat-landing").waitFor({ state: "detached", timeout: 90_000 })
    await expectAssistantPageText(page, secondRunId)

    // A full reload of the `?session=` deep link re-mounts the SAME detail
    // (WorkspaceModeSync skips its `/`→`/workspace` redirect for deep links).
    await page.reload({ waitUntil: "domcontentloaded", timeout: 60_000 })
    await waitForComposerReady(page)
    await waitForCurrentAssistantSession(
      testEnv.serverBaseUrl,
      (sessionId) => sessionId === secondSessionId
    )

    const sessions = await listSessions(testEnv.serverBaseUrl)
    expect(sessions.some((item) => item.id === session.id)).toBe(true)
    expect(sessions.some((item) => item.id === secondSessionId)).toBe(true)
  })
})

function requireEnv(): E2eRuntimeEndpoints {
  if (!env) {
    throw new Error("e2e shared runtime endpoints were not provided.")
  }

  return env
}
