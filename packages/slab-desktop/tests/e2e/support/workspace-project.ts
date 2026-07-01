import { execFileSync } from "node:child_process"
import { mkdirSync, writeFileSync } from "node:fs"
import { dirname, join } from "node:path"

import type { Locator } from "playwright"

export type WorkspaceProjectFixture = {
  deepFileContent: string
  deepFilePath: string
  deepFileRelativePath: string
  deepPathSegments: string[]
  diffModifiedNewContent: string
  diffModifiedOldContent: string
  diffModifiedPath: string
  diffUntrackedContent: string
  diffUntrackedPath: string
  jsonConfigContent: string
  jsonConfigPathSegments: string[]
  jsonConfigRelativePath: string
  mathFileRelativePath: string
  noteFileRelativePath: string
  terminalSentinelContent: string
  terminalSentinelFileName: string
  tsMainFileContent: string
  tsMainFileRelativePath: string
}

const deepPathSegments = ["src", "features", "deep", "level-four", "level-five"]
const deepFileName = "deep-note.txt"
const terminalSentinelFileName = "terminal-sentinel.txt"

export function prepareWorkspaceProject(root: string, title = "Workspace E2E"): WorkspaceProjectFixture {
  const deepFileRelativePath = [...deepPathSegments, deepFileName].join("/")
  const deepFilePath = join(root, ...deepPathSegments, deepFileName)
  const deepFileContent = "Deep workspace sentinel\nNested file content\n"
  const terminalSentinelContent = "terminal workspace cwd sentinel"
  const mathFileRelativePath = "src/lib/math.ts"
  const noteFileRelativePath = "src/note.txt"
  const tsMainFileRelativePath = "src/main.ts"
  const tsMainFileContent = [
    "import { addNumbers, answer } from './lib/math';",
    "",
    "export const total = addNumbers(answer, 8);",
    "",
  ].join("\n")
  const diffModifiedPath = "src/diff-target.txt"
  const diffModifiedOldContent = "Original tracked diff line\nShared line\n"
  const diffModifiedNewContent = "Modified tracked diff line\nShared line\n"
  const diffUntrackedPath = "src/untracked-diff.txt"
  const diffUntrackedContent = "Untracked workspace diff line\n"
  const jsonConfigRelativePath = "config/app.json"
  const jsonConfigContent = `${JSON.stringify({
    app: "workspace",
    enabled: true,
  }, null, 2)}\n`

  mkdirSync(dirname(deepFilePath), { recursive: true })
  mkdirSync(join(root, "config"), { recursive: true })
  mkdirSync(join(root, "src", "lib"), { recursive: true })
  writeFileSync(join(root, ".gitignore"), ".slab/\n", "utf8")
  writeFileSync(join(root, "README.md"), `# ${title}\n`, "utf8")
  writeFileSync(join(root, noteFileRelativePath), "Initial workspace note\n", "utf8")
  writeFileSync(join(root, "src", "second.txt"), "Second workspace note\n", "utf8")
  writeFileSync(join(root, jsonConfigRelativePath), jsonConfigContent, "utf8")
  writeFileSync(join(root, tsMainFileRelativePath), tsMainFileContent, "utf8")
  writeFileSync(
    join(root, mathFileRelativePath),
    [
      "export const answer = 34;",
      "",
      "export function addNumbers(left: number, right: number) {",
      "  return left + right;",
      "}",
      "",
    ].join("\n"),
    "utf8",
  )
  writeFileSync(join(root, diffModifiedPath), diffModifiedOldContent, "utf8")
  writeFileSync(deepFilePath, deepFileContent, "utf8")
  writeFileSync(join(root, terminalSentinelFileName), `${terminalSentinelContent}\n`, "utf8")

  execFileSync("git", ["init"], { cwd: root, stdio: "pipe" })
  execFileSync("git", ["config", "user.email", "slab-e2e@example.local"], {
    cwd: root,
    stdio: "pipe",
  })
  execFileSync("git", ["config", "user.name", "Slab E2E"], { cwd: root, stdio: "pipe" })
  execFileSync("git", ["add", "."], { cwd: root, stdio: "pipe" })
  execFileSync("git", ["commit", "-m", "Initial workspace"], { cwd: root, stdio: "pipe" })
  writeFileSync(join(root, diffModifiedPath), diffModifiedNewContent, "utf8")
  writeFileSync(join(root, diffUntrackedPath), diffUntrackedContent, "utf8")

  return {
    deepFileContent,
    deepFilePath,
    deepFileRelativePath,
    deepPathSegments: [...deepPathSegments, deepFileName],
    diffModifiedNewContent,
    diffModifiedOldContent,
    diffModifiedPath,
    diffUntrackedContent,
    diffUntrackedPath,
    jsonConfigContent,
    jsonConfigPathSegments: jsonConfigRelativePath.split("/"),
    jsonConfigRelativePath,
    mathFileRelativePath,
    noteFileRelativePath,
    terminalSentinelContent,
    terminalSentinelFileName,
    tsMainFileContent,
    tsMainFileRelativePath,
  }
}

export async function clickExplorerPath(explorer: Locator, pathSegments: string[]): Promise<void> {
  /* eslint-disable no-await-in-loop -- each directory must expand before its child exists in the VS Code tree. */
  for (const [index, segment] of pathSegments.entries()) {
    const entry = explorer.getByText(segment, { exact: true }).first()
    await entry.waitFor({ state: "visible", timeout: 60_000 }).catch(async (error: unknown) => {
      const explorerText = await explorer.textContent().catch(() => "<unavailable>")
      const mountState = await explorer.getAttribute("data-mount-state").catch(() => "<unavailable>")
      const mountStage = await explorer.getAttribute("data-mount-stage").catch(() => "<unavailable>")
      const mountError = await explorer.getAttribute("data-mount-error").catch(() => "<unavailable>")
      const lspStage = await explorer.page().evaluate(() => {
        const target = window as typeof window & { __SLAB_WORKSPACE_LSP_STAGE__?: string }
        return target["__SLAB_WORKSPACE_LSP_STAGE__"] ?? null
      }).catch(() => "<unavailable>")
      const lspContext = await explorer.page().evaluate(() => {
        const target = window as typeof window & { __SLAB_WORKSPACE_LSP_CONTEXT__?: unknown }
        return JSON.stringify(target["__SLAB_WORKSPACE_LSP_CONTEXT__"] ?? null)
      }).catch(() => "<unavailable>")
      const lspDirectories = await explorer.page().evaluate(() => {
        const target = window as typeof window & { __SLAB_WORKSPACE_LSP_DIRECTORIES__?: unknown[] }
        return JSON.stringify(target["__SLAB_WORKSPACE_LSP_DIRECTORIES__"] ?? [])
      }).catch(() => "<unavailable>")
      const lspStats = await explorer.page().evaluate(() => {
        const target = window as typeof window & { __SLAB_WORKSPACE_LSP_STATS__?: unknown[] }
        return JSON.stringify(target["__SLAB_WORKSPACE_LSP_STATS__"] ?? [])
      }).catch(() => "<unavailable>")
      const rows = await explorer.locator('[role="treeitem"]').evaluateAll((elements) =>
        elements.slice(0, 30).map((element) => ({
          ariaExpanded: element.getAttribute("aria-expanded"),
          ariaLabel: element.getAttribute("aria-label"),
          level: element.getAttribute("aria-level"),
          text: element.textContent,
        })),
      ).catch(() => "<unavailable>")
      throw new Error(
        `workspace explorer entry '${segment}' did not become visible. Mount state: ${mountState}. Mount stage: ${mountStage}. Mount error: ${mountError}. LSP stage: ${lspStage}. LSP context: ${lspContext}. Directory reads: ${lspDirectories}. Stat reads: ${lspStats}. Rows: ${JSON.stringify(rows)}. Explorer text: ${explorerText ?? "<empty>"}. Cause: ${error instanceof Error ? error.message : String(error)}`,
      )
    })
    if (index < pathSegments.length - 1) {
      const treeItem = entry.locator('xpath=ancestor-or-self::*[@role="treeitem"][1]').first()
      await expandExplorerTreeItem(treeItem, entry)
    } else {
      await entry.click()
    }
  }
  /* eslint-enable no-await-in-loop */
}

export async function expandWorkspaceExplorerRoot(
  explorer: Locator,
  rootName: string,
  expectedChildName: string,
): Promise<void> {
  const expectedChild = explorer.getByText(expectedChildName, { exact: true }).first()
  if (await expectedChild.isVisible().catch(() => false)) {
    return
  }

  const root = explorer.getByText(rootName, { exact: true }).first()
  if (await root.isVisible().catch(() => false)) {
    const rootItem = root.locator('xpath=ancestor-or-self::*[@role="treeitem"][1]').first()
    await expandExplorerTreeItem(rootItem, root)
    if (await expectedChild.isVisible().catch(() => false)) {
      return
    }
  }

  const dotRoot = explorer.getByText(".", { exact: true }).first()
  if (await dotRoot.isVisible().catch(() => false)) {
    const dotRootItem = dotRoot.locator('xpath=ancestor-or-self::*[@role="treeitem"][1]').first()
    await expandExplorerTreeItem(dotRootItem, dotRoot)
    if (await expectedChild.isVisible().catch(() => false)) {
      return
    }
  }

  const firstTreeItem = explorer.locator('[role="treeitem"]').first()
  if (await firstTreeItem.isVisible().catch(() => false)) {
    await expandExplorerTreeItem(firstTreeItem, firstTreeItem)
  }
}

async function expandExplorerTreeItem(treeItem: Locator, entry: Locator): Promise<void> {
  if ((await treeItem.getAttribute("aria-expanded").catch(() => null)) === "true") {
    return
  }

  const twistie = treeItem.locator(".monaco-tl-twistie").first()
  if (await twistie.isVisible().catch(() => false)) {
    await twistie.click()
  } else {
    await entry.click()
    await entry.press("ArrowRight").catch(() => {})
  }

  if ((await treeItem.getAttribute("aria-expanded").catch(() => null)) === "true") {
    return
  }

  await entry.click()
  await entry.press("ArrowRight").catch(() => {})
}

export async function readMonacoEditorText(editor: Locator): Promise<string> {
  const text = await editor
    .locator(".view-line")
    .evaluateAll((lines) => lines.map((line) => line.textContent ?? "").join("\n"))
  return text.replace(/\u00a0/g, " ")
}

export async function focusMonacoEditor(editor: Locator): Promise<void> {
  await editor.locator(".view-lines").click({ position: { x: 8, y: 8 } })
}

export function workspaceTerminalReadSentinelCommand(
  marker: string,
  sentinelFileName = terminalSentinelFileName,
): string {
  return process.platform === "win32"
    ? `Write-Output ${marker}; Get-Content ${sentinelFileName}`
    : `printf '${marker}\\n'; cat ${sentinelFileName}`
}

export function isWorkspaceRuntimeFailure(message: string): boolean {
  return /extensionHost\.worker|Failed to fetch dynamically imported module|LocalWebWorker|ipc\.localhost|request_script_injection|failed to start workspace terminal|FileOperationError|Unable to resolve nonexistent file/i.test(message)
}
