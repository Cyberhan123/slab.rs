import { afterAll, beforeAll, describe, expect, it } from "vitest"
import { chromium, type Browser, type BrowserContext, type Page } from "playwright"

import {
  bootstrapLocalModel,
  cleanupFullstackDevEnvironment,
  createFullstackDevEnvironment,
  createSession,
  restoreSession,
  selectAssistantSession,
  startFullstackDev,
  type FullstackDevEnvironment,
  type ManagedDevProcess,
  type SessionResponse,
} from "./support/fullstack-dev"
import {
  approvePendingToolCall,
  expectAssistantPageText,
  openAssistant,
  parseToolJson,
  sendAssistantMessage,
  waitForCompletedAssistantReply,
  waitForToolExecution,
} from "./support/assistant-ui"

let env: FullstackDevEnvironment | undefined

describe.sequential("agent e2e", () => {
  let browser: Browser | undefined
  let context: BrowserContext | undefined
  let dev: ManagedDevProcess | undefined
  let page: Page
  let session: SessionResponse

  beforeAll(async () => {
    env = await createFullstackDevEnvironment()
    dev = await startFullstackDev(env)
    await bootstrapLocalModel(env.serverBaseUrl, {
      modelId: "Qwen3.5-9B",
      selectedVariantId: "Q8_0",
    })
    session = await createSession(env.serverBaseUrl, `agent-e2e-${Date.now()}`)
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

  it("runs a UI-started slab-agent tool call with SSE, approval, persistence, and context", async () => {
    const testEnv = requireEnv()
    const marker = `SLAB_AGENT_E2E_${Date.now()}`
    const prompt = [
      `Use the shell tool exactly once to run this PowerShell command: Write-Output ${marker}`,
      "Wait for approval if it is required.",
      `After the tool result, reply with a short sentence containing ${marker}.`,
    ].join("\n")

    await sendAssistantMessage(page, prompt)
    await expectAssistantPageText(page, prompt)
    await approvePendingToolCall(page)

    const toolResult = await waitForToolExecution(
      testEnv.serverBaseUrl,
      session.id,
      prompt,
      "shell",
      1_200_000
    )
    expect(toolResult.finalText).toContain(marker)
    expect(toolResult.toolCalls.length).toBeGreaterThanOrEqual(1)

    const output = parseToolJson(toolResult.toolMessages[0].content)
    expect(String(output.stdout ?? "")).toContain(marker)
    expect(output.exit_code).toBe(0)
    await expectAssistantPageText(page, toolResult.finalText)

    const recallPrompt = `What exact marker did the shell tool output in the previous turn? Reply with only the marker.`
    await sendAssistantMessage(page, recallPrompt)
    const recall = await waitForCompletedAssistantReply(
      testEnv.serverBaseUrl,
      session.id,
      recallPrompt,
      900_000
    )
    expect(recall.text).toContain(marker)

    const restored = await restoreSession(testEnv.serverBaseUrl, session.id)
    expect(restored.thread?.status).toBe("completed")
    expect(
      restored.messages.some(
        (message) =>
          message.role === "assistant" &&
          (message.tool_calls ?? []).some((toolCall) => toolCall.function.name === "shell")
      )
    ).toBe(true)
    expect(
      restored.messages.some(
        (message) => message.role === "tool" && message.content.includes(marker)
      )
    ).toBe(true)
  })
})

function requireEnv(): FullstackDevEnvironment {
  if (!env) {
    throw new Error("Fullstack dev environment was not initialized.")
  }

  return env
}
