import { appendFileSync, existsSync } from "node:fs"
import { join } from "node:path"

import { afterEach, inject } from "vitest"
import type { Page } from "playwright"

import { readFileTail, setDiagnosticsLogsDir } from "./e2e-runtime"

/**
 * Per-worker e2e diagnostics wiring (setupFiles):
 *
 * 1. Publishes the injected run's `logsDir` to e2e-runtime so UI helpers can
 *    mirror browser console output next to the server log without threading
 *    the runtime through every call site (see `openAssistant`).
 * 2. On a FAILED test, prints a bounded tail of the slab-server log to stderr
 *    plus a snapshot of every registered assistant page's DOM state (bubble
 *    texts + composer attributes) — post-mortems for "reply never rendered"
 *    and "composer stuck disabled" need page state, which the server log
 *    cannot show. The full log survives at `<logsDir>/slab-server.log`
 *    (outside the run root, so teardown never deletes it); the tails just
 *    save a context switch at the moment the failure is on screen. Passing
 *    tests produce no output.
 */

type InjectedEndpoints = {
  logsDir?: string
  serverLogPath?: string
}

const serverLogTailBytes = 32 * 1024
const serverLogTailLines = 120

const endpoints: InjectedEndpoints | undefined = readInjectedEndpoints()

setDiagnosticsLogsDir(endpoints?.logsDir)

/** Pages opened via `openAssistant` (weak — closed pages drop out). */
const registeredPages = new Set<Page>()

export function registerDiagnosticsPage(page: Page): void {
  registeredPages.add(page)
  page.once("close", () => registeredPages.delete(page))
}

afterEach(async (context) => {
  // Assertion errors are recorded on the task result before afterEach hooks
  // run; hook failures of LATER hooks cannot be seen here (acceptable).
  const failed = (context.task?.result?.errors?.length ?? 0) > 0
  if (!failed) {
    return
  }

  // Page-state snapshot first: a wedged page (the very failure under
  // investigation) must not block the log tail below. Best-effort.
  await Promise.all(
    [...registeredPages].map(async (page) => {
      if (page.isClosed()) {
        return
      }
      try {
        const snapshot = await page
          .evaluate(() => {
            const bubbles = [...document.querySelectorAll('[data-testid^="assistant-message-"]')]
              .map((node) => `${(node as HTMLElement).dataset.testid}: ${(node.textContent ?? "").slice(0, 160)}`)
            const composer = document.querySelector('[data-testid="assistant-composer-input"]')
            const composerState = composer
              ? `disabled=${(composer as HTMLTextAreaElement).disabled} value_len=${(composer as HTMLTextAreaElement).value.length}`
              : "absent"
            const sendMode = document
              .querySelector('[data-testid="assistant-send-button"]')
              ?.getAttribute("data-mode")
            return { bubbles, composerState, sendMode, url: location.href }
          })
          .catch(() => null)
        if (snapshot) {
          const rendered = `[e2e page snapshot] ${snapshot.url}\n  composer: ${snapshot.composerState} send-mode: ${String(snapshot.sendMode)}\n${snapshot.bubbles.map((line) => `  ${line}`).join("\n")}\n[e2e page snapshot] end`
          console.error(rendered)
          // Also persist next to the server log — piped vitest output can
          // swallow stderr, and the run dir outlives the console scrollback.
          const logsDir = endpoints?.logsDir
          if (logsDir) {
            try {
              appendFileSync(join(logsDir, "page-snapshot.log"), `${rendered}\n`, "utf8")
            } catch {
              // Best-effort diagnostics.
            }
          }
        }
      } catch {
        // Best-effort diagnostics.
      }
    })
  )

  if (!endpoints?.serverLogPath || !existsSync(endpoints.serverLogPath)) {
    return
  }

  let tail: string
  try {
    tail = await readFileTail(endpoints.serverLogPath, serverLogTailBytes)
  } catch (error) {
    console.error(`[e2e server-log tail] failed to read ${endpoints.serverLogPath}: ${String(error)}`)
    return
  }

  const lines = tail.split(/\r?\n/).filter((line) => line.trim() !== "")
  const clipped = lines.slice(-serverLogTailLines).join("\n")
  console.error(
    `\n[e2e server-log tail] full log: ${endpoints.serverLogPath}\n${clipped}\n[e2e server-log tail] end\n`
  )
})

function readInjectedEndpoints(): InjectedEndpoints | undefined {
  // The real-model suite provides "e2e-runtime", the scripted subagent suite
  // "e2e-subagent-runtime"; both carry logsDir/serverLogPath.
  for (const key of ["e2e-runtime", "e2e-subagent-runtime"] as const) {
    try {
      const value = inject(key) as InjectedEndpoints | undefined
      if (value?.serverLogPath) {
        return value
      }
    } catch {
      // Key not provided in this suite.
    }
  }
  return undefined
}
