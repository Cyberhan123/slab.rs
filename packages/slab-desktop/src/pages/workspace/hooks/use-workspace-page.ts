import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import { open } from "@tauri-apps/plugin-dialog"
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { useTranslation } from "@slab/i18n"
import { FolderKanban } from "lucide-react"
import { toast } from "sonner"

import { usePageHeader } from "@/hooks/use-global-header-meta"
import { isTauri } from "@/hooks/use-tauri"
import {
  workspaceClose,
  workspaceGitCommit,
  workspaceGitDiscard,
  workspaceGitDiff,
  workspaceGitStage,
  workspaceGitStatus,
  workspaceGitUnstage,
  workspaceOpen,
  workspaceReadDirectory,
  workspaceReadFile,
  workspaceSearchFiles,
  workspaceSearchText,
  workspaceState,
  workspaceWriteFile,
  WORKSPACE_STATE_QUERY_KEY,
  type WorkspaceFileContent,
  type WorkspaceGitDiff,
  type WorkspaceGitStatus,
  type WorkspaceGitStatusEntry,
  type WorkspaceTextSearchLineMatch,
} from "@/lib/workspace-bridge"
import {
  emptyWorkspaceUiSnapshot,
  useWorkspaceUiStore,
  type WorkspaceEditorSettings,
  type WorkspaceExplorerPanel,
  type WorkspaceMarkdownMode,
} from "@/store/useWorkspaceUiStore"
import { getErrorMessage } from "@slab/api"
import {
  directoryAncestors,
  entryToTreeNode,
  insertChildren,
  sortDirectoryPaths,
  upsertFileTab,
  type WorkspaceTreeNode,
} from "../lib/workspace-page-utils"

export function useWorkspacePage() {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const isDesktopTauri = isTauri()
  const [treeData, setTreeData] = useState<WorkspaceTreeNode[]>([])
  const [selectedFile, setSelectedFile] = useState<WorkspaceFileContent | null>(null)
  const [editorContent, setEditorContent] = useState("")
  const [fileError, setFileError] = useState<string | null>(null)
  const [textSearchQuery, setTextSearchQuery] = useState("")
  const [selectedGitDiffEntry, setSelectedGitDiffEntry] = useState<WorkspaceGitStatusEntry | null>(null)
  const [editorRevealTarget, setEditorRevealTarget] = useState<{
    relativePath: string
    lineNumber: number
    matchStart: number
    matchEnd: number
  } | null>(null)
  const [loadingPaths, setLoadingPaths] = useState<Set<string>>(new Set())
  const [editorTheme, setEditorTheme] = useState(() =>
    typeof document !== "undefined" && document.documentElement.classList.contains("dark")
      ? "vs-dark"
      : "vs",
  )
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
  const explorerPanel = workspaceUi.explorerPanel
  const markdownMode = workspaceUi.markdownMode
  const consoleOpen = workspaceUi.consoleOpen
  const editorSettings = workspaceUi.editorSettings
  const trimmedTextSearchQuery = textSearchQuery.trim()
  const initialOpenState = useMemo(
    () =>
      Object.fromEntries(openDirectoryPaths.map((relativePath) => [relativePath, true])),
    [openDirectoryPaths],
  )
  const {
    data: gitStatus,
    isFetching: gitStatusFetching,
    refetch: refetchGitStatus,
  } = useQuery({
    queryKey: ["workspace-git-status", workspace?.rootPath],
    queryFn: workspaceGitStatus,
    enabled: isDesktopTauri && Boolean(workspace),
    refetchInterval: 30_000,
    retry: false,
  })
  const {
    data: fileSearchResult,
    isFetching: fileSearchFetching,
  } = useQuery({
    queryKey: ["workspace-file-search", workspace?.rootPath, trimmedTextSearchQuery],
    queryFn: () => workspaceSearchFiles(trimmedTextSearchQuery),
    enabled: isDesktopTauri && Boolean(workspace && trimmedTextSearchQuery),
    retry: false,
  })
  const {
    data: textSearchResult,
    isFetching: textSearchFetching,
  } = useQuery({
    queryKey: ["workspace-text-search", workspace?.rootPath, trimmedTextSearchQuery],
    queryFn: () => workspaceSearchText(trimmedTextSearchQuery),
    enabled: isDesktopTauri && Boolean(workspace && trimmedTextSearchQuery),
    retry: false,
  })
  const visibleGitDiffEntry = useMemo(
    () =>
      gitStatus?.entries.find(
        (entry) =>
          entry.path === selectedGitDiffEntry?.path &&
          entry.staged === selectedGitDiffEntry.staged,
      ) ?? null,
    [gitStatus, selectedGitDiffEntry],
  )
  const {
    data: selectedGitDiff,
    isFetching: gitDiffFetching,
  } = useQuery({
    queryKey: ["workspace-git-diff", workspace?.rootPath, visibleGitDiffEntry?.path, visibleGitDiffEntry?.staged],
    queryFn: () =>
      workspaceGitDiff({
        path: visibleGitDiffEntry?.path ?? "",
        staged: visibleGitDiffEntry?.staged ?? false,
      }),
    enabled: Boolean(gitStatus?.available && gitStatus.isRepository && visibleGitDiffEntry),
    retry: false,
  })
  const saveFileMutation = useMutation({
    mutationFn: workspaceWriteFile,
  })
  const gitStageMutation = useMutation({
    mutationFn: workspaceGitStage,
  })
  const gitUnstageMutation = useMutation({
    mutationFn: workspaceGitUnstage,
  })
  const gitDiscardMutation = useMutation({
    mutationFn: workspaceGitDiscard,
  })
  const gitCommitMutation = useMutation({
    mutationFn: workspaceGitCommit,
  })
  const savingFile = saveFileMutation.isPending
  const gitOperationPending =
    gitStageMutation.isPending ||
    gitUnstageMutation.isPending ||
    gitDiscardMutation.isPending ||
    gitCommitMutation.isPending
  const selectedFileDirty = Boolean(selectedFile && editorContent !== selectedFile.content)

  useEffect(() => {
    if (typeof document === "undefined") {
      return
    }

    const updateEditorTheme = () => {
      setEditorTheme(document.documentElement.classList.contains("dark") ? "vs-dark" : "vs")
    }

    updateEditorTheme()
    const observer = new MutationObserver(updateEditorTheme)
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["class"],
    })

    return () => {
      observer.disconnect()
    }
  }, [])

  useEffect(() => {
    const element = treeHostRef.current
    if (!element) {
      return
    }

    const updateHeight = () => {
      setTreeHeight(Math.max(320, Math.floor(element.getBoundingClientRect().height)))
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
  }, [explorerPanel, workspace?.rootPath])

  useEffect(() => {
    if (explorerPanel !== "files") {
      return
    }

    const frame = window.requestAnimationFrame(() => {
      const element = treeHostRef.current
      if (!element) {
        return
      }

      setTreeHeight(Math.max(320, Math.floor(element.getBoundingClientRect().height)))
    })

    return () => {
      window.cancelAnimationFrame(frame)
    }
  }, [explorerPanel, workspace?.rootPath])

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

  const revealFileInTree = useCallback(
    async (relativePath: string) => {
      if (!workspace) {
        return
      }

      const paths = directoryAncestors(relativePath)
      await paths.reduce<Promise<void>>(async (chain, path) => {
        await chain
        await loadDirectory(path)
      }, Promise.resolve())
      if (paths.length > 0) {
        patchWorkspaceState(workspace.rootPath, {
          openDirectoryPaths: sortDirectoryPaths([...openDirectoryPaths, ...paths]),
        })
      }
    },
    [loadDirectory, openDirectoryPaths, patchWorkspaceState, workspace],
  )

  const revealDirectoryInTree = useCallback(
    async (relativePath: string) => {
      if (!workspace) {
        return
      }

      const paths = directoryAncestors(relativePath, true)
      await paths.reduce<Promise<void>>(async (chain, path) => {
        await chain
        await loadDirectory(path)
      }, Promise.resolve())
      if (paths.length > 0) {
        patchWorkspaceState(workspace.rootPath, {
          openDirectoryPaths: sortDirectoryPaths([...openDirectoryPaths, ...paths]),
        })
      }
    },
    [loadDirectory, openDirectoryPaths, patchWorkspaceState, workspace],
  )

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
      setEditorContent("")
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
        setSelectedGitDiffEntry(null)
        setSelectedFile(file)
        setEditorContent(file.content)
        return file
      } catch (error) {
        setSelectedFile(null)
        setEditorContent("")
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
    async (relativePath: string, options: { revealInTree?: boolean } = {}) => {
      setEditorRevealTarget(null)
      if (selectedFileDirty && !window.confirm(t("pages.workspace.confirm.discardUnsaved"))) {
        return null
      }

      const file = await openFileContent(relativePath)
      if (!file || !workspace) {
        return file
      }

      patchWorkspaceState(workspace.rootPath, {
        activeFilePath: file.relativePath,
        openFiles: upsertFileTab(openFileTabs, {
          relativePath: file.relativePath,
          name: file.name,
        }),
      })
      if (options.revealInTree ?? true) {
        await revealFileInTree(file.relativePath)
      }
      return file
    },
    [
      openFileContent,
      openFileTabs,
      patchWorkspaceState,
      revealFileInTree,
      selectedFileDirty,
      t,
      workspace,
    ],
  )

  const handleOpenTextSearchMatch = useCallback(
    async (relativePath: string, match: WorkspaceTextSearchLineMatch) => {
      const file = await handleOpenFile(relativePath)
      if (!file) {
        return
      }

      setEditorRevealTarget({
        relativePath,
        lineNumber: match.lineNumber,
        matchStart: match.matchStart,
        matchEnd: match.matchEnd,
      })
    },
    [handleOpenFile],
  )

  useEffect(() => {
    if (!workspace) {
      setTreeData([])
      setSelectedFile(null)
      setEditorContent("")
      setFileError(null)
      setTextSearchQuery("")
      setSelectedGitDiffEntry(null)
      setEditorRevealTarget(null)
      restoredWorkspaceRootRef.current = null
      return
    }

    if (!workspaceUiHasHydrated || restoredWorkspaceRootRef.current === workspace.rootPath) {
      return
    }

    restoredWorkspaceRootRef.current = workspace.rootPath
    setTreeData([])
    setSelectedFile(null)
    setEditorContent("")
    setFileError(null)
    setTextSearchQuery("")
    setSelectedGitDiffEntry(null)
    setEditorRevealTarget(null)

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

  const handleSelectExplorerPanel = useCallback(
    (panel: WorkspaceExplorerPanel) => {
      if (!workspace || explorerPanel === panel) {
        return
      }

      patchWorkspaceState(workspace.rootPath, {
        explorerPanel: panel,
      })
    },
    [explorerPanel, patchWorkspaceState, workspace],
  )

  const handleSetMarkdownMode = useCallback(
    (mode: WorkspaceMarkdownMode) => {
      if (!workspace || markdownMode === mode) {
        return
      }

      patchWorkspaceState(workspace.rootPath, {
        markdownMode: mode,
      })
    },
    [markdownMode, patchWorkspaceState, workspace],
  )

  const handleToggleConsole = useCallback(() => {
    if (!workspace) {
      return
    }

    patchWorkspaceState(workspace.rootPath, {
      consoleOpen: !consoleOpen,
    })
  }, [consoleOpen, patchWorkspaceState, workspace])

  const handleUpdateEditorSettings = useCallback(
    (patch: Partial<WorkspaceEditorSettings>) => {
      if (!workspace) {
        return
      }
      patchWorkspaceState(workspace.rootPath, {
        editorSettings: { ...editorSettings, ...patch },
      })
    },
    [editorSettings, patchWorkspaceState, workspace],
  )

  const handleRefreshGitStatus = useCallback(async () => {
    await refetchGitStatus()
  }, [refetchGitStatus])

  const handleSelectGitDiff = useCallback(
    (entry: WorkspaceGitStatusEntry) => {
      if (selectedFileDirty && !window.confirm(t("pages.workspace.confirm.discardUnsaved"))) {
        return
      }

      setSelectedFile(null)
      setEditorContent("")
      setFileError(null)
      setEditorRevealTarget(null)
      setSelectedGitDiffEntry(entry)
    },
    [selectedFileDirty, t],
  )

  const handleSaveFile = useCallback(async () => {
    if (!selectedFile) {
      return
    }

    try {
      const result = await saveFileMutation.mutateAsync({
        relativePath: selectedFile.relativePath,
        content: editorContent,
        expectedHash: selectedFile.contentHash,
      })
      setSelectedFile({
        ...selectedFile,
        content: editorContent,
        contentHash: result.contentHash,
        sizeBytes: result.sizeBytes,
      })
      toast.success(t("pages.workspace.toast.fileSaved"))
      await Promise.all([loadDirectory(""), refetchGitStatus()])
    } catch (error) {
      toast.error(t("pages.workspace.toast.saveFailed"), {
        description: getErrorMessage(error),
      })
    }
  }, [editorContent, loadDirectory, refetchGitStatus, saveFileMutation, selectedFile, t])

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "s") {
        event.preventDefault()
        void handleSaveFile()
      }
    }

    window.addEventListener("keydown", handleKeyDown)
    return () => {
      window.removeEventListener("keydown", handleKeyDown)
    }
  }, [handleSaveFile])

  const applyGitStatus = useCallback(
    (status: WorkspaceGitStatus) => {
      queryClient.setQueryData(["workspace-git-status", workspace?.rootPath], status)
    },
    [queryClient, workspace?.rootPath],
  )

  const handleGitStage = useCallback(
    async (path: string) => {
      try {
        const result = await gitStageMutation.mutateAsync(path)
        applyGitStatus(result.status)
      } catch (error) {
        toast.error(t("pages.workspace.toast.gitFailed"), {
          description: getErrorMessage(error),
        })
      }
    },
    [applyGitStatus, gitStageMutation, t],
  )

  const handleGitUnstage = useCallback(
    async (path: string) => {
      try {
        const result = await gitUnstageMutation.mutateAsync(path)
        applyGitStatus(result.status)
      } catch (error) {
        toast.error(t("pages.workspace.toast.gitFailed"), {
          description: getErrorMessage(error),
        })
      }
    },
    [applyGitStatus, gitUnstageMutation, t],
  )

  const handleGitDiscard = useCallback(
    async (path: string) => {
      try {
        const result = await gitDiscardMutation.mutateAsync(path)
        applyGitStatus(result.status)
        if (selectedGitDiffEntry?.path === path) {
          setSelectedGitDiffEntry(null)
        }
        if (selectedFile?.relativePath === path) {
          await openFileContent(path)
        }
        await loadDirectory("")
      } catch (error) {
        toast.error(t("pages.workspace.toast.gitFailed"), {
          description: getErrorMessage(error),
        })
      }
    },
    [
      applyGitStatus,
      gitDiscardMutation,
      loadDirectory,
      openFileContent,
      selectedFile?.relativePath,
      selectedGitDiffEntry?.path,
      t,
    ],
  )

  const handleGitCommit = useCallback(
    async (message: string) => {
      try {
        const result = await gitCommitMutation.mutateAsync(message)
        applyGitStatus(result.status)
        toast.success(t("pages.workspace.toast.gitCommitted"))
      } catch (error) {
        toast.error(t("pages.workspace.toast.gitFailed"), {
          description: getErrorMessage(error),
        })
      }
    },
    [applyGitStatus, gitCommitMutation, t],
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

      if (
        activeFilePath === relativePath &&
        selectedFileDirty &&
        !window.confirm(t("pages.workspace.confirm.closeUnsaved"))
      ) {
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
      setEditorContent("")
      setFileError(null)
    },
    [activeFilePath, openFileContent, openFileTabs, patchWorkspaceState, selectedFileDirty, t, workspace],
  )

  const handleSelectFileTab = useCallback(
    async (relativePath: string) => {
      if (!workspace || activeFilePath === relativePath) {
        return
      }

      if (selectedFileDirty && !window.confirm(t("pages.workspace.confirm.discardUnsaved"))) {
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
    [activeFilePath, openFileContent, openFileTabs, patchWorkspaceState, selectedFileDirty, t, workspace],
  )

  return {
    activeFilePath,
    consoleOpen,
    editorContent,
    editorRevealTarget:
      selectedFile?.relativePath === editorRevealTarget?.relativePath ? editorRevealTarget : null,
    editorSettings,
    editorTheme,
    explorerPanel,
    fileError,
    fileSearchFetching,
    fileSearchResults: fileSearchResult?.entries ?? [],
    fileSearchTruncated: fileSearchResult?.truncated ?? false,
    gitStatus,
    gitStatusFetching,
    gitOperationPending,
    gitDiffFetching,
    handleCloseFileTab,
    handleCloseWorkspace,
    handleGitCommit,
    handleGitDiscard,
    handleGitStage,
    handleGitUnstage,
    handleOpenFile,
    handleOpenFolder,
    handleOpenTextSearchMatch,
    handleRefreshGitStatus,
    handleRevealDirectoryInTree: revealDirectoryInTree,
    handleSaveFile,
    handleSelectExplorerPanel,
    handleSelectFileTab,
    handleSelectGitDiff,
    handleSetMarkdownMode,
    handleTreeToggle,
    handleToggleConsole,
    handleUpdateEditorSettings,
    initialOpenState,
    isDesktopTauri,
    loadDirectory,
    loadingPaths,
    markdownMode,
    openFileTabs,
    openWorkspacePath,
    recentWorkspaces,
    selectedGitDiff: visibleGitDiffEntry
      ? selectedGitDiff ?? ({
          path: visibleGitDiffEntry.path,
          staged: visibleGitDiffEntry.staged,
          diff: "",
        } satisfies WorkspaceGitDiff)
      : null,
    selectedGitDiffEntry: visibleGitDiffEntry,
    selectedFile,
    selectedFileDirty,
    setEditorContent,
    savingFile,
    setTextSearchQuery,
    textSearchFetching,
    textSearchQuery,
    textSearchResults: textSearchResult?.matches ?? [],
    textSearchTruncated: textSearchResult?.truncated ?? false,
    treeData,
    treeHeight,
    treeHostRef,
    workspace,
    workspaceUiHasHydrated,
  }
}

export type WorkspacePageState = ReturnType<typeof useWorkspacePage>
