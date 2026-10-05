/**
 * Scripted questionnaire e2e (dedicated SLAB_E2E_MODE=1 stack, no model).
 *
 * The stack booted by `e2e-questionnaire-global-setup.ts` makes the debug-build
 * `ServerLlmAdapter` return deterministic prompt-keyed responses
 * (`crates/slab-app-core/src/infra/agent/adapter.rs`): a prompt carrying
 * `questionnaire-e2e/ask` triggers a `questionnaire` tool call (single-select,
 * custom input allowed, optional), `questionnaire-e2e/ask-multi` a
 * multi-select required one; the resumed turn (after the answer tool result)
 * echoes the answer payload verbatim so the suite can assert the full
 * round-trip: tool call → answer notification → banner card → UI answer →
 * `questionnaire/resolve` → answer JSON fed back to the model.
 *
 * Covered scenarios:
 *   1. single-select: pick a choice, submit, answer flows back to the model
 *   2. custom free-text answer instead of a choice
 *   3. multi-select required: empty submit is blocked, then two choices land
 */
import { afterAll, beforeAll, beforeEach, afterEach, describe, expect, inject, it } from "vitest"
import { chromium, type Browser, type Page } from "playwright"

import { createSession, type SessionResponse } from "./support/e2e-runtime"
import {
  openAssistant,
  sendAssistantMessage,
  waitForCompletedAssistantReply,
} from "./support/assistant-ui"
import type { QuestionnaireRuntimeEndpoints } from "./support/e2e-questionnaire-global-setup"

const endpoints = inject("e2e-questionnaire-runtime") as QuestionnaireRuntimeEndpoints
const baseUrl = endpoints.serverBaseUrl

let browser: Browser
let session: SessionResponse
let page: Page
const debugConsoleErrors: string[] = []

beforeAll(async () => {
  browser = await chromium.launch({ headless: true })
})

beforeEach(async () => {
  session = await createSession(baseUrl)
  const context = await browser.newContext({ viewport: { width: 1440, height: 960 } })
  await context.addInitScript(() => {
    window.localStorage.setItem("slab.ui.language", "en-US")
  })
  page = await context.newPage()
  page.on("close", () => context.close().catch(() => {}))
  debugConsoleErrors.length = 0
  page.on("console", (message) => {
    if (message.type() === "error") debugConsoleErrors.push(message.text())
  })
  page.on("pageerror", (error) => debugConsoleErrors.push(`pageerror: ${error.message}`))
  await openAssistant(page, endpoints.uiBaseUrl, session.id)
})

afterEach(async () => {
  await page.close().catch(() => {})
})

afterAll(async () => {
  await browser.close().catch(() => {})
})

describe("questionnaire tool round-trip (scripted)", () => {
  it("single-select: a picked choice resolves and reaches the model", async () => {
    const prompt = "please questionnaire-e2e/ask now"
    await sendAssistantMessage(page, prompt)

    // The answer card renders above the composer with the scripted question.
    const card = page.getByTestId("assistant-questionnaire-card")
    await card.waitFor({ state: "visible", timeout: 60_000 })
    await page.getByTestId("assistant-questionnaire-choice-vim").waitFor({ state: "visible" })
    await page.getByTestId("assistant-questionnaire-choice-vscode").waitFor({ state: "visible" })

    // Pick a choice and submit; the card disappears once resolved.
    await page.getByTestId("assistant-questionnaire-choice-vscode").click()
    await page.getByTestId("assistant-questionnaire-submit").click()
    await card.waitFor({ state: "detached", timeout: 30_000 })

    // The resumed turn echoes the answer payload the UI sent.
    const reply = await waitForCompletedAssistantReply(baseUrl, session.id, prompt)
    expect(reply.text).toContain("E2E questionnaire answers")
    expect(reply.text).toContain('"selected"')
    expect(reply.text).toContain("vscode")
  })

  it("custom free-text answer flows back instead of a choice", async () => {
    const prompt = "please questionnaire-e2e/ask with custom"
    await sendAssistantMessage(page, prompt)

    const card = page.getByTestId("assistant-questionnaire-card")
    await card.waitFor({ state: "visible", timeout: 60_000 })

    // The custom input is offered (the only textbox inside the card).
    const custom = card.getByRole("textbox")
    await custom.fill("Emacs")
    await page.getByTestId("assistant-questionnaire-submit").click()
    await card.waitFor({ state: "detached", timeout: 30_000 })

    const reply = await waitForCompletedAssistantReply(baseUrl, session.id, prompt)
    expect(reply.text).toContain("E2E questionnaire answers")
    expect(reply.text).toContain("Emacs")
  })

  it("multi-select required: empty submit blocked, then both choices land", async () => {
    const prompt = "please questionnaire-e2e/ask-multi now"
    await sendAssistantMessage(page, prompt)

    const card = page.getByTestId("assistant-questionnaire-card")
    await card.waitFor({ state: "visible", timeout: 60_000 })

    // Checkboxes (multi-select), no skip button (required).
    await card.getByRole("checkbox", { name: "Vim" }).waitFor({ state: "visible" })
    await card.getByRole("checkbox", { name: "VS Code" }).waitFor({ state: "visible" })
    expect(await page.getByTestId("assistant-questionnaire-skip").count()).toBe(0)

    // Empty submit is refused client-side: the card stays and nags.
    await page.getByTestId("assistant-questionnaire-submit").click()
    await card.waitFor({ state: "visible", timeout: 5_000 })

    // Select both, submit, and the answer carries both values.
    await card.getByRole("checkbox", { name: "Vim" }).click()
    await card.getByRole("checkbox", { name: "VS Code" }).click()
    await page.getByTestId("assistant-questionnaire-submit").click()
    await card.waitFor({ state: "detached", timeout: 30_000 })

    const reply = await waitForCompletedAssistantReply(baseUrl, session.id, prompt)
    expect(reply.text).toContain("E2E questionnaire answers")
    expect(reply.text).toContain("vim")
    expect(reply.text).toContain("vscode")
  })
})
