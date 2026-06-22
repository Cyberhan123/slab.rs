import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import { open } from "@tauri-apps/plugin-dialog"
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { useLocation, useNavigate } from "react-router-dom"
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
  workspaceReadFile,
  workspaceSearchFiles,
  workspaceSearchText,
  workspaceState,
  workspaceStatPath,
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
import { useAssistantDraftStore } from "@/store/useAssistantDraftStore"
import { getErrorMessage } from "@slab/api"
import {
  getWorkspaceThemeMode,
  type WorkspaceThemeMode,
} from "../lib/monaco-theme"
import { upsertFileTab } from "../lib/workspace-page-utils"
import type { WorkspaceLspOpenFileOptions } from "../lib/workspace-lsp-utils"
import { useWorkspaceEditorDirty } from "./use-workspace-editor-dirty"
import { useWorkspaceConfirmDialog } from "./use-workspace-confirm"

type WorkspaceOpenFileOptions = WorkspaceLspOpenFileOptions & {
  revealInTree?: boolean
}

type WorkspaceEditorSelection = {
  endColumn: number
  endLineNumber: number
  relativePath: string
  startColumn: number
  startLineNumber: number
  text: string
}

const MAX_WORKSPACE_PREVIEW_BYTES = 1024 * 1024

export function useWorkspacePage() {
  const { t } = useTranslation()
  const navigate = useNavigate()
  const location = useLocation()
  const queryClient = useQueryClient()
  const isDesktopTauri = isTauri()
  const [selectedFile, setSelectedFile] = useState<WorkspaceFileContent | null>(null)
  const [editorContent, setEditorContent] = useState("")
  const [fileError, setFileError] = useState<string | null>(null)
  const [browserEditorSelection, setBrowserEditorSelection] = useState<WorkspaceEditorSelection | null>(null)
  const [textSearchQuery, setTextSearchQuery] = useState("")
  const [selectedGitDiffEntry, setSelectedGitDiffEntry] = useState<WorkspaceGitStatusEntry | null>(null)
  const [editorRevealTarget, setEditorRevealTarget] = useState<{
    relativePath: string
    lineNumber: number
    matchStart: number
    matchEnd: number
  } | null>(null)
  const [editorThemeMode, setEditorThemeMode] = useState<WorkspaceThemeMode>(getWorkspaceThemeMode)
  const restoredWorkspaceRootRef = useRef<string | null>(null)
  const activeVscodeFileGenerationRef = useRef(0)
  const consumedRevealPathRef = useRef<string | null>(null)

  usePageHeader({
    icon: FolderKanban,
    title: t("pages.workspace.header.title"),
    subtitle: t("pages.workspace.header.subtitle"),
  })

  const workspaceQuery = useQuery({
    queryKey: WORKSPACE_STATE_QUERY_KEY,
    queryFn: workspaceState,
    // Workspace state comes from the Tauri bridge, not /v1 HTTP. The bridge has
    // its own recovery path, so React Query retry would duplicate local probes.
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
  const openFileTabs = workspaceUi.openFiles
  const activeFilePath = workspaceUi.activeFilePath
  const explorerPanel = workspaceUi.explorerPanel
  const markdownMode = workspaceUi.markdownMode
  const consoleOpen = workspaceUi.consoleOpen
  const editorSettings = workspaceUi.editorSettings
  const trimmedTextSearchQuery = textSearchQuery.trim()
  const {
    data: gitStatus,
    isFetching: gitStatusFetching,
    refetch: refetchGitStatus,
  } = useQuery({
    queryKey: ["workspace-git-status", workspace?.rootPath],
    queryFn: workspaceGitStatus,
    enabled: Boolean(workspace),
    refetchInterval: 30_000,
    // Git status already polls on an interval while a workspace is open.
    retry: false,
  })
  const {
    data: fileSearchResult,
    isFetching: fileSearchFetching,
  } = useQuery({
    queryKey: ["workspace-file-search", workspace?.rootPath, trimmedTextSearchQuery],
    queryFn: () => workspaceSearchFiles(trimmedTextSearchQuery),
    enabled: Boolean(workspace && trimmedTextSearchQuery),
    // User search input changes quickly; failed bridge searches should wait for
    // the next typed query instead of retrying stale text.
    retry: false,
  })
  const {
    data: textSearchResult,
    isFetching: textSearchFetching,
  } = useQuery({
    queryKey: ["workspace-text-search", workspace?.rootPath, trimmedTextSearchQuery],
    queryFn: () => workspaceSearchText(trimmedTextSearchQuery),
    enabled: Boolean(workspace && trimmedTextSearchQuery),
    // User search input changes quickly; failed bridge searches should wait for
    // the next typed query instead of retrying stale text.
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
    // Diff requests are tied to the selected row; a new selection should drive
    // the next fetch rather than retrying an obsolete local bridge request.
    retry: false,
  })
  const saveFileMutation = useMutation({
    meta: {
      skipGlobalErrorToast: true,
    },
    mutationFn: workspaceWriteFile,
  })
  const gitStageMutation = useMutation({
    meta: {
      skipGlobalErrorToast: true,
    },
    mutationFn: workspaceGitStage,
  })
  const gitUnstageMutation = useMutation({
    meta: {
      skipGlobalErrorToast: true,
    },
    mutationFn: workspaceGitUnstage,
  })
  const gitDiscardMutation = useMutation({
    meta: {
      skipGlobalErrorToast: true,
    },
    mutationFn: workspaceGitDiscard,
  })
  const gitCommitMutation = useMutation({
    meta: {
      skipGlobalErrorToast: true,
    },
    mutationFn: workspaceGitCommit,
  })
  const savingFile = saveFileMutation.isPending
  const gitOperationPending =
    gitStageMutation.isPending ||
    gitUnstageMutation.isPending ||
    gitDiscardMutation.isPending ||
    gitCommitMutation.isPending
  const selectedFileDirty = useWorkspaceEditorDirty({
    workspaceRoot: isDesktopTauri ? workspace?.rootPath ?? null : null,
    selectedFile,
    editorContent,
  })
  const { confirm: confirmDiscardUnsaved, dialog: confirmDiscardDialog } = useWorkspaceConfirmDialog()

  useEffect(() => {
    if (typeof document === "undefined") {
      return
    }

    const updateEditorTheme = () => {
      setEditorThemeMode(getWorkspaceThemeMode())
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
    if (!isDesktopTauri) {
      return
    }

    const selected = await open({ directory: true, multiple: false })
    if (typeof selected === "string") {
      await openWorkspacePath(selected)
    }
  }, [isDesktopTauri, openWorkspacePath])

  const handleCloseWorkspace = useCallback(async () => {
    try {
      const nextState = await workspaceClose()
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
        const metadata = await workspaceStatPath(relativePath)
        if (metadata.sizeBytes > MAX_WORKSPACE_PREVIEW_BYTES) {
          const message = t("pages.workspace.editor.fileTooLarge", {
            limit: "1 MiB",
            size: `${Math.ceil(metadata.sizeBytes / 1024)} KiB`,
          })
          setSelectedFile(null)
          setEditorContent("")
          setFileError(message)
          toast.error(t("pages.workspace.toast.fileFailed"), {
            description: message,
          })
          return null
        }
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

  const revealActiveFileInExplorer = useCallback(
    async (relativePath: string) => {
      if (!workspace) {
        return
      }

      patchWorkspaceState(workspace.rootPath, {
        explorerPanel: "files",
      })
      if (!isDesktopTauri) {
        return
      }
      const { runWorkspaceVscodeCommand } = await import("../lib/workspace-lsp")
      await runWorkspaceVscodeCommand("workbench.files.action.showActiveFileInExplorer", workspace.rootPath).catch(
        (error) => {
          console.debug("workspace VS Code reveal command failed", { relativePath, error })
        },
      )
    },
    [isDesktopTauri, patchWorkspaceState, workspace],
  )

  const handleOpenFile = useCallback(
    async (relativePath: string, options: WorkspaceOpenFileOptions = {}) => {
      const { revealInTree = false, ...editorOptions } = options
      setEditorRevealTarget(null)
      if (
        selectedFileDirty &&
        !(await confirmDiscardUnsaved({
          messageKey: "pages.workspace.confirm.discardUnsaved",
          confirmKey: "pages.workspace.confirm.discard",
          tone: "danger",
        }))
      ) {
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
      if (isDesktopTauri) {
        try {
          const { openWorkspaceVscodeFile } = await import("../lib/workspace-lsp")
          await openWorkspaceVscodeFile({
            options: editorOptions,
            relativePath: file.relativePath,
            workspaceRoot: workspace.rootPath,
          })
        } catch (error) {
          toast.error(t("pages.workspace.toast.fileFailed"), {
            description: getErrorMessage(error),
          })
        }
      }
      if (editorOptions.startLineNumber && editorOptions.startColumn) {
        setEditorRevealTarget({
          relativePath: file.relativePath,
          lineNumber: editorOptions.startLineNumber,
          matchStart: editorOptions.startColumn - 1,
          matchEnd: editorOptions.endColumn ? editorOptions.endColumn - 1 : editorOptions.startColumn - 1,
        })
      }
      if (revealInTree) {
        await revealActiveFileInExplorer(file.relativePath)
      }
      return file
    },
    [
      confirmDiscardUnsaved,
      openFileContent,
      openFileTabs,
      isDesktopTauri,
      patchWorkspaceState,
      revealActiveFileInExplorer,
      selectedFileDirty,
      t,
      workspace,
    ],
  )

  const handleOpenTextSearchMatch = useCallback(
    async (relativePath: string, match: WorkspaceTextSearchLineMatch) => {
      const file = await handleOpenFile(relativePath, {
        endColumn: match.matchEnd + 1,
        endLineNumber: match.lineNumber,
        startColumn: match.matchStart + 1,
        startLineNumber: match.lineNumber,
      })
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
    setSelectedFile(null)
    setEditorContent("")
    setFileError(null)
    setTextSearchQuery("")
    setSelectedGitDiffEntry(null)
    setEditorRevealTarget(null)

    const savedActiveFilePath = activeFilePath
    const savedFileTabs = openFileTabs
    const workspaceRoot = workspace.rootPath

    async function restoreWorkspaceEditor() {
      if (!savedActiveFilePath || !savedFileTabs.some((tab) => tab.relativePath === savedActiveFilePath)) {
        return
      }

      const file = await openFileContent(savedActiveFilePath)
      if (file && isDesktopTauri) {
        const { openWorkspaceVscodeFile } = await import("../lib/workspace-lsp")
        await openWorkspaceVscodeFile({
          relativePath: savedActiveFilePath,
          workspaceRoot,
        })
      }
    }

    void restoreWorkspaceEditor().catch((error) => {
      toast.error(t("pages.workspace.toast.openFailed"), {
        description: getErrorMessage(error),
      })
    })
  }, [
    activeFilePath,
    isDesktopTauri,
    openFileContent,
    openFileTabs,
    t,
    workspace,
    workspaceUiHasHydrated,
  ])

  useEffect(() => {
    if (!workspace || !isDesktopTauri) {
      return
    }

    let cancelled = false
    let disposable: { dispose(): void } | null = null
    const workspaceRoot = workspace.rootPath

    void import("../lib/workspace-lsp").then(({ watchWorkspaceVscodeEditorState }) =>
      watchWorkspaceVscodeEditorState(workspaceRoot, ({ activeRelativePath, openFiles }) => {
      if (cancelled) {
        return
      }

      activeVscodeFileGenerationRef.current += 1
      const generation = activeVscodeFileGenerationRef.current

      const snapshot = useWorkspaceUiStore.getState().workspaces[workspaceRoot] ?? emptyWorkspaceUiSnapshot
      const openFilesChanged =
        snapshot.openFiles.length !== openFiles.length ||
        snapshot.openFiles.some((tab, index) => {
          const nextTab = openFiles[index]
          return !nextTab || tab.relativePath !== nextTab.relativePath || tab.name !== nextTab.name
        })

      if (snapshot.activeFilePath !== activeRelativePath || openFilesChanged) {
        patchWorkspaceState(workspaceRoot, {
          activeFilePath: activeRelativePath,
          openFiles,
        })
      }

      if (!activeRelativePath) {
        setSelectedFile(null)
        setEditorContent("")
        return
      }

      setSelectedGitDiffEntry(null)
      setEditorRevealTarget(null)
      setFileError(null)

      void workspaceReadFile(activeRelativePath)
        .then((file) => {
          if (cancelled || generation !== activeVscodeFileGenerationRef.current) {
            return
          }

          setSelectedFile(file)
          setEditorContent(file.content)
        })
        .catch((error) => {
          if (cancelled || generation !== activeVscodeFileGenerationRef.current) {
            return
          }

          setSelectedFile(null)
          setEditorContent("")
          setFileError(getErrorMessage(error))
        })
    })
    )
      .then((nextDisposable) => {
        if (cancelled) {
          nextDisposable.dispose()
          return
        }

        disposable = nextDisposable
      })
      .catch((error) => {
        console.debug("workspace VS Code active editor watch unavailable", { workspaceRoot, error })
      })

    return () => {
      cancelled = true
      activeVscodeFileGenerationRef.current += 1
      disposable?.dispose()
    }
  }, [isDesktopTauri, patchWorkspaceState, workspace])

  useEffect(() => {
    const workspaceRoot = workspace?.rootPath
    if (!workspaceRoot || !isDesktopTauri) {
      return
    }

    let cancelled = false
    let disposable: { dispose(): void } | null = null

    void import("../lib/workspace-lsp").then(({ watchWorkspaceVscodeEditorCloseRequests }) =>
      watchWorkspaceVscodeEditorCloseRequests(workspaceRoot, async () => {
      if (cancelled) {
        return false
      }
      return confirmDiscardUnsaved({
        messageKey: "pages.workspace.confirm.closeUnsaved",
        confirmKey: "pages.workspace.confirm.closeAnyway",
        tone: "danger",
      })
    })
    )
      .then((nextDisposable) => {
        if (cancelled) {
          nextDisposable.dispose()
          return
        }

        disposable = nextDisposable
      })
      .catch((error) => {
        console.debug("workspace VS Code close guard unavailable", { workspaceRoot, error })
      })

    return () => {
      cancelled = true
      disposable?.dispose()
    }
  }, [confirmDiscardUnsaved, isDesktopTauri, workspace?.rootPath])

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

  useEffect(() => {
    const revealPath = (location.state as { workspaceRevealPath?: unknown } | null)?.workspaceRevealPath
    if (typeof revealPath !== "string" || !revealPath.trim() || consumedRevealPathRef.current === revealPath) {
      return
    }

    consumedRevealPathRef.current = revealPath
    const trimmedRevealPath = revealPath.trim()
    const currentRelativePath = workspace?.rootPath
      ? relativePathFromRoot(trimmedRevealPath, workspace.rootPath)
      : null

    if (currentRelativePath && workspace) {
      patchWorkspaceState(workspace.rootPath, {
        activeFilePath: currentRelativePath,
        explorerPanel: "files",
      })
      void revealActiveFileInExplorer(currentRelativePath)
      return
    }

    const parentDirectory = parentDirectoryPath(trimmedRevealPath)
    const fileName = fileNameFromPath(trimmedRevealPath)
    if (!parentDirectory || !fileName) {
      return
    }

    void (async () => {
      await openWorkspacePath(parentDirectory)
      patchWorkspaceState(parentDirectory, {
        activeFilePath: fileName,
        explorerPanel: "files",
      })
    })()
  }, [location.state, openWorkspacePath, patchWorkspaceState, revealActiveFileInExplorer, workspace])

  const handleExplainWithAssistant = useCallback(async () => {
    if (!selectedFile) {
      return
    }

    const vscodeSelection = isDesktopTauri && workspace
      ? await import("../lib/workspace-lsp").then(({ getWorkspaceVscodeSelection }) =>
          getWorkspaceVscodeSelection(workspace.rootPath),
        ).catch((error) => {
          console.debug("workspace VS Code selection lookup failed", { error })
          return null
        })
      : null
    const selectedText = vscodeSelection?.text.trim()
      ? vscodeSelection
      : browserEditorSelection?.text.trim()
        ? browserEditorSelection
        : null
    const relativePath = selectedText?.relativePath ?? selectedFile.relativePath
    const language = relativePath.split(".").pop() ?? "text"
    const content = selectedText?.text ?? (editorContent || selectedFile.content)
    const excerpt = content.length > 12_000 ? `${content.slice(0, 12_000)}\n\n...` : content
    const locationLabel = selectedText
      ? `${relativePath}:${selectedText.startLineNumber}-${selectedText.endLineNumber}`
      : relativePath
    useAssistantDraftStore.getState().setDraft({
      autoSubmit: false,
      prompt: [
        `Explain this code from ${locationLabel}.`,
        "",
        `\`\`\`${language}`,
        excerpt,
        "```",
      ].join("\n"),
      source: {
        label: selectedFile.name,
        path: relativePath,
      },
    })
    navigate("/assistant")
  }, [browserEditorSelection, editorContent, isDesktopTauri, navigate, selectedFile, workspace])

  useEffect(() => {
    if (!workspace) {
      return
    }

    if (!isDesktopTauri) {
      return
    }

    void import("../lib/workspace-lsp").then(({ applyWorkspaceEditorSettings }) =>
      applyWorkspaceEditorSettings(editorSettings, workspace.rootPath),
    ).catch((error) => {
      console.debug("workspace editor settings sync failed", { error })
    })
  }, [editorSettings, isDesktopTauri, workspace])

  const handleRefreshGitStatus = useCallback(async () => {
    await refetchGitStatus()
  }, [refetchGitStatus])

  const handleSelectGitDiff = useCallback(
    async (entry: WorkspaceGitStatusEntry) => {
      if (
        selectedFileDirty &&
        !(await confirmDiscardUnsaved({
          messageKey: "pages.workspace.confirm.discardUnsaved",
          confirmKey: "pages.workspace.confirm.discard",
          tone: "danger",
        }))
      ) {
        return
      }

      setSelectedFile(null)
      setEditorContent("")
      setFileError(null)
      setEditorRevealTarget(null)
      setSelectedGitDiffEntry(entry)
    },
    [confirmDiscardUnsaved, selectedFileDirty],
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
      await refetchGitStatus()
    } catch (error) {
      toast.error(t("pages.workspace.toast.saveFailed"), {
        description: getErrorMessage(error),
      })
    }
  }, [editorContent, refetchGitStatus, saveFileMutation, selectedFile, t])

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
      } catch (error) {
        toast.error(t("pages.workspace.toast.gitFailed"), {
          description: getErrorMessage(error),
        })
      }
    },
    [
      applyGitStatus,
      gitDiscardMutation,
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
        !(await confirmDiscardUnsaved({
          messageKey: "pages.workspace.confirm.closeUnsaved",
          confirmKey: "pages.workspace.confirm.closeAnyway",
          tone: "danger",
        }))
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
        const file = await openFileContent(nextActiveFilePath)
        if (file && isDesktopTauri) {
          const { openWorkspaceVscodeFile } = await import("../lib/workspace-lsp")
          await openWorkspaceVscodeFile({
            relativePath: nextActiveFilePath,
            workspaceRoot: workspace.rootPath,
          })
        }
        return
      }

      setSelectedFile(null)
      setEditorContent("")
      setFileError(null)
    },
    [activeFilePath, confirmDiscardUnsaved, isDesktopTauri, openFileContent, openFileTabs, patchWorkspaceState, selectedFileDirty, workspace],
  )

  const handleSelectFileTab = useCallback(
    async (relativePath: string) => {
      if (!workspace || activeFilePath === relativePath) {
        return
      }

      if (
        selectedFileDirty &&
        !(await confirmDiscardUnsaved({
          messageKey: "pages.workspace.confirm.discardUnsaved",
          confirmKey: "pages.workspace.confirm.discard",
          tone: "danger",
        }))
      ) {
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
      if (isDesktopTauri) {
        const { openWorkspaceVscodeFile } = await import("../lib/workspace-lsp")
        await openWorkspaceVscodeFile({
          relativePath: file.relativePath,
          workspaceRoot: workspace.rootPath,
        })
      }
    },
    [activeFilePath, confirmDiscardUnsaved, isDesktopTauri, openFileContent, openFileTabs, patchWorkspaceState, selectedFileDirty, workspace],
  )

  return {
    activeFilePath,
    confirmDiscardDialog,
    consoleOpen,
    editorContent,
    editorRevealTarget:
      selectedFile?.relativePath === editorRevealTarget?.relativePath ? editorRevealTarget : null,
    editorSettings,
    editorThemeMode,
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
    handleRevealDirectoryInTree: revealActiveFileInExplorer,
    handleSaveFile,
    handleSelectExplorerPanel,
    handleSelectFileTab,
    handleSelectGitDiff,
    handleSetMarkdownMode,
    handleToggleConsole,
    handleUpdateEditorSettings,
    handleExplainWithAssistant,
    isDesktopTauri,
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
    setBrowserEditorSelection,
    savingFile,
    setTextSearchQuery,
    textSearchFetching,
    textSearchQuery,
    textSearchResults: textSearchResult?.matches ?? [],
    textSearchTruncated: textSearchResult?.truncated ?? false,
    workspace,
    workspaceUiHasHydrated,
  }
}

export type WorkspacePageState = ReturnType<typeof useWorkspacePage>

function fileNameFromPath(path: string) {
  return path.match(/[^/\\]+$/)?.[0] ?? ""
}

function parentDirectoryPath(path: string) {
  const normalized = path.trim()
  const separatorIndex = Math.max(normalized.lastIndexOf("\\"), normalized.lastIndexOf("/"))
  return separatorIndex > 0 ? normalized.slice(0, separatorIndex) : null
}

function relativePathFromRoot(path: string, rootPath: string) {
  const comparablePath = normalizeFsPathForCompare(path)
  const comparableRoot = normalizeFsPathForCompare(rootPath)
  if (comparablePath === comparableRoot || !comparablePath.startsWith(`${comparableRoot}/`)) {
    return null
  }

  return path.replaceAll("\\", "/").replace(/\/+$/, "").slice(comparableRoot.length + 1)
}

function normalizeFsPathForCompare(path: string) {
  return path.replaceAll("\\", "/").replace(/\/+$/, "").toLowerCase()
}
