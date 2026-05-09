import Editor from "@monaco-editor/react"
import { useTranslation } from "@slab/i18n"
import { Button } from "@slab/components/button"
import { SoftPanel, StageEmptyState, StatusPill } from "@slab/components/workspace"
import { Tree } from "react-arborist"
import { FileCode2, Folder, FolderKanban, FolderOpen, Loader2, X } from "lucide-react"

import { cn } from "@/lib/utils"
import type { WorkspacePageState } from "../hooks/use-workspace-page"
import { languageForFile, SLAB_DIR_NAME } from "../lib/workspace-page-utils"
import { RecentWorkspaceList } from "./recent-workspace-list"
import { WorkspaceTreeRow } from "./workspace-tree-row"

export function WorkspaceWorkbench({
  activeFilePath,
  fileError,
  handleCloseFileTab,
  handleCloseWorkspace,
  handleOpenFile,
  handleOpenFolder,
  handleSelectFileTab,
  handleTreeToggle,
  initialOpenState,
  isDesktopTauri,
  loadDirectory,
  loadingPaths,
  openFileTabs,
  openWorkspacePath,
  recentWorkspaces,
  selectedFile,
  treeData,
  treeHeight,
  treeHostRef,
  workspace,
  workspaceUiHasHydrated,
}: WorkspacePageState) {
  const { t } = useTranslation()

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
              <Button variant="cta" size="pill" onClick={handleOpenFolder}>
                <FolderOpen className="size-4" />
                {t("pages.workspace.actions.openFolder")}
              </Button>
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
        </div>
      </div>

      <div className="grid h-full min-h-0 flex-1 items-stretch gap-4 lg:grid-cols-[320px_minmax(0,1fr)]">
        <SoftPanel className="flex h-full min-h-0 flex-col gap-3 overflow-hidden rounded-[18px] px-3 py-3">
          <div className="flex items-center justify-between gap-3 px-1">
            <div className="flex items-center gap-2 text-sm font-semibold">
              <Folder className="size-4 text-[var(--brand-teal)]" />
              {t("pages.workspace.tree.title")}
            </div>
            {loadingPaths.has("") ? <Loader2 className="size-4 animate-spin text-muted-foreground" /> : null}
          </div>
          <div ref={treeHostRef} className="h-full min-h-0 flex-1 overflow-hidden rounded-[12px] bg-[var(--surface-1)]">
            {workspaceUiHasHydrated ? (
              <Tree
                key={workspace.rootPath}
                data={treeData}
                idAccessor="id"
                childrenAccessor="children"
                rowHeight={32}
                indent={18}
                height={treeHeight}
                width="100%"
                disableDrag
                disableDrop
                disableEdit
                openByDefault={false}
                initialOpenState={initialOpenState}
                selection={activeFilePath ?? undefined}
                onToggle={handleTreeToggle}
              >
                {(props) => (
                  <WorkspaceTreeRow
                    {...props}
                    selectedPath={activeFilePath}
                    loadingPaths={loadingPaths}
                    onOpenDirectory={loadDirectory}
                    onOpenFile={handleOpenFile}
                  />
                )}
              </Tree>
            ) : (
              <div className="flex h-full min-h-[240px] items-center justify-center">
                <Loader2 className="size-4 animate-spin text-muted-foreground" />
              </div>
            )}
          </div>
        </SoftPanel>

        <SoftPanel className="flex h-full min-h-0 flex-col overflow-hidden rounded-[18px] p-0">
          {openFileTabs.length > 0 ? (
            <div className="flex h-10 shrink-0 items-end overflow-x-auto border-b border-border/60 bg-[var(--surface-1)] px-2 pt-2">
              {openFileTabs.map((tab) => {
                const active = activeFilePath === tab.relativePath

                return (
                  <div
                    key={tab.relativePath}
                    className={cn(
                      "group flex h-8 max-w-48 shrink-0 items-center gap-2 rounded-t-[8px] border border-transparent px-3 text-xs outline-none transition hover:bg-background/80",
                      active && "border-border/70 border-b-background bg-background text-[var(--brand-teal)]",
                    )}
                    title={tab.relativePath}
                  >
                    <button
                      type="button"
                      className="flex min-w-0 flex-1 items-center gap-2 outline-none"
                      onClick={() => {
                        void handleSelectFileTab(tab.relativePath)
                      }}
                    >
                      <FileCode2 className="size-3.5 shrink-0 text-muted-foreground" />
                      <span className="truncate">{tab.name}</span>
                    </button>
                    <button
                      type="button"
                      className="flex size-4 shrink-0 items-center justify-center rounded-[4px] text-muted-foreground opacity-70 transition hover:bg-muted hover:text-foreground group-hover:opacity-100"
                      aria-label={t("pages.workspace.tabs.close", { name: tab.name })}
                      onClick={(event) => {
                        event.stopPropagation()
                        void handleCloseFileTab(tab.relativePath)
                      }}
                    >
                      <X className="size-3" />
                    </button>
                  </div>
                )
              })}
            </div>
          ) : null}
          {selectedFile ? (
            <div className="min-h-0 flex-1">
              <Editor
                height="100%"
                language={languageForFile(selectedFile.name)}
                path={selectedFile.relativePath}
                theme={document.documentElement.classList.contains("dark") ? "vs-dark" : "light"}
                value={selectedFile.content}
                options={{
                  readOnly: true,
                  minimap: { enabled: false },
                  scrollBeyondLastLine: false,
                  wordWrap: "on",
                  fontSize: 13,
                }}
              />
            </div>
          ) : (
            <StageEmptyState
              icon={FileCode2}
              title={fileError ? t("pages.workspace.editor.tooLarge") : t("pages.workspace.editor.emptyTitle")}
              description={fileError ?? t("pages.workspace.editor.emptyDescription")}
              className="h-full min-h-[420px] flex-1 rounded-[18px] bg-transparent"
            />
          )}
        </SoftPanel>
      </div>
    </div>
  )
}
