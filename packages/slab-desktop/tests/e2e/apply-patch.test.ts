import { mkdir, readFile, writeFile } from "node:fs/promises"
import { dirname, join } from "node:path"

import { afterAll, beforeAll, describe, expect, inject, it } from "vitest"
import { chromium, type Browser, type BrowserContext, type Page } from "playwright"

import type { E2eRuntimeEndpoints } from "./support/e2e-global-setup"
import {
  createSession,
  type SessionResponse,
} from "./support/e2e-runtime"
import {
  approvePendingToolCall,
  expectFileChangeCard,
  openAssistant,
  parseToolJson,
  sendAssistantMessage,
  waitForToolExecution,
} from "./support/assistant-ui"

let env: E2eRuntimeEndpoints | undefined
let marker = ""
let markerRoot = ""

// apply_patch drives the `slab-apply-patch` `*** Begin Patch` engine. The model
// is handed a correct `*** Begin Patch` patch VERBATIM and asked to echo it into
// the tool call — we never hand it a unified diff and hope it translates (the
// engine only accepts the `*** Begin Patch` dialect). apply_patch is a FileEdit,
// so under the default request_approval mode each edit surfaces an approval
// banner that the test approves via the UI.
//
// Scope note: partial-failure deltas, Unicode fuzzy matching, and the
// always_in_workspace persistence rule are exercised at the unit level
// (slab-apply-patch `lib.rs` / `seek_sequence.rs`, slab-exec-policy) and the
// rule mechanism is covered end-to-end by the shell always_in_workspace case in
// agent.test.ts. This real-model suite covers the end-to-end apply_patch happy
// path, the file-edit approval flow, and fuzzy matching through the full stack.
describe("apply_patch e2e", () => {
  let browser: Browser | undefined
  let context: BrowserContext | undefined
  let page: Page
  let session: SessionResponse

  beforeAll(async () => {
    env = inject("e2e-runtime")
    session = await createSession(env.serverBaseUrl, `apply-patch-e2e-${Date.now()}`)
    marker = `slab-apply-patch-e2e-${Date.now()}`
    markerRoot = `.slab-e2e-apply-patch/${marker}`

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

  it("edits a workspace file via apply_patch after a file-edit approval", async () => {
    const testEnv = requireEnv()
    const target = rel("happy", "target.txt")
    await seed(target, "alpha\nbeta\n")
    const patch = beginPatchUpdate(target, ["-alpha", "+alpha PATCHED"])
    const prompt = applyPatchPrompt(patch, fileNote(target, "alpha\nbeta\n"))

    await sendAssistantMessage(page, prompt)
    // FileEdit surfaces an approval banner under the default request_approval mode.
    await approvePendingToolCall(page)

    const result = await waitForToolExecution(
      testEnv.serverBaseUrl,
      session.id,
      prompt,
      "apply_patch",
      900_000
    )
    expect(result.toolCalls[0].function.arguments).toContain("*** Begin Patch")
    const output = parseToolJson(result.toolMessages[0].content)
    expect(output.result).toBe("ok")
    expectAppliedFile(output, target)
    expect(await read(target)).toBe("alpha PATCHED\nbeta\n")
    // The file-change diff card must render in the DOM (proves the live
    // FileChangeOutputDelta → MessageToolFileChangePart path end-to-end).
    await expectFileChangeCard(page, { path: target, contains: "*** Begin Patch" })
  }, 900_000)

  it("applies a patch whose context matches the file only after Unicode normalization", async () => {
    const testEnv = requireEnv()
    const target = rel("fuzzy", "target.txt")
    // Seed with an en-dash (U+2013); the patch below writes the context line
    // with an ASCII hyphen. seek_sequence normalizes both sides (en-dash -> '-')
    // so the patch still locates the line. The file contents are NOT shown to
    // the model so it echoes the ASCII patch verbatim instead of "correcting"
    // it to the en-dash.
    await seed(target, "price – cost\ntotal\n")
    const patch = beginPatchUpdate(target, ["-price - cost", "+price - cost PAID"])
    const prompt = applyPatchPrompt(patch)

    await sendAssistantMessage(page, prompt)
    await approvePendingToolCall(page)

    const result = await waitForToolExecution(
      testEnv.serverBaseUrl,
      session.id,
      prompt,
      "apply_patch",
      900_000
    )
    expect(result.toolCalls[0].function.arguments).toContain("*** Begin Patch")
    const output = parseToolJson(result.toolMessages[0].content)
    expect(output.result).toBe("ok")
    expect(await read(target)).toBe("price - cost PAID\ntotal\n")
  }, 900_000)
})

function requireEnv(): E2eRuntimeEndpoints {
  if (!env) {
    throw new Error("e2e shared runtime endpoints were not provided.")
  }
  return env
}

function rel(...segments: string[]): string {
  return [markerRoot, ...segments].join("/")
}

function absOf(relativePath: string): string {
  if (!env) {
    throw new Error("Fullstack dev environment is not initialized.")
  }
  return join(env.workspaceRoot, ...relativePath.split("/"))
}

async function seed(relativePath: string, content: string): Promise<void> {
  const path = absOf(relativePath)
  await mkdir(dirname(path), { recursive: true })
  await writeFile(path, content, "utf8")
}

async function read(relativePath: string): Promise<string> {
  return readFile(absOf(relativePath), "utf8")
}

function beginPatchUpdate(relativePath: string, changeLines: string[]): string {
  return [
    "*** Begin Patch",
    `*** Update File: ${relativePath}`,
    "@@",
    ...changeLines,
    "*** End Patch",
    "",
  ].join("\n")
}

function fileNote(relativePath: string, content: string): string {
  return `The workspace file ${relativePath} currently contains exactly:\n\`\`\`\n${content}\n\`\`\``
}

function applyPatchPrompt(patch: string, note = ""): string {
  return [
    "You are running a Slab apply_patch e2e test.",
    note,
    "Call the apply_patch tool exactly once with the following patch as its `patch` argument, VERBATIM (reproduce every character exactly, including the `*** Begin Patch` and `*** End Patch` lines and all leading `+`/`-`/space prefixes):",
    "```",
    patch,
    "```",
    "Do not call any other tool (do not read_file, list_dir, write_file, etc.). Your first assistant output must be the apply_patch tool call only. After the tool result, reply with one short sentence.",
  ]
    .filter((line) => line.length > 0)
    .join("\n")
}

/** Assert the tool-result JSON lists the edited file. `applied_files` holds
 * absolute paths (OS separators); compare against the relative path after
 * normalizing backslashes so the assertion holds on both Windows and Linux. */
function expectAppliedFile(output: Record<string, unknown>, relativePath: string): void {
  const applied = (output.applied_files as unknown[] | undefined) ?? []
  const normalized = applied.map((entry) => String(entry).replace(/\\/g, "/"))
  expect(
    normalized.some((path) => path.endsWith(relativePath)),
    `expected applied_files ${JSON.stringify(applied)} to include ${relativePath}`
  ).toBe(true)
}
