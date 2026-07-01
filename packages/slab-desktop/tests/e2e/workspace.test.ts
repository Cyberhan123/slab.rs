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

type WorkspaceEditorDebugState = {
  activeRelativePath: string | null
  openFiles: Array<{ name: string; relativePath: string }>
  tabCount: number
}

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
    page.on("websocket", (socket) => {
      if (socket.url().includes("/v1/workspace/lsp/")) {
        lspRequests.push(socket.url())
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

    await openWorkspaceFromUi(testEnv, page, workspaceRoot)

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

  it("keeps VS Code syntax, LSP navigation, multiple tabs, and diff rendering working", async () => {
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

    await requestJson<Schema["WorkspaceStateResponse"]>(testEnv.serverBaseUrl, "/v1/workspace/close", {
      method: "POST",
    })
    await page.goto(`${testEnv.uiBaseUrl}/workspace`, {
      waitUntil: "domcontentloaded",
      timeout: 60_000,
    })
    await openWorkspaceFromUi(testEnv, page, workspaceRoot)

    const explorer = page.getByTestId("workspace-vscode-explorer")
    await explorer.waitFor({ state: "visible", timeout: 60_000 })
    await expandWorkspaceExplorerRoot(explorer, workspaceRoot.split(/[\\/]/).findLast(Boolean) ?? "Workspace", "src")

    const editor = page.getByTestId("workspace-vscode-editor")
    const monacoEditor = editor.locator(".monaco-editor")
    await openExplorerFile(explorer, project.tsMainFileRelativePath)
    await monacoEditor.waitFor({ state: "visible", timeout: 60_000 })
    await eventually("workspace Monaco editor renders TypeScript file", async () =>
      (await readMonacoEditorText(editor)).includes("import { addNumbers, answer }") ? true : null
    )
    await expectTypeScriptTokenization(editor)
    await waitForWorkspaceLspSession(page, project.tsMainFileRelativePath, "typescript", 20_000).catch(async (error: unknown) => {
      const lspClient = await readWorkspaceLspClientDebug(page)
      const lspSession = await readWorkspaceLspSessionDebug(page)
      const runtime = await workspaceRuntimeDiagnostics(editor)
      throw new Error(
        `workspace TypeScript LSP session did not start. LSP session: ${JSON.stringify(lspSession)}. LSP client: ${JSON.stringify(lspClient)}. Runtime: ${runtime}. Dev logs: ${JSON.stringify(dev?.logs.slice(-80) ?? [])}. Cause: ${error instanceof Error ? error.message : String(error)}`,
      )
    })

    await focusMonacoEditor(monacoEditor)
    await placeCursorAfterText(page, editor, "addNumbers", 1)
    await page.keyboard.press(process.platform === "darwin" ? "Meta+Space" : "Control+Space")
    await eventually("workspace TypeScript suggestions render", async () =>
      (await page.locator(".suggest-widget").count()) > 0 ? true : null,
      20_000,
      500,
    ).catch(async (error: unknown) => {
      const lspClient = await readWorkspaceLspClientDebug(page)
      const lspSession = await readWorkspaceLspSessionDebug(page)
      throw new Error(
        `workspace TypeScript suggestions did not render. LSP session: ${JSON.stringify(lspSession)}. LSP client: ${JSON.stringify(lspClient)}. Dev logs: ${JSON.stringify(dev?.logs.slice(-80) ?? [])}. Cause: ${error instanceof Error ? error.message : String(error)}`,
      )
    })
    await page.keyboard.press("Escape")
    await hoverText(editor, "addNumbers", 1)
    await eventually("workspace TypeScript hover renders", async () =>
      (await page.locator(".monaco-hover").count()) > 0 ? true : null,
      20_000,
      500,
    ).catch(async (error: unknown) => {
      const lspClient = await readWorkspaceLspClientDebug(page)
      const lspSession = await readWorkspaceLspSessionDebug(page)
      throw new Error(
        `workspace TypeScript hover did not render. LSP session: ${JSON.stringify(lspSession)}. LSP client: ${JSON.stringify(lspClient)}. Dev logs: ${JSON.stringify(dev?.logs.slice(-80) ?? [])}. Cause: ${error instanceof Error ? error.message : String(error)}`,
      )
    })

    await focusMonacoEditor(monacoEditor)
    await placeCursorOnText(page, editor, "addNumbers", 1)
    await runWorkspaceVscodeCommand(page, "editor.action.revealDefinition")
    await eventually("workspace go-to-definition opens math.ts", async () => {
      const state = await readWorkspaceEditorDebugState(page)
      return state?.activeRelativePath === project.mathFileRelativePath ? state : null
    }, 60_000, 500).catch(async (error: unknown) => {
      const definitionTarget = await readWorkspaceDefinitionTarget(page)
      const debugState = await readWorkspaceEditorDebugState(page)
      const lspClient = await readWorkspaceLspClientDebug(page)
      throw new Error(
        `workspace go-to-definition did not open math.ts. Definition target: ${JSON.stringify(definitionTarget)}. Editor state: ${JSON.stringify(debugState)}. LSP client: ${JSON.stringify(lspClient)}. Dev logs: ${JSON.stringify(dev?.logs.slice(-80) ?? [])}. Cause: ${error instanceof Error ? error.message : String(error)}`,
      )
    })
    await eventually("workspace definition target is rendered", async () =>
      (await readMonacoEditorText(editor)).includes("export function addNumbers") ? true : null
    )

    await openExplorerFile(explorer, project.tsMainFileRelativePath)
    await openExplorerFile(explorer, project.mathFileRelativePath)
    await openExplorerFile(explorer, project.noteFileRelativePath)
    await eventually("workspace VS Code keeps three tabs", async () => {
      const state = await readWorkspaceEditorDebugState(page)
      return state && state.openFiles.length >= 3 ? state : null
    }).catch(async (error: unknown) => {
      const debugState = await readWorkspaceEditorDebugState(page)
      const tabs = await editor.locator("[data-resource-name]").evaluateAll((elements) =>
        elements.map((element) => ({
          text: element.textContent,
          resourceName: element.getAttribute("data-resource-name"),
        })),
      )
      throw new Error(
        `workspace VS Code did not keep three tabs. Editor state: ${JSON.stringify(debugState)}. DOM tabs: ${JSON.stringify(tabs)}. Cause: ${error instanceof Error ? error.message : String(error)}`,
      )
    })
    await clickWorkspaceTab(editor, "main.ts")
    const tabsAfterSwitch = await eventually("workspace tab switch preserves all tabs", async () => {
      const state = await readWorkspaceEditorDebugState(page)
      return state?.activeRelativePath === project.tsMainFileRelativePath && state.openFiles.length >= 3
        ? state
        : null
    })
    expect(tabsAfterSwitch.openFiles.map((file) => file.relativePath)).toContain(project.mathFileRelativePath)
    expect(tabsAfterSwitch.openFiles.map((file) => file.relativePath)).toContain(project.noteFileRelativePath)

    await page.getByRole("button", { name: /git/i }).click()
    const gitPanel = page.getByTestId("workspace-active-screen")
    await eventually("workspace Git status shows modified fixture", async () =>
      (await gitPanel.textContent())?.includes(project.diffModifiedPath) ? true : null
    )
    await page
      .getByTestId("workspace-git-diff-entry")
      .filter({ hasText: project.diffModifiedPath })
      .click()
    await editor.locator(".monaco-diff-editor").waitFor({ state: "visible", timeout: 60_000 })
    await eventually("workspace VS Code diff renders original and modified text", async () => {
      const text = await readMonacoEditorText(editor)
      return text.includes(project.diffModifiedOldContent.trim())
        && text.includes(project.diffModifiedNewContent.trim())
        ? true
        : null
    }, 60_000, 500)

    const diff = await requestJson<Schema["WorkspaceGitDiffView"]>(
      testEnv.serverBaseUrl,
      "/v1/workspace/git/diff",
      {
        json: { path: project.diffUntrackedPath, staged: false } satisfies Schema["WorkspaceGitDiffCommand"],
        method: "POST",
      },
    )
    expect(diff.originalContent).toBe("")
    expect(diff.modifiedContent).toBe(project.diffUntrackedContent)
    expect(lspRequests.some((request) => request.includes("/v1/workspace/lsp/typescript"))).toBe(true)
    expect(workspaceFailures).toEqual([])
  })

  it("keeps the workspace stable when JSON LSP initialization closes early", async () => {
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

    await requestJson<Schema["WorkspaceStateResponse"]>(testEnv.serverBaseUrl, "/v1/workspace/close", {
      method: "POST",
    })
    await page.goto(`${testEnv.uiBaseUrl}/workspace`, {
      waitUntil: "domcontentloaded",
      timeout: 60_000,
    })
    await openWorkspaceFromUi(testEnv, page, workspaceRoot)

    const explorer = page.getByTestId("workspace-vscode-explorer")
    await page.getByRole("button", { name: /files/i }).click()
    await explorer.waitFor({ state: "visible", timeout: 60_000 })
    await expandWorkspaceExplorerRoot(explorer, workspaceRoot.split(/[\\/]/).findLast(Boolean) ?? "Workspace", "config")
    await installClosingJsonLspWebSocket(page)
    await clickExplorerPath(explorer, project.jsonConfigPathSegments)

    const editor = page.getByTestId("workspace-vscode-editor")
    const monacoEditor = editor.locator(".monaco-editor")
    await monacoEditor.waitFor({ state: "visible", timeout: 60_000 })
    await eventually("workspace JSON file remains visible after LSP close", async () =>
      (await readMonacoEditorText(editor)).includes('"app": "workspace"') ? true : null
    )
    await expectWorkspaceWorkerLabelLoads(page, "OutputLinkDetectionWorker")
    await eventually("workspace JSON LSP close is recorded without page teardown", async () => {
      const lspClient = await readWorkspaceLspClientDebug(page) as {
        connections?: Array<{ event?: string; language?: string }>
      } | null
      return lspClient?.connections?.some((connection) =>
        (connection.event === "closed" || connection.event === "failed") && connection.language === "json"
      )
        ? lspClient
        : null
    }, 30_000, 500)
    await page.waitForTimeout(1_000)
    const jsonLspRequestCount = lspRequests.filter((request) => request.includes("/v1/workspace/lsp/json")).length
    await page.waitForTimeout(1_000)

    await page.getByTestId("workspace-active-screen").waitFor({ state: "visible", timeout: 10_000 })
    expect(await readMonacoEditorText(editor)).toContain('"enabled": true')
    expect(lspRequests.filter((request) => request.includes("/v1/workspace/lsp/json")).length).toBe(jsonLspRequestCount)
    expect(browserErrors.filter(isUnhandledWorkspaceLspFailure)).toEqual([])
    expect(workspaceFailures).toEqual([])
  })
})

function requireEnv(): FullstackDevEnvironment {
  if (!env) {
    throw new Error("Fullstack dev environment was not initialized.")
  }

  return env
}

async function openWorkspaceFromUi(
  testEnv: FullstackDevEnvironment,
  page: Page,
  root: string,
) {
  await page.goto(`${testEnv.uiBaseUrl}/workspace`, {
    waitUntil: "domcontentloaded",
    timeout: 60_000,
  })
  await page.getByTestId("workspace-open-screen").waitFor({ state: "visible", timeout: 60_000 })
  expect(await page.getByRole("button", { name: "Open folder" }).count()).toBe(1)

  await page.getByTestId("workspace-path-input").fill(root)
  await page.getByTestId("workspace-open-path-button").click()
  await page.getByTestId("workspace-active-screen").waitFor({ state: "visible", timeout: 60_000 })
  await eventually("workspace active screen shows opened root", async () =>
    (await page.getByTestId("workspace-active-screen").textContent())?.includes(root) ? true : null
  )
}

async function openExplorerFile(explorer: ReturnType<Page["getByTestId"]>, relativePath: string) {
  await clickExplorerPath(explorer, relativePath.split("/"))
}

async function expectTypeScriptTokenization(editor: ReturnType<Page["getByTestId"]>) {
  await eventually("workspace TypeScript syntax tokens are colored", async () => {
    const tokenSummary = await editor.locator(".view-line").evaluateAll((lines) =>
      lines.flatMap((line) =>
        Array.from(line.querySelectorAll("span")).map((span) => ({
          className: span.className,
          color: getComputedStyle(span).color,
          text: span.textContent ?? "",
        })),
      ),
    )
    const importToken = tokenSummary.find((token) => token.text.includes("import"))
    const distinctColors = new Set(tokenSummary.map((token) => token.color).filter(Boolean))
    return importToken && distinctColors.size > 1 ? true : null
  }, 60_000, 500)
}

async function placeCursorAfterText(
  page: Page,
  editor: ReturnType<Page["getByTestId"]>,
  text: string,
  occurrence = 0,
) {
  const position = await textPositionInEditor(editor, text, occurrence, true)

  if (!position) {
    throw new Error(`Unable to find text '${text}' in Monaco editor.`)
  }

  await page.mouse.click(position.x, position.y)
}

async function placeCursorOnText(
  page: Page,
  editor: ReturnType<Page["getByTestId"]>,
  text: string,
  occurrence = 0,
) {
  const position = await textPositionInEditor(editor, text, occurrence, false)

  if (!position) {
    throw new Error(`Unable to find text '${text}' in Monaco editor.`)
  }

  await page.mouse.click(position.x, position.y)
}

async function hoverText(
  editor: ReturnType<Page["getByTestId"]>,
  text: string,
  occurrence = 0,
) {
  const position = await textPositionInEditor(editor, text, occurrence, false)
  if (!position) {
    throw new Error(`Unable to find text '${text}' in Monaco editor.`)
  }
  await editor.page().mouse.move(position.x, position.y)
}

async function textPositionInEditor(
  editor: ReturnType<Page["getByTestId"]>,
  text: string,
  occurrence: number,
  afterText: boolean,
) {
  return editor.locator(".view-lines").evaluate(
    (viewLines, payload) => {
      // eslint-disable-next-line unicorn/consistent-function-scoping -- Runs inside the browser context.
      const rangeRectForTextOffset = (element: Element, offset: number) => {
        const walker = document.createTreeWalker(element, NodeFilter.SHOW_TEXT)
        let remaining = offset
        let textNode = walker.nextNode()
        while (textNode) {
          const length = textNode.textContent?.length ?? 0
          if (remaining <= length) {
            const range = document.createRange()
            range.setStart(textNode, Math.max(0, remaining))
            range.setEnd(textNode, Math.max(0, remaining))
            const rect = range.getBoundingClientRect()
            range.detach()
            return rect
          }
          remaining -= length
          textNode = walker.nextNode()
        }
        return null
      }

      let seen = 0
      for (const line of Array.from(viewLines.querySelectorAll(".view-line"))) {
        const content = line.textContent ?? ""
        const index = content.indexOf(payload.text)
        if (index < 0) {
          continue
        }
        if (seen < payload.occurrence) {
          seen += 1
          continue
        }

        const lineBox = line.getBoundingClientRect()
        for (const span of Array.from(line.querySelectorAll("span"))) {
          const spanText = span.textContent ?? ""
          const spanIndex = spanText.indexOf(payload.text)
          if (spanIndex < 0) {
            continue
          }

          const targetOffset = spanIndex
            + (payload.afterText ? payload.text.length : Math.max(1, Math.floor(payload.text.length / 2)))
          const rect = rangeRectForTextOffset(span, targetOffset)
          const fallbackBox = span.getBoundingClientRect()
          const x = rect?.left || fallbackBox.left + fallbackBox.width / 2
          return { x, y: lineBox.top + lineBox.height / 2 }
        }

        return { x: lineBox.left + 8, y: lineBox.top + lineBox.height / 2 }
      }
      return null
    },
    { afterText, occurrence, text },
  )
}

async function readWorkspaceEditorDebugState(page: Page): Promise<WorkspaceEditorDebugState | null> {
  return page.evaluate(() => {
    const target = window as typeof window & { __SLAB_WORKSPACE_EDITOR_STATE__?: WorkspaceEditorDebugState }
    return target["__SLAB_WORKSPACE_EDITOR_STATE__"] ?? null
  })
}

async function readWorkspaceDefinitionTarget(page: Page): Promise<unknown> {
  return page.evaluate(() => {
    const target = window as typeof window & { __SLAB_WORKSPACE_DEFINITION_TARGET__?: unknown }
    return target["__SLAB_WORKSPACE_DEFINITION_TARGET__"] ?? null
  })
}

async function readWorkspaceLspClientDebug(page: Page): Promise<unknown> {
  return page.evaluate(() => {
    const target = window as typeof window & { __SLAB_WORKSPACE_LSP_CLIENT__?: unknown }
    return target["__SLAB_WORKSPACE_LSP_CLIENT__"] ?? null
  })
}

async function readWorkspaceLspSessionDebug(page: Page): Promise<unknown> {
  return page.evaluate(() => {
    const target = window as typeof window & { __SLAB_WORKSPACE_LSP_SESSION__?: unknown }
    return target["__SLAB_WORKSPACE_LSP_SESSION__"] ?? null
  })
}

async function runWorkspaceVscodeCommand(page: Page, commandId: string) {
  await page.evaluate(async (nextCommandId) => {
    const workspaceEditor = await (0, eval)(
      'import("/src/pages/workspace/lib/workspace-editor.ts")',
    ) as typeof import("../../src/pages/workspace/lib/workspace-editor")
    const lspState = (window as typeof window & {
      __SLAB_WORKSPACE_LSP_SESSION__?: { workspaceRoot?: string }
    })["__SLAB_WORKSPACE_LSP_SESSION__"]
    await workspaceEditor.runWorkspaceVscodeCommand(nextCommandId, lspState?.workspaceRoot)
  }, commandId)
}

async function clickWorkspaceTab(editor: ReturnType<Page["getByTestId"]>, label: string) {
  const tab = editor.locator("[data-resource-name]").filter({ hasText: label }).first()
  if (await tab.isVisible().catch(() => false)) {
    await tab.click()
    return
  }
  await editor.getByText(label, { exact: true }).first().click()
}

async function waitForWorkspaceLspSession(
  page: Page,
  relativePath: string,
  language: string,
  timeoutMs = 60_000,
) {
  await eventually(`workspace ${language} LSP session starts`, async () => {
    const session = await page.evaluate(() =>
      (window as typeof window & {
        __SLAB_WORKSPACE_LSP_SESSION__?: {
          activeRelativePath?: string
          language?: string
          ready?: boolean
        }
      })["__SLAB_WORKSPACE_LSP_SESSION__"] ?? null,
    )
    return session?.ready && session.activeRelativePath === relativePath && session.language === language
      ? session
      : null
  }, timeoutMs, 500)
}

function isUnhandledWorkspaceLspFailure(message: string) {
  return /Client is not running and can't be stopped|Pending response rejected since connection got disposed|MCP\]\[BRIDGE\]\[UNHANDLED_REJECTION|Uncaught \(in promise\).*ResponseError|Uncaught \(in promise\).*Client is not running/i
    .test(message)
}

async function expectWorkspaceWorkerLabelLoads(page: Page, label: string) {
  await page.evaluate(async (workerLabel) => {
    const environment = window.MonacoEnvironment as {
      getWorkerOptions?: (moduleId: string, label: string) => WorkerOptions | undefined
      getWorkerUrl?: (moduleId: string, label: string) => string | undefined
    } | undefined
    const workerUrl = environment?.getWorkerUrl?.("workerMain.js", workerLabel)
    if (!workerUrl) {
      throw new Error(`Missing worker URL for ${workerLabel}.`)
    }
    if (workerUrl === window.location.href || workerUrl.includes(`/workspace#${workerLabel}`)) {
      throw new Error(`Worker URL for ${workerLabel} resolves to the workspace page: ${workerUrl}`)
    }

    await new Promise<void>((resolve, reject) => {
      const bootstrapUrl = URL.createObjectURL(new Blob([
        `await import(${JSON.stringify(workerUrl)}); postMessage("ready");`,
      ], { type: "application/javascript" }))
      const worker = new Worker(bootstrapUrl, {
        name: workerLabel,
        type: environment?.getWorkerOptions?.("workerMain.js", workerLabel)?.type ?? "module",
      })
      const timeout = window.setTimeout(() => {
        worker.terminate()
        URL.revokeObjectURL(bootstrapUrl)
        reject(new Error(`Timed out loading worker ${workerLabel} from ${workerUrl}.`))
      }, 10_000)
      worker.addEventListener("message", () => {
        window.clearTimeout(timeout)
        worker.terminate()
        URL.revokeObjectURL(bootstrapUrl)
        resolve()
      }, { once: true })
      worker.addEventListener("error", (error) => {
        window.clearTimeout(timeout)
        worker.terminate()
        URL.revokeObjectURL(bootstrapUrl)
        reject(new Error(`Failed to load worker ${workerLabel} from ${workerUrl}: ${error.message}`))
      }, { once: true })
    })
  }, label)
}

async function installClosingJsonLspWebSocket(page: Page) {
  await page.evaluate(() => {
    const NativeWebSocket = window.WebSocket
    let jsonLspCloseInjected = false
    class ClosingJsonLspWebSocket extends NativeWebSocket {
      constructor(url: string | URL, protocols?: string | string[]) {
        if (protocols === undefined) {
          super(url)
        } else {
          super(url, protocols)
        }
        if (String(url).includes("/v1/workspace/lsp/json") && !jsonLspCloseInjected) {
          jsonLspCloseInjected = true
          this.addEventListener("open", () => {
            this.close(1000, "test json lsp init close")
          }, { once: true })
        }
      }
    }
    Object.defineProperty(ClosingJsonLspWebSocket, "CONNECTING", { value: NativeWebSocket.CONNECTING })
    Object.defineProperty(ClosingJsonLspWebSocket, "OPEN", { value: NativeWebSocket.OPEN })
    Object.defineProperty(ClosingJsonLspWebSocket, "CLOSING", { value: NativeWebSocket.CLOSING })
    Object.defineProperty(ClosingJsonLspWebSocket, "CLOSED", { value: NativeWebSocket.CLOSED })
    window.WebSocket = ClosingJsonLspWebSocket
  })
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
  const lspState = await editor.page().evaluate(() => {
    const target = window as typeof window & {
      __SLAB_WORKSPACE_LSP_CONTEXT__?: unknown
      __SLAB_WORKSPACE_LSP_DIRECTORIES__?: unknown[]
      __SLAB_WORKSPACE_LSP_SESSION__?: unknown
      __SLAB_WORKSPACE_LSP_STAGE__?: string
      __SLAB_WORKSPACE_LSP_STATS__?: unknown[]
    }
    return {
      context: target["__SLAB_WORKSPACE_LSP_CONTEXT__"] ?? null,
      directories: target["__SLAB_WORKSPACE_LSP_DIRECTORIES__"] ?? [],
      stage: target["__SLAB_WORKSPACE_LSP_STAGE__"] ?? null,
      stats: target["__SLAB_WORKSPACE_LSP_STATS__"] ?? [],
      session: target["__SLAB_WORKSPACE_LSP_SESSION__"] ?? null,
    }
  }).catch(() => "<unavailable>")

  return JSON.stringify({
    editorSummary,
    lspState,
    mountError,
    mountStage,
    mountState,
  })
}
