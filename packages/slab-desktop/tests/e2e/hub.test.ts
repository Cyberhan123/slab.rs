import { afterAll, beforeAll, describe, expect, inject, it } from "vitest"
import { chromium, type Browser, type BrowserContext, type Page } from "playwright"

import type { E2eRuntimeEndpoints } from "./support/e2e-global-setup"
import {
  completeSetup,
  ensureModelDownloaded,
  getModel,
  importLocalModelPack,
  selectModelConfigVariant,
} from "./support/e2e-runtime"

let env: E2eRuntimeEndpoints | undefined

describe("hub e2e", () => {
  let browser: Browser | undefined
  let context: BrowserContext | undefined
  let page: Page

  beforeAll(async () => {
    env = inject("e2e-runtime")
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

function requireEnv(): E2eRuntimeEndpoints {
  if (!env) {
    throw new Error("e2e shared runtime endpoints were not provided.")
  }

  return env
}
