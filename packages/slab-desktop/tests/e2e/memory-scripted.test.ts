/**
 * Scripted memory phase2 consolidation e2e (dedicated SLAB_E2E_MODE=1 stack
 * with the agent memory pipeline ENABLED — no real model).
 *
 * The stack booted by `e2e-memory-global-setup.ts` enables
 * `agent.memories` against a per-run memory root and seeds one `succeeded`
 * phase1 output row for the workspace's project key. The first root agent
 * start fires `AgentMemoryStartupHook::OnAgentStart` → the pipeline claims
 * the phase2 lease (watermark NULL ≠ claimed) → syncs the memory workspace →
 * `spawn_system` launches the `Consolidate memory workspace (…)` system
 * subagent (cdb0cdd3 subagent chain: registry-visible, stoppable, no parent
 * relay). The scripted adapter stalls the consolidation child for
 * SLAB_E2E_STALL_MS, which is the window this suite has to observe and stop
 * it.
 *
 * Assertions ride the registry tools (`subagent_status` / `subagent_stop`)
 * through the root session's REST agent-history — a system subagent has no
 * parent thread, so no UI card and no notification path exist to assert on.
 *
 * Semantics worth pinning:
 *   - a failed/stopped phase2 run deliberately keeps the previous
 *     completed_watermark (retry semantics), so later root turns may spawn a
 *     NEW consolidation task — the terminal assertion pins the ORIGINAL
 *     task_id (via `subagent-e2e/status task_id=<id>`);
 *   - the memory child's session id is `memory-phase2-<uuid>`; the pipeline
 *     creates a synthetic `chat_sessions` row for it BEFORE the spawn (the
 *     FK parent its `agent_threads` row needs), kept out of
 *     `GET /v1/sessions` by the store-level prefix filter;
 */
import { afterAll, beforeAll, beforeEach, afterEach, describe, expect, inject, it } from "vitest"
import { chromium, type Browser, type Page } from "playwright"
import { DatabaseSync } from "node:sqlite"

import { createSession, type SessionResponse } from "./support/e2e-runtime"
import {
  nonBlank,
  openAssistant,
  parseToolJson,
  sendAssistantMessage,
  waitForCompletedAssistantReply,
  waitForToolExecution,
} from "./support/assistant-ui"
import type { MemoryRuntimeEndpoints } from "./support/e2e-memory-global-setup"

const endpoints = inject("e2e-memory-runtime") as MemoryRuntimeEndpoints
const baseUrl = endpoints.serverBaseUrl

/** Snapshot of one registry task inside `subagent_status` output. */
type SubagentSnapshot = {
  task_id: string
  parent_thread_id: string | null
  child_thread_id: string | null
  task: string
  status: string
  result: unknown
}

let browser: Browser
let session: SessionResponse
let page: Page
const debugConsoleErrors: string[] = []

beforeAll(async () => {
  browser = await chromium.launch({ headless: true })
})

beforeEach(async () => {
  session = await createSession(baseUrl)
  const context = await browser.newContext({ viewport: { width: 1440, height: 960 } })
  await context.addInitScript(() => {
    window.localStorage.setItem("slab.ui.language", "en-US")
  })
  page = await context.newPage()
  page.on("close", () => context.close().catch(() => {}))
  debugConsoleErrors.length = 0
  page.on("console", (message) => {
    if (message.type() === "error") debugConsoleErrors.push(message.text())
  })
  page.on("pageerror", (error) => debugConsoleErrors.push(`pageerror: ${error.message}`))
  await openAssistant(page, endpoints.uiBaseUrl, session.id)
})

afterEach(async () => {
  if (expect.getState().currentTestName && process.env.SLAB_E2E_DEBUG) {
    console.log(
      "[debug] console errors:",
      debugConsoleErrors.slice(0, 12).map((line) => line.slice(0, 300)),
    )
  }
  await page.close().catch(() => {})
})

afterAll(async () => {
  await browser.close().catch(() => {})
})

describe("scripted memory phase2 consolidation", () => {
  it(
    "1. the consolidation task surfaces in subagent_status and subagent_stop stops it",
    async () => {
      // Turn 1 — any non-transient root start fires the memory pipeline
      // hook. The echo reply completes instantly; the pipeline keeps running
      // in the background (phase1 no-op → phase2 claim → workspace sync →
      // spawn the stalled consolidation child).
      const kickoffPrompt = "start the memory pipeline"
      await sendAssistantMessage(page, kickoffPrompt)
      const kickoff = await waitForCompletedAssistantReply(
        baseUrl,
        session.id,
        kickoffPrompt,
        90_000
      )
      expect(nonBlank(kickoff.text)).toBe(true)

      // Poll `subagent_status` (list mode) until the consolidation task is
      // RUNNING. Each retry needs a UNIQUE prompt so waitForToolExecution's
      // exact-content match binds to the latest turn, not a stale one; the
      // extra `probe=N` suffix keeps the marker marker-shaped.
      const expectedSummary = `Consolidate memory workspace (${endpoints.projectKey})`
      const pollDeadline = Date.now() + 120_000
      let consolidation: SubagentSnapshot | undefined
      let lastPayload = ""
      for (let attempt = 1; attempt <= 8 && consolidation === undefined; attempt += 1) {
        // eslint-disable-next-line no-await-in-loop
        const statusPrompt = `subagent-e2e/status probe=${attempt}`
        // eslint-disable-next-line no-await-in-loop
        await sendAssistantMessage(page, statusPrompt)
        // eslint-disable-next-line no-await-in-loop
        const status = await waitForToolExecution(
          baseUrl,
          session.id,
          statusPrompt,
          "subagent_status",
          60_000
        )
        const payload = parseToolJson(status.toolMessages[0].content) as { tasks?: SubagentSnapshot[] }
        lastPayload = JSON.stringify(payload)
        consolidation = (payload.tasks ?? []).find(
          (task) =>
            task.task.startsWith("Consolidate memory workspace (") && task.status === "running"
        )
        if (!consolidation && Date.now() > pollDeadline) {
          break
        }
      }
      if (!consolidation) {
        throw new Error(
          `consolidation task never appeared (expected '${expectedSummary}'): ${lastPayload}`
        )
      }

      // Verifies the TS-side project-key port matches the pipeline's Rust
      // derivation (git toplevel → sanitize_project_key).
      expect(consolidation.task).toBe(expectedSummary)
      // A system subagent is SELF-OWNED: the snapshot's owner slot carries
      // the child's own thread id (no parent agent thread to relay onto).
      expect(nonBlank(consolidation.child_thread_id)).toBe(true)
      expect(consolidation.parent_thread_id).toBe(consolidation.child_thread_id)
      const taskId = consolidation.task_id

      // The synthetic FK-parent row is inserted BEFORE the spawn, so
      // observing the RUNNING task implies the row exists (literal SQL, no
      // binds — node:sqlite's binder rejects ?NNN).
      const database = new DatabaseSync(endpoints.databasePath)
      try {
        database.exec("PRAGMA busy_timeout = 5000")
        const rows = database
          .prepare("SELECT id, name FROM chat_sessions WHERE id LIKE 'memory-phase2-%'")
          .all() as Array<{ id: string; name: string }>
        expect(rows.length).toBeGreaterThan(0)
      } finally {
        database.close()
      }

      // Stop it through the registry — the same surface a user-visible kill
      // uses (the pipeline itself also stops the task on lease loss).
      const stopPrompt = `subagent-e2e/stop task_id=${taskId}`
      await sendAssistantMessage(page, stopPrompt)
      const stopped = await waitForToolExecution(
        baseUrl,
        session.id,
        stopPrompt,
        "subagent_stop",
        60_000
      )
      const stopPayload = parseToolJson(stopped.toolMessages[0].content) as {
        stopped: SubagentSnapshot
      }
      expect(stopPayload.stopped.task_id).toBe(taskId)
      expect(stopPayload.stopped.status).toBe("stopped")

      // Terminal state pinned to the ORIGINAL task id: the failed run keeps
      // the old watermark, so later turns may already have re-spawned a NEW
      // consolidation task — the snapshot query must target the stopped one.
      const pinnedPrompt = `subagent-e2e/status task_id=${taskId} pinned=1`
      await sendAssistantMessage(page, pinnedPrompt)
      const pinned = await waitForToolExecution(
        baseUrl,
        session.id,
        pinnedPrompt,
        "subagent_status",
        60_000
      )
      const pinnedPayload = parseToolJson(pinned.toolMessages[0].content) as {
        task: SubagentSnapshot
      }
      expect(pinnedPayload.task.task_id).toBe(taskId)
      expect(pinnedPayload.task.status).toBe("stopped")
    },
    240_000
  )
})
