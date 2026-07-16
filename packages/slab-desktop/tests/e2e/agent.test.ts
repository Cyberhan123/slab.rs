import { afterAll, beforeAll, describe, expect, it } from "vitest"
import { chromium, type Browser, type BrowserContext, type Page } from "playwright"

import {
  bootstrapLocalModel,
  cleanupFullstackDevEnvironment,
  createFullstackDevEnvironment,
  createSession,
  restoreSession,
  selectAssistantSession,
  startFullstackDev,
  type AgentThreadMessageResponse,
  type ChatToolCall,
  type FullstackDevEnvironment,
  type ManagedDevProcess,
  type SessionResponse,
} from "./support/fullstack-dev"
import {
  approvePendingToolCall,
  approveToolCallWithScope,
  expectAssistantPageText,
  openAssistant,
  parseToolJson,
  sendAssistantMessage,
  waitForCompletedAssistantReply,
  waitForToolExecution,
} from "./support/assistant-ui"

let env: FullstackDevEnvironment | undefined

describe.sequential("agent e2e", () => {
  let browser: Browser | undefined
  let context: BrowserContext | undefined
  let dev: ManagedDevProcess | undefined
  let page: Page
  let session: SessionResponse

  beforeAll(async () => {
    env = await createFullstackDevEnvironment()
    dev = await startFullstackDev(env)
    await bootstrapLocalModel(env.serverBaseUrl, {
      modelId: "Qwen3.5-9B",
      selectedVariantId: "Q8_0",
    })
    session = await createSession(env.serverBaseUrl, `agent-e2e-${Date.now()}`)
    await selectAssistantSession(env.serverBaseUrl, session.id, session.name)

    browser = await chromium.launch({ headless: true })
    context = await browser.newContext({
      viewport: { width: 1440, height: 960 },
    })
    await context.addInitScript(() => {
      window.localStorage.setItem("slab.ui.language", "en-US")
    })
    page = await context.newPage()
    await openAssistant(page, env.uiBaseUrl)
  })

  afterAll(async () => {
    await context?.close().catch(() => {})
    await browser?.close().catch(() => {})
    await dev?.stop().catch(() => {})
    cleanupFullstackDevEnvironment(env)
  })

  it("runs a UI-started slab-agent tool call with SSE, approval, persistence, and context", async () => {
    const testEnv = requireEnv()
    const marker = `SLAB_AGENT_E2E_${Date.now()}`
    const prompt = [
      // POSIX command (echo) — the default `auto` shell launcher resolves to
      // Git Bash on Windows, so PowerShell-isms like `Write-Output` would exit
      // 127. `echo` validates the launcher fix (POSIX commands now succeed) AND
      // exercises the post-approval execute path (hangs here → test times out).
      `Use the shell tool exactly once to run this POSIX shell command: echo ${marker}`,
      "Wait for approval if it is required.",
      `After the tool result, reply with a short sentence containing ${marker}.`,
    ].join("\n")

    await sendAssistantMessage(page, prompt)
    await expectAssistantPageText(page, prompt)
    await approvePendingToolCall(page)

    const toolResult = await waitForToolExecution(
      testEnv.serverBaseUrl,
      session.id,
      prompt,
      "shell",
      1_200_000
    )
    expect(toolResult.finalText).toContain(marker)
    expect(toolResult.toolCalls.length).toBeGreaterThanOrEqual(1)

    const output = parseToolJson(toolResult.toolMessages[0].content)
    expect(String(output.stdout ?? "")).toContain(marker)
    expect(output.exit_code).toBe(0)
    await expectAssistantPageText(page, toolResult.finalText)

    const recallPrompt = `What exact marker did the shell tool output in the previous turn? Reply with only the marker.`
    await sendAssistantMessage(page, recallPrompt)
    const recall = await waitForCompletedAssistantReply(
      testEnv.serverBaseUrl,
      session.id,
      recallPrompt,
      900_000
    )
    expect(recall.text).toContain(marker)

    const restored = await restoreSession(testEnv.serverBaseUrl, session.id)
    expect(restored.thread?.status).toBe("completed")
    expect(
      restored.messages.some(
        (message: AgentThreadMessageResponse) =>
          message.role === "assistant" &&
          (message.tool_calls ?? []).some((toolCall: ChatToolCall) => toolCall.function.name === "shell")
      )
    ).toBe(true)
    expect(
      restored.messages.some(
        (message: AgentThreadMessageResponse) => message.role === "tool" && message.content.includes(marker)
      )
    ).toBe(true)
  })

  // Regression for the `wait_for_child` pipe-EOF deadlock: a shell command that
  // backgrounds a long-lived child (which inherits + holds the stdout pipe) must
  // NOT hang the turn after approval. Before the tree-kill fix the read tasks
  // waited for pipe EOF forever. Env-gated: requires a local model + a freshly
  // staged sidecar (see CLAUDE.md memory slab-app-sidecar-staging-gotcha).
  it("completes a shell turn that backgrounds a long-lived child (tree-kill hang fix)", async () => {
    const testEnv = requireEnv()
    const marker = `SLAB_AGENT_E2E_BG_${Date.now()}`
    // `sleep 300 &` leaves a descendant holding the pipe open after the shell
    // exits; under Git Bash (auto launcher) this reproduces the post-approval
    // hang that tree-kill in `wait_for_child` resolves.
    const prompt = [
      `Use the shell tool exactly once to run this POSIX shell command verbatim: echo ${marker}; sleep 300 &`,
      "Wait for approval if it is required.",
      `After the tool result, reply with a short sentence containing ${marker}.`,
    ].join("\n")

    const started = Date.now()
    await sendAssistantMessage(page, prompt)
    await approvePendingToolCall(page)

    const toolResult = await waitForToolExecution(
      testEnv.serverBaseUrl,
      session.id,
      prompt,
      "shell",
      180_000
    )
    const elapsedMs = Date.now() - started

    const output = parseToolJson(toolResult.toolMessages[0].content)
    expect(String(output.stdout ?? "")).toContain(marker)
    expect(output.exit_code).toBe(0)
    // The fix makes the turn complete in a few seconds (sleep is tree-killed);
    // a regression hangs for the 300s background sleep (well past this bound).
    expect(elapsedMs).toBeLessThan(90_000)
  })

  // Validates the `ApprovalScope` persistence path: approving a shell with
  // `always_in_workspace` silences the approval for an equivalent subsequent
  // command (it runs directly, no second prompt). Env-gated (local model).
  it("silences the next equivalent shell after an always_in_workspace approval", async () => {
    const testEnv = requireEnv()
    const firstMarker = `SLAB_AGENT_E2E_SCOPE_A_${Date.now()}`
    const firstPrompt = [
      `Use the shell tool exactly once to run: echo ${firstMarker}`,
      "Wait for approval if it is required.",
    ].join("\n")

    await sendAssistantMessage(page, firstPrompt)
    // Approve with a persistent workspace scope so the rule is remembered.
    await approveToolCallWithScope(page, "always_in_workspace")
    await waitForToolExecution(testEnv.serverBaseUrl, session.id, firstPrompt, "shell", 180_000)

    // An equivalent shell command should run WITHOUT another approval prompt.
    // If a prompt were required, `waitForToolExecution` would never resolve
    // (we don't approve again) — so resolving within the window proves the
    // remembered rule auto-allowed it.
    const secondMarker = `SLAB_AGENT_E2E_SCOPE_B_${Date.now()}`
    const secondPrompt = `Use the shell tool exactly once to run: echo ${secondMarker}`
    await sendAssistantMessage(page, secondPrompt)

    const toolResult = await waitForToolExecution(
      testEnv.serverBaseUrl,
      session.id,
      secondPrompt,
      "shell",
      180_000
    )
    const output = parseToolJson(toolResult.toolMessages[0].content)
    expect(String(output.stdout ?? "")).toContain(secondMarker)
    expect(output.exit_code).toBe(0)
  })
})

function requireEnv(): FullstackDevEnvironment {
  if (!env) {
    throw new Error("Fullstack dev environment was not initialized.")
  }

  return env
}
