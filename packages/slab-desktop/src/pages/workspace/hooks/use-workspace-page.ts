import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import { open } from "@tauri-apps/plugin-dialog"
import { useQuery, useQueryClient } from "@tanstack/react-query"
import { useTranslation } from "@slab/i18n"
import { FolderKanban } from "lucide-react"
import { toast } from "sonner"

import { usePageHeader } from "@/hooks/use-global-header-meta"
import { isTauri } from "@/hooks/use-tauri"
import {
  workspaceClose,
  workspaceOpen,
  workspaceReadDirectory,
  workspaceReadFile,
  workspaceState,
  WORKSPACE_STATE_QUERY_KEY,
  type WorkspaceFileContent,
} from "@/lib/workspace-bridge"
import {
  emptyWorkspaceUiSnapshot,
  useWorkspaceUiStore,
} from "@/store/useWorkspaceUiStore"
import { getErrorMessage } from "@slab/api"
import {
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

  return {
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
  }
}

export type WorkspacePageState = ReturnType<typeof useWorkspacePage>
