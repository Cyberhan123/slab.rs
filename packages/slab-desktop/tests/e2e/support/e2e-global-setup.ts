import type { Vitest } from "vitest/node"

import {
  bootstrapLocalModel,
  cleanupE2eEnvironment,
  createE2eEnvironment,
  startE2eRuntime,
  type E2eRuntime,
  type ManagedProcess,
} from "./e2e-runtime"

/**
 * E2E Concurrency Contract
 *
 * The whole e2e suite shares ONE slab-server + ONE slab-runtime + ONE loaded
 * model + ONE workspace root (booted once here). To stay safe under
 * file-parallel execution:
 *   1. Distinct workspace paths — every file op uses a `Date.now()`-marked
 *      relative path (the `markerRoot` pattern). Never write a fixed path.
 *   2. Session-scoped assertions only — assert via `restoreSession(baseUrl,
 *      sessionId)` by id/marker membership; never assert on
 *      `listSessions().length`, model counts, or task counts.
 *   3. Never unload/switch the model — it is loaded once here. Do not call
 *      `/v1/models/unload`, `/v1/models/load` for a different id, or
 *      `workspace/migrate` (which interrupts ALL threads process-wide).
 *   4. Permission mode per-message — do not call `selectPermissionMode` (it
 *      writes the shared `zustand:assistant-ui` store and races other files).
 *   5. One session per file — `beforeAll` creates its own session and binds it
 *      via the `?session=` URL override.
 */

declare module "vitest" {
  interface ProvidedContext {
    "e2e-runtime": E2eRuntimeEndpoints
  }
}

/**
 * JSON-serializable endpoint snapshot handed to every test file via
 * `inject("e2e-runtime")`. vitest JSON-stringifies the provided value before
 * sending it to each worker, so it must NOT carry functions, the
 * `ManagedProcess` handle, or the live `logs` array — those stay in this
 * process so the returned teardown can stop the stack.
 */
export type E2eRuntimeEndpoints = Pick<
  E2eRuntime,
  | "databasePath"
  | "databaseUrl"
  | "e2eRootDir"
  | "modelConfigDir"
  | "pluginsDir"
  | "repoRoot"
  | "rootDir"
  | "serverBaseUrl"
  | "serverBind"
  | "serverPort"
  | "sessionStateDir"
  | "settingsOverlayPath"
  | "settingsPath"
  | "uiBaseUrl"
  | "uiPort"
  | "workspaceRoot"
> & {
  modelId: string
  selectedVariantId: string
}

const MODEL_ID = "Qwen3.5-9B"
const SELECTED_VARIANT_ID = "Q8_0"

/**
 * Boots the shared e2e stack (slab-server + slab-runtime + Vite) and loads the
 * local model ONCE for the whole suite, then exposes the endpoints to every
 * test file. Returns the teardown that stops the stack when the suite ends.
 */
export default async function e2eGlobalSetup(vitest: Vitest) {
  const runtime = await createE2eEnvironment()
  const dev: ManagedProcess = await startE2eRuntime(runtime)
  const model = await bootstrapLocalModel(runtime.serverBaseUrl, {
    modelId: MODEL_ID,
    selectedVariantId: SELECTED_VARIANT_ID,
  })

  vitest.provide("e2e-runtime", {
    databasePath: runtime.databasePath,
    databaseUrl: runtime.databaseUrl,
    e2eRootDir: runtime.e2eRootDir,
    modelConfigDir: runtime.modelConfigDir,
    modelId: model.id,
    pluginsDir: runtime.pluginsDir,
    repoRoot: runtime.repoRoot,
    rootDir: runtime.rootDir,
    selectedVariantId: SELECTED_VARIANT_ID,
    serverBaseUrl: runtime.serverBaseUrl,
    serverBind: runtime.serverBind,
    serverPort: runtime.serverPort,
    sessionStateDir: runtime.sessionStateDir,
    settingsOverlayPath: runtime.settingsOverlayPath,
    settingsPath: runtime.settingsPath,
    uiBaseUrl: runtime.uiBaseUrl,
    uiPort: runtime.uiPort,
    workspaceRoot: runtime.workspaceRoot,
  })

  return async function e2eGlobalTeardown() {
    await dev.stop().catch(() => {})
    cleanupE2eEnvironment(runtime)
  }
}
