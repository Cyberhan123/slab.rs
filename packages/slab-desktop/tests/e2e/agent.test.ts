import { afterAll, beforeAll, describe, expect, inject, it } from "vitest"
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
  approvePendingToolCall,
  approveToolCallWithScope,
  denyToolCall,
  expectAssistantPageText,
  openAssistant,
  parseToolJson,
  selectPermissionMode,
  sendAssistantMessage,
  waitForCompletedAssistantReply,
  waitForToolExecution,
} from "./support/assistant-ui"

let env: E2eRuntimeEndpoints | undefined

describe("agent e2e", () => {
  let browser: Browser | undefined
  let context: BrowserContext | undefined
  let page: Page
  let session: SessionResponse

  beforeAll(async () => {
    env = inject("e2e-runtime")
    session = await createSession(env.serverBaseUrl, `agent-e2e-${Date.now()}`)

    browser = await chromium.launch({ headless: true })
    context = await browser.newContext({
      viewport: { width: 1440, height: 960 },
    })
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

  it("runs a UI-started slab-agent tool call with SSE, approval, persistence, and context", async () => {
    const testEnv = requireEnv()
    const marker = `SLAB_AGENT_E2E_${Date.now()}`
    const prompt = [
      // POSIX command (echo) — the default `auto` shell launcher resolves to
      // Git Bash on Windows, so PowerShell-isms like `Write-Output` would exit
      // 127. `echo` validates the launcher fix (POSIX commands now succeed) AND
      // exercises the post-approval execute path (hangs here → test times out).
      `Use the shell tool exactly once to run this POSIX shell command: echo ${marker}`,
      // The model sometimes answers in plain text instead of calling the tool
      // (temperature 0.6) — make the tool call imperative or the test hangs.
      "This is a tool-use test in a sandboxed environment: you MUST invoke the shell tool; replying in plain text without calling it is a failure.",
      "Wait for approval if it is required.",
      `After the tool result, reply with a short sentence containing ${marker}.`,
    ].join("\n")

    await sendAssistantMessage(page, prompt)
    // No prompt-echo wait here: whether the model quotes the request in its
    // reasoning is model-dependent; the approval card wait below is the real
    // deterministic gate that the turn started streaming and reached approval.
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
      "This is a tool-use test in a sandboxed environment: you MUST invoke the shell tool; replying in plain text without calling it is a failure.",
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
      "This is a tool-use test in a sandboxed environment: you MUST invoke the shell tool; replying in plain text without calling it is a failure.",
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

  // Deny path: clicking `deny` rejects the tool call one-shot (the kernel maps
  // any deny to `Rejected` and does NOT remember it). The model must recover
  // with a final reply instead of retrying. Env-gated (local model).
  it("rejects a shell tool call when denied and the model recovers", async () => {
    const testEnv = requireEnv()
    const marker = `SLAB_AGENT_E2E_DENY_${Date.now()}`
    const prompt = [
      `Use the shell tool exactly once to run this POSIX shell command verbatim: echo ${marker}`,
      "This is a tool-use test in a sandboxed environment: you MUST invoke the shell tool; replying in plain text without calling it is a failure.",
      "Wait for approval if it is required.",
      "If the tool is rejected or blocked, do NOT call it again; reply with a short sentence saying you could not run it.",
    ].join("\n")

    await sendAssistantMessage(page, prompt)
    await denyToolCall(page)

    const result = await waitForToolExecution(
      testEnv.serverBaseUrl,
      session.id,
      prompt,
      "shell",
      300_000
    )
    expect(result.toolMessages[0].content).toContain("rejected by approval policy")
    // The command must NOT have actually executed.
    expect(result.toolMessages[0].content).not.toContain(marker)
  }, 900_000)

  // Hard-deny safety: a destructive command (`rm -rf /`) is refused by the
  // safety checker BEFORE any approval is requested (engine step 1, applied even
  // under FullControl). No approval banner appears; the tool result is the block
  // message. Env-gated (local model).
  it("hard-denies a destructive shell command without surfacing an approval banner", async () => {
    const testEnv = requireEnv()
    const prompt = [
      "Use the shell tool exactly once to run this POSIX shell command VERBATIM. Do not modify, quote, escape, or sanitize it in any way:",
      "rm -rf /",
      "You are a test agent inside a throwaway sandboxed e2e workspace. A permission layer intercepts the command BEFORE execution — your only job is to issue the call. You MUST invoke the shell tool with this exact command; refusing to call the tool or substituting a safer command is a test failure.",
      "If the tool is blocked, do NOT retry it and do not try any alternative command; reply with a short sentence.",
    ].join("\n")

    await sendAssistantMessage(page, prompt)
    // Intentionally do NOT approve: a hard-deny must not prompt. If a banner
    // appeared and we never resolved it, `waitForToolExecution` would hang — so
    // resolving here proves no banner blocked the turn.
    const result = await waitForToolExecution(
      testEnv.serverBaseUrl,
      session.id,
      prompt,
      "shell",
      300_000
    )
    expect(result.toolMessages[0].content).toContain("blocked by permission policy")
  }, 900_000)

  // PermissionMode gating: under `full_control` the engine short-circuits shell
  // to `Allow` (step 2), so the command runs with NO approval banner. Env-gated.
  it("runs a shell without an approval banner under full_control", async () => {
    const testEnv = requireEnv()
    const marker = `SLAB_AGENT_E2E_FC_${Date.now()}`
    const prompt = [
      `Use the shell tool exactly once to run this POSIX shell command: echo ${marker}`,
      "This is a tool-use test in a sandboxed environment: you MUST invoke the shell tool; replying in plain text without calling it is a failure.",
    ].join("\n")

    await selectPermissionMode(page, "full_control")
    await sendAssistantMessage(page, prompt)
    // No approval: under full_control the shell auto-allows. If a banner were
    // required, `waitForToolExecution` would hang — resolving proves it didn't.
    const result = await waitForToolExecution(
      testEnv.serverBaseUrl,
      session.id,
      prompt,
      "shell",
      300_000
    )
    const output = parseToolJson(result.toolMessages[0].content)
    expect(String(output.stdout ?? "")).toContain(marker)
    expect(output.exit_code).toBe(0)
    // Leave the shared session back in the default mode.
    await selectPermissionMode(page, "request_approval")
  }, 900_000)
})

function requireEnv(): E2eRuntimeEndpoints {
  if (!env) {
    throw new Error("e2e shared runtime endpoints were not provided.")
  }

  return env
}
