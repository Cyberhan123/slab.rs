import { afterAll, beforeAll, describe, expect, it } from "vitest"
import { chromium, type Browser, type BrowserContext, type Page } from "playwright"

import {
  cleanupFullstackDevEnvironment,
  completeSetup,
  createFullstackDevEnvironment,
  ensureModelDownloaded,
  getModel,
  importLocalModelPack,
  selectModelConfigVariant,
  startFullstackDev,
  type FullstackDevEnvironment,
  type ManagedDevProcess,
} from "./support/fullstack-dev"

let env: FullstackDevEnvironment | undefined

describe.sequential("hub e2e", () => {
  let browser: Browser | undefined
  let context: BrowserContext | undefined
  let dev: ManagedDevProcess | undefined
  let page: Page

  beforeAll(async () => {
    env = await createFullstackDevEnvironment()
    dev = await startFullstackDev(env)
    await completeSetup(env.serverBaseUrl)
    await importLocalModelPack(env.serverBaseUrl, "Qwen2.5-0.5B-Instruct")
    await selectModelConfigVariant(env.serverBaseUrl, "Qwen2.5-0.5B-Instruct", "Q4_K_M")

    browser = await chromium.launch({ headless: true })
    context = await browser.newContext({
      viewport: { width: 1440, height: 960 },
    })
    await context.addInitScript(() => {
      window.localStorage.setItem("slab.ui.language", "en-US")
    })
    page = await context.newPage()
  })

  afterAll(async () => {
    await context?.close().catch(() => {})
    await browser?.close().catch(() => {})
    await dev?.stop().catch(() => {})
    cleanupFullstackDevEnvironment(env)
  })

  it("imports the fixed model pack and downloads it from the Hub UI", async () => {
    const testEnv = requireEnv()
    await page.goto(`${testEnv.uiBaseUrl}/hub`, { waitUntil: "domcontentloaded", timeout: 60_000 })

    const card = page.getByTestId("hub-model-card-Qwen2.5-0.5B-Instruct")
    await card.waitFor({ state: "visible", timeout: 60_000 })

    const beforeDownload = await getModel(testEnv.serverBaseUrl, "Qwen2.5-0.5B-Instruct")
    expect(beforeDownload.spec.filename).toContain("Q4_K_M")

    const download = page.getByTestId("hub-model-download-Qwen2.5-0.5B-Instruct")
    await download.waitFor({ state: "visible", timeout: 60_000 })
    await download.click()

    const downloaded = await ensureModelDownloaded(testEnv.serverBaseUrl, "Qwen2.5-0.5B-Instruct")
    expect(downloaded.status).toBe("ready")
    expect(downloaded.spec.local_path?.trim()).toBeTruthy()

    const localPath = downloaded.spec.local_path ?? ""
    const visibleTail = localPath.split(/[\\/]/).pop() ?? localPath
    await page.waitForFunction(
      ([testId, needle]) =>
        document.querySelector(`[data-testid="${testId}"]`)?.textContent?.includes(needle) ?? false,
      ["hub-model-card-Qwen2.5-0.5B-Instruct", visibleTail],
      { timeout: 90_000 }
    )
  })
})

function requireEnv(): FullstackDevEnvironment {
  if (!env) {
    throw new Error("Fullstack dev environment was not initialized.")
  }

  return env
}
