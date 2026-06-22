import { execFileSync } from "node:child_process"
import { mkdirSync, readFileSync, writeFileSync } from "node:fs"
import { join } from "node:path"

import { chromium, type Browser, type BrowserContext, type Locator, type Page } from "playwright"
import { afterAll, beforeAll, describe, expect, it } from "vitest"
import type { components } from "@slab/api/v1"

import {
  cleanupFullstackDevEnvironment,
  completeSetup,
  createFullstackDevEnvironment,
  eventually,
  requestJson,
  startFullstackDev,
  type FullstackDevEnvironment,
  type ManagedDevProcess,
} from "./support/fullstack-dev"

type Schema = components["schemas"]

let env: FullstackDevEnvironment | undefined

describe.sequential("workspace e2e", () => {
  let browser: Browser | undefined
  let context: BrowserContext | undefined
  let dev: ManagedDevProcess | undefined
  let page: Page
  let browserErrors: string[] = []
  let consoleFailures: string[] = []
  let workspaceRoot: string
  let notePath: string

  beforeAll(async () => {
    env = await createFullstackDevEnvironment()
    workspaceRoot = join(env.rootDir, "browser-workspace")
    notePath = join(workspaceRoot, "src", "note.txt")
    prepareGitWorkspace(workspaceRoot)

    dev = await startFullstackDev(env)
    await completeSetup(env.serverBaseUrl)

    browser = await chromium.launch({ headless: true })
    context = await browser.newContext({
      viewport: { width: 1440, height: 960 },
    })
    await context.addInitScript((apiBaseUrl) => {
      ;(window as typeof window & { __SLAB_API_BASE_URL__?: string }).__SLAB_API_BASE_URL__ = apiBaseUrl
      window.localStorage.setItem("slab.ui.language", "en-US")
    }, env.uiBaseUrl)
    page = await context.newPage()
    page.on("console", (message) => {
      if (message.type() !== "error") {
        return
      }
      const text = message.text()
      browserErrors.push(text)
      if (isWorkspaceConsoleFailure(text)) {
        consoleFailures.push(text)
      }
    })
    page.on("pageerror", (error) => {
      const message = error.message
      browserErrors.push(message)
      if (isWorkspaceConsoleFailure(message)) {
        consoleFailures.push(message)
      }
    })
  })

  afterAll(async () => {
    await context?.close().catch(() => {})
    await browser?.close().catch(() => {})
    await dev?.stop().catch(() => {})
    cleanupFullstackDevEnvironment(env)
  })

  it("opens a browser workspace path, edits a file, and reflects Git changes through the server", async () => {
    const testEnv = requireEnv()
    browserErrors = []
    consoleFailures = []
    const runId = `workspace-${Date.now()}`
    const updatedContent = `Initial workspace note\n\nEdited by ${runId}\n`

    const workspaceStateRequest = page.waitForRequest(
      (request) =>
        request.method() === "GET" &&
        new URL(request.url()).pathname === "/v1/workspace",
      { timeout: 60_000 },
    )
    await page.goto(`${testEnv.uiBaseUrl}/workspace`, {
      waitUntil: "domcontentloaded",
      timeout: 60_000,
    })
    const workspaceApiBaseUrl = new URL((await workspaceStateRequest).url()).origin

    const openedWorkspace = await requestJson<Schema["WorkspaceStateResponse"]>(
      workspaceApiBaseUrl,
      "/v1/workspace/open",
      {
        json: { rootPath: workspaceRoot } satisfies Schema["WorkspaceOpenCommand"],
        method: "POST",
      },
    )
    expect(openedWorkspace.current?.rootPath.endsWith(workspaceRoot)).toBe(true)
    const workspaceStateAfterOpen = await requestJson<Schema["WorkspaceStateResponse"]>(
      workspaceApiBaseUrl,
      "/v1/workspace",
    )
    expect(workspaceStateAfterOpen.current?.rootPath.endsWith(workspaceRoot)).toBe(true)
    await page.reload({ waitUntil: "domcontentloaded", timeout: 60_000 })
    const workspaceStateAfterReload = await requestJson<Schema["WorkspaceStateResponse"]>(
      workspaceApiBaseUrl,
      "/v1/workspace",
    )
    expect(workspaceStateAfterReload.current?.rootPath.endsWith(workspaceRoot)).toBe(true)

    await page.getByTestId("workspace-file-tree").waitFor({ state: "visible", timeout: 60_000 })
    await page.getByTestId("workspace-directory-src").waitFor({ state: "visible", timeout: 60_000 })
    await page.getByTestId("workspace-directory-src").click()
    await page.getByTestId("workspace-file-src-note-txt").waitFor({ state: "visible", timeout: 60_000 })
    await page.getByTestId("workspace-file-src-note-txt").click()

    await page.getByTestId("workspace-browser-editor").waitFor({ state: "visible", timeout: 60_000 })
    const editor = page.getByTestId("workspace-editor-monaco")
    const monacoEditor = editor.locator(".monaco-editor")
    await monacoEditor.waitFor({ state: "visible", timeout: 60_000 }).catch(async (error: unknown) => {
      const editorHtml = await editor.evaluate((element) => element.outerHTML).catch(() => "<unavailable>")
      throw new Error(
        `workspace Monaco editor did not render. Browser errors: ${browserErrors.join(" | ") || "none"}. Editor HTML: ${editorHtml}. Cause: ${error instanceof Error ? error.message : String(error)}`,
      )
    })
    await eventually("workspace Monaco editor renders selected file", async () =>
      (await readMonacoEditorText(editor)).includes("Initial workspace note") ? true : null
    )

    await focusMonacoEditor(monacoEditor)
    await page.keyboard.press(process.platform === "darwin" ? "Meta+A" : "Control+A")
    await page.keyboard.type(updatedContent)
    await eventually("workspace Monaco editor accepts edits", async () =>
      (await readMonacoEditorText(editor)).includes(runId) ? true : null
    )

    await page.getByTestId("workspace-file-src-second-txt").click()
    await page.getByTestId("workspace-confirm-dialog").waitFor({ state: "visible", timeout: 60_000 })
    await page.getByTestId("workspace-confirm-cancel").click()
    await eventually("workspace dirty guard keeps unsaved editor content", async () =>
      (await readMonacoEditorText(editor)).includes(runId) ? true : null
    )

    await page.getByTestId("workspace-file-src-second-txt").click()
    await page.getByTestId("workspace-confirm-dialog").waitFor({ state: "visible", timeout: 60_000 })
    await page.getByTestId("workspace-confirm-accept").click()
    await eventually("workspace dirty guard can discard and switch files", async () =>
      (await readMonacoEditorText(editor)).includes("Second workspace note") ? true : null
    )

    await page.getByTestId("workspace-file-src-note-txt").click()
    await eventually("workspace note reopens after dirty guard discard", async () =>
      (await readMonacoEditorText(editor)).includes("Initial workspace note") ? true : null
    )
    await focusMonacoEditor(monacoEditor)
    await page.keyboard.press(process.platform === "darwin" ? "Meta+A" : "Control+A")
    await page.keyboard.type(updatedContent)
    await eventually("workspace Monaco editor accepts final persisted edits", async () =>
      (await readMonacoEditorText(editor)).includes(runId) ? true : null
    )
    await page.getByTestId("workspace-save-button").click()

    await eventually("workspace file persisted to disk", () =>
      readFileSync(notePath, "utf8") === updatedContent
    )

    const serverFile = await requestJson<Schema["WorkspaceFileContent"]>(
      workspaceApiBaseUrl,
      `/v1/workspace/files?relativePath=${encodeURIComponent("src/note.txt")}`
    )
    expect(serverFile.content).toBe(updatedContent)

    const status = await eventually("workspace Git status reports modified note", async () => {
      const nextStatus = await requestJson<Schema["WorkspaceGitStatusView"]>(
        workspaceApiBaseUrl,
        "/v1/workspace/git/status"
      )
      return nextStatus.entries.some(
        (entry) => entry.path === "src/note.txt" && entry.status === "modified" && !entry.staged
      )
        ? nextStatus
        : null
    })
    expect(status.isRepository).toBe(true)

    const diff = await requestJson<Schema["WorkspaceGitDiffView"]>(
      workspaceApiBaseUrl,
      "/v1/workspace/git/diff",
      {
        json: { path: "src/note.txt", staged: false } satisfies Schema["WorkspaceGitDiffCommand"],
        method: "POST",
      }
    )
    expect(diff.diff).toContain(runId)

    await page.getByTestId("workspace-console-toggle").click()
    await page.getByTestId("workspace-console-panel").waitFor({ state: "visible", timeout: 60_000 })
    const terminal = page.getByTestId("workspace-terminal")
    await terminal.locator(".xterm").waitFor({ state: "visible", timeout: 60_000 })
    await terminal.locator(".xterm-helper-textarea").waitFor({ state: "attached", timeout: 60_000 })
    await eventually(
      "workspace terminal websocket opens",
      async () => ((await terminal.getAttribute("data-connection-state")) === "open" ? true : null),
      60_000,
      500,
    )
    await terminal.locator(".xterm").click()
    await terminal.locator(".xterm-helper-textarea").evaluate((element) => {
      ;(element as HTMLElement).focus()
    })
    const marker = `workspace-terminal-${Date.now()}`
    const command = process.platform === "win32"
      ? `Write-Output ${marker}`
      : `printf '${marker}\\n'`
    await page.keyboard.type(command)
    await page.keyboard.press("Enter")
    await eventually("workspace terminal prints command output", async () => {
      const terminalText = await terminal.locator(".xterm-rows").textContent()
      return terminalText?.includes(marker) ? true : null
    }, 60_000, 500)
    expect(consoleFailures).toEqual([])
  })
})

function prepareGitWorkspace(root: string): void {
  mkdirSync(join(root, "src"), { recursive: true })
  writeFileSync(join(root, ".gitignore"), ".slab/\n", "utf8")
  writeFileSync(join(root, "README.md"), "# Browser Workspace\n", "utf8")
  writeFileSync(join(root, "src", "note.txt"), "Initial workspace note\n", "utf8")
  writeFileSync(join(root, "src", "second.txt"), "Second workspace note\n", "utf8")

  execFileSync("git", ["init"], { cwd: root, stdio: "pipe" })
  execFileSync("git", ["config", "user.email", "slab-e2e@example.local"], {
    cwd: root,
    stdio: "pipe",
  })
  execFileSync("git", ["config", "user.name", "Slab E2E"], { cwd: root, stdio: "pipe" })
  execFileSync("git", ["add", "."], { cwd: root, stdio: "pipe" })
  execFileSync("git", ["commit", "-m", "Initial workspace"], { cwd: root, stdio: "pipe" })
}

function requireEnv(): FullstackDevEnvironment {
  if (!env) {
    throw new Error("Fullstack dev environment was not initialized.")
  }

  return env
}

async function readMonacoEditorText(editor: Locator): Promise<string> {
  const text = await editor
    .locator(".view-line")
    .evaluateAll((lines) => lines.map((line) => line.textContent ?? "").join("\n"))
  return text.replace(/\u00a0/g, " ")
}

async function focusMonacoEditor(editor: Locator): Promise<void> {
  await editor.locator(".view-lines").click({ position: { x: 8, y: 8 } })
}

function isWorkspaceConsoleFailure(message: string): boolean {
  return /ipc\.localhost|workspace terminal|failed to start workspace terminal|FileOperationError|Unable to resolve nonexistent file/i.test(message)
}
