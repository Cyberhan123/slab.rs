import { useCallback, useEffect, useState } from "react"
import { useTranslation } from "@slab/i18n"
import { Button } from "@slab/components/button"
import { Tooltip, TooltipContent, TooltipTrigger } from "@slab/components/tooltip"
import { SoftPanel, StageEmptyState, StatusPill } from "@slab/components/workspace"
import {
  Command as CommandPaletteIcon,
  Files,
  FolderKanban,
  FolderOpen,
  GitBranch,
  Loader2,
  Search,
  Terminal,
  X,
} from "lucide-react"

import { cn } from "@/lib/utils"
import type { WorkspacePageState } from "../hooks/use-workspace-page"
import { runWorkspaceVscodeCommand } from "../lib/workspace-lsp"
import { SLAB_DIR_NAME } from "../lib/workspace-page-utils"
import { RecentWorkspaceList } from "./recent-workspace-list"
import { WorkspaceCommandPalette } from "./workspace-command-palette"
import { WorkspaceConsolePanel } from "./workspace-console-panel"
import { WorkspaceDiffEditor } from "./workspace-diff-editor"
import { WorkspaceGitPanel } from "./workspace-git-panel"
import { WorkspaceSearchPanel } from "./workspace-search-panel"
import { WorkspaceVscodePart } from "./workspace-vscode-part"

export function WorkspaceWorkbench({
  activeFilePath,
  consoleOpen,
  editorSettings,
  editorTheme,
  explorerPanel,
  fileSearchFetching,
  fileSearchResults,
  fileSearchTruncated,
  gitDiffFetching,
  gitStatus,
  gitStatusFetching,
  gitOperationPending,
  handleCloseWorkspace,
  handleGitCommit,
  handleGitDiscard,
  handleGitStage,
  handleGitUnstage,
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
  setTextSearchQuery,
  textSearchFetching,
  textSearchQuery,
  textSearchResults,
  textSearchTruncated,
  workspace,
}: WorkspacePageState) {
  const { t } = useTranslation()
  const [commandPaletteOpen, setCommandPaletteOpen] = useState(false)
  const terminalThemeMode = editorTheme === "vs-dark" ? "dark" : "light"

  useEffect(() => {
    if (!isDesktopTauri) {
      return
    }

    const handleKeyDown = (event: KeyboardEvent) => {
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
    }

    window.addEventListener("keydown", handleKeyDown)
    return () => {
      window.removeEventListener("keydown", handleKeyDown)
    }
  }, [isDesktopTauri])

  useEffect(() => {
    if (!workspace) {
      setCommandPaletteOpen(false)
    }
  }, [workspace])

  const runEditorAction = useCallback(
    async (actionId: string) => {
      if (!workspace) {
        return
      }

      await runWorkspaceVscodeCommand(actionId, workspace.rootPath).catch((error) => {
        console.debug("workspace VS Code command failed", { actionId, error })
      })
    },
    [workspace],
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
    />
  )

  if (!isDesktopTauri) {
    return (
      <div className="h-full w-full overflow-y-auto px-1 pb-10">
        <StageEmptyState
          icon={FolderKanban}
          title={t("pages.workspace.empty.title")}
          description={t("pages.plugins.desktopOnly.description")}
          className="min-h-[520px]"
        />
      </div>
    )
  }

  if (!workspace) {
    return (
      <div className="h-full w-full overflow-y-auto px-1 pb-10">
        <div className="mx-auto grid w-full max-w-5xl gap-5">
          <StageEmptyState
            icon={FolderKanban}
            title={t("pages.workspace.empty.title")}
            description={t("pages.workspace.empty.description")}
            action={
              <div className="flex items-center gap-2">
                <Button variant="cta" size="pill" onClick={handleOpenFolder}>
                  <FolderOpen className="size-4" />
                  {t("pages.workspace.actions.openFolder")}
                </Button>
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
    <div className="flex h-full min-h-0 w-full flex-col gap-4 overflow-hidden">
      <div className="flex flex-wrap items-center justify-between gap-3 px-1">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <h2 className="truncate text-xl font-semibold tracking-tight">{workspace.name}</h2>
            <StatusPill status="success">{SLAB_DIR_NAME}</StatusPill>
          </div>
          <p className="mt-1 truncate text-xs text-muted-foreground">{workspace.rootPath}</p>
        </div>
        <div className="flex items-center gap-2">
          <Button variant="pill" size="sm" onClick={handleOpenFolder}>
            <FolderOpen className="size-4" />
            {t("pages.workspace.actions.openFolder")}
          </Button>
          <Button variant="quiet" size="sm" onClick={handleCloseWorkspace}>
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

      <div className="grid h-full min-h-0 flex-1 items-stretch gap-4 lg:grid-cols-[320px_minmax(0,1fr)]">
        <SoftPanel className="flex h-full min-h-0 flex-col gap-3 overflow-hidden rounded-[18px] px-3 py-3">
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
              <WorkspaceVscodePart part="explorer" workspaceRoot={workspace.rootPath} />
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

        <div className="flex h-full min-h-0 flex-col gap-4">
          <SoftPanel className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-[18px] p-0">
            {selectedGitDiffEntry ? (
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
                <div className="min-h-0 flex-1">
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
              <WorkspaceVscodePart
                part="editor"
                workspaceRoot={workspace.rootPath}
                className="min-h-[420px] flex-1"
              />
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
    </div>
  )
}
