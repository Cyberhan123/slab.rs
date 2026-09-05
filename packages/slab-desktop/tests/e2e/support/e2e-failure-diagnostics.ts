import { existsSync } from "node:fs"

import { afterEach, inject } from "vitest"

import { readFileTail, setDiagnosticsLogsDir } from "./e2e-runtime"

/**
 * Per-worker e2e diagnostics wiring (setupFiles):
 *
 * 1. Publishes the injected run's `logsDir` to e2e-runtime so UI helpers can
 *    mirror browser console output next to the server log without threading
 *    the runtime through every call site (see `openAssistant`).
 * 2. On a FAILED test, prints a bounded tail of the slab-server log to stderr.
 *    The full log survives at `<logsDir>/slab-server.log` (outside the run
 *    root, so teardown never deletes it) — the tail just saves a context
 *    switch at the exact moment the failure is on screen. Passing tests
 *    produce no output.
 */

type InjectedEndpoints = {
  logsDir?: string
  serverLogPath?: string
}

const serverLogTailBytes = 32 * 1024
const serverLogTailLines = 120

const endpoints: InjectedEndpoints | undefined = readInjectedEndpoints()

setDiagnosticsLogsDir(endpoints?.logsDir)

afterEach(async (context) => {
  // Assertion errors are recorded on the task result before afterEach hooks
  // run; hook failures of LATER hooks cannot be seen here (acceptable).
  const failed = (context.task?.result?.errors?.length ?? 0) > 0
  if (!failed || !endpoints?.serverLogPath) {
    return
  }
  if (!existsSync(endpoints.serverLogPath)) {
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
