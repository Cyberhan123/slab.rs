import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type CSSProperties,
} from "react"
import { useDrag, useWindowEvent } from "@mantine/hooks"
import { clamp } from "lodash-es"
import { useTranslation } from "@slab/i18n"
import { Button } from "@slab/components/button"
import { Tooltip, TooltipContent, TooltipTrigger } from "@slab/components/tooltip"
import { SoftPanel, StageEmptyState, StatusPill } from "@slab/components/workspace"
import {
  Command as CommandPaletteIcon,
  FileCode2,
  Files,
  Folder,
  FolderKanban,
  FolderOpen,
  GitBranch,
  Loader2,
  Search,
  Terminal,
  X,
} from "lucide-react"

import {
  workspaceReadDirectory,
  type WorkspaceDirectoryResponse,
} from "@/lib/workspace-bridge"
import { cn } from "@/lib/utils"
import type { WorkspacePageState } from "../hooks/use-workspace-page"
import { languageForFile, SLAB_DIR_NAME } from "../lib/workspace-page-utils"
import { RecentWorkspaceList } from "./recent-workspace-list"
import { WorkspaceCommandPalette } from "./workspace-command-palette"
import { WorkspaceCodeEditor } from "./workspace-code-editor"
import { WorkspaceConsolePanel } from "./workspace-console-panel"
import { WorkspaceDiffEditor } from "./workspace-diff-editor"
import { WorkspaceGitPanel } from "./workspace-git-panel"
import { WorkspaceSearchPanel } from "./workspace-search-panel"
import { WorkspaceVscodePart } from "./workspace-vscode-part/index"

const EXPLORER_MIN_WIDTH = 300
const EXPLORER_MAX_WIDTH = 640

export function WorkspaceWorkbench({
  activeFilePath,
  confirmDiscardDialog,
  consoleOpen,
  editorContent,
  editorSettings,
  editorThemeMode,
  explorerPanel,
  fileSearchFetching,
  fileSearchResults,
  fileSearchTruncated,
  fileError,
  gitDiffFetching,
  gitStatus,
  gitStatusFetching,
  gitOperationPending,
  handleCloseWorkspace,
  handleGitCommit,
  handleGitDiscard,
  handleGitStage,
  handleGitUnstage,
  handleExplainWithAssistant,
  handleOpenFile,
  handleOpenFolder,
  handleOpenTextSearchMatch,
  handleRefreshGitStatus,
  handleRevealDirectoryInTree,
  handleSaveFile,
  handleSelectExplorerPanel,
  handleSelectFileTab,
  handleSelectGitDiff,
  handleSetMarkdownMode,
  handleToggleConsole,
  isDesktopTauri,
  markdownMode,
  openFileTabs,
  openWorkspacePath,
  recentWorkspaces,
  selectedFile,
  selectedGitDiff,
  selectedGitDiffEntry,
  selectedFileDirty,
  savingFile,
  setBrowserEditorSelection,
  setEditorContent,
  setTextSearchQuery,
  textSearchFetching,
  textSearchQuery,
  textSearchResults,
  textSearchTruncated,
  workspace,
}: WorkspacePageState) {
  const { t } = useTranslation()
  const [commandPaletteOpen, setCommandPaletteOpen] = useState(false)
  const [explorerWidth, setExplorerWidth] = useState(380)
  const [pathInput, setPathInput] = useState("")
  const explorerResizeStartWidthRef = useRef(explorerWidth)
  const previousResizeCursorRef = useRef("")
  const previousResizeUserSelectRef = useRef("")
  const terminalThemeMode = editorThemeMode
  const workspaceGridStyle = {
    "--workspace-explorer-width": `${explorerWidth}px`,
  } as CSSProperties

  useWindowEvent("keydown", (event) => {
    if (!isDesktopTauri) {
      return
    }

    if (event.defaultPrevented || event.repeat || (!event.ctrlKey && !event.metaKey)) {
      return
    }

    if (event.key.toLowerCase() !== "p") {
      return
    }

    const target = event.target as HTMLElement | null
    const isTextInput = target?.matches("input, textarea, select") || target?.isContentEditable
    if (isTextInput && !target?.closest(".monaco-editor")) {
      return
    }

    event.preventDefault()
    setCommandPaletteOpen(true)
  })

  useEffect(() => {
    if (!workspace) {
      setCommandPaletteOpen(false)
    }
  }, [workspace])

  const explorerResize = useDrag<HTMLButtonElement>((state) => {
    state.event.preventDefault()
    if (state.first) {
      explorerResizeStartWidthRef.current = explorerWidth
      previousResizeCursorRef.current = document.body.style.cursor
      previousResizeUserSelectRef.current = document.body.style.userSelect
      document.body.style.cursor = "col-resize"
      document.body.style.userSelect = "none"
    }

    setExplorerWidth(
      clamp(explorerResizeStartWidthRef.current + state.movement[0], EXPLORER_MIN_WIDTH, EXPLORER_MAX_WIDTH),
    )

    if (state.last) {
      window.requestAnimationFrame(() => {
        document.body.style.cursor = previousResizeCursorRef.current
        document.body.style.userSelect = previousResizeUserSelectRef.current
      })
    }
  })

  const runEditorAction = useCallback(
    async (actionId: string) => {
      if (!workspace || !isDesktopTauri) {
        return
      }

      const { runWorkspaceVscodeCommand } = await import("../lib/workspace-lsp")
      await runWorkspaceVscodeCommand(actionId, workspace.rootPath).catch((error) => {
        console.debug("workspace VS Code command failed", { actionId, error })
      })
    },
    [isDesktopTauri, workspace],
  )

  const commandPaletteButton = (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          type="button"
          variant="quiet"
          size="icon-sm"
          aria-label={t("pages.workspace.commandPalette.trigger")}
          onClick={() => setCommandPaletteOpen(true)}
        >
          <CommandPaletteIcon className="size-4" />
        </Button>
      </TooltipTrigger>
      <TooltipContent>{t("pages.workspace.commandPalette.trigger")}</TooltipContent>
    </Tooltip>
  )
  const commandPalette = (
    <WorkspaceCommandPalette
      open={commandPaletteOpen}
      onOpenChange={setCommandPaletteOpen}
      workspaceRoot={workspace?.rootPath ?? null}
      recentWorkspaces={recentWorkspaces}
      openFileTabs={openFileTabs}
      explorerPanel={explorerPanel}
      consoleOpen={consoleOpen}
      markdownMode={markdownMode}
      selectedFile={selectedFile}
      selectedFileDirty={selectedFileDirty}
      gitStatusFetching={gitStatusFetching}
      gitOperationPending={gitOperationPending}
      onOpenFolder={handleOpenFolder}
      onCloseWorkspace={handleCloseWorkspace}
      onToggleConsole={handleToggleConsole}
      onSelectExplorerPanel={handleSelectExplorerPanel}
      onRefreshGitStatus={handleRefreshGitStatus}
      onOpenFile={handleOpenFile}
      onSelectFileTab={handleSelectFileTab}
      onRevealDirectoryInTree={handleRevealDirectoryInTree}
      onSaveFile={handleSaveFile}
      onSetMarkdownMode={handleSetMarkdownMode}
      onOpenWorkspacePath={openWorkspacePath}
      onEditorAction={runEditorAction}
      onExplainWithAssistant={handleExplainWithAssistant}
    />
  )

  const browserPathForm = !isDesktopTauri ? (
    <WorkspacePathOpenForm
      pathInput={pathInput}
      setPathInput={setPathInput}
      onOpenWorkspacePath={openWorkspacePath}
    />
  ) : null

  if (!workspace) {
    return (
      <div className="h-full w-full overflow-y-auto px-1 pb-10">
        <div className="mx-auto grid w-full max-w-5xl gap-5">
          <StageEmptyState
            icon={FolderKanban}
            title={t("pages.workspace.empty.title")}
            description={t("pages.workspace.empty.description")}
            action={
              <div className="flex flex-col items-center gap-3">
                {isDesktopTauri ? (
                  <Button variant="cta" size="pill" onClick={handleOpenFolder}>
                    <FolderOpen className="size-4" />
                    {t("pages.workspace.actions.openFolder")}
                  </Button>
                ) : (
                  browserPathForm
                )}
                {commandPaletteButton}
              </div>
            }
            className="min-h-[360px]"
          />
          <RecentWorkspaceList
            recentWorkspaces={recentWorkspaces}
            onOpen={openWorkspacePath}
            emptyLabel={t("pages.workspace.recent.empty")}
            title={t("pages.workspace.recent.title")}
            openLabel={t("pages.workspace.actions.reopen")}
          />
        </div>
        {commandPalette}
      </div>
    )
  }

  return (
    <div className="vs flex h-full min-h-0 w-full flex-col gap-4 overflow-hidden">
      <div className="flex flex-wrap items-center justify-between gap-3 px-1">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <h2 className="truncate text-xl font-semibold tracking-tight">{workspace.name}</h2>
            <StatusPill status="success">{SLAB_DIR_NAME}</StatusPill>
          </div>
          <p className="mt-1 truncate text-xs text-muted-foreground">{workspace.rootPath}</p>
        </div>
        <div className="flex items-center gap-2">
          {isDesktopTauri ? (
            <Button variant="pill" size="sm" onClick={handleOpenFolder}>
              <FolderOpen className="size-4" />
              {t("pages.workspace.actions.openFolder")}
            </Button>
          ) : null}
          <Button
            variant="quiet"
            size="sm"
            onClick={handleCloseWorkspace}
            data-testid="workspace-close-button"
          >
            <X className="size-4" />
            {t("pages.workspace.actions.closeWorkspace")}
          </Button>
          {commandPaletteButton}
          <Button variant={consoleOpen ? "pill" : "quiet"} size="sm" onClick={handleToggleConsole}>
            <Terminal className="size-4" />
            {consoleOpen ? t("pages.workspace.console.hide") : t("pages.workspace.console.show")}
          </Button>
        </div>
      </div>

      <div
        className="grid h-full min-h-0 flex-1 items-stretch gap-4 lg:grid-cols-[var(--workspace-explorer-width)_minmax(0,1fr)]"
        style={workspaceGridStyle}
      >
        <div className="relative min-h-0">
          <SoftPanel className="flex h-full min-h-0 flex-col gap-3 overflow-hidden rounded-2xl px-3 py-3">
            <div className="flex items-center justify-between gap-3 px-1">
              <div className="flex items-center gap-2 text-sm font-semibold">
                <FolderKanban className="size-4 text-[var(--brand-teal)]" />
                {t("pages.workspace.explorer.title")}
              </div>
              {(explorerPanel === "search" && textSearchFetching) ||
              (explorerPanel === "git" && gitStatusFetching) ? (
                <Loader2 className="size-4 animate-spin text-muted-foreground" />
              ) : null}
            </div>

            <div className="grid grid-cols-3 gap-1 rounded-full bg-[var(--surface-1)] p-1">
              <button
                type="button"
                className={cn(
                  "flex h-8 items-center justify-center gap-1.5 rounded-full text-xs font-medium text-muted-foreground transition hover:text-foreground",
                  "focus-ring duration-[var(--dur-180)] ease-out-expo",
                  explorerPanel === "files" && "bg-background text-foreground shadow-sm",
                )}
                onClick={() => handleSelectExplorerPanel("files")}
              >
                <Files className="size-3.5" />
                {t("pages.workspace.explorer.files")}
              </button>
              <button
                type="button"
                className={cn(
                  "flex h-8 items-center justify-center gap-1.5 rounded-full text-xs font-medium text-muted-foreground transition hover:text-foreground",
                  "focus-ring duration-[var(--dur-180)] ease-out-expo",
                  explorerPanel === "search" && "bg-background text-foreground shadow-sm",
                )}
                onClick={() => handleSelectExplorerPanel("search")}
              >
                <Search className="size-3.5" />
                {t("pages.workspace.explorer.search")}
              </button>
              <button
                type="button"
                className={cn(
                  "flex h-8 items-center justify-center gap-1.5 rounded-full text-xs font-medium text-muted-foreground transition hover:text-foreground",
                  "focus-ring duration-[var(--dur-180)] ease-out-expo",
                  explorerPanel === "git" && "bg-background text-foreground shadow-sm",
                )}
                onClick={() => handleSelectExplorerPanel("git")}
              >
                <GitBranch className="size-3.5" />
                {t("pages.workspace.explorer.git")}
              </button>
            </div>

            {explorerPanel === "files" ? (
              <div className="h-full min-h-0 flex-1 overflow-hidden rounded-[12px] bg-[var(--surface-1)]">
                {isDesktopTauri ? (
                  <WorkspaceVscodePart
                    part="explorer"
                    themeMode={editorThemeMode}
                    workspaceRoot={workspace.rootPath}
                  />
                ) : (
                  <WorkspaceServerFileTree
                    activeFilePath={activeFilePath}
                    onOpenFile={handleOpenFile}
                  />
                )}
              </div>
            ) : explorerPanel === "search" ? (
              <div className="h-full min-h-0 flex-1 overflow-hidden">
                <WorkspaceSearchPanel
                  activeFilePath={activeFilePath}
                  fileFetching={fileSearchFetching}
                  fileResults={fileSearchResults}
                  fileTruncated={fileSearchTruncated}
                  query={textSearchQuery}
                  textFetching={textSearchFetching}
                  textResults={textSearchResults}
                  textTruncated={textSearchTruncated}
                  onOpenFile={handleOpenFile}
                  onOpenMatch={handleOpenTextSearchMatch}
                  onQueryChange={setTextSearchQuery}
                />
              </div>
            ) : (
              <div className="h-full min-h-0 flex-1 overflow-hidden">
                <WorkspaceGitPanel
                  gitStatus={gitStatus}
                  gitStatusFetching={gitStatusFetching}
                  operationPending={gitOperationPending}
                  onCommit={handleGitCommit}
                  onDiscard={handleGitDiscard}
                  onRefresh={handleRefreshGitStatus}
                  onSelectDiff={handleSelectGitDiff}
                  onStage={handleGitStage}
                  selectedEntry={selectedGitDiffEntry}
                  onUnstage={handleGitUnstage}
                />
              </div>
            )}
          </SoftPanel>
          <button
            type="button"
            aria-label="Resize file explorer"
            className="focus-ring absolute bottom-3 right-[-10px] top-3 z-10 hidden w-5 cursor-col-resize items-center justify-center rounded-full text-muted-foreground/70 transition duration-[var(--dur-180)] ease-out-expo hover:bg-muted/70 hover:text-foreground lg:flex"
            ref={explorerResize.ref}
          >
            <span className="h-12 w-1 rounded-full bg-current" />
          </button>
        </div>

        <div className="flex h-full min-h-0 flex-col gap-4">
          <SoftPanel className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-2xl p-0">
            {fileError ? (
              <StageEmptyState
                icon={FileCode2}
                title={t("pages.workspace.editor.tooLarge")}
                description={fileError}
                className="min-h-[420px]"
                data-testid="file-too-large"
              />
            ) : selectedGitDiffEntry ? (
              <>
                <div className="flex h-9 shrink-0 items-center justify-between gap-3 border-b border-border/60 bg-background/80 px-3">
                  <div
                    className="min-w-0 truncate font-mono text-xs text-muted-foreground"
                    title={selectedGitDiffEntry.path}
                  >
                    {selectedGitDiffEntry.path}
                  </div>
                  {gitDiffFetching ? <Loader2 className="size-3.5 shrink-0 animate-spin text-muted-foreground" /> : null}
                </div>
                <div className="min-h-0 flex-1 overflow-hidden rounded-[6px]">
                  <WorkspaceDiffEditor
                    diffText={selectedGitDiff?.diff.trim() ?? ""}
                    filePath={selectedGitDiffEntry.path}
                    fontSize={editorSettings.fontSize}
                    minimapEnabled={editorSettings.minimapEnabled}
                    wordWrap={editorSettings.wordWrap}
                  />
                </div>
              </>
            ) : (
              isDesktopTauri ? (
                <WorkspaceVscodePart
                  part="editor"
                  workspaceRoot={workspace.rootPath}
                  editorSettings={editorSettings}
                  themeMode={editorThemeMode}
                  className="min-h-[420px] flex-1 rounded-[6px]"
                />
              ) : (
                <WorkspaceBrowserEditor
                  editorSettings={editorSettings}
                  editorContent={editorContent}
                  onChange={setEditorContent}
                  onSelectionChange={setBrowserEditorSelection}
                  onSave={handleSaveFile}
                  savingFile={savingFile}
                  selectedFile={selectedFile}
                  selectedFileDirty={selectedFileDirty}
                  themeMode={editorThemeMode}
                />
              )
            )}
          </SoftPanel>

          {consoleOpen ? (
            <WorkspaceConsolePanel
              themeMode={terminalThemeMode}
              workspaceRoot={workspace.rootPath}
            />
          ) : null}
        </div>
      </div>
      {commandPalette}
      {confirmDiscardDialog}
    </div>
  )
}

function WorkspacePathOpenForm({
  onOpenWorkspacePath,
  pathInput,
  setPathInput,
}: {
  onOpenWorkspacePath: (rootPath: string) => Promise<void>
  pathInput: string
  setPathInput: (value: string) => void
}) {
  const { t } = useTranslation()
  const trimmedPath = pathInput.trim()

  return (
    <form
      className="flex w-full max-w-2xl flex-col gap-2 sm:flex-row"
      onSubmit={(event) => {
        event.preventDefault()
        if (trimmedPath) {
          void onOpenWorkspacePath(trimmedPath)
        }
      }}
    >
      <input
        value={pathInput}
        onChange={(event) => setPathInput(event.target.value)}
        className="focus-ring h-10 min-w-0 flex-1 rounded-lg border border-border/60 bg-background px-3 text-sm transition duration-[var(--dur-180)] ease-out-expo focus:border-[var(--brand-teal)]"
        placeholder={t("pages.workspace.actions.pathPlaceholder")}
        aria-label={t("pages.workspace.actions.pathPlaceholder")}
        data-testid="workspace-path-input"
      />
      <Button
        type="button"
        variant="cta"
        size="pill"
        disabled={!trimmedPath}
        onClick={() => {
          if (trimmedPath) {
            void onOpenWorkspacePath(trimmedPath)
          }
        }}
        data-testid="workspace-open-path-button"
      >
        <FolderOpen className="size-4" />
        {t("pages.workspace.actions.openFolder")}
      </Button>
    </form>
  )
}

function WorkspaceServerFileTree({
  activeFilePath,
  onOpenFile,
}: {
  activeFilePath: string | null
  onOpenFile: (relativePath: string) => Promise<unknown>
}) {
  const { t } = useTranslation()
  const [directoryPath, setDirectoryPath] = useState("")
  const [directory, setDirectory] = useState<WorkspaceDirectoryResponse | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    let cancelled = false
    setLoading(true)
    setError(null)
    void workspaceReadDirectory(directoryPath)
      .then((response) => {
        if (!cancelled) {
          setDirectory(response)
        }
      })
      .catch((readError: unknown) => {
        if (!cancelled) {
          setError(readError instanceof Error ? readError.message : String(readError))
        }
      })
      .finally(() => {
        if (!cancelled) {
          setLoading(false)
        }
      })

    return () => {
      cancelled = true
    }
  }, [directoryPath])

  const parentPath = directoryPath.split("/").slice(0, -1).join("/")

  return (
    <div className="flex h-full min-h-0 flex-col" data-testid="workspace-file-tree">
      <div className="flex h-9 shrink-0 items-center gap-2 border-b border-border/50 px-2">
        <Button
          type="button"
          variant="quiet"
          size="xs"
          disabled={!directoryPath}
          onClick={() => setDirectoryPath(parentPath)}
          data-testid="workspace-tree-up"
        >
          {t("pages.workspace.actions.upDirectory")}
        </Button>
        <span
          className="min-w-0 truncate font-mono text-caption text-muted-foreground"
          data-testid="workspace-current-directory"
        >
          {directoryPath || t("pages.workspace.tree.root")}
        </span>
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto py-1" data-testid="workspace-file-list">
        {loading ? (
          <div className="flex h-full min-h-[160px] items-center justify-center">
            <Loader2 className="size-4 animate-spin text-muted-foreground" />
          </div>
        ) : error ? (
          <div className="px-3 py-2 text-sm text-destructive">{error}</div>
        ) : directory?.entries.length ? (
          directory.entries.map((entry) => {
            const active = activeFilePath === entry.relativePath
            const isDirectory = entry.kind === "directory"
            const Icon = isDirectory ? Folder : FileCode2

            return (
              <button
                key={entry.relativePath}
                type="button"
                className={cn(
                  "focus-ring flex w-full min-w-0 items-center gap-2 px-3 py-1.5 text-left text-sm transition duration-[var(--dur-180)] ease-out-expo hover:bg-[var(--surface-selected)]",
                  active && "bg-[var(--surface-selected)] text-[var(--brand-teal)]",
                )}
                title={entry.relativePath}
                data-testid={`workspace-${isDirectory ? "directory" : "file"}-${testIdPath(entry.relativePath)}`}
                onClick={() => {
                  if (isDirectory) {
                    setDirectoryPath(entry.relativePath)
                    return
                  }
                  void onOpenFile(entry.relativePath)
                }}
              >
                <Icon className={cn("size-4 shrink-0", isDirectory ? "text-[var(--brand-teal)]" : "text-muted-foreground")} />
                <span className="min-w-0 flex-1 truncate">{entry.name}</span>
              </button>
            )
          })
        ) : (
          <div className="flex h-full min-h-[160px] items-center justify-center px-4 text-center text-sm text-muted-foreground">
            {t("pages.workspace.tree.empty")}
          </div>
        )}
      </div>
    </div>
  )
}

function WorkspaceBrowserEditor({
  editorContent,
  editorSettings,
  onChange,
  onSelectionChange,
  onSave,
  savingFile,
  selectedFile,
  selectedFileDirty,
  themeMode,
}: {
  editorContent: string
  editorSettings: WorkspacePageState["editorSettings"]
  onChange: (value: string) => void
  onSelectionChange: WorkspacePageState["setBrowserEditorSelection"]
  onSave: () => Promise<void>
  savingFile: boolean
  selectedFile: WorkspacePageState["selectedFile"]
  selectedFileDirty: boolean
  themeMode: WorkspacePageState["editorThemeMode"]
}) {
  const { t } = useTranslation()

  if (!selectedFile) {
    return (
      <StageEmptyState
        icon={FileCode2}
        title={t("pages.workspace.editor.emptyTitle")}
        description={t("pages.workspace.editor.emptyDescription")}
        className="min-h-[420px]"
      />
    )
  }

  return (
    <div className="flex h-full min-h-[420px] flex-col" data-testid="workspace-browser-editor">
          <div className="flex h-10 shrink-0 items-center justify-between gap-3 border-b border-border/60 bg-background/80 px-3">
        <div className="min-w-0 truncate font-mono text-caption text-muted-foreground" title={selectedFile.relativePath}>
          {selectedFile.relativePath}
        </div>
        <Button
          type="button"
          variant={selectedFileDirty ? "cta" : "quiet"}
          size="sm"
          disabled={savingFile || !selectedFileDirty}
          onClick={() => {
            void onSave()
          }}
          data-testid="workspace-save-button"
        >
          {savingFile ? <Loader2 className="size-3.5 animate-spin" /> : null}
          {t("pages.workspace.editor.save")}
        </Button>
      </div>
      <div className="min-h-0 flex-1 overflow-hidden rounded-[6px] bg-[var(--surface-1)]" data-testid="workspace-editor-monaco">
        <WorkspaceCodeEditor
          filePath={`memory://workspace/${encodeURIComponent(selectedFile.relativePath)}`}
          language={languageForFile(selectedFile.relativePath)}
          memoryModel
          onChange={onChange}
          onSelectionChange={(selection) => {
            onSelectionChange(selection
              ? {
                  ...selection,
                  relativePath: selectedFile.relativePath,
                }
              : null)
          }}
          options={{
            automaticLayout: true,
            fontSize: editorSettings.fontSize,
            minimap: { enabled: editorSettings.minimapEnabled },
            readOnly: false,
            tabSize: editorSettings.tabSize,
            wordWrap: editorSettings.wordWrap,
          }}
          themeMode={themeMode}
          value={editorContent}
        />
      </div>
    </div>
  )
}

function testIdPath(path: string) {
  return path.replace(/[^A-Za-z0-9_-]/g, "-")
}
