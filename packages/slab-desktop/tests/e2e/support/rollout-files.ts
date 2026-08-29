/**
 * Rollout-JSONL reader for e2e — reads the on-disk append-only rollout file
 * (`<sessionStateDir>/.../*.jsonl`; the state dir IS the rollout store root,
 * date-partitioned as `YYYY/MM/DD/`) that is slab's conversation true
 * source after the rollout refactor. The desktop test suite otherwise only
 * observes persistence via the REST history endpoint; reading the file itself
 * is the strongest check that the async observer + barrier actually wrote what
 * was emitted (no lost messages, no crossed turns).
 *
 * Line shape (parity with `slab_agent_rollout::RolloutLine` / `RolloutItem`):
 *   { timestamp, rolloutType: "sessionMeta" | "turnItem" | "eventMsg" | "compacted" | "turnContext", item: {...} }
 * `turnContext.item.kind` is `"messageAppend"` | `"turnState"` (camelCase).
 */
import { existsSync, readdirSync, readFileSync } from "node:fs"
import { join } from "node:path"

export type RolloutLine = {
  timestamp?: string
  rolloutType: string
  item: Record<string, unknown>
}

/** Recursively yield every `*.jsonl` file under `root` (date-tree + flat). */
function* walkJsonl(root: string): Generator<string> {
  if (!existsSync(root)) return
  for (const entry of readdirSync(root, { withFileTypes: true })) {
    const full = join(root, entry.name)
    if (entry.isDirectory()) {
      yield* walkJsonl(full)
    } else if (entry.name.endsWith(".jsonl")) {
      yield full
    }
  }
}

function readSessionMeta(file: string): Record<string, unknown> | undefined {
  try {
    const firstLine = readFileSync(file, "utf8").split(/\r?\n/, 1)[0]
    if (!firstLine) return undefined
    const line = JSON.parse(firstLine) as RolloutLine
    return line.rolloutType === "sessionMeta" ? line.item : undefined
  } catch {
    return undefined
  }
}

/** Find the rollout file whose `SessionMeta` first line carries `threadId`. */
export function findRolloutFile(sessionStateDir: string, threadId: string): string | undefined {
  for (const file of walkJsonl(sessionStateDir)) {
    if (readSessionMeta(file)?.threadId === threadId) return file
  }
  return undefined
}

/** Find a rollout file whose `SessionMeta.parentId` equals `parentThreadId` (a fork child). */
export function findRolloutChildFile(
  sessionStateDir: string,
  parentThreadId: string,
): string | undefined {
  for (const file of walkJsonl(sessionStateDir)) {
    if (readSessionMeta(file)?.parentId === parentThreadId) return file
  }
  return undefined
}

/** Parse every line of the thread's rollout file (skips unparseable lines). */
export function readRolloutLines(sessionStateDir: string, threadId: string): RolloutLine[] {
  const file = findRolloutFile(sessionStateDir, threadId)
  if (!file) return []
  return readRolloutFileLines(file)
}

/** Parse every line of a rollout file at a known path (skips unparseable lines). */
export function readRolloutFileLines(file: string): RolloutLine[] {
  const lines: RolloutLine[] = []
  for (const raw of readFileSync(file, "utf8").split(/\r?\n/)) {
    if (!raw.trim()) continue
    try {
      lines.push(JSON.parse(raw) as RolloutLine)
    } catch {
      // tolerate a malformed line, mirroring the Rust reader
    }
  }
  return lines
}

export const isSessionMeta = (line: RolloutLine): boolean => line.rolloutType === "sessionMeta"
export const isCompacted = (line: RolloutLine): boolean => line.rolloutType === "compacted"
export const isTurnContext = (line: RolloutLine): boolean => line.rolloutType === "turnContext"
export const isMessageAppend = (line: RolloutLine): boolean =>
  line.rolloutType === "turnContext" && line.item.kind === "messageAppend"
export const isTurnState = (line: RolloutLine): boolean =>
  line.rolloutType === "turnContext" && line.item.kind === "turnState"
export const isTurnItemLine = (line: RolloutLine): boolean => line.rolloutType === "turnItem"
