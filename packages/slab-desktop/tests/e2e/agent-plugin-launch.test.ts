import { afterAll, beforeAll, describe, expect, it } from "vitest"
import { chromium, type Browser, type BrowserContext, type Page } from "playwright"

import {
  bootstrapLocalModel,
  cleanupFullstackDevEnvironment,
  createFullstackDevEnvironment,
  createSession,
  selectAssistantSession,
  startFullstackDev,
  type FullstackDevEnvironment,
  type ManagedDevProcess,
  type SessionResponse,
} from "./support/fullstack-dev"
import {
  expectAssistantPageText,
  openAssistant,
  parseToolJson,
  sendAssistantMessage,
  waitForToolExecution,
} from "./support/assistant-ui"

/**
 * B-7 / ADR-009 plugin a2u four-segment closure (declare→call→render→feedback):
 * the agent must be able to invoke the trusted `plugin.launch` a2u tool through
 * the full stack and the tool must return the plugin surface metadata the
 * frontend dispatcher consumes. `plugin.launch` is statically classified Low
 * risk, so no approval gate is expected. The tool is a pure surface opener — it
 * emits metadata and does not require the plugin to be installed.
 */
let env: FullstackDevEnvironment | undefined

describe.sequential("agent plugin.launch e2e", () => {
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
    session = await createSession(env.serverBaseUrl, `agent-plugin-launch-e2e-${Date.now()}`)
    await selectAssistantSession(env.serverBaseUrl, session.id, session.name)

    browser = await chromium.launch({ headless: true })
    context = await browser.newContext({ viewport: { width: 1440, height: 960 } })
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

  it("dispatches a plugin.launch a2u tool call and returns the plugin surface metadata", async () => {
    const testEnv = requireEnv()
    const prompt = [
      "Use the plugin.launch tool exactly once with plugin_id set to demo-plugin",
      "to open that plugin's trusted surface.",
      "Do not use any other tool. After the tool result, reply with one short sentence.",
    ].join("\n")

    await sendAssistantMessage(page, prompt)
    await expectAssistantPageText(page, prompt)

    const toolResult = await waitForToolExecution(
      testEnv.serverBaseUrl,
      session.id,
      prompt,
      "plugin.launch",
      1_200_000
    )

    expect(toolResult.toolCalls.length).toBeGreaterThanOrEqual(1)
    const output = parseToolJson(toolResult.toolMessages[0].content)
    expect(output.surface).toBe("plugin")
    expect(output.opened).toBe(true)
    // pluginId is passed through (trimmed); keep the assertion robust to casing.
    expect(typeof output.pluginId).toBe("string")
    expect(String(output.pluginId).toLowerCase()).toContain("demo")
  })
})

function requireEnv(): FullstackDevEnvironment {
  if (!env) {
    throw new Error("Fullstack dev environment was not initialized.")
  }
  return env
}
