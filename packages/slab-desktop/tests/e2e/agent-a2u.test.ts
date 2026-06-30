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
 * Phase 1 exit criterion ("E2E 打开 slab.rs"): the agent must be able to invoke
 * a trusted a2u surface tool (`workspace.open`) through the full stack —
 * UI → /v1/agents → app-core agent → ToolRouter → a2u_tools → ToolOutput — and
 * the tool must return the trusted surface metadata the frontend dispatcher
 * consumes. `workspace.open` is statically classified Low risk, so no approval
 * gate is expected.
 */
let env: FullstackDevEnvironment | undefined

describe.sequential("agent a2u e2e", () => {
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
    session = await createSession(env.serverBaseUrl, `agent-a2u-e2e-${Date.now()}`)
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

  it("dispatches a workspace.open a2u tool call and returns the trusted surface metadata", async () => {
    const testEnv = requireEnv()
    // Direct, single-tool instruction mirroring the proven agent.test.ts prompt
    // shape. README.md is seeded into the e2e workspace by the support harness.
    const prompt = [
      "Use the workspace.open tool exactly once with path set to README.md",
      "to reveal that file in the trusted workspace surface.",
      "Do not use any other tool. After the tool result, reply with one short sentence.",
    ].join("\n")

    await sendAssistantMessage(page, prompt)
    await expectAssistantPageText(page, prompt)

    const toolResult = await waitForToolExecution(
      testEnv.serverBaseUrl,
      session.id,
      prompt,
      "workspace.open",
      1_200_000
    )

    // Exactly one workspace.open call, with trusted surface metadata.
    expect(toolResult.toolCalls.length).toBeGreaterThanOrEqual(1)
    const output = parseToolJson(toolResult.toolMessages[0].content)
    expect(output.surface).toBe("workspace")
    expect(output.opened).toBe(true)
    // revealPath is normalized to a workspace-relative identifier; the model was
    // asked for README.md (case-insensitive containment keeps this robust to
    // casing differences in the model's tool args).
    expect(typeof output.revealPath).toBe("string")
    expect(String(output.revealPath).toLowerCase()).toContain("readme.md")
  })
})

function requireEnv(): FullstackDevEnvironment {
  if (!env) {
    throw new Error("Fullstack dev environment was not initialized.")
  }
  return env
}
