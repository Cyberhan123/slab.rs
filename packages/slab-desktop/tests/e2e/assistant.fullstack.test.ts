import { afterAll, beforeAll, describe, expect, it } from "vitest"
import { chromium, type Browser, type BrowserContext, type Locator, type Page } from "playwright"

import {
  cleanupFullstackDevEnvironment,
  createFullstackDevEnvironment,
  eventually,
  getPersistedUiState,
  listSessions,
  restoreSession,
  seedBackend,
  startFullstackDev,
  type FullstackDevEnvironment,
  type ManagedDevProcess,
  type SessionResponse,
} from "./support/fullstack-dev"

let env: FullstackDevEnvironment | undefined

describe.sequential("assistant fullstack e2e", () => {
  let browser: Browser | undefined
  let context: BrowserContext | undefined
  let dev: ManagedDevProcess | undefined
  let page: Page

  beforeAll(async () => {
    env = await createFullstackDevEnvironment()
    dev = await startFullstackDev(env)
    await seedBackend(env.serverBaseUrl)

    browser = await chromium.launch({ headless: true })
    context = await browser.newContext({
      viewport: { width: 1440, height: 960 },
    })
    await context.addInitScript(() => {
      window.localStorage.setItem("slab.ui.language", "en-US")
    })
    page = await context.newPage()
    await page.goto(env.uiBaseUrl, { waitUntil: "domcontentloaded", timeout: 60_000 })
    await waitForComposerReady(page)
  })

  afterAll(async () => {
    await context?.close().catch(() => {})
    await browser?.close().catch(() => {})
    await dev?.stop().catch(() => {})
    cleanupFullstackDevEnvironment(env)
  })

  it("drives assistant execution, tool loops, persistence, and session management", async () => {
    const testEnv = requireEnv()
    const runId = `assistant-fullstack-${Date.now()}`
    const firstPrompt = `Persist this assistant e2e message ${runId}`
    const firstReply = `E2E assistant persisted reply: ${firstPrompt}`
    const firstSessionLabel =
      firstPrompt.length > 42 ? `${firstPrompt.slice(0, 42)}...` : firstPrompt

    await sendAssistantMessage(page, firstPrompt)
    await expectAssistantMessage(page, firstPrompt)
    await expectAssistantMessage(page, firstReply)

    const firstSession = await waitForSessionNamed(testEnv, firstSessionLabel)
    const firstRestore = await restoreSession(testEnv.serverBaseUrl, firstSession.id)
    expect(firstRestore.thread?.status).toBe("completed")
    expect(firstRestore.messages.some((message) => message.role === "user" && message.content === firstPrompt)).toBe(true)
    expect(firstRestore.messages.some((message) => message.role === "assistant" && message.content === firstReply)).toBe(true)

    const loopPrompt = `Run the tool loop ${runId}`
    await sendAssistantMessage(page, loopPrompt)
    await expectAssistantMessage(page, loopPrompt)
    await expectAssistantMessage(page, "e2e assistant loop")
    await expectAssistantMessage(page, "in_progress: record plan")
    await expectAssistantMessage(page, "E2E loop complete after plan_update tool output.")

    const loopRestore = await restoreSession(testEnv.serverBaseUrl, firstSession.id)
    expect(loopRestore.thread?.status).toBe("completed")
    expect(
      loopRestore.messages.some(
        (message) =>
          message.role === "assistant" &&
          (message.tool_calls ?? []).some((toolCall) => toolCall.function.name === "plan_update")
      )
    ).toBe(true)
    expect(
      loopRestore.messages.some(
        (message) =>
          message.role === "tool" &&
          message.content.includes("\"summary\":\"e2e assistant loop\"")
      )
    ).toBe(true)
    expect(
      loopRestore.messages.some(
        (message) =>
          message.role === "assistant" &&
          message.content === "E2E loop complete after plan_update tool output."
      )
    ).toBe(true)

    await page.getByTestId("assistant-summary-desktop-new-session-button").click()
    const secondSessionId = await waitForCurrentAssistantSession(
      testEnv,
      (sessionId) => sessionId !== firstSession.id
    )
    await page.getByTestId("assistant-empty-state").waitFor({ state: "visible", timeout: 60_000 })

    const restoredAfterSessionSwitch = await restoreSession(testEnv.serverBaseUrl, firstSession.id)
    expect(restoredAfterSessionSwitch.thread?.status).toBe("completed")
    expect(restoredAfterSessionSwitch.messages.some((message) => message.content === firstReply)).toBe(true)
    expect(
      restoredAfterSessionSwitch.messages.some(
        (message) => message.role === "tool" && message.content.includes("e2e assistant loop")
      )
    ).toBe(true)

    await page.reload({ waitUntil: "domcontentloaded", timeout: 60_000 })
    await waitForComposerReady(page)
    await waitForCurrentAssistantSession(testEnv, (sessionId) => sessionId === secondSessionId)
    await page.getByTestId("assistant-empty-state").waitFor({ state: "visible", timeout: 60_000 })
    await expectNoAssistantMessage(page, firstPrompt)

    await deleteSessionFromSheet(page, firstSession)

    await eventually("deleted session disappears from backend list", async () => {
      const sessions = await listSessions(testEnv.serverBaseUrl)
      return sessions.every((session) => session.id !== firstSession.id)
    })
  })
})

async function sendAssistantMessage(page: Page, message: string): Promise<void> {
  const composer = await waitForComposerReady(page)
  await composer.fill(message)
  await page.getByTestId("assistant-send-button").locator("button").click()
}

async function waitForComposerReady(page: Page): Promise<Locator> {
  const composer = page.getByTestId("assistant-composer-input").locator("textarea")
  await composer.waitFor({ state: "visible", timeout: 60_000 })
  await eventually("assistant composer is editable", async () => composer.isEditable())
  return composer
}

async function expectAssistantMessage(page: Page, text: string): Promise<void> {
  await assistantMessageByText(page, text).waitFor({ state: "visible", timeout: 60_000 })
}

async function expectNoAssistantMessage(page: Page, text: string): Promise<void> {
  await eventually("assistant message is absent", async () => {
    const count = await assistantMessageByText(page, text).count()
    return count === 0
  })
}

function assistantMessageByText(page: Page, text: string): Locator {
  return page.locator('[data-testid^="assistant-message-"]').filter({ hasText: text }).first()
}

async function waitForSessionNamed(
  testEnv: FullstackDevEnvironment,
  name: string
): Promise<SessionResponse> {
  return eventually("assistant session label persisted", async () => {
    const sessions = await listSessions(testEnv.serverBaseUrl)
    return sessions.find((session) => session.name === name)
  })
}

async function waitForCurrentAssistantSession(
  testEnv: FullstackDevEnvironment,
  predicate: (sessionId: string) => boolean
): Promise<string> {
  return eventually("assistant current session persisted", async () => {
    const state = await getPersistedUiState<{ currentSessionId?: string }>(
      testEnv.serverBaseUrl,
      "zustand:assistant-ui"
    )
    const currentSessionId = state?.currentSessionId
    if (!currentSessionId || !predicate(currentSessionId)) {
      return null
    }
    const sessions = await listSessions(testEnv.serverBaseUrl)
    return sessions.some((session) => session.id === currentSessionId) ? currentSessionId : null
  })
}

async function deleteSessionFromSheet(page: Page, session: SessionResponse): Promise<void> {
  await page.getByTestId("assistant-summary-desktop-manage-sessions-button").click()
  await page.getByTestId("assistant-session-sheet").waitFor({ state: "visible", timeout: 30_000 })
  await page.getByTestId(`assistant-session-actions-${session.id}`).click()
  await page.getByTestId(`assistant-session-delete-${session.id}`).click()
}

function requireEnv(): FullstackDevEnvironment {
  if (!env) {
    throw new Error("Fullstack dev environment was not initialized.")
  }

  return env
}
