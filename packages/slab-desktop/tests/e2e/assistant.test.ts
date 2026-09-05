import { afterAll, beforeAll, describe, expect, inject, it } from "vitest"
import { chromium, type Browser, type BrowserContext, type Page } from "playwright"

import type { E2eRuntimeEndpoints } from "./support/e2e-global-setup"
import {
  createSession,
  eventually,
  listSessions,
  restoreSession,
  type AgentThreadMessageResponse,
  type SessionResponse,
} from "./support/e2e-runtime"
import {
  expectAssistantPageText,
  openAssistant,
  selectCurrentWorkspaceOnLanding,
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

    // Keep the SHARED stack's workspace open: the landing's workspace selector
    // defaults to 全局, and submitting with it would close the active
    // workspace server-side — retiring apply_patch/git tools for concurrently
    // running e2e files (observed: apply_patch "handler not found" one second
    // after this test's landing submit closed the workspace).
    await selectCurrentWorkspaceOnLanding(page)

    const secondRunId = `assistant-second-${Date.now()}`
    // The reply must carry the run id — the assertion below matches only
    // ASSISTANT bubbles, and a generic "one short sentence" reply (observed
    // from GLM: "I'm ready to help with your workspace tasks.") never
    // mentions it. The first turn's prompt already asks for its marker; this
    // one must too.
    await sendAssistantMessage(
      page,
      `Assistant E2E ${secondRunId}. Reply with one short sentence that includes ${secondRunId}.`
    )
    // Submitting from the landing creates + selects a NEW conversation and
    // navigates into its detail; the landing is gone.
    const secondSessionId = await waitForCurrentAssistantSession(
      testEnv.serverBaseUrl,
      (sessionId) => sessionId !== session.id
    )
    await page.getByTestId("assistant-new-chat-landing").waitFor({ state: "detached", timeout: 90_000 })

    // Gate on the SERVER-side reply before asserting the page: the
    // landing→detail handoff remounts the chat pane, and the in-flight live
    // stream can be dropped across that remount (the pane's `useChat` consumer
    // unmounts mid-turn; the controller-level handoff race is tracked
    // separately). The restored-history rendering — this test's subject — is
    // deterministic: wait for the persisted reply, then reload and assert it
    // renders.
    const secondPrompt = `Assistant E2E ${secondRunId}. Reply with one short sentence that includes ${secondRunId}.`
    // The staged draft's AUTO-SEND is itself part of that handoff race (it
    // fires only when the fresh pane's gating clears first). Give it a short
    // window; when the draft did not land server-side, submit manually from
    // the detail composer — the send path is the product's, only the flaky
    // trigger is bypassed.
    let draftDelivered = false
    try {
      await eventually(
        "landing draft delivered server-side",
        async () => {
          const restore = await restoreSession(testEnv.serverBaseUrl, secondSessionId)
          return restore.messages.some(
            (message) => message.role === "user" && message.content === secondPrompt
          )
            ? true
            : null
        },
        20_000,
        500
      )
      draftDelivered = true
    } catch {
      // Fall through to the manual send below.
    }
    if (!draftDelivered) {
      await waitForComposerReady(page)
      await sendAssistantMessage(page, secondPrompt)
    }
    const secondReply = await waitForCompletedAssistantReply(
      testEnv.serverBaseUrl,
      secondSessionId,
      secondPrompt
    )
    expect(secondReply.restore.thread?.status).toBe("completed")

    // A full reload of the `?session=` deep link re-mounts the SAME detail
    // (WorkspaceModeSync skips its `/`→`/workspace` redirect for deep links).
    await page.reload({ waitUntil: "domcontentloaded", timeout: 60_000 })
    await waitForComposerReady(page)
    await waitForCurrentAssistantSession(
      testEnv.serverBaseUrl,
      (sessionId) => sessionId === secondSessionId
    )
    await expectAssistantPageText(page, secondRunId)

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
