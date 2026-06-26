import { readFileSync } from "node:fs"
import { join } from "node:path"

import { chromium, type Browser, type BrowserContext, type Page } from "playwright"
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
import {
  clickExplorerPath,
  expandWorkspaceExplorerRoot,
  focusMonacoEditor,
  isWorkspaceRuntimeFailure,
  prepareWorkspaceProject,
  readMonacoEditorText,
  workspaceTerminalReadSentinelCommand,
  type WorkspaceProjectFixture,
} from "./support/workspace-project"

type Schema = components["schemas"]

let env: FullstackDevEnvironment | undefined

describe.sequential("workspace e2e", () => {
  let browser: Browser | undefined
  let context: BrowserContext | undefined
  let dev: ManagedDevProcess | undefined
  let page: Page
  let browserErrors: string[] = []
  let workspaceFailures: string[] = []
  let directoryRequests: string[] = []
  let fileRequests: string[] = []
  let lspRequests: string[] = []
  let statRequests: string[] = []
  let failedRequests: string[] = []
  let fileResponses: string[] = []
  let notFoundResponses: string[] = []
  let workspaceRoot: string
  let project: WorkspaceProjectFixture

  beforeAll(async () => {
    env = await createFullstackDevEnvironment()
    workspaceRoot = join(env.rootDir, "browser-workspace")
    project = prepareWorkspaceProject(workspaceRoot, "Browser Workspace")

    dev = await startFullstackDev(env)
    await completeSetup(env.serverBaseUrl)
    await requestJson<Schema["WorkspaceStateResponse"]>(env.serverBaseUrl, "/v1/workspace/close", {
      method: "POST",
    })

    browser = await chromium.launch({ headless: true })
    context = await browser.newContext({
      viewport: { width: 1440, height: 960 },
    })
    await context.addInitScript((apiBaseUrl) => {
      // eslint-disable-next-line no-underscore-dangle -- runtime contract read by @slab/api (config.ts) and slab-plugin-sdk
      ;(window as typeof window & { __SLAB_API_BASE_URL__?: string }).__SLAB_API_BASE_URL__ = apiBaseUrl
      window.localStorage.setItem("slab.ui.language", "en-US")
    }, env.uiBaseUrl)
    page = await context.newPage()
    page.on("request", (request) => {
      if (request.url().includes("/v1/workspace/directory")) {
        directoryRequests.push(request.url())
      }
      if (request.url().includes("/v1/workspace/files")) {
        fileRequests.push(request.url())
      }
      if (request.url().includes("/v1/workspace/lsp/")) {
        lspRequests.push(request.url())
      }
      if (request.url().includes("/v1/workspace/path/stat")) {
        statRequests.push(request.url())
      }
    })
    page.on("requestfailed", (request) => {
      failedRequests.push(`${request.method()} ${request.url()} ${request.failure()?.errorText ?? ""}`)
    })
    page.on("response", (response) => {
      if (response.url().includes("/v1/workspace/files")) {
        fileResponses.push(`${response.status()} ${response.url()}`)
      }
      if (response.status() === 404) {
        notFoundResponses.push(`${response.request().method()} ${response.url()}`)
      }
    })
    page.on("console", (message) => {
      if (message.type() !== "error") {
        return
      }
      const text = message.text()
      browserErrors.push(text)
      if (isWorkspaceRuntimeFailure(text)) {
        workspaceFailures.push(text)
      }
    })
    page.on("pageerror", (error) => {
      const message = error.message
      browserErrors.push(message)
      if (isWorkspaceRuntimeFailure(message)) {
        workspaceFailures.push(message)
      }
    })
  })

  afterAll(async () => {
    await context?.close().catch(() => {})
    await browser?.close().catch(() => {})
    await dev?.stop().catch(() => {})
    cleanupFullstackDevEnvironment(env)
  })

  it("opens a workspace from the UI, expands deep files, edits, saves, runs terminal, and reopens from recents", async () => {
    const testEnv = requireEnv()
    browserErrors = []
    workspaceFailures = []
    directoryRequests = []
    fileRequests = []
    lspRequests = []
    statRequests = []
    failedRequests = []
    fileResponses = []
    notFoundResponses = []
    const runId = `workspace-${Date.now()}`
    const updatedContent = `Deep workspace sentinel\nEdited by ${runId}\n`

    await page.goto(`${testEnv.uiBaseUrl}/workspace`, {
      waitUntil: "domcontentloaded",
      timeout: 60_000,
    })
    await page.getByTestId("workspace-open-screen").waitFor({ state: "visible", timeout: 60_000 })
    expect(await page.getByRole("button", { name: "Open folder" }).count()).toBe(1)

    await page.getByTestId("workspace-path-input").fill(workspaceRoot)
    await page.getByTestId("workspace-open-path-button").click()
    await page.getByTestId("workspace-active-screen").waitFor({ state: "visible", timeout: 60_000 })
    await eventually("workspace active screen shows opened root", async () =>
      (await page.getByTestId("workspace-active-screen").textContent())?.includes(workspaceRoot) ? true : null
    )

    const workspaceStateAfterOpen = await requestJson<Schema["WorkspaceStateResponse"]>(
      testEnv.serverBaseUrl,
      "/v1/workspace",
    )
    expect(workspaceStateAfterOpen.current?.rootPath.endsWith(workspaceRoot)).toBe(true)

    const explorer = page.getByTestId("workspace-vscode-explorer")
    await explorer.waitFor({ state: "visible", timeout: 60_000 })
    await expandWorkspaceExplorerRoot(explorer, workspaceRoot.split(/[\\/]/).findLast(Boolean) ?? "Workspace", "src")
    await clickExplorerPath(explorer, project.deepPathSegments)

    const editor = page.getByTestId("workspace-vscode-editor")
    const monacoEditor = editor.locator(".monaco-editor")
    await monacoEditor.waitFor({ state: "visible", timeout: 60_000 }).catch(async (error: unknown) => {
      const editorHtml = await editor.evaluate((element) => element.outerHTML).catch(() => "<unavailable>")
      const diagnostics = await workspaceRuntimeDiagnostics(editor)
      throw new Error(
        `workspace Monaco editor did not render. Browser errors: ${browserErrors.join(" | ") || "none"}. 404s: ${notFoundResponses.join(" | ") || "none"}. Failed requests: ${failedRequests.join(" | ") || "none"}. File requests: ${fileRequests.join(" | ") || "none"}. File responses: ${fileResponses.join(" | ") || "none"}. Stat requests: ${statRequests.join(" | ") || "none"}. LSP requests: ${lspRequests.join(" | ") || "none"}. Diagnostics: ${diagnostics}. Editor HTML: ${editorHtml}. Cause: ${error instanceof Error ? error.message : String(error)}`,
      )
    })
    await eventually("workspace Monaco editor renders deep selected file", async () =>
      (await readMonacoEditorText(editor)).includes(project.deepFileContent) ? true : null
    )

    await focusMonacoEditor(monacoEditor)
    await page.keyboard.press(process.platform === "darwin" ? "Meta+A" : "Control+A")
    await page.keyboard.type(updatedContent)
    await eventually("workspace Monaco editor accepts edits", async () =>
      (await readMonacoEditorText(editor)).includes(runId) ? true : null
    )

    await focusMonacoEditor(monacoEditor)
    await page.keyboard.press(process.platform === "darwin" ? "Meta+S" : "Control+S")

    await eventually("workspace file persisted to disk", () =>
      readFileSync(project.deepFilePath, "utf8") === updatedContent
    )

    const serverFile = await requestJson<Schema["WorkspaceFileContent"]>(
      testEnv.serverBaseUrl,
      `/v1/workspace/files?relativePath=${encodeURIComponent(project.deepFileRelativePath)}`,
    )
    expect(serverFile.content).toBe(updatedContent)

    const status = await eventually("workspace Git status reports modified deep file", async () => {
      const nextStatus = await requestJson<Schema["WorkspaceGitStatusView"]>(
        testEnv.serverBaseUrl,
        "/v1/workspace/git/status",
      )
      return nextStatus.entries.some(
        (entry) => entry.path === project.deepFileRelativePath && entry.status === "modified" && !entry.staged,
      )
        ? nextStatus
        : null
    })
    expect(status.isRepository).toBe(true)

    const diff = await requestJson<Schema["WorkspaceGitDiffView"]>(
      testEnv.serverBaseUrl,
      "/v1/workspace/git/diff",
      {
        json: { path: project.deepFileRelativePath, staged: false } satisfies Schema["WorkspaceGitDiffCommand"],
        method: "POST",
      },
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
    await page.keyboard.type(workspaceTerminalReadSentinelCommand(marker, project.terminalSentinelFileName))
    await page.keyboard.press("Enter")
    await eventually("workspace terminal prints sentinel file output", async () => {
      const terminalText = await terminal.locator(".xterm-rows").textContent()
      return terminalText?.includes(marker) && terminalText.includes(project.terminalSentinelContent)
        ? true
        : null
    }, 60_000, 500)

    await page.getByTestId("workspace-console-close-button").click()
    await page.getByTestId("workspace-console-panel").waitFor({ state: "hidden", timeout: 60_000 })

    await page.getByTestId("workspace-back-button").click()
    await page.getByTestId("workspace-open-screen").waitFor({ state: "visible", timeout: 60_000 })
    expect(await page.getByRole("button", { name: "Open folder" }).count()).toBe(1)
    const recentRow = page.getByTestId("recent-workspace-row").filter({ hasText: workspaceRoot })
    await recentRow.waitFor({ state: "visible", timeout: 60_000 })
    await recentRow.getByTestId("recent-workspace-open-button").click()
    await page.getByTestId("workspace-active-screen").waitFor({ state: "visible", timeout: 60_000 })
    await eventually("workspace active screen shows reopened root", async () =>
      (await page.getByTestId("workspace-active-screen").textContent())?.includes(workspaceRoot) ? true : null
    )

    // The file tree is bulk-fetched in one deep request and served from cache, so the chatty
    // per-folder `readdir` pattern must NOT generate hundreds of /v1/workspace/directory calls.
    expect(directoryRequests.length).toBeLessThan(30)
    expect(workspaceFailures).toEqual([])
  })
})

function requireEnv(): FullstackDevEnvironment {
  if (!env) {
    throw new Error("Fullstack dev environment was not initialized.")
  }

  return env
}

async function workspaceRuntimeDiagnostics(editor: ReturnType<Page["getByTestId"]>) {
  const mountState = await editor.getAttribute("data-mount-state").catch(() => "<unavailable>")
  const mountStage = await editor.getAttribute("data-mount-stage").catch(() => "<unavailable>")
  const mountError = await editor.getAttribute("data-mount-error").catch(() => "<unavailable>")
  const editorSummary = await editor.evaluate((element) => ({
    editorInstances: element.querySelectorAll(".editor-instance").length,
    monacoEditors: element.querySelectorAll(".monaco-editor").length,
    progressBars: element.querySelectorAll('[role="progressbar"]').length,
    tabs: Array.from(element.querySelectorAll("[data-resource-name]")).map((tab) =>
      tab.getAttribute("data-resource-name"),
    ),
  })).catch(() => "<unavailable>")
  const lspState = await editor.page().evaluate(() => ({
    context: (window as typeof window & { __SLAB_WORKSPACE_LSP_CONTEXT__?: unknown })
      .__SLAB_WORKSPACE_LSP_CONTEXT__ ?? null,
    directories: (window as typeof window & { __SLAB_WORKSPACE_LSP_DIRECTORIES__?: unknown[] })
      .__SLAB_WORKSPACE_LSP_DIRECTORIES__ ?? [],
    stage: (window as typeof window & { __SLAB_WORKSPACE_LSP_STAGE__?: string }).__SLAB_WORKSPACE_LSP_STAGE__ ?? null,
    stats: (window as typeof window & { __SLAB_WORKSPACE_LSP_STATS__?: unknown[] }).__SLAB_WORKSPACE_LSP_STATS__ ?? [],
  })).catch(() => "<unavailable>")

  return JSON.stringify({
    editorSummary,
    lspState,
    mountError,
    mountStage,
    mountState,
  })
}
