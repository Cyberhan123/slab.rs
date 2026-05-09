import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import Editor from "@monaco-editor/react"
import { open } from "@tauri-apps/plugin-dialog"
import { useQuery, useQueryClient } from "@tanstack/react-query"
import { useTranslation } from "@slab/i18n"
import { Button } from "@slab/components/button"
import { SoftPanel, StageEmptyState, StatusPill } from "@slab/components/workspace"
import { Tree, type NodeRendererProps } from "react-arborist"
import {
  ChevronDown,
  ChevronRight,
  FileCode2,
  Folder,
  FolderKanban,
  FolderOpen,
  Loader2,
  X,
} from "lucide-react"
import { toast } from "sonner"

import { usePageHeader } from "@/hooks/use-global-header-meta"
import { isTauri } from "@/hooks/use-tauri"
import { cn } from "@/lib/utils"
import {
  workspaceClose,
  workspaceOpen,
  workspaceReadDirectory,
  workspaceReadFile,
  workspaceState,
  WORKSPACE_STATE_QUERY_KEY,
  type WorkspaceFileContent,
  type WorkspaceFileEntry,
} from "@/lib/workspace-bridge"
import {
  emptyWorkspaceUiSnapshot,
  useWorkspaceUiStore,
  type WorkspaceFileTab,
} from "@/store/useWorkspaceUiStore"
import { getErrorMessage } from "@slab/api"

type WorkspaceTreeNode = WorkspaceFileEntry & {
  children?: WorkspaceTreeNode[]
  loaded?: boolean
}

export default function WorkspacePage() {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const isDesktopTauri = isTauri()
  const [treeData, setTreeData] = useState<WorkspaceTreeNode[]>([])
  const [selectedFile, setSelectedFile] = useState<WorkspaceFileContent | null>(null)
  const [fileError, setFileError] = useState<string | null>(null)
  const [loadingPaths, setLoadingPaths] = useState<Set<string>>(new Set())
  const treeHostRef = useRef<HTMLDivElement | null>(null)
  const restoredWorkspaceRootRef = useRef<string | null>(null)
  const [treeHeight, setTreeHeight] = useState(320)

  usePageHeader({
    icon: FolderKanban,
    title: t("pages.workspace.header.title"),
    subtitle: t("pages.workspace.header.subtitle"),
  })

  const workspaceQuery = useQuery({
    queryKey: WORKSPACE_STATE_QUERY_KEY,
    queryFn: workspaceState,
    enabled: isDesktopTauri,
    retry: false,
  })
  const workspace = workspaceQuery.data?.current ?? null
  const recentWorkspaces = workspaceQuery.data?.recent ?? []
  const workspaceUiHasHydrated = useWorkspaceUiStore((state) => state.hasHydrated)
  const workspaceUiByRoot = useWorkspaceUiStore((state) => state.workspaces)
  const patchWorkspaceState = useWorkspaceUiStore((state) => state.patchWorkspaceState)
  const workspaceUi = workspace
    ? workspaceUiByRoot[workspace.rootPath] ?? emptyWorkspaceUiSnapshot
    : emptyWorkspaceUiSnapshot
  const openDirectoryPaths = workspaceUi.openDirectoryPaths
  const openFileTabs = workspaceUi.openFiles
  const activeFilePath = workspaceUi.activeFilePath
  const initialOpenState = useMemo(
    () =>
      Object.fromEntries(openDirectoryPaths.map((relativePath) => [relativePath, true])),
    [openDirectoryPaths],
  )

  useEffect(() => {
    const element = treeHostRef.current
    if (!element) {
      return
    }

    const updateHeight = () => {
      setTreeHeight(Math.max(240, Math.floor(element.getBoundingClientRect().height)))
    }

    updateHeight()
    if (typeof ResizeObserver === "undefined") {
      return
    }

    const observer = new ResizeObserver(updateHeight)
    observer.observe(element)

    return () => {
      observer.disconnect()
    }
  }, [workspace?.rootPath])

  const loadDirectory = useCallback(async (relativePath = "") => {
    setLoadingPaths((current) => new Set(current).add(relativePath))
    try {
      const directory = await workspaceReadDirectory(relativePath)
      const children = directory.entries.map(entryToTreeNode)
      setTreeData((current) =>
        relativePath === "" ? children : insertChildren(current, relativePath, children),
      )
      return directory
    } finally {
      setLoadingPaths((current) => {
        const next = new Set(current)
        next.delete(relativePath)
        return next
      })
    }
  }, [])

  const openWorkspacePath = useCallback(
    async (rootPath: string) => {
      try {
        const nextState = await workspaceOpen(rootPath)
        queryClient.setQueryData(WORKSPACE_STATE_QUERY_KEY, nextState)
        await queryClient.invalidateQueries()
      } catch (error) {
        toast.error(t("pages.workspace.toast.openFailed"), {
          description: getErrorMessage(error),
        })
      }
    },
    [queryClient, t],
  )

  const handleOpenFolder = useCallback(async () => {
    const selected = await open({ directory: true, multiple: false })
    if (typeof selected === "string") {
      await openWorkspacePath(selected)
    }
  }, [openWorkspacePath])

  const handleCloseWorkspace = useCallback(async () => {
    try {
      const nextState = await workspaceClose()
      setTreeData([])
      setSelectedFile(null)
      setFileError(null)
      queryClient.setQueryData(WORKSPACE_STATE_QUERY_KEY, nextState)
      await queryClient.invalidateQueries()
    } catch (error) {
      toast.error(t("pages.workspace.toast.closeFailed"), {
        description: getErrorMessage(error),
      })
    }
  }, [queryClient, t])

  const openFileContent = useCallback(
    async (relativePath: string) => {
      setFileError(null)
      try {
        const file = await workspaceReadFile(relativePath)
        setSelectedFile(file)
        return file
      } catch (error) {
        setSelectedFile(null)
        setFileError(getErrorMessage(error))
        toast.error(t("pages.workspace.toast.fileFailed"), {
          description: getErrorMessage(error),
        })
        return null
      }
    },
    [t],
  )

  const handleOpenFile = useCallback(
    async (relativePath: string) => {
      const file = await openFileContent(relativePath)
      if (!file || !workspace) {
        return
      }

      patchWorkspaceState(workspace.rootPath, {
        activeFilePath: file.relativePath,
        openFiles: upsertFileTab(openFileTabs, {
          relativePath: file.relativePath,
          name: file.name,
        }),
      })
    },
    [openFileContent, openFileTabs, patchWorkspaceState, workspace],
  )

  useEffect(() => {
    if (!workspace) {
      setTreeData([])
      setSelectedFile(null)
      setFileError(null)
      restoredWorkspaceRootRef.current = null
      return
    }

    if (!workspaceUiHasHydrated || restoredWorkspaceRootRef.current === workspace.rootPath) {
      return
    }

    restoredWorkspaceRootRef.current = workspace.rootPath
    setTreeData([])
    setSelectedFile(null)
    setFileError(null)

    const savedOpenDirectoryPaths = sortDirectoryPaths(openDirectoryPaths)
    const savedActiveFilePath = activeFilePath
    const savedFileTabs = openFileTabs

    async function restoreWorkspaceTree() {
      await loadDirectory("")
      await savedOpenDirectoryPaths.reduce<Promise<void>>(
        async (chain, relativePath) => {
          await chain
          try {
            await loadDirectory(relativePath)
          } catch (error) {
            console.warn(`failed to restore workspace directory '${relativePath}'`, error)
          }
        },
        Promise.resolve(),
      )

      if (savedActiveFilePath && savedFileTabs.some((tab) => tab.relativePath === savedActiveFilePath)) {
        await openFileContent(savedActiveFilePath)
      }
    }

    void restoreWorkspaceTree().catch((error) => {
      toast.error(t("pages.workspace.toast.openFailed"), {
        description: getErrorMessage(error),
      })
    })
  }, [
    activeFilePath,
    loadDirectory,
    openDirectoryPaths,
    openFileContent,
    openFileTabs,
    t,
    workspace,
    workspaceUiHasHydrated,
  ])

  const handleTreeToggle = useCallback(
    (relativePath: string) => {
      if (!workspace) {
        return
      }

      const isOpen = openDirectoryPaths.includes(relativePath)
      patchWorkspaceState(workspace.rootPath, {
        openDirectoryPaths: isOpen
          ? openDirectoryPaths.filter((path) => path !== relativePath)
          : sortDirectoryPaths([...openDirectoryPaths, relativePath]),
      })
    },
    [openDirectoryPaths, patchWorkspaceState, workspace],
  )

  const handleCloseFileTab = useCallback(
    async (relativePath: string) => {
      if (!workspace) {
        return
      }

      const tabIndex = openFileTabs.findIndex((tab) => tab.relativePath === relativePath)
      if (tabIndex < 0) {
        return
      }

      const nextTabs = openFileTabs.filter((tab) => tab.relativePath !== relativePath)
      const nextActiveFilePath =
        activeFilePath === relativePath
          ? nextTabs[Math.min(tabIndex, nextTabs.length - 1)]?.relativePath ?? null
          : activeFilePath

      patchWorkspaceState(workspace.rootPath, {
        activeFilePath: nextActiveFilePath,
        openFiles: nextTabs,
      })

      if (activeFilePath !== relativePath) {
        return
      }

      if (nextActiveFilePath) {
        await openFileContent(nextActiveFilePath)
        return
      }

      setSelectedFile(null)
      setFileError(null)
    },
    [activeFilePath, openFileContent, openFileTabs, patchWorkspaceState, workspace],
  )

  const handleSelectFileTab = useCallback(
    async (relativePath: string) => {
      if (!workspace || activeFilePath === relativePath) {
        return
      }

      const file = await openFileContent(relativePath)
      if (!file) {
        return
      }

      patchWorkspaceState(workspace.rootPath, {
        activeFilePath: file.relativePath,
        openFiles: upsertFileTab(openFileTabs, {
          relativePath: file.relativePath,
          name: file.name,
        }),
      })
    },
    [activeFilePath, openFileContent, openFileTabs, patchWorkspaceState, workspace],
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

function RecentWorkspaceList({
  recentWorkspaces,
  onOpen,
  title,
  emptyLabel,
  openLabel,
}: {
  recentWorkspaces: Array<{ rootPath: string; name: string }>
  onOpen: (rootPath: string) => Promise<void>
  title: string
  emptyLabel: string
  openLabel: string
}) {
  return (
    <SoftPanel className="rounded-[18px] px-5 py-5">
      <h3 className="text-sm font-semibold">{title}</h3>
      <div className="mt-4 grid gap-2">
        {recentWorkspaces.length === 0 ? (
          <p className="text-sm text-muted-foreground">{emptyLabel}</p>
        ) : (
          recentWorkspaces.map((workspace) => (
            <div
              key={workspace.rootPath}
              className="flex min-w-0 items-center justify-between gap-3 rounded-[12px] bg-[var(--surface-1)] px-3 py-3"
            >
              <div className="min-w-0">
                <p className="truncate text-sm font-medium">{workspace.name}</p>
                <p className="mt-0.5 truncate text-xs text-muted-foreground">{workspace.rootPath}</p>
              </div>
              <Button variant="pill" size="xs" onClick={() => void onOpen(workspace.rootPath)}>
                {openLabel}
              </Button>
            </div>
          ))
        )}
      </div>
    </SoftPanel>
  )
}

function WorkspaceTreeRow({
  node,
  style,
  selectedPath,
  loadingPaths,
  onOpenDirectory,
  onOpenFile,
}: NodeRendererProps<WorkspaceTreeNode> & {
  selectedPath: string | null
  loadingPaths: Set<string>
  onOpenDirectory: (relativePath: string) => Promise<unknown>
  onOpenFile: (relativePath: string) => Promise<void>
}) {
  const isDirectory = node.data.kind === "directory"
  const selected = selectedPath === node.data.relativePath
  const loading = loadingPaths.has(node.data.relativePath)
  const Icon = isDirectory ? Folder : FileCode2

  return (
    <button
      type="button"
      style={style}
      className={cn(
        "flex w-full min-w-0 items-center gap-1.5 px-2 text-left text-sm outline-none transition hover:bg-[var(--surface-selected)]",
        selected && "bg-[var(--surface-selected)] text-[var(--brand-teal)]",
      )}
      onClick={() => {
        node.select()
        if (isDirectory) {
          if (!node.data.loaded) {
            void onOpenDirectory(node.data.relativePath)
          }
          node.toggle()
          return
        }
        void onOpenFile(node.data.relativePath)
      }}
    >
      <span className="flex size-4 items-center justify-center text-muted-foreground">
        {isDirectory ? (
          loading ? (
            <Loader2 className="size-3.5 animate-spin" />
          ) : node.isOpen ? (
            <ChevronDown className="size-3.5" />
          ) : (
            <ChevronRight className="size-3.5" />
          )
        ) : null}
      </span>
      <Icon className={cn("size-4 shrink-0", isDirectory ? "text-[var(--brand-teal)]" : "text-muted-foreground")} />
      <span className="truncate">{node.data.name}</span>
    </button>
  )
}

function entryToTreeNode(entry: WorkspaceFileEntry): WorkspaceTreeNode {
  return {
    ...entry,
    loaded: entry.kind === "file",
    children: entry.kind === "directory" ? [] : undefined,
  }
}

function insertChildren(
  nodes: WorkspaceTreeNode[],
  relativePath: string,
  children: WorkspaceTreeNode[],
): WorkspaceTreeNode[] {
  return nodes.map((node) => {
    if (node.relativePath === relativePath) {
      return { ...node, children, loaded: true }
    }
    if (!node.children) {
      return node
    }
    return { ...node, children: insertChildren(node.children, relativePath, children) }
  })
}

function languageForFile(fileName: string) {
  const extension = fileName.split(".").pop()?.toLowerCase()
  switch (extension) {
    case "ts":
    case "tsx":
      return "typescript"
    case "js":
    case "jsx":
      return "javascript"
    case "rs":
      return "rust"
    case "json":
      return "json"
    case "md":
      return "markdown"
    case "css":
      return "css"
    case "html":
      return "html"
    case "toml":
      return "toml"
    case "yaml":
    case "yml":
      return "yaml"
    default:
      return "plaintext"
  }
}

function upsertFileTab(tabs: WorkspaceFileTab[], tab: WorkspaceFileTab) {
  if (tabs.some((item) => item.relativePath === tab.relativePath)) {
    return tabs.map((item) => (item.relativePath === tab.relativePath ? tab : item))
  }

  return [...tabs, tab]
}

function sortDirectoryPaths(paths: string[]) {
  return [...new Set(paths)]
    .filter((path) => path.trim().length > 0)
    .toSorted((left, right) => {
      const leftDepth = left.split("/").length
      const rightDepth = right.split("/").length

      if (leftDepth !== rightDepth) {
        return leftDepth - rightDepth
      }

      return left.localeCompare(right)
    })
}

const SLAB_DIR_NAME = ".slab"
