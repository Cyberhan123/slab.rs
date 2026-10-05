import type { Vitest } from "vitest/node"

import {
  cleanupE2eEnvironment,
  completeSetup,
  createE2eEnvironment,
  selectAssistantModel,
  startScriptedE2eRuntime,
  type E2eRuntime,
  type ManagedProcess,
} from "./e2e-runtime"

/**
 * Global setup for the scripted questionnaire suite
 * (`vitest.e2e.questionnaire.config.ts`).
 *
 * Boots a DEDICATED slab-server + Vite stack with `SLAB_E2E_MODE=1`: the
 * debug-build `ServerLlmAdapter` returns deterministic prompt-keyed responses
 * (see `crates/slab-app-core/src/infra/agent/adapter.rs`), so the
 * `questionnaire` tool round-trip — tool call, answer notification, banner
 * card, `questionnaire/resolve`, answer payload fed back to the model — needs
 * no real model. Fully owned by this suite (never concurrent with the
 * real-model `test:e2e` stack or the other scripted stacks).
 */

declare module "vitest" {
  interface ProvidedContext {
    "e2e-questionnaire-runtime": QuestionnaireRuntimeEndpoints
  }
}

/** JSON-serializable endpoint snapshot for the scripted stack. */
export type QuestionnaireRuntimeEndpoints = Pick<
  E2eRuntime,
  "logsDir" | "repoRoot" | "serverBaseUrl" | "serverLogPath" | "uiBaseUrl" | "workspaceRoot"
>

/** The model id the scripted harness advertises (accepted as-is in e2e mode). */
const SCRIPTED_MODEL_ID = "slab-llama"

export default async function e2eQuestionnaireGlobalSetup(vitest: Vitest) {
  const runtime = await createE2eEnvironment()
  const dev: ManagedProcess = await startScriptedE2eRuntime(runtime)

  await completeSetup(runtime.serverBaseUrl)
  await selectAssistantModel(runtime.serverBaseUrl, SCRIPTED_MODEL_ID)

  vitest.provide("e2e-questionnaire-runtime", {
    logsDir: runtime.logsDir,
    repoRoot: runtime.repoRoot,
    serverBaseUrl: runtime.serverBaseUrl,
    serverLogPath: runtime.serverLogPath,
    uiBaseUrl: runtime.uiBaseUrl,
    workspaceRoot: runtime.workspaceRoot,
  })

  return async function e2eQuestionnaireGlobalTeardown() {
    await dev.stop().catch(() => {})
    cleanupE2eEnvironment(runtime)
  }
}
