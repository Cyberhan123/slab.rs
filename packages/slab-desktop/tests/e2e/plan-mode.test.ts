import { afterAll, beforeAll, describe, expect, inject, it } from "vitest"
import { chromium, type Browser, type BrowserContext, type Page } from "playwright"

import type { E2eRuntimeEndpoints } from "./support/e2e-global-setup"
import {
  createSession,
  eventually,
  restoreSession,
  type AgentSessionRestored,
  type AgentThreadMessageResponse,
  type ChatToolCall,
  type SessionResponse,
} from "./support/e2e-runtime"
import {
  expectPlanApprovalCard,
  expectPlanModeBanner,
  openAssistant,
  selectInteractionMode,
  sendAssistantMessage,
  waitForCompletedAssistantReply,
} from "./support/assistant-ui"

// Slice 5 e2e — the plan-mode full flow (master-plan headline acceptance
// scenario). Plan mode narrows the tool set to read-only
// (`interaction_constraint(Plan) → read_only()`), so a directive prompt can
// deterministically steer the model through `plan` → `present_plan` → approval
// gate. The plan-via-delegate (agent) scenario is NOT covered here: there is no
// UI surface to select an agent_type (`agent_type` is internal-only), so it is
// deferred to the future "agent_type → UI" slice.
//
// Env-gated: requires the shared local model (Qwen3.5-9B) + staged slab-server
// sidecar provided by the e2e global setup. Run via `bun run test:e2e`.

let env: E2eRuntimeEndpoints | undefined

describe("plan mode e2e", () => {
  let browser: Browser | undefined
  let context: BrowserContext | undefined
  let page: Page
  let session: SessionResponse

  beforeAll(async () => {
    env = inject("e2e-runtime")
    session = await createSession(env.serverBaseUrl, `plan-mode-e2e-${Date.now()}`)

    browser = await chromium.launch({ headless: true })
    context = await browser.newContext({ viewport: { width: 1440, height: 960 } })
    await context.addInitScript(() => {
      window.localStorage.setItem("slab.ui.language", "en-US")
    })
    page = await context.newPage()
    await openAssistant(page, env.uiBaseUrl, session.id)
  })

  afterAll(async () => {
    await context?.close().catch(() => {})
    await browser?.close().catch(() => {})
  })

  // Headline scenario: toggle Plan mode → model drafts a plan (`plan`) and
  // requests approval (`present_plan`) → approve → InteractionMode atomically
  // flips back to Default (banner clears) and the turn completes. Proves the
  // full Slice 2/3 plan loop end-to-end against a real model.
  it("drafts a plan, requests approval, and flips out of plan mode when approved", async () => {
    const testEnv = requireEnv()
    const prompt = [
      "Use the plan tool exactly once to draft a concise 2-step plan to list all Markdown (*.md) files in the workspace.",
      "Then call present_plan to request my approval.",
      "Do not call any other tools.",
      // Bound the post-approval turn: once approved, mutation tools unlock. Keep
      // the turn to a text reply so it completes instead of blocking on a
      // fresh shell approval.
      "After your plan is approved, do NOT execute it and do not call any tools; reply with a short sentence confirming the plan was approved.",
    ].join("\n")

    await selectInteractionMode(page, "plan")
    await expectPlanModeBanner(page, true)

    await sendAssistantMessage(page, prompt)
    // `present_plan` blocks the turn on the approval gate, so wait for the rich
    // plan approval card (which implicitly proves `plan` + `present_plan` both
    // ran) and approve it. A cold real-model first turn can be slow, so allow a
    // generous card-appearance window.
    await expectPlanApprovalCard(page, "approve", 600_000)
    // Approving a plan flips the thread to Default both server-side (turn loop)
    // and client-side (resolveApproval mirrors it); the banner clears.
    await expectPlanModeBanner(page, false)

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

  // Reject path: denying the plan keeps the thread in Plan mode (banner stays)
  // and the model recovers with a final reply instead of re-presenting.
  it("keeps plan mode and recovers when the plan is rejected", async () => {
    const testEnv = requireEnv()
    const prompt = [
      "Use the plan tool exactly once to draft a concise 2-step plan to list all Markdown (*.md) files in the workspace.",
      "Then call present_plan to request my approval.",
      "Do not call any other tools.",
      "If the plan is rejected, do NOT call present_plan again; reply with a short sentence saying the plan was not approved.",
    ].join("\n")

    await selectInteractionMode(page, "plan")
    await expectPlanModeBanner(page, true)

    await sendAssistantMessage(page, prompt)
    await expectPlanApprovalCard(page, "reject", 600_000)
    // Rejection does NOT flip the mode — the banner stays.
    await expectPlanModeBanner(page, true)

    const reply = await waitForCompletedAssistantReply(
      testEnv.serverBaseUrl,
      session.id,
      prompt,
      600_000
    )
    expect(reply.restore.thread?.status).toBe("completed")
    expect(calledTool(reply.restore.messages, "present_plan")).toBe(true)
  }, 900_000)

  // Read-only enforcement: in Plan mode mutation tools are blocked by the
  // approval policy — the model may still attempt `shell`, but the command is
  // rejected and never executes. Assert the marker never appears in any tool
  // output. Resolves as soon as the shell attempt is ruled on (or the thread
  // ends), independent of how the model recovers afterwards.
  it("blocks the shell tool from executing in plan mode", async () => {
    const testEnv = requireEnv()
    const marker = `SLAB_PLAN_E2E_RO_${Date.now()}`
    const prompt = [
      `Use the shell tool to run this POSIX shell command: echo ${marker}`,
      "If the shell tool is rejected, blocked, or unavailable, do NOT call any other tool (no plan, no read_file); reply with one short sentence and stop.",
    ].join("\n")

    await selectInteractionMode(page, "plan")
    await expectPlanModeBanner(page, true)

    await sendAssistantMessage(page, prompt)
    const { restore } = await waitForShellBlocked(
      testEnv.serverBaseUrl,
      session.id,
      prompt,
      marker,
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

function calledTool(
  messages: AgentThreadMessageResponse[],
  name: string
): boolean {
  return messages.some(
    (message: AgentThreadMessageResponse) =>
      message.role === "assistant" &&
      (message.tool_calls ?? []).some((toolCall: ChatToolCall) => toolCall.function.name === name)
  )
}

/// Resolve once the shell tool has been ruled on in plan mode (its result is
/// present — rejected by the approval policy) OR the thread completed without
/// ever running shell. Throws if the marker appears in any tool output (shell
/// actually executed). Independent of the model's recovery behavior, so it
/// resolves quickly on rejection instead of waiting for the turn to complete.
async function waitForShellBlocked(
  baseUrl: string,
  sessionId: string,
  prompt: string,
  marker: string,
  timeoutMs: number
): Promise<{ restore: AgentSessionRestored }> {
  return eventually(
    "shell blocked in plan mode",
    async () => {
      const restore = await restoreSession(baseUrl, sessionId)
      if (restore.thread?.status === "errored") {
        throw new Error(
          `Agent thread errored: ${restore.thread.completion_text ?? "unknown error"}`
        )
      }
      const promptIndex = restore.messages.findIndex(
        (message) => message.role === "user" && message.content === prompt
      )
      if (promptIndex < 0) {
        return null
      }
      const after = restore.messages.slice(promptIndex + 1)
      const shellIds = after.flatMap((message) =>
        message.role === "assistant"
          ? (message.tool_calls ?? [])
              .filter((toolCall: ChatToolCall) => toolCall.function.name === "shell")
              .map((toolCall) => toolCall.id)
              .filter((id): id is string => typeof id === "string" && id.length > 0)
          : []
      )
      const shellRuled = after.some(
        (message) =>
          message.role === "tool" &&
          typeof message.tool_call_id === "string" &&
          shellIds.includes(message.tool_call_id)
      )
      // Fail fast if the command actually executed.
      if (after.some((message) => message.role === "tool" && message.content.includes(marker))) {
        throw new Error("shell executed in plan mode — marker present in tool output")
      }
      if (shellRuled || restore.thread?.status === "completed") {
        return { restore }
      }
      return null
    },
    timeoutMs,
    1_000
  )
}
