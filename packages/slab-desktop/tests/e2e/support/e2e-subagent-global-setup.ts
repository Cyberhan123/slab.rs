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
 * Global setup for the scripted subagent delegation suite
 * (`vitest.e2e.subagent.config.ts`).
 *
 * Boots a DEDICATED slab-server + Vite stack with `SLAB_E2E_MODE=1`: the
 * debug-build `ServerLlmAdapter` returns deterministic prompt-keyed responses
 * (see `crates/slab-app-core/src/infra/agent/adapter.rs`), so parent/child
 * delegation flows — tool calls, completion notifications, steering, cascade
 * interrupts — need no real model at all. This stack is fully owned by the
 * suite (never runs concurrently with the real-model `test:e2e` stack), so the
 * shared-suite concurrency contract (no `selectPermissionMode`, etc.) does not
 * apply here.
 */

declare module "vitest" {
  interface ProvidedContext {
    "e2e-subagent-runtime": SubagentRuntimeEndpoints
  }
}

/** JSON-serializable endpoint snapshot for the scripted stack. */
export type SubagentRuntimeEndpoints = Pick<
  E2eRuntime,
  | "repoRoot"
  | "serverBaseUrl"
  | "uiBaseUrl"
  | "workspaceRoot"
>

/**
 * The model id the scripted harness advertises. No model with this id exists
 * in the store: `ensure_turn_model_loaded` short-circuits under
 * `SLAB_E2E_MODE=1` and the LlmPort never consults a runtime, so the id is
 * accepted as-is.
 */
const SCRIPTED_MODEL_ID = "slab-llama"

export default async function e2eSubagentGlobalSetup(vitest: Vitest) {
  const runtime = await createE2eEnvironment()
  const dev: ManagedProcess = await startScriptedE2eRuntime(runtime)

  await completeSetup(runtime.serverBaseUrl)
  await selectAssistantModel(runtime.serverBaseUrl, SCRIPTED_MODEL_ID)

  vitest.provide("e2e-subagent-runtime", {
    repoRoot: runtime.repoRoot,
    serverBaseUrl: runtime.serverBaseUrl,
    uiBaseUrl: runtime.uiBaseUrl,
    workspaceRoot: runtime.workspaceRoot,
  })

  return async function e2eSubagentGlobalTeardown() {
    await dev.stop().catch(() => {})
    cleanupE2eEnvironment(runtime)
  }
}
