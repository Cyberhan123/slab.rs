import { execFileSync } from "node:child_process"
import { existsSync } from "node:fs"
import { join } from "node:path"
import { DatabaseSync } from "node:sqlite"

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
 * Global setup for the scripted memory phase2 consolidation suite
 * (`vitest.e2e.memory.config.ts`).
 *
 * Boots a DEDICATED slab-server + Vite stack with `SLAB_E2E_MODE=1` AND the
 * agent memory pipeline enabled (`agent.memories.enabled: true`, memory root
 * inside the run dir, model `slab-llama`). The pipeline fires on the first
 * root agent start (`AgentMemoryStartupHook::OnAgentStart`), so a dedicated
 * stack is mandatory: a failed/stopped phase2 run deliberately keeps the old
 * watermark, meaning every later root turn would re-spawn a consolidation
 * subagent and contaminate any suite sharing the stack.
 *
 * Pre-state (seeded into the stack DB after boot, before any turn):
 * one `succeeded` phase1 output row for the workspace's project key — the
 * minimum that makes `load_phase2_project_keys` return the project and the
 * phase2 watermark claim differ from `completed_watermark` (NULL), which is
 * what spawns the `Consolidate memory workspace (…)` system subagent the
 * suite observes via `subagent_status` and stops via `subagent_stop`.
 */

declare module "vitest" {
  interface ProvidedContext {
    "e2e-memory-runtime": MemoryRuntimeEndpoints
  }
}

/** JSON-serializable endpoint snapshot for the scripted memory stack. */
export type MemoryRuntimeEndpoints = Pick<
  E2eRuntime,
  | "databasePath"
  | "logsDir"
  | "repoRoot"
  | "serverBaseUrl"
  | "serverLogPath"
  | "uiBaseUrl"
  | "workspaceRoot"
> & {
  memoryRoot: string
  projectKey: string
}

/** The scripted model id — accepted as-is by the e2e-mode LlmPort. */
const SCRIPTED_MODEL_ID = "slab-llama"

const SEED_SESSION_ID = "memory-e2e-session"
const SEED_THREAD_ID = "memory-e2e-thread"

/** Exact TS port of `slab_agent_memories::fs::sanitize_project_key`:
 * lowercase first, collapse every non-`[a-z0-9]` run to one `-` (leading
 * dash suppressed, trailing dash never emitted), cap at 120 chars, empty →
 * `_global`. The project key is the git toplevel of the workspace (the e2e
 * stack lives inside the repo, so every run maps to the repo root's key). */
function sanitizeProjectKey(project: string): string {
  let sanitized = ""
  let pendingDash = false
  for (const character of project.toLowerCase()) {
    if (sanitized.length >= 120) {
      break
    }
    if (/^[a-z0-9]$/.test(character)) {
      if (pendingDash && sanitized.length > 0) {
        sanitized += "-"
      }
      pendingDash = false
      sanitized += character
    } else {
      pendingDash = true
    }
  }
  return sanitized === "" ? "_global" : sanitized
}

function resolveProjectKey(workspaceRoot: string): string {
  const toplevel = execFileSync(
    "git",
    ["-C", workspaceRoot, "rev-parse", "--show-toplevel"],
    { encoding: "utf8" }
  ).trim()
  if (toplevel.length === 0) {
    throw new Error("git rev-parse --show-toplevel returned empty for the e2e workspace")
  }
  return sanitizeProjectKey(toplevel)
}

function seedMemoryPrestate(runtime: E2eRuntime, projectKey: string): void {
  if (!existsSync(runtime.databasePath)) {
    throw new Error(`stack database missing after boot: ${runtime.databasePath}`)
  }
  const now = new Date().toISOString()
  const database = new DatabaseSync(runtime.databasePath)
  try {
    database.exec("PRAGMA busy_timeout = 5000")
    // node:sqlite binds anonymous `?` placeholders positionally — SQLite's
    // `?NNN` indexed syntax is not accepted by the binder.
    database
      .prepare(
        "INSERT OR IGNORE INTO chat_sessions (id, name, created_at, updated_at) \
         VALUES (?, '', ?, ?)"
      )
      .run(SEED_SESSION_ID, now, now)
    database
      .prepare(
        "INSERT INTO agent_threads \
           (id, session_id, parent_id, depth, status, config_json, created_at, updated_at) \
         VALUES (?, ?, NULL, 0, 'completed', '{}', ?, ?)"
      )
      .run(SEED_THREAD_ID, SEED_SESSION_ID, now, now)
    // The phase2 gates this row must satisfy: status='succeeded' with
    // raw_memory (load_phase2_project_keys), rollout_summary non-null
    // (load_phase2_inputs_in_pool), source_updated_at non-null and newer
    // than the NULL completed_watermark (delta-skip), generated_at fresh
    // (max_unused_days retain).
    database
      .prepare(
        "INSERT INTO agent_memory_phase1_outputs \
           (thread_id, session_id, status, raw_memory, rollout_summary, rollout_slug, \
            source_updated_at, generated_at, project_key) \
         VALUES (?, ?, 'succeeded', ?, ?, ?, ?, ?, ?)"
      )
      .run(
        SEED_THREAD_ID,
        SEED_SESSION_ID,
        "Seeded raw memory body for the scripted memory phase2 e2e.",
        "Seeded rollout summary for the scripted memory phase2 e2e.",
        "memory-e2e",
        now,
        now,
        projectKey
      )
  } finally {
    database.close()
  }
}

export default async function e2eMemoryGlobalSetup(vitest: Vitest) {
  // The scripted consolidation child stalls its first LLM call for this many
  // ms (adapter memory branch) — the window the suite has to observe the
  // running task and stop it. Wider than the subagent suite's 18s: the
  // pipeline must first run phase1, claim the phase2 lease, sync the memory
  // workspace, and compute the git diff before the child even spawns. The
  // same env arms the stall watchdog (warn-only for self-owned tasks). Set
  // BEFORE boot — the runtime spreads process.env into the server env.
  process.env.SLAB_E2E_STALL_MS = process.env.SLAB_E2E_STALL_MS ?? "30000"
  const runtime = await createE2eEnvironment({ memories: true })
  const dev: ManagedProcess = await startScriptedE2eRuntime(runtime)

  await completeSetup(runtime.serverBaseUrl)
  await selectAssistantModel(runtime.serverBaseUrl, SCRIPTED_MODEL_ID)

  const projectKey = resolveProjectKey(runtime.workspaceRoot)
  seedMemoryPrestate(runtime, projectKey)

  vitest.provide("e2e-memory-runtime", {
    databasePath: runtime.databasePath,
    logsDir: runtime.logsDir,
    repoRoot: runtime.repoRoot,
    serverBaseUrl: runtime.serverBaseUrl,
    serverLogPath: runtime.serverLogPath,
    uiBaseUrl: runtime.uiBaseUrl,
    workspaceRoot: runtime.workspaceRoot,
    memoryRoot: join(runtime.rootDir, "memories"),
    projectKey,
  })

  return async function e2eMemoryGlobalTeardown() {
    await dev.stop().catch(() => {})
    cleanupE2eEnvironment(runtime)
  }
}
