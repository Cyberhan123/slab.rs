import { afterAll, beforeAll, describe, expect, it } from "vitest"
import { chromium, type Browser, type BrowserContext, type Page } from "playwright"

import {
  bootstrapLocalModel,
  cleanupFullstackDevEnvironment,
  createFullstackDevEnvironment,
  createSession,
  listSessions,
  restoreSession,
  selectAssistantSession,
  startFullstackDev,
  type FullstackDevEnvironment,
  type ManagedDevProcess,
  type SessionResponse,
} from "./support/fullstack-dev"
import {
  expectAssistantPageText,
  openAssistant,
  sendAssistantMessage,
  waitForCompletedAssistantReply,
  waitForComposerReady,
  waitForCurrentAssistantSession,
} from "./support/assistant-ui"

let env: FullstackDevEnvironment | undefined

describe.sequential("assistant e2e", () => {
  let browser: Browser | undefined
  let context: BrowserContext | undefined
  let dev: ManagedDevProcess | undefined
  let page: Page
  let session: SessionResponse

  beforeAll(async () => {
    env = await createFullstackDevEnvironment()
    dev = await startFullstackDev(env)
    await bootstrapLocalModel(env.serverBaseUrl, {
      modelId: "Qwen2.5-0.5B-Instruct",
      selectedVariantId: "Q4_K_M",
    })
    session = await createSession(env.serverBaseUrl, `assistant-e2e-${Date.now()}`)
    await selectAssistantSession(env.serverBaseUrl, session.id, session.name)

    browser = await chromium.launch({ headless: true })
    context = await browser.newContext({
      viewport: { width: 1440, height: 960 },
    })
    await context.addInitScript(() => {
      window.localStorage.setItem("slab.ui.language", "en-US")
    })
    page = await context.newPage()
    await openAssistant(page, env.uiBaseUrl)
  })

  afterAll(async () => {
    await context?.close().catch(() => {})
    await browser?.close().catch(() => {})
    await dev?.stop().catch(() => {})
    cleanupFullstackDevEnvironment(env)
  })

  it("drives UI assistant inference through /v1/agents/responses and restores the session", async () => {
    const testEnv = requireEnv()
    const runId = `assistant-${Date.now()}`
    const prompt = `Assistant E2E ${runId}. Reply with one short sentence that includes ${runId}.`

    await sendAssistantMessage(page, prompt)
    await expectAssistantPageText(page, prompt)

    const reply = await waitForCompletedAssistantReply(testEnv.serverBaseUrl, session.id, prompt)
    expect(reply.restore.thread?.status).toBe("completed")
    expect(reply.text.trim().length).toBeGreaterThan(0)
    await expectAssistantPageText(page, reply.text)

    const restored = await restoreSession(testEnv.serverBaseUrl, session.id)
    expect(restored.messages.some((message) => message.role === "user" && message.content === prompt)).toBe(true)
    expect(restored.messages.some((message) => message.role === "assistant" && message.content.trim().length > 0)).toBe(true)

    await page.getByTestId("assistant-summary-desktop-new-session-button").click()
    const secondSessionId = await waitForCurrentAssistantSession(
      testEnv.serverBaseUrl,
      (sessionId) => sessionId !== session.id
    )
    await page.getByTestId("assistant-empty-state").waitFor({ state: "visible", timeout: 90_000 })

    await page.reload({ waitUntil: "domcontentloaded", timeout: 60_000 })
    // A full reload re-mounts the app, which re-fires the WorkspaceModeSync
    // `/`→`/workspace` redirect. Re-enter the Assistant route via the sidebar
    // link (client-side nav) so the composer renders. See openAssistant().
    await page.getByTestId("sidebar-link-assistant").click()
    await waitForComposerReady(page)
    await waitForCurrentAssistantSession(
      testEnv.serverBaseUrl,
      (sessionId) => sessionId === secondSessionId
    )
    await page.getByTestId("assistant-empty-state").waitFor({ state: "visible", timeout: 90_000 })

    const sessions = await listSessions(testEnv.serverBaseUrl)
    expect(sessions.some((item) => item.id === session.id)).toBe(true)
    expect(sessions.some((item) => item.id === secondSessionId)).toBe(true)
  })
})

function requireEnv(): FullstackDevEnvironment {
  if (!env) {
    throw new Error("Fullstack dev environment was not initialized.")
  }

  return env
}
