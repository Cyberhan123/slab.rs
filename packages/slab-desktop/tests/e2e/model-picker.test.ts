import { afterAll, beforeAll, describe, expect, inject, it } from "vitest"
import { chromium, type Browser, type BrowserContext, type Page } from "playwright"

import type { E2eRuntimeEndpoints } from "./support/e2e-global-setup"
import {
  CLOUD_MODEL_REMOTE_ID,
  e2eLlmMode,
  eventually,
  getPersistedUiState,
  requestJson,
  selectAssistantModel,
  type UnifiedModelResponse,
} from "./support/e2e-runtime"
import { openAssistant, selectHeaderModel } from "./support/assistant-ui"

let env: E2eRuntimeEndpoints | undefined

/**
 * Header model-picker smoke: the assistant header picker (local/cloud groups,
 * `use-assistant-header` → `HeaderSelect`) must surface the cloud catalog and
 * a UI switch must land in the persisted `zustand:header-ui` selection — the
 * same slot the harness turn reads its model id from. Runs in both stack
 * modes (the app_home providers registry is merged into the generated
 * settings in either mode, so the cloud group is present even under a local
 * assistant model).
 */
describe("header model picker e2e", () => {
  let browser: Browser | undefined
  let context: BrowserContext | undefined
  let page: Page
  let flagship: UnifiedModelResponse
  let alternate: UnifiedModelResponse | undefined

  beforeAll(async () => {
    env = inject("e2e-runtime")
    browser = await chromium.launch({ headless: true })
    context = await browser.newContext({ viewport: { width: 1440, height: 960 } })
    await context.addInitScript(() => {
      window.localStorage.setItem("slab.ui.language", "en-US")
    })
    page = await context.newPage()
    // New-chat landing: the picker is enabled there (no busy session) and a
    // selection writes straight to the persisted store (no switch dialog).
    await openAssistant(page, env.uiBaseUrl)

    const models = await requestJson<UnifiedModelResponse[]>(
      env.serverBaseUrl,
      "/v1/models?capability=chat_generation"
    )
    const cloud = models.filter((model) => model.kind === "cloud")
    flagship = cloud.find(
      (model) => model.spec.remote_model_id === CLOUD_MODEL_REMOTE_ID
    )!
    alternate = cloud.find((model) => model.id !== flagship.id)
  })

  afterAll(async () => {
    // Defensive: converge the shared selection back to the suite default in
    // case a switch assertion raced the second click.
    if (env && e2eLlmMode === "cloud") {
      await selectAssistantModel(env.serverBaseUrl, flagship.id).catch(() => {})
    }
    await context?.close().catch(() => {})
    await browser?.close().catch(() => {})
  })

  it("lists the curated cloud group in the header picker", async () => {
    expect(flagship, "curated cloud flagship row must exist").toBeTruthy()

    // The picker loads model options asynchronously; wait for the trigger to
    // be present AND enabled before opening it.
    const trigger = page.getByTestId("header-model-trigger")
    await trigger.waitFor({ state: "visible", timeout: 90_000 })
    await eventually("header model trigger enabled", async () => trigger.isEnabled())
    await trigger.click()
    const option = page.getByTestId(`header-model-option-${flagship.id}`)
    await option.waitFor({ state: "visible", timeout: 30_000 })
    // The option renders the curated display name (not the raw id).
    expect(await option.textContent()).toContain(flagship.display_name)
    await page.keyboard.press("Escape")
  })

  // Cloud mode only: switching the shared selection away from the local model
  // mid-suite would re-route concurrent files' turns (they run against the
  // same server), so the switch-path exercise is limited to cloud stacks,
  // where every file already runs the flagship.
  it.skipIf(e2eLlmMode !== "cloud")(
    "persists a UI model switch into the header-ui selection",
    async () => {
      const testEnv = requireEnv()
      if (!alternate) {
        throw new Error("expected a second curated cloud model to switch to")
      }
      const alt = alternate

      // Precondition: the cloud bootstrap selected the flagship.
      const before = await getPersistedUiState<{ selections?: Record<string, string> }>(
        testEnv.serverBaseUrl,
        "zustand:header-ui"
      )
      expect(before?.selections?.["assistant:model"]).toBe(flagship.id)

      await selectHeaderModel(page, alt.id, alt.display_name)
      await eventually(
        `persisted assistant:model becomes ${alt.id}`,
        async () => {
          const state = await getPersistedUiState<{ selections?: Record<string, string> }>(
            testEnv.serverBaseUrl,
            "zustand:header-ui"
          )
          return state?.selections?.["assistant:model"] === alt.id ? true : null
        },
        30_000
      )

      // Switch back so the rest of the suite keeps the flagship selected.
      await selectHeaderModel(page, flagship.id, flagship.display_name)
      await eventually(
        `persisted assistant:model returns to ${flagship.id}`,
        async () => {
          const state = await getPersistedUiState<{ selections?: Record<string, string> }>(
            testEnv.serverBaseUrl,
            "zustand:header-ui"
          )
          return state?.selections?.["assistant:model"] === flagship.id ? true : null
        },
        30_000
      )
    },
    180_000
  )
})

function requireEnv(): E2eRuntimeEndpoints {
  if (!env) {
    throw new Error("e2e shared runtime endpoints were not provided.")
  }
  return env
}
