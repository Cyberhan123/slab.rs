import { afterAll, beforeAll, beforeEach, afterEach, describe, expect, inject, it } from "vitest"
import { chromium, type Browser, type BrowserContext, type Page } from "playwright"

import type { E2eRuntimeEndpoints } from "./support/e2e-global-setup"
import {
  createSession,
  restoreSession,
  type AgentThreadMessageResponse,
  type ChatToolCall,
  type SessionResponse,
} from "./support/e2e-runtime"
import {
  expectPlanApprovalCard,
  expectPlanChip,
  openAssistant,
  sendAssistantMessage,
  togglePlanMode,
  waitForCompletedAssistantReply,
} from "./support/assistant-ui"

// Plan-as-agent e2e — the plan-mode full flow (master-plan headline acceptance
// scenario). Plan mode runs the turn as the built-in read-only `plan` agent
// (`turn/start` `agentType: "plan"`): the agent's tool denylist hides mutation
// tools (shell/write_file/...) from the model, and its system prompt steers it
// to explore read-only → `plan` → `present_plan` → approval gate. A directive
// prompt makes the model deterministically call the plan tools.
//
// Each test uses a FRESH session + thread so every plan turn is turn 0 (the
// plan-agent system prompt injects) and there is no cross-test state leakage.
//
// Env-gated: requires the shared local model (Qwen3.5-9B) + staged slab-server
// sidecar provided by the e2e global setup. Run via `bun run test:e2e`.

let env: E2eRuntimeEndpoints | undefined
let browser: Browser | undefined
let page: Page
let session: SessionResponse

describe("plan mode e2e", () => {
  beforeAll(async () => {
    env = inject("e2e-runtime")
    browser = await chromium.launch({ headless: true })
  })

  afterAll(async () => {
    await browser?.close().catch(() => {})
  })

  beforeEach(async () => {
    const endpoints = requireEnv()
    session = await createSession(
      endpoints.serverBaseUrl,
      `plan-mode-e2e-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`
    )
    const context: BrowserContext = await browser!.newContext({ viewport: { width: 1440, height: 960 } })
    await context.addInitScript(() => {
      window.localStorage.setItem("slab.ui.language", "en-US")
    })
    page = await context.newPage()
    // Close the per-test context when the page navigates away / test ends.
    page.on("close", () => {
      void context.close().catch(() => {})
    })
    await openAssistant(page, endpoints.uiBaseUrl, session.id)
  })

  afterEach(async () => {
    await page.close().catch(() => {})
  })

  // Headline scenario: toggle plan mode → the plan agent drafts a plan (`plan`)
  // and requests approval (`present_plan`) → approve → plan mode clears (chip
  // disappears) and the turn completes. Proves the full plan-as-agent loop
  // end-to-end against a real model.
  it("drafts a plan, requests approval, and clears plan mode when approved", async () => {
    const testEnv = requireEnv()
    const prompt = [
      "Use the plan tool exactly once to draft a concise 2-step plan to list all Markdown (*.md) files in the workspace.",
      "Then call present_plan to request my approval.",
      "Do not call any other tools.",
      // Bound the post-approval turn: once approved the chip clears and the next
      // turn would run as the default agent. Keep this turn to a text reply so it
      // completes instead of starting execution work.
      "After your plan is approved, do NOT execute it and do not call any tools; reply with a short sentence confirming the plan was approved.",
    ].join("\n")

    await togglePlanMode(page)
    await expectPlanChip(page, true)

    await sendAssistantMessage(page, prompt)
    // `present_plan` blocks the turn on the approval gate, so wait for the rich
    // plan approval card (which implicitly proves `plan` + `present_plan` both
    // ran) and approve it. A cold real-model first turn can be slow, so allow a
    // generous card-appearance window.
    await expectPlanApprovalCard(page, "approve", 600_000)
    // Approving a plan clears plan mode client-side (the chip disappears); the
    // next turn/start carries no `agentType` and runs as the default agent.
    await expectPlanChip(page, false)

    const reply = await waitForCompletedAssistantReply(
      testEnv.serverBaseUrl,
      session.id,
      prompt,
      600_000
    )
    expect(reply.restore.thread?.status).toBe("completed")
    expect(calledTool(reply.restore.messages, "plan")).toBe(true)
    expect(calledTool(reply.restore.messages, "present_plan")).toBe(true)
  }, 900_000)

  // Reject path: denying the plan keeps plan mode on (chip stays) and the model
  // recovers with a final reply instead of re-presenting.
  it("keeps plan mode on and recovers when the plan is rejected", async () => {
    const testEnv = requireEnv()
    const prompt = [
      "Use the plan tool exactly once to draft a concise 2-step plan to list all Markdown (*.md) files in the workspace.",
      "Then call present_plan to request my approval.",
      "Do not call any other tools.",
      "If the plan is rejected, do NOT call present_plan again; reply with a short sentence saying the plan was not approved.",
    ].join("\n")

    await togglePlanMode(page)
    await expectPlanChip(page, true)

    await sendAssistantMessage(page, prompt)
    await expectPlanApprovalCard(page, "reject", 600_000)
    // Rejection does not clear plan mode — the chip stays.
    await expectPlanChip(page, true)

    const reply = await waitForCompletedAssistantReply(
      testEnv.serverBaseUrl,
      session.id,
      prompt,
      600_000
    )
    expect(reply.restore.thread?.status).toBe("completed")
    expect(calledTool(reply.restore.messages, "present_plan")).toBe(true)
  }, 900_000)

  // Read-only enforcement: the plan agent's denylist hides `shell` from the
  // model's tool list, so a shell command can never execute. Drive the turn to
  // completion, rejecting any `present_plan` the plan agent happens to call
  // (its system prompt biases it toward presenting), then assert the shell
  // marker never appears in any tool output.
  it("blocks the shell tool from executing in plan mode", async () => {
    const testEnv = requireEnv()
    const marker = `SLAB_PLAN_E2E_RO_${Date.now()}`
    const prompt = [
      `Use the shell tool to run this POSIX shell command: echo ${marker}`,
      "The shell tool is not available to you. Do not call plan or present_plan.",
      "Reply with one short sentence saying the shell tool is unavailable and stop.",
    ].join("\n")

    await togglePlanMode(page)
    await expectPlanChip(page, true)

    await sendAssistantMessage(page, prompt)
    const { restore } = await waitForTurnCompletedRejectingPlans(
      page,
      testEnv.serverBaseUrl,
      session.id,
      600_000
    )
    // The mutation command never executed: the marker is absent from every tool
    // result in the thread.
    const executed = restore.messages.some(
      (message: AgentThreadMessageResponse) =>
        message.role === "tool" && message.content.includes(marker)
    )
    expect(executed).toBe(false)
  }, 900_000)
})

function requireEnv(): E2eRuntimeEndpoints {
  if (!env) {
    throw new Error("e2e shared runtime endpoints were not provided.")
  }
  return env
}

function calledTool(messages: AgentThreadMessageResponse[], name: string): boolean {
  return messages.some(
    (message: AgentThreadMessageResponse) =>
      message.role === "assistant" &&
      (message.tool_calls ?? []).some((toolCall: ChatToolCall) => toolCall.function.name === name)
  )
}

/// Poll the thread until it reaches a terminal status, rejecting any `present_plan`
/// approval card that appears along the way (the plan agent's system prompt may
/// steer it to present even when the user asked it not to). Used by the
/// read-only test so a stray `present_plan` can't block the turn indefinitely.
async function waitForTurnCompletedRejectingPlans(
  page: Page,
  baseUrl: string,
  sessionId: string,
  timeoutMs: number
): Promise<{ restore: Awaited<ReturnType<typeof restoreSession>> }> {
  const deadline = Date.now() + timeoutMs
  // eslint-disable-next-line no-constant-condition
  while (true) {
    // Reject any visible plan approval card to unblock the turn.
    const card = page.locator('[data-testid="assistant-approval-plan"]')
    if (await card.first().isVisible().catch(() => false)) {
      await page.getByTestId("assistant-approval-deny").click({ timeout: 10_000 }).catch(() => {})
    }
    const restore = await restoreSession(baseUrl, sessionId)
    if (restore.thread?.status === "errored") {
      throw new Error(`Agent thread errored: ${restore.thread.completion_text ?? "unknown error"}`)
    }
    if (restore.thread?.status === "completed") {
      return { restore }
    }
    if (Date.now() > deadline) {
      throw new Error("turn did not complete within timeout while rejecting plan approvals")
    }
    await new Promise((resolve) => setTimeout(resolve, 2_000))
  }
}
