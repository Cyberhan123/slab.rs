/**
 * Scripted subagent delegation e2e (dedicated SLAB_E2E_MODE=1 stack, no model).
 *
 * The stack booted by `e2e-subagent-global-setup.ts` makes the debug-build
 * `ServerLlmAdapter` return deterministic prompt-keyed responses
 * (`crates/slab-app-core/src/infra/agent/adapter.rs`): parent turns follow
 * `subagent-e2e/…` markers, child turns the fixed `Objective:` /
 * `[steering from parent agent]` / `[subagent task finished]` prefixes the
 * delegation machinery produces. `slow=<ms>` inside the delegated task holds
 * the child inside its first LLM call, making stop/interrupt/steering races
 * deterministic.
 *
 * Covered scenarios:
 *   1. background delegate returns immediately + parent continues
 *   2. completion notification arrives + parent auto-resumes
 *   3. subagent_message steering reaches the running child
 *   4. subagent_stop cancels without a completion notification
 *   5. interrupting the parent cascades a stop to the child
 *   6. UI card flips running → completed end-to-end
 *   7. no_resume suppresses the parent follow-up (result stays queryable)
 *   8. the stall watchdog notices a silent child (short window via
 *      SLAB_E2E_STALL_MS, set by the global setup)
 */
import { readFile } from "node:fs/promises"
import { join } from "node:path"

import { afterAll, beforeAll, beforeEach, afterEach, describe, expect, inject, it } from "vitest"
import { chromium, type Browser, type Page } from "playwright"

import {
  createSession,
  eventually,
  restoreSession,
  type SessionResponse,
} from "./support/e2e-runtime"
import {
  assertNoUserMessageWithPrefixWithin,
  clickStopButton,
  expandSubagentToolRows,
  nonBlank,
  openAssistant,
  parseToolJson,
  selectPermissionMode,
  sendAssistantMessage,
  waitForSubagentCardStatus,
  waitForSubagentToolState,
  waitForToolExecution,
  waitForUserMessageWithPrefix,
} from "./support/assistant-ui"
import type { SubagentRuntimeEndpoints } from "./support/e2e-subagent-global-setup"

const endpoints = inject("e2e-subagent-runtime") as SubagentRuntimeEndpoints
const baseUrl = endpoints.serverBaseUrl

const NOTIFICATION_PREFIX = "[subagent task finished]"
const STALL_PREFIX = "[subagent task stalled]"

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
  // Env-gated failure diagnostics (SLAB_E2E_DEBUG=1): dump what the server
  // and page actually hold — the REST polls give no failure context alone.
  if (expect.getState().currentTestName && process.env.SLAB_E2E_DEBUG) {
    try {
      const restore = await restoreSession(baseUrl, session.id)
      console.log(
        `[debug] thread=${restore.thread?.status ?? "?"} messages=`,
        JSON.stringify(
          restore.messages.map(
            (message) =>
              `${message.role}${message.tool_calls ? `(${message.tool_calls.map((c) => c.function?.name).join(",")})` : ""}: ${(message.content ?? "").slice(0, 300)}`,
          ),
        ),
      )
      if (debugConsoleErrors.length > 0) {
        console.log("[debug] console errors:")
        for (const line of debugConsoleErrors.slice(0, 12)) console.log("  -", line.slice(0, 300))
      }
      // Expand the delegate rows and dump what the card body actually holds.
      const buttons = page.getByRole("button", { name: /Agent:/ })
      const buttonCount = await buttons.count()
      for (let index = 0; index < buttonCount; index += 1) {
        // eslint-disable-next-line no-await-in-loop
        const expanded = await buttons.nth(index).getAttribute("aria-expanded")
        if (expanded !== "true") {
          // eslint-disable-next-line no-await-in-loop
          await buttons.nth(index).click().catch(() => {})
        }
      }
      const cards = page.locator('[data-testid="tool-detail-subagent"]')
      const cardCount = await cards.count()
      for (let index = 0; index < cardCount; index += 1) {
        console.log(
          `[debug] card[${index}]:`,
          // eslint-disable-next-line no-await-in-loop
          ((await cards.nth(index).textContent().catch(() => "")) ?? "").slice(0, 200),
        )
      }
      console.log(
        "[debug] tool states:",
        await page
          .locator("[data-tool-state]")
          .evaluateAll((nodes) => nodes.map((n) => n.getAttribute("data-tool-state")))
          .catch(() => []),
      )
    } catch (error) {
      console.log("[debug] dump failed:", String(error))
    }
  }
  await page.close().catch(() => {})
})

afterAll(async () => {
  await browser.close().catch(() => {})
})

function needleFor(scenario: string): string {
  return `SUBAGENT_${scenario}_${Date.now().toString(36)}`
}

/** Assert some delegate card eventually shows `contains` in its detail body. */
async function expectSubagentCardText(target: Page, contains: string): Promise<void> {
  await eventually(`subagent card contains '${contains}'`, async () => {
    await expandSubagentToolRows(target)
    const cards = target.locator('[data-testid="tool-detail-subagent"]')
    const count = await cards.count()
    for (let index = 0; index < count; index += 1) {
      // eslint-disable-next-line no-await-in-loop
      const raw = await cards.nth(index).textContent()
      if (raw && raw.includes(contains)) {
        return true
      }
    }
    return null
  }, 60_000)
}

/** Wait until the session's thread reaches `status` (e.g. interrupted). The
 * interrupt pass runs a process-tree teardown of the in-flight shell tool,
 * which on Windows can take a while — generous default. */
async function waitForThreadStatus(
  sessionId: string,
  status: string,
  timeoutMs = 90_000
): Promise<void> {
  await eventually(`thread status '${status}'`, async () => {
    const restore = await restoreSession(baseUrl, sessionId)
    return restore.thread?.status === status
  }, timeoutMs, 1_000)
}

describe("subagent delegation (scripted LLM)", () => {
  it(
    "1. background delegate returns immediately and the parent continues",
    async () => {
      const needle = needleFor("S1")
      const prompt = `subagent-e2e/delegate task="count widgets needle=${needle} slow=30000"`

      await sendAssistantMessage(page, prompt)
      const result = await waitForToolExecution(baseUrl, session.id, prompt, "delegate_subagent", 90_000)
      const envelope = parseToolJson(result.toolMessages[0].content)

      // The background tool result lands immediately with a running task.
      expect(envelope.background).toBe(true)
      expect(envelope.status).toBe("running")
      expect(nonBlank(String(envelope.task_id ?? ""))).toBe(true)
      expect(nonBlank(String(envelope.child_thread_id ?? ""))).toBe(true)

      // The parent closed its own turn without waiting for the child.
      expect(nonBlank(result.finalText)).toBe(true)
      expect(result.finalText).toContain("E2E delegated in the background")

      // The dedicated card tracks the still-running child.
      await waitForSubagentCardStatus(page, "running")
      await waitForSubagentToolState(page, "input-available")
    },
    180_000
  )

  it(
    "2. completion notification arrives and the parent auto-resumes",
    async () => {
      const needle = needleFor("S2")
      const prompt = `subagent-e2e/delegate task="summarize needle=${needle}"`

      await sendAssistantMessage(page, prompt)
      const result = await waitForToolExecution(baseUrl, session.id, prompt, "delegate_subagent", 90_000)
      const envelope = parseToolJson(result.toolMessages[0].content)
      expect(envelope.background).toBe(true)

      // The bridge injects the notification as a user message…
      const notification = await waitForUserMessageWithPrefix(
        baseUrl,
        session.id,
        NOTIFICATION_PREFIX,
        90_000
      )
      expect(notification.content).toContain("status=completed")
      expect(notification.content).toContain(needle)
      // Bounded results are INLINED alongside the artifact reference — the
      // parent (and the card summary) see the text without a file read.
      expect(notification.content).toContain("Result: SUBAGENT_RESULT")
      expect(notification.content).toMatch(/Result artifact: \.slab\/artifacts\//)

      // …which auto-resumes the parent for a follow-up assistant turn.
      await eventually("parent resumed after the notification", async () => {
        const restore = await restoreSession(baseUrl, session.id)
        const index = restore.messages.findIndex(
          (message) => message.role === "user" && message.content.startsWith(NOTIFICATION_PREFIX)
        )
        if (index < 0) {
          return null
        }
        const after = restore.messages
          .slice(index + 1)
          .find((message) => message.role === "assistant" && nonBlank(message.content))
        return after?.content.includes("E2E parent resumed") ? after : null
      }, 90_000, 1_000)

      // The card flips to completed. Bounded results are inlined since the
      // notification-carry change, so the card summary also shows the
      // child's answer text.
      await waitForSubagentCardStatus(page, "completed")
      await waitForSubagentToolState(page, "output-available")
    },
    180_000
  )

  it(
    "3. subagent_message steers the running child",
    async () => {
      const needle = needleFor("S3A")
      const needle2 = needleFor("S3B")
      const delegatePrompt = `subagent-e2e/delegate task="draft needle=${needle} slow=12000"`

      await sendAssistantMessage(page, delegatePrompt)
      const delegated = await waitForToolExecution(
        baseUrl,
        session.id,
        delegatePrompt,
        "delegate_subagent",
        90_000
      )
      const taskId = String(parseToolJson(delegated.toolMessages[0].content).task_id)

      // Steer while the child is still inside its (slow) first turn.
      const steerPrompt = `subagent-e2e/steer task_id=${taskId} message="also cover needle2=${needle2}"`
      await sendAssistantMessage(page, steerPrompt)
      const steered = await waitForToolExecution(
        baseUrl,
        session.id,
        steerPrompt,
        "subagent_message",
        90_000
      )
      const steerResult = parseToolJson(steered.toolMessages[0].content)
      expect(steerResult.queued).toBe(true)

      // The steering's effect is asserted on the artifact (the child's final
      // answer is the steering ack carrying the steered needle) — the
      // artifact is the durable full record of the child's answer.
      const childThreadId = String(
        parseToolJson(delegated.toolMessages[0].content).child_thread_id,
      )
      const artifactPath = join(
        endpoints.workspaceRoot,
        ".slab",
        "artifacts",
        childThreadId,
        "result.json",
      )
      const artifact = await eventually(
        "subagent artifact contains the steering ack",
        async () => {
          const raw = await readFile(artifactPath, "utf8").catch(() => null)
          return raw?.includes(needle2) ? raw : null
        },
        120_000,
        1_000,
      )
      expect(artifact).toContain(needle2)
    },
    180_000
  )

  it(
    "4. subagent_stop cancels the child without a completion notification",
    async () => {
      const needle = needleFor("S4")
      const delegatePrompt = `subagent-e2e/delegate task="explore needle=${needle} slow=60000"`

      await sendAssistantMessage(page, delegatePrompt)
      const delegated = await waitForToolExecution(
        baseUrl,
        session.id,
        delegatePrompt,
        "delegate_subagent",
        90_000
      )
      const taskId = String(parseToolJson(delegated.toolMessages[0].content).task_id)

      const stopPrompt = `subagent-e2e/stop task_id=${taskId}`
      await sendAssistantMessage(page, stopPrompt)
      const stopped = await waitForToolExecution(
        baseUrl,
        session.id,
        stopPrompt,
        "subagent_stop",
        90_000
      )
      expect(JSON.stringify(parseToolJson(stopped.toolMessages[0].content))).toContain("stopped")

      // The card reflects the stopped state…
      await waitForSubagentCardStatus(page, "stopped")
      await waitForSubagentToolState(page, "output-denied")

      // …and explicit stops are never reported to the parent.
      await assertNoUserMessageWithPrefixWithin(
        baseUrl,
        session.id,
        NOTIFICATION_PREFIX,
        15_000
      )
    },
    180_000
  )

  it(
    "5. interrupting the parent cascades a stop to the child",
    async () => {
      const needle = needleFor("S5")
      // This stack is fully owned by the suite: full_control is safe here.
      await selectPermissionMode(page, "full_control")

      // `no_resume=1` keeps this delegation out of the stall watchdog's
      // scope (its 90s silent child would otherwise trip the suite-wide
      // short stall window used by scenario 8) — the cascade stop itself is
      // independent of the notification path, so the assertions below are
      // unaffected.
      const delegatePrompt = `subagent-e2e/delegate no_resume=1 task="probe needle=${needle} slow=90000"`
      await sendAssistantMessage(page, delegatePrompt)
      const delegated = await waitForToolExecution(
        baseUrl,
        session.id,
        delegatePrompt,
        "delegate_subagent",
        90_000
      )
      expect(delegated.toolCalls.length).toBe(1)
      await waitForSubagentCardStatus(page, "running")

      // Keep the parent turn generating (a 30s foreground shell call) so the
      // Stop control is live, then interrupt.
      await sendAssistantMessage(page, "subagent-e2e/shell-sleep/30000")
      await clickStopButton(page)

      await waitForThreadStatus(session.id, "interrupted")
      await waitForSubagentCardStatus(page, "stopped")
      await assertNoUserMessageWithPrefixWithin(
        baseUrl,
        session.id,
        NOTIFICATION_PREFIX,
        15_000
      )
    },
    180_000
  )

  it(
    "6. the dedicated card flips running → completed end-to-end",
    async () => {
      const needle = needleFor("S6")
      const prompt = `subagent-e2e/delegate task="report needle=${needle} slow=10000"`

      await sendAssistantMessage(page, prompt)
      const result = await waitForToolExecution(baseUrl, session.id, prompt, "delegate_subagent", 90_000)
      const envelope = parseToolJson(result.toolMessages[0].content)
      expect(envelope.background).toBe(true)

      // While the child works: running card, live spinner state.
      await waitForSubagentCardStatus(page, "running")
      await waitForSubagentToolState(page, "input-available")

      // Once it finishes: completed card with the task id meta line, the
      // relayed child activity, and the inlined result summary.
      await waitForSubagentCardStatus(page, "completed", 90_000)
      await waitForSubagentToolState(page, "output-available")
      await expectSubagentCardText(page, `task: ${String(envelope.task_id)}`)
      // The relayed child turn items render in the dedicated activity region
      // (Radix collapsible content must be expanded — see
      // `expandSubagentToolRows` inside `expectSubagentCardText`).
      await eventually("subagent activity region shows child items", async () => {
        await expandSubagentToolRows(page)
        const activity = page.locator('[data-testid="tool-detail-subagent-activity"]')
        const count = await activity.count()
        for (let index = 0; index < count; index += 1) {
          // eslint-disable-next-line no-await-in-loop
          const text = (await activity.nth(index).textContent()) ?? ""
          if (text.trim().length > 0) return true
        }
        return null
      }, 60_000)
      // The terminal backgroundTask event carries the truncated completion
      // text — the card's resultSummary box renders it (only the child's
      // answer text reaches this div, so the prefix pins the P1.1 fix).
      await expectSubagentCardText(page, "SUBAGENT_RESULT")
    },
    180_000
  )

  it(
    "7. no_resume suppresses the parent follow-up but keeps the result queryable",
    async () => {
      const needle = needleFor("S7")
      const delegatePrompt =
        `subagent-e2e/delegate no_resume=1 task="quiet probe needle=${needle} slow=10000"`

      await sendAssistantMessage(page, delegatePrompt)
      const delegated = await waitForToolExecution(
        baseUrl,
        session.id,
        delegatePrompt,
        "delegate_subagent",
        90_000
      )
      const envelope = parseToolJson(delegated.toolMessages[0].content)
      const childThreadId = String(envelope.child_thread_id)

      // The child still runs to completion (the card tracks it as usual)…
      await waitForSubagentCardStatus(page, "running")
      await waitForSubagentCardStatus(page, "completed", 90_000)

      // …but the parent is NEVER resumed: no completion notification arrives…
      await assertNoUserMessageWithPrefixWithin(
        baseUrl,
        session.id,
        NOTIFICATION_PREFIX,
        15_000
      )
      // …and no auto-resume turn happened either.
      const restore = await restoreSession(baseUrl, session.id)
      expect(
        restore.messages.some(
          (message) =>
            message.role === "assistant" &&
            message.content.includes("E2E parent resumed"),
        ),
      ).toBe(false)

      // The result stays queryable: the artifact carries the child's answer.
      const artifactPath = join(
        endpoints.workspaceRoot,
        ".slab",
        "artifacts",
        childThreadId,
        "result.json",
      )
      const artifact = await eventually("no_resume artifact written", async () => {
        const raw = await readFile(artifactPath, "utf8").catch(() => null)
        return raw?.includes(needle) ? raw : null
      }, 60_000, 1_000)
      expect(artifact).toContain(needle)
    },
    180_000
  )

  it(
    "8. the stall watchdog notices a silent child once, then the child finishes",
    async () => {
      const needle = needleFor("S8")
      // The child parks inside its first LLM call (silent, no open items) for
      // 30s; the global setup shortens the stall window to 18s via
      // SLAB_E2E_STALL_MS, so the watchdog fires mid-run.
      const delegatePrompt = `subagent-e2e/delegate task="stall probe needle=${needle} slow=30000"`

      await sendAssistantMessage(page, delegatePrompt)
      const delegated = await waitForToolExecution(
        baseUrl,
        session.id,
        delegatePrompt,
        "delegate_subagent",
        90_000
      )
      const envelope = parseToolJson(delegated.toolMessages[0].content)
      const childThreadId = String(envelope.child_thread_id)

      // The stall notice reaches the parent as a user message (assert by
      // user-message prefix — the parent's echo reply repeats the notice
      // text as an assistant message, so substring searches double-count).
      const stall = await waitForUserMessageWithPrefix(
        baseUrl,
        session.id,
        STALL_PREFIX,
        90_000
      )
      expect(stall.content).toContain(childThreadId)

      // One-shot: exactly one stall notice across the whole run.
      const restore = await restoreSession(baseUrl, session.id)
      expect(
        restore.messages.filter(
          (message) =>
            message.role === "user" && message.content.startsWith(STALL_PREFIX),
        ),
      ).toHaveLength(1)

      // The child was never interrupted: it completes normally and the
      // ordinary finished notification still arrives.
      await waitForSubagentCardStatus(page, "completed", 90_000)
      const notification = await waitForUserMessageWithPrefix(
        baseUrl,
        session.id,
        NOTIFICATION_PREFIX,
        90_000
      )
      expect(notification.content).toContain("status=completed")
    },
    180_000
  )
})
