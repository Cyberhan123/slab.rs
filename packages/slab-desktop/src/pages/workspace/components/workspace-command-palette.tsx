import { useEffect, useMemo, useState } from "react"
import { useQuery } from "@tanstack/react-query"
import { useTranslation } from "@slab/i18n"
import {
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandSeparator,
} from "@slab/components/command"
import {
  Code2,
  Eye,
  FileCode2,
  FolderOpen,
  FolderSearch,
  FolderKanban,
  GitBranch,
  Lightbulb,
  ListTree,
  RefreshCcw,
  Save,
  Search,
  SearchCode,
  Terminal,
  TextCursorInput,
  WandSparkles,
  X,
} from "lucide-react"

import { workspaceSearchFiles, type RecentWorkspace, type WorkspaceFileContent } from "@/lib/workspace-bridge"
import type {
  WorkspaceExplorerPanel,
  WorkspaceFileTab,
  WorkspaceMarkdownMode,
} from "@/store/useWorkspaceUiStore"

import { languageForFile } from "../lib/workspace-page-utils"

type WorkspaceCommandPaletteProps = {
  open: boolean
  onOpenChange: (open: boolean) => void
  workspaceRoot: string | null
  recentWorkspaces: RecentWorkspace[]
  openFileTabs: WorkspaceFileTab[]
  explorerPanel: WorkspaceExplorerPanel
  consoleOpen: boolean
  markdownMode: WorkspaceMarkdownMode
  selectedFile: WorkspaceFileContent | null
  selectedFileDirty: boolean
  gitStatusFetching: boolean
  gitOperationPending: boolean
  onOpenFolder: () => void
  onCloseWorkspace: () => void
  onToggleConsole: () => void
  onSelectExplorerPanel: (panel: WorkspaceExplorerPanel) => void
  onRefreshGitStatus: () => Promise<void>
  onOpenFile: (relativePath: string) => Promise<unknown>
  onSelectFileTab: (relativePath: string) => Promise<void>
  onRevealDirectoryInTree: (relativePath: string) => Promise<void>
  onSaveFile: () => Promise<void>
  onSetMarkdownMode: (mode: WorkspaceMarkdownMode) => void
  onOpenWorkspacePath: (rootPath: string) => Promise<void>
  onEditorAction: (actionId: string) => Promise<void>
}

export function WorkspaceCommandPalette({
  open,
  onOpenChange,
  workspaceRoot,
  recentWorkspaces,
  openFileTabs,
  explorerPanel,
  consoleOpen,
  markdownMode,
  selectedFile,
  selectedFileDirty,
  gitStatusFetching,
  gitOperationPending,
  onOpenFolder,
  onCloseWorkspace,
  onToggleConsole,
  onSelectExplorerPanel,
  onRefreshGitStatus,
  onOpenFile,
  onSelectFileTab,
  onRevealDirectoryInTree,
  onSaveFile,
  onSetMarkdownMode,
  onOpenWorkspacePath,
  onEditorAction,
}: WorkspaceCommandPaletteProps) {
  const { t } = useTranslation()
  const [query, setQuery] = useState("")
  const trimmedQuery = query.trim()
  const isMarkdownFile = selectedFile ? languageForFile(selectedFile.name) === "markdown" : false

  const {
    data: fileSearchResult,
    isFetching: fileSearchFetching,
  } = useQuery({
    queryKey: ["workspace-command-palette-search", workspaceRoot, trimmedQuery],
    queryFn: () => workspaceSearchFiles(trimmedQuery),
    enabled: open && Boolean(workspaceRoot && trimmedQuery),
    // Command palette search is bound to transient user input; a changed query
    // should own the next probe instead of retrying a stale bridge request.
    retry: false,
  })

  const fileSearchEntries = useMemo(() => fileSearchResult?.entries ?? [], [fileSearchResult])

  useEffect(() => {
    if (!open) {
      setQuery("")
    }
  }, [open])

  if (!open) {
    return null
  }

  return (
    <CommandDialog
      open={open}
      onOpenChange={onOpenChange}
      showCloseButton={false}
      className="max-w-[720px] overflow-hidden p-0"
      title={t("pages.workspace.commandPalette.title")}
      description={t("pages.workspace.commandPalette.description")}
    >
      <CommandInput
        value={query}
        onValueChange={setQuery}
        placeholder={t("pages.workspace.commandPalette.placeholder")}
      />
      <CommandList className="max-h-[70vh]">
        <CommandEmpty>{t("pages.workspace.commandPalette.empty")}</CommandEmpty>

        {workspaceRoot ? (
          <>
            <CommandGroup heading={t("pages.workspace.commandPalette.actions")}>
              <CommandItem
                value={t("pages.workspace.actions.openFolder")}
                onSelect={() => {
                  onOpenChange(false)
                  onOpenFolder()
                }}
              >
                <FolderOpen className="size-4" />
                <span>{t("pages.workspace.actions.openFolder")}</span>
              </CommandItem>
              <CommandItem
                value={t("pages.workspace.actions.closeWorkspace")}
                onSelect={() => {
                  onOpenChange(false)
                  onCloseWorkspace()
                }}
              >
                <X className="size-4" />
                <span>{t("pages.workspace.actions.closeWorkspace")}</span>
              </CommandItem>
              <CommandItem
                value={consoleOpen ? t("pages.workspace.console.hide") : t("pages.workspace.console.show")}
                onSelect={() => {
                  onOpenChange(false)
                  onToggleConsole()
                }}
              >
                <Terminal className="size-4" />
                <span>{consoleOpen ? t("pages.workspace.console.hide") : t("pages.workspace.console.show")}</span>
              </CommandItem>
              <CommandItem
                value={t("pages.workspace.explorer.files")}
                disabled={explorerPanel === "files"}
                onSelect={() => {
                  onOpenChange(false)
                  onSelectExplorerPanel("files")
                }}
              >
                <FolderKanban className="size-4" />
                <span>{t("pages.workspace.explorer.files")}</span>
              </CommandItem>
              <CommandItem
                value={t("pages.workspace.explorer.search")}
                disabled={explorerPanel === "search"}
                onSelect={() => {
                  onOpenChange(false)
                  onSelectExplorerPanel("search")
                }}
              >
                <Search className="size-4" />
                <span>{t("pages.workspace.explorer.search")}</span>
              </CommandItem>
              <CommandItem
                value={t("pages.workspace.explorer.git")}
                disabled={explorerPanel === "git"}
                onSelect={() => {
                  onOpenChange(false)
                  onSelectExplorerPanel("git")
                }}
              >
                <GitBranch className="size-4" />
                <span>{t("pages.workspace.explorer.git")}</span>
              </CommandItem>
              <CommandItem
                value={t("pages.workspace.git.refresh")}
                disabled={gitStatusFetching || gitOperationPending}
                onSelect={() => {
                  onOpenChange(false)
                  void onRefreshGitStatus()
                }}
              >
                <RefreshCcw className="size-4" />
                <span>{t("pages.workspace.git.refresh")}</span>
              </CommandItem>
              <CommandItem
                value={t("pages.workspace.editor.save")}
                disabled={!selectedFileDirty}
                onSelect={() => {
                  onOpenChange(false)
                  void onSaveFile()
                }}
              >
                <Save className="size-4" />
                <span>{t("pages.workspace.editor.save")}</span>
              </CommandItem>
              {selectedFile && (!isMarkdownFile || markdownMode === "source") ? (
                <>
                  <CommandItem
                    value={t("pages.workspace.editor.find")}
                    onSelect={() => {
                      onOpenChange(false)
                      void onEditorAction("actions.find")
                    }}
                  >
                    <SearchCode className="size-4" />
                    <span>{t("pages.workspace.editor.find")}</span>
                  </CommandItem>
                  <CommandItem
                    value={t("pages.workspace.editor.goToSymbol")}
                    onSelect={() => {
                      onOpenChange(false)
                      void onEditorAction("editor.action.quickOutline")
                    }}
                  >
                    <ListTree className="size-4" />
                    <span>{t("pages.workspace.editor.goToSymbol")}</span>
                  </CommandItem>
                  <CommandItem
                    value={t("pages.workspace.editor.quickFix")}
                    onSelect={() => {
                      onOpenChange(false)
                      void onEditorAction("editor.action.quickFix")
                    }}
                  >
                    <Lightbulb className="size-4" />
                    <span>{t("pages.workspace.editor.quickFix")}</span>
                  </CommandItem>
                  <CommandItem
                    value={t("pages.workspace.editor.renameSymbol")}
                    onSelect={() => {
                      onOpenChange(false)
                      void onEditorAction("editor.action.rename")
                    }}
                  >
                    <TextCursorInput className="size-4" />
                    <span>{t("pages.workspace.editor.renameSymbol")}</span>
                  </CommandItem>
                  <CommandItem
                    value={t("pages.workspace.editor.formatDocument")}
                    onSelect={() => {
                      onOpenChange(false)
                      void onEditorAction("editor.action.formatDocument")
                    }}
                  >
                    <WandSparkles className="size-4" />
                    <span>{t("pages.workspace.editor.formatDocument")}</span>
                  </CommandItem>
                  <CommandItem
                    value={t("pages.workspace.editor.toggleLineComment")}
                    onSelect={() => {
                      onOpenChange(false)
                      void onEditorAction("editor.action.commentLine")
                    }}
                  >
                    <Code2 className="size-4" />
                    <span>{t("pages.workspace.editor.toggleLineComment")}</span>
                  </CommandItem>
                </>
              ) : null}
              {selectedFile ? (
                <CommandItem
                  value={`${t("pages.workspace.commandPalette.revealCurrentFile")} ${selectedFile.name} ${selectedFile.relativePath}`}
                  onSelect={() => {
                    onOpenChange(false)
                    void onRevealDirectoryInTree(
                      selectedFile.relativePath
                        .split("/")
                        .filter(Boolean)
                        .slice(0, -1)
                        .join("/"),
                    )
                  }}
                >
                  <FolderSearch className="size-4" />
                  <span>{t("pages.workspace.commandPalette.revealCurrentFile")}</span>
                </CommandItem>
              ) : null}
              {selectedFile && isMarkdownFile ? (
                <>
                  <CommandItem
                    value={t("pages.workspace.editor.preview")}
                    disabled={markdownMode === "preview"}
                    onSelect={() => {
                      onOpenChange(false)
                      onSetMarkdownMode("preview")
                    }}
                  >
                    <Eye className="size-4" />
                    <span>{t("pages.workspace.editor.preview")}</span>
                  </CommandItem>
                  <CommandItem
                    value={t("pages.workspace.editor.source")}
                    disabled={markdownMode === "source"}
                    onSelect={() => {
                      onOpenChange(false)
                      onSetMarkdownMode("source")
                    }}
                  >
                    <Code2 className="size-4" />
                    <span>{t("pages.workspace.editor.source")}</span>
                  </CommandItem>
                </>
              ) : null}
            </CommandGroup>

            {trimmedQuery.length === 0 ? (
              <>
                {openFileTabs.length > 0 ? (
                  <>
                    <CommandSeparator />
                    <CommandGroup heading={t("pages.workspace.commandPalette.tabs")}>
                      {openFileTabs.map((tab) => (
                        <CommandItem
                          key={tab.relativePath}
                          value={`${tab.name} ${tab.relativePath}`}
                          onSelect={() => {
                            onOpenChange(false)
                            void onSelectFileTab(tab.relativePath)
                          }}
                        >
                          <FileCode2 className="size-4" />
                          <span className="truncate">{tab.name}</span>
                          <span className="ml-auto truncate text-xs text-muted-foreground">
                            {tab.relativePath}
                          </span>
                        </CommandItem>
                      ))}
                    </CommandGroup>
                  </>
                ) : null}

                {recentWorkspaces.length > 0 ? (
                  <>
                    <CommandSeparator />
                    <CommandGroup heading={t("pages.workspace.commandPalette.recent")}>
                      {recentWorkspaces.map((workspace) => (
                        <CommandItem
                          key={workspace.rootPath}
                          value={`${workspace.name} ${workspace.rootPath}`}
                          onSelect={() => {
                            onOpenChange(false)
                            void onOpenWorkspacePath(workspace.rootPath)
                          }}
                        >
                          <FolderOpen className="size-4" />
                          <span className="truncate">{workspace.name}</span>
                          <span className="ml-auto truncate text-xs text-muted-foreground">
                            {workspace.rootPath}
                          </span>
                        </CommandItem>
                      ))}
                    </CommandGroup>
                  </>
                ) : null}
              </>
            ) : (
              <>
                <CommandSeparator />
                <CommandGroup heading={t("pages.workspace.commandPalette.files")}>
                  {fileSearchFetching ? (
                    <CommandItem value={`loading ${trimmedQuery}`} disabled>
                      <Search className="size-4 animate-pulse" />
                      <span>{t("pages.workspace.tree.loading")}</span>
                    </CommandItem>
                  ) : null}
                  {fileSearchEntries.map((entry) => {
                    const isDirectory = entry.kind === "directory"

                    return (
                      <CommandItem
                        key={entry.relativePath}
                        value={`${entry.name} ${entry.relativePath}`}
                        onSelect={() => {
                          onOpenChange(false)
                          if (isDirectory) {
                            void onRevealDirectoryInTree(entry.relativePath)
                            return
                          }

                          void onOpenFile(entry.relativePath)
                        }}
                      >
                        {isDirectory ? <FolderKanban className="size-4" /> : <FileCode2 className="size-4" />}
                        <span className="truncate">{entry.name}</span>
                        <span className="ml-auto truncate text-xs text-muted-foreground">
                          {entry.relativePath}
                        </span>
                      </CommandItem>
                    )
                  })}
                  {fileSearchResult?.truncated ? (
                    <div className="px-2 py-2 text-xs text-muted-foreground">
                      {t("pages.workspace.search.truncated")}
                    </div>
                  ) : null}
                </CommandGroup>
              </>
            )}
          </>
        ) : (
          <>
            <CommandGroup heading={t("pages.workspace.commandPalette.actions")}>
              <CommandItem
                value={t("pages.workspace.actions.openFolder")}
                onSelect={() => {
                  onOpenChange(false)
                  onOpenFolder()
                }}
              >
                <FolderOpen className="size-4" />
                <span>{t("pages.workspace.actions.openFolder")}</span>
              </CommandItem>
            </CommandGroup>
            {recentWorkspaces.length > 0 ? (
              <>
                <CommandSeparator />
                <CommandGroup heading={t("pages.workspace.commandPalette.recent")}>
                  {recentWorkspaces.map((workspace) => (
                    <CommandItem
                      key={workspace.rootPath}
                      value={`${workspace.name} ${workspace.rootPath}`}
                      onSelect={() => {
                        onOpenChange(false)
                        void onOpenWorkspacePath(workspace.rootPath)
                      }}
                    >
                      <FolderOpen className="size-4" />
                      <span className="truncate">{workspace.name}</span>
                      <span className="ml-auto truncate text-xs text-muted-foreground">
                        {workspace.rootPath}
                      </span>
                    </CommandItem>
                  ))}
                </CommandGroup>
              </>
            ) : null}
          </>
        )}
      </CommandList>
    </CommandDialog>
  )
}
