import { createServer, type Server } from "node:http"
import { readFileSync, writeFileSync } from "node:fs"
import { afterAll, beforeAll, describe, expect, inject, it } from "vitest"
import type { components } from "@slab/api/v1"

import type { E2eRuntimeEndpoints } from "./support/e2e-global-setup"
import { eventually, requestJson } from "./support/e2e-runtime"

type Schema = components["schemas"]
type UnifiedModelResponse = Schema["UnifiedModelResponse"]
type SettingPropertyView = Schema["SettingPropertyView"]

/**
 * Cloud-provider model catalog e2e (shared-server suite).
 *
 * Contract notes (see e2e-global-setup.ts): providers use a `Date.now()` marker id and every
 * assertion is membership-only — never model counts — because other test files share this
 * server's catalog. The registry is restored in afterAll, which also triggers orphan cleanup
 * for the marker providers.
 */
describe("cloud model catalog e2e", () => {
  let env: E2eRuntimeEndpoints
  const marker = `e2e-cloud-${Date.now()}`

  // Shared `/v1/models` mock state for the live-discovery cases.
  let mockServer: Server | undefined
  let mockPort = 0
  let mockModels: string[] = []
  let mockFail = false

  let originalRegistry: unknown[] = []

  beforeAll(async () => {
    env = inject("e2e-runtime")
    const current = await requestJson<SettingPropertyView>(
      env.serverBaseUrl,
      "/v1/settings/providers.registry"
    )
    originalRegistry = Array.isArray(current.effective_value) ? current.effective_value : []

    mockServer = createServer((request, response) => {
      if (mockFail) {
        response.statusCode = 500
        response.end("scripted discovery failure")
        return
      }
      if (request.url?.endsWith("/models")) {
        response.setHeader("content-type", "application/json")
        response.end(JSON.stringify({ data: mockModels.map((id) => ({ id })) }))
        return
      }
      response.statusCode = 404
      response.end()
    })
    await new Promise<void>((resolve) => {
      mockServer?.listen(0, "127.0.0.1", () => {
        const address = mockServer?.address()
        if (address && typeof address === "object") {
          mockPort = address.port
        }
        resolve()
      })
    })
  })

  afterAll(async () => {
    // Restore the shared registry; the settings PUT hook prunes the marker providers' rows.
    if (env) {
      await putRegistry(originalRegistry).catch(() => {})
    }
    mockServer?.close()
  })

  async function putRegistry(entries: unknown[]): Promise<SettingPropertyView> {
    return requestJson<SettingPropertyView>(env.serverBaseUrl, "/v1/settings/providers.registry", {
      json: { op: "set", value: entries },
      method: "PUT",
    })
  }

  async function listChatModels(): Promise<UnifiedModelResponse[]> {
    return requestJson<UnifiedModelResponse[]>(
      env.serverBaseUrl,
      "/v1/models?capability=chat_generation"
    )
  }

  async function findModel(id: string): Promise<UnifiedModelResponse | undefined> {
    return (await listChatModels()).find((model) => model.id === id)
  }

  function bigModelEntry() {
    return {
      id: `${marker}-big`,
      family: "big_model",
      display_name: "BigModel (e2e)",
      api_base: "https://open.bigmodel.cn/api/coding/paas/v4",
      auth: { api_key: "sk-e2e-not-a-real-key" },
    }
  }

  function liveEntry() {
    return {
      id: `${marker}-live`,
      family: "openai_compatible",
      display_name: "Mock OpenAI (e2e)",
      api_base: `http://127.0.0.1:${mockPort}/v1`,
      auth: { api_key: "sk-e2e-not-a-real-key" },
    }
  }

  it("activates the curated GLM catalog for a configured BigModel provider on save", async () => {
    await putRegistry([...originalRegistry, bigModelEntry()])

    const models = await listChatModels()
    for (const remote of ["glm-4.6", "glm-4.5", "glm-4.5-air", "glm-4-flash"]) {
      const entry = models.find((model) => model.id === `cloud:${marker}-big:${remote}`)
      expect(entry, `missing curated row cloud:${marker}-big:${remote}`).toBeTruthy()
      expect(entry?.kind).toBe("cloud")
      expect(entry?.spec.provider_id).toBe(`${marker}-big`)
      expect(entry?.spec.remote_model_id).toBe(remote)
    }
    const flagship = models.find((model) => model.id === `cloud:${marker}-big:glm-4.6`)
    expect(flagship?.display_name).toBe("GLM-4.6")
  })

  it("discovers OpenAI-compatible models live from the provider /models endpoint", async () => {
    mockFail = false
    mockModels = ["mock-a", "mock-b"]
    await putRegistry([...originalRegistry, bigModelEntry(), liveEntry()])

    await eventually(
      `live-discovered rows appear for ${marker}-live`,
      async () => {
        const a = await findModel(`cloud:${marker}-live:mock-a`)
        const b = await findModel(`cloud:${marker}-live:mock-b`)
        return a && b ? { a, b } : null
      },
      30_000,
      500
    )

    const discovered = await findModel(`cloud:${marker}-live:mock-a`)
    expect(discovered?.kind).toBe("cloud")
    expect(discovered?.spec.remote_model_id).toBe("mock-a")
  })

  it("keeps discovered rows when the endpoint fails and prunes them when the list shrinks", async () => {
    // Failure: a repeated save probes the endpoint again; the known rows must survive.
    mockFail = true
    await putRegistry([...originalRegistry, bigModelEntry(), liveEntry()])
    expect(await findModel(`cloud:${marker}-live:mock-b`)).toBeTruthy()

    // Shrunk success: mock-b disappears from the endpoint, so it is pruned.
    mockFail = false
    mockModels = ["mock-a"]
    await putRegistry([...originalRegistry, bigModelEntry(), liveEntry()])

    await eventually(
      `cloud:${marker}-live:mock-b is pruned after successful discovery`,
      async () => {
        const pruned = await findModel(`cloud:${marker}-live:mock-b`)
        const kept = await findModel(`cloud:${marker}-live:mock-a`)
        return pruned === undefined && kept ? kept : null
      },
      30_000,
      500
    )
  })

  it("self-heals the catalog when providers are added by editing settings.json directly", async () => {
    const entry = {
      id: `${marker}-zai`,
      family: "zai",
      display_name: "Zai (e2e)",
      api_base: "https://api.z.ai/api/coding/v4",
      auth: { api_key: "sk-e2e-not-a-real-key" },
    }

    // Read-modify-write the overlay settings file — the one settings PUTs persist to when an
    // overlay is configured (see `SettingsDocumentProvider` write-path selection). Preserve
    // unrelated sections; a parse failure retries once.
    let wrote = false
    for (let attempt = 0; attempt < 2 && !wrote; attempt += 1) {
      try {
        const document = JSON.parse(
          readFileSync(env.settingsOverlayPath, "utf8")
        ) as Record<string, unknown>
        const providers = (document.providers ?? {}) as Record<string, unknown>
        const registry = Array.isArray(providers.registry) ? providers.registry : []
        document.providers = { ...providers, registry: [...registry, entry] }
        writeFileSync(env.settingsOverlayPath, `${JSON.stringify(document, null, 2)}\n`, "utf8")
        wrote = true
      } catch {
        // retry the read-modify-write
      }
    }
    expect(wrote, "settings overlay read-modify-write should succeed").toBe(true)

    try {
      // The pmid refresh (file fingerprint + 5s ticker) picks the file up; the next models
      // read self-heals the catalog.
      await eventually(
        `cloud:${marker}-zai:glm-4.6 appears after external settings edit`,
        async () => {
          return (await findModel(`cloud:${marker}-zai:glm-4.6`)) ?? null
        },
        30_000,
        500
      )
    } finally {
      // Converge the file back to the server-managed state (the PUT rewrites the overlay).
      await putRegistry(originalRegistry)
    }
  })
})
