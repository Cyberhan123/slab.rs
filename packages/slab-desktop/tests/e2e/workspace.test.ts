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
  })

  afterAll(async () => {
    await context?.close().catch(() => {})
    await browser?.close().catch(() => {})
    await dev?.stop().catch(() => {})
    cleanupFullstackDevEnvironment(env)
  })

  it("opens a browser workspace path, edits a file, and reflects Git changes through the server", async () => {
    const testEnv = requireEnv()
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
    await monacoEditor.waitFor({ state: "visible", timeout: 60_000 })
    await eventually("workspace Monaco editor renders selected file", async () =>
      (await readMonacoEditorText(editor)).includes("Initial workspace note") ? true : null
    )

    await monacoEditor.locator('[role="textbox"]').click()
    await page.keyboard.press(process.platform === "darwin" ? "Meta+A" : "Control+A")
    await page.keyboard.type(updatedContent)
    await eventually("workspace Monaco editor accepts edits", async () =>
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
  })
})

function prepareGitWorkspace(root: string): void {
  mkdirSync(join(root, "src"), { recursive: true })
  writeFileSync(join(root, ".gitignore"), ".slab/\n", "utf8")
  writeFileSync(join(root, "README.md"), "# Browser Workspace\n", "utf8")
  writeFileSync(join(root, "src", "note.txt"), "Initial workspace note\n", "utf8")

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
