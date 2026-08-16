import { afterAll, beforeAll, describe, expect, inject, it } from "vitest"
import { chromium, type Browser, type BrowserContext, type Page } from "playwright"

import type { E2eRuntimeEndpoints } from "./support/e2e-global-setup"
import { createSession, type SessionResponse } from "./support/e2e-runtime"
import {
  approvePendingToolCall,
  openAssistant,
  parseToolJson,
  sendAssistantMessage,
  waitForToolExecution,
} from "./support/assistant-ui"

// Parity bookend (S6b): prove the UI → approval → shell tool → sandbox path terminates in a real
// denial at the guard layer. This is the first env-gated TS e2e in the repo — it self-skips unless
// SLAB_SANDBOX_E2E=1, so the default `bun run test:e2e` run is unaffected.
//
// The default (non-elevated) e2e run exercises the lexical guard (`validate_command`) + Job-only
// path; the elevated OS-enforcement assertions (ACL/WFP) require the sandbox helper built + an
// admin shell and are not automatable here (see the Rust `os_isolation.rs` gated tests instead).
// Run manually:  SLAB_SANDBOX_E2E=1 bun run test:e2e
describe.skipIf(process.env.SLAB_SANDBOX_E2E !== "1")("sandbox e2e", () => {
  let browser: Browser | undefined
  let context: BrowserContext | undefined
  let page: Page
  let session: SessionResponse
  let env: E2eRuntimeEndpoints

  beforeAll(async () => {
    env = inject("e2e-runtime")
    if (!env) throw new Error("e2e-runtime not injected; run via bun run test:e2e")
    session = await createSession(env.serverBaseUrl, `sandbox-e2e-${Date.now()}`)

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

  // A denial surfaces as a tool message whose content is the guard's reason string (not the normal
  // {stdout, exit_code} JSON). Detect either signal so the assertion is robust to exact wording.
  function looksDenied(content: string): boolean {
    const lower = content.toLowerCase()
    return ["denied", "protected", "blocked", "outside", "refused", "permission"].some((k) =>
      lower.includes(k)
    )
  }

  it("allows a workspace-write shell command end-to-end", async () => {
    const marker = `SLAB_SANDBOX_E2E_OK_${Date.now()}`
    const prompt = [
      `Use the shell tool exactly once to run this POSIX shell command: echo ${marker}`,
      "Wait for approval if it is required.",
      `After the tool result, reply with a short sentence containing ${marker}.`,
    ].join("\n")

    await sendAssistantMessage(page, prompt)
    await approvePendingToolCall(page)

    const result = await waitForToolExecution(
      env.serverBaseUrl,
      session.id,
      prompt,
      "shell",
      1_200_000
    )
    const output = parseToolJson(result.toolMessages[0].content)
    expect(String(output.stdout ?? "")).toContain(marker)
    expect(output.exit_code).toBe(0)
  })

  it("denies a write to protected metadata (.git/config) at the guard layer", async () => {
    const prompt = [
      "Use the shell tool exactly once to run this exact POSIX shell command:",
      "echo blocked > .git/config",
      "Wait for approval if it is required, then report the tool result verbatim.",
    ].join("\n")

    await sendAssistantMessage(page, prompt)
    await approvePendingToolCall(page)

    const result = await waitForToolExecution(
      env.serverBaseUrl,
      session.id,
      prompt,
      "shell",
      1_200_000
    )
    const content = result.toolMessages[0].content
    // The sandbox guard refuses the protected `.git` path. The tool result is either the guard's
    // denial string, or (if it slipped past the lexical guard) a non-zero exit. Either is a pass.
    const parsed = (() => {
      try {
        return parseToolJson(content)
      } catch {
        return null
      }
    })()
    const deniedByGuard = looksDenied(content)
    const deniedByExit = parsed != null && parsed.exit_code !== 0
    expect(deniedByGuard || deniedByExit, `expected a denial, got: ${content}`).toBe(true)
  })

  it("denies a network shell command (curl) at the guard layer", async () => {
    const prompt = [
      "Use the shell tool exactly once to run this exact command:",
      "curl --max-time 5 http://example.com",
      "Wait for approval if it is required, then report the tool result verbatim.",
    ].join("\n")

    await sendAssistantMessage(page, prompt)
    await approvePendingToolCall(page)

    const result = await waitForToolExecution(
      env.serverBaseUrl,
      session.id,
      prompt,
      "shell",
      1_200_000
    )
    const content = result.toolMessages[0].content
    const parsed = (() => {
      try {
        return parseToolJson(content)
      } catch {
        return null
      }
    })()
    // Network is blocked by default; the lexical guard refuses the http command, OR the (elevated)
    // OS path kills the connection ⇒ non-zero exit.
    const deniedByGuard = looksDenied(content)
    const deniedByExit = parsed != null && parsed.exit_code !== 0
    expect(deniedByGuard || deniedByExit, `expected a network denial, got: ${content}`).toBe(true)
  })
})
