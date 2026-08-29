import { afterAll, beforeAll, describe, expect, inject, it } from "vitest"
import { chromium, type Browser, type BrowserContext, type Page } from "playwright"

import type { E2eRuntimeEndpoints } from "./support/e2e-global-setup"
import { createSession, restoreSession } from "./support/e2e-runtime"
import {
  expectAssistantPageText,
  openAssistant,
  sendAssistantMessage,
  waitForCompletedAssistantReply,
  waitForComposerReady,
} from "./support/assistant-ui"
import {
  findRolloutChildFile,
  isCompacted,
  isMessageAppend,
  isSessionMeta,
  readRolloutFileLines,
  readRolloutLines,
  type RolloutLine,
} from "./support/rollout-files"

let env: E2eRuntimeEndpoints | undefined

/**
 * Rollout-persistence coverage for the E.2 refactor. The headline risk: the
 * conversation write path is now async (`EventMsg` → observer → rollout file),
 * coordinated by a FIFO-sentinel `Barrier` (`await_durable` before
 * compact/fork/rollback/restore re-reads). These cases drive the real server
 * (live observer + barrier) and verify the on-disk rollout file — not just the
 * REST history — so a lost message or crossed turn fails the test.
 */
describe("rollout persistence e2e", () => {
  let browser: Browser | undefined
  let context: BrowserContext | undefined
  let page: Page

  beforeAll(async () => {
    env = inject("e2e-runtime")
    browser = await chromium.launch({ headless: true })
    context = await browser.newContext({ viewport: { width: 1440, height: 960 } })
    await context.addInitScript(() => {
      window.localStorage.setItem("slab.ui.language", "en-US")
    })
    page = await context.newPage()
  })

  afterAll(async () => {
    await context?.close().catch(() => {})
    await browser?.close().catch(() => {})
  })

  // ① conversation persistence + restore — the async observer must not lose
  // messages or cross turns under real server timing.
  it("persists and restores a multi-turn conversation in the rollout file", async () => {
    const testEnv = requireEnv()
    const session = await createSession(testEnv.serverBaseUrl, `rollout-conv-${Date.now()}`)
    await openAssistant(page, testEnv.uiBaseUrl, session.id)

    const markerA = `SLAB_ROLLOUT_CONV_A_${Date.now()}`
    const markerB = `SLAB_ROLLOUT_CONV_B_${Date.now()}`
    const promptA = `Reply with only the token ${markerA}.`
    const promptB = `Reply with only the token ${markerB}.`

    await sendAssistantMessage(page, promptA)
    await waitForCompletedAssistantReply(testEnv.serverBaseUrl, session.id, promptA, 900_000)
    await sendAssistantMessage(page, promptB)
    await waitForCompletedAssistantReply(testEnv.serverBaseUrl, session.id, promptB, 900_000)

    const restored = await restoreSession(testEnv.serverBaseUrl, session.id)
    expect(restored.thread?.id).toBeTruthy()
    const threadId = restored.thread?.id ?? ""
    expect(restored.messages.some((m) => m.role === "user" && m.content === promptA)).toBe(true)
    expect(restored.messages.some((m) => m.role === "user" && m.content === promptB)).toBe(true)

    // Reload + re-enter via the deep link: the restored history must render.
    await page.reload({ waitUntil: "domcontentloaded", timeout: 60_000 })
    await openAssistant(page, testEnv.uiBaseUrl, session.id)
    await expectAssistantPageText(page, markerB)

    // The rollout file: SessionMeta header, then a MessageAppend per appended
    // message, each carrying a numeric turn index (no turn-crossing). NOTE:
    // `TurnContextPayload` renames only the enum TAG (`kind`) to camelCase —
    // the variant fields stay snake_case (`turn_index`).
    const lines = readRolloutLines(testEnv.sessionStateDir, threadId)
    expect(lines.length).toBeGreaterThan(0)
    expect(isSessionMeta(lines[0]!)).toBe(true)
    const appends = lines.filter(isMessageAppend)
    expect(appends.length).toBeGreaterThanOrEqual(4) // 2 user + 2 assistant
    for (const line of appends) {
      expect(typeof line.item.turn_index).toBe("number")
    }
  }, 900_000)

  // ② compact — verifies the `Compacted` row is written and compact's
  // `await_durable` fenced the observer before its conversation read.
  it("writes a Compacted row after a manual /compact", async () => {
    const testEnv = requireEnv()
    const session = await createSession(testEnv.serverBaseUrl, `rollout-compact-${Date.now()}`)
    await openAssistant(page, testEnv.uiBaseUrl, session.id)

    // Manual compact still SKIPS a minimal history: the keep window is
    // `context_length × 60%` (~9.8k tokens at the pinned 16384), and anything
    // inside it is kept verbatim — a tiny one-turn conversation is entirely
    // "recent", so nothing is summarized and no Compacted row is written.
    // Pad the first turn past the keep window (~9k tokens of filler + marker)
    // so the manual compact has older-than-window content to summarize.
    const marker = `SLAB_ROLLOUT_COMPACT_${Date.now()}`
    const filler = (
      "The quick brown fox jumps over the lazy dog while the rollout compaction e2e fills the context window. "
    ).repeat(180)
    const prompt = `Reply with only the token ${marker}.\n\n${filler}`
    await sendAssistantMessage(page, prompt)
    await waitForCompletedAssistantReply(testEnv.serverBaseUrl, session.id, prompt, 900_000)
    const shortPrompt = `Reply with only the token ${marker}_END.`
    await sendAssistantMessage(page, shortPrompt)
    await waitForCompletedAssistantReply(testEnv.serverBaseUrl, session.id, shortPrompt, 900_000)

    const threadId = (await restoreSession(testEnv.serverBaseUrl, session.id)).thread?.id ?? ""

    // `/compact` is intercepted by the sender before reaching the model.
    await sendAssistantMessage(page, "/compact")
    await page
      .locator('[data-testid^="assistant-compact-marker-manual:"]')
      .first()
      .waitFor({ state: "visible", timeout: 120_000 })
    await waitForComposerReady(page)

    expect(readRolloutLines(testEnv.sessionStateDir, threadId).some(isCompacted)).toBe(true)
  }, 900_000)

  // ③a fork — verifies `fork_thread`'s `await_durable` copied the parent history
  // (incl. turn 0) into the child rollout file.
  it("writes a child rollout file with the parent history after /fork", async () => {
    const testEnv = requireEnv()
    const session = await createSession(testEnv.serverBaseUrl, `rollout-fork-${Date.now()}`)
    await openAssistant(page, testEnv.uiBaseUrl, session.id)

    const prompt = `Reply with only the token SLAB_ROLLOUT_FORK_${Date.now()}.`
    await sendAssistantMessage(page, prompt)
    await waitForCompletedAssistantReply(testEnv.serverBaseUrl, session.id, prompt, 900_000)

    const parentThreadId = (await restoreSession(testEnv.serverBaseUrl, session.id)).thread?.id ?? ""

    await sendAssistantMessage(page, "/fork")
    await waitForComposerReady(page)

    const childFile = findRolloutChildFile(testEnv.sessionStateDir, parentThreadId)
    expect(childFile, "fork child rollout file was not written").toBeDefined()
    const childLines = readRolloutFileLines(childFile!)
    expect(childLines.length).toBeGreaterThan(0)
    expect(isSessionMeta(childLines[0]!)).toBe(true)
    expect((childLines[0]!.item.parentId as unknown) ?? null).toBe(parentThreadId)
    // The child must carry the parent's copied history (at least the one turn).
    expect(childLines.filter(isMessageAppend).length).toBeGreaterThanOrEqual(2)
  }, 900_000)

  // ③b rollback — verifies `rollback_thread`'s `await_durable` + truncate. The
  // rollback affordance lives under each retracable user bubble.
  // NOTE: selectors for the confirm dialog are written from the exploration; if
  // the dialog's role/label differs they may need a one-line tweak after a run.
  it("truncates the rollout when a user message is rolled back", async () => {
    const testEnv = requireEnv()
    const session = await createSession(testEnv.serverBaseUrl, `rollout-rb-${Date.now()}`)
    await openAssistant(page, testEnv.uiBaseUrl, session.id)

    for (const token of ["ONE", "TWO", "THREE"]) {
      const prompt = `Reply with only the token SLAB_ROLLOUT_RB_${token}_${Date.now()}.`
      await sendAssistantMessage(page, prompt)
      await waitForCompletedAssistantReply(testEnv.serverBaseUrl, session.id, prompt, 900_000)
    }

    const before = await restoreSession(testEnv.serverBaseUrl, session.id)
    const threadId = before.thread?.id ?? ""
    expect(before.messages.length).toBeGreaterThanOrEqual(6) // 3 user + 3 assistant

    // Reload + re-enter: the rollback affordance maps RESTORED item ids to
    // turn indexes (`userMessageTurnIndex`); live AI-SDK messages carry
    // client-generated ids that map to nothing, so the button only exists on
    // restored bubbles.
    await page.reload({ waitUntil: "domcontentloaded", timeout: 60_000 })
    await openAssistant(page, testEnv.uiBaseUrl, session.id)

    // Retract the latest user message and everything after it.
    const lastUser = page.getByTestId("assistant-message-user").last()
    await lastUser.locator('[data-testid="assistant-message-rollback"]').click()
    await page.getByRole("dialog").getByRole("button", { name: "Rollback" }).click()
    await waitForComposerReady(page)

    const after = await restoreSession(testEnv.serverBaseUrl, session.id)
    expect(after.messages.length).toBeLessThan(before.messages.length)

    // The rollout file must no longer carry turn items at turn >= the retracted one.
    const lines = readRolloutLines(testEnv.sessionStateDir, threadId)
    const retractedTurn = Number(before.messages.at(-1)?.turn_index ?? 0)
    const beyond = lines.filter((line: RolloutLine) => {
      const idx = Number(line.item.turn_index ?? -1)
      return !Number.isNaN(idx) && idx >= retractedTurn
    })
    expect(beyond.length).toBe(0)
  }, 900_000)
})

function requireEnv(): E2eRuntimeEndpoints {
  if (!env) {
    throw new Error("e2e shared runtime endpoints were not provided.")
  }
  return env
}
