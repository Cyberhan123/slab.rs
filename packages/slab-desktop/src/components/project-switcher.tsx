import { useCallback, useEffect, useRef, useState } from "react"
import { useQueryClient } from "@tanstack/react-query"
import { ChevronDown, Folder } from "lucide-react"
import { toast } from "sonner"

import { useTranslation } from "@slab/i18n"
import { WORKSPACE_STATE_QUERY_KEY, workspaceOpen } from "@slab/core/workspace/bridge"
import { useWorkspaceUiStore } from "@/store/useWorkspaceUiStore"

type RecentWorkspaceView = {
  rootPath: string
  name: string
}

type ProjectSwitcherProps = {
  activeName?: string
  disabled?: boolean
  labels: { toggle: string; noActive: string }
  recentWorkspaces: RecentWorkspaceView[]
  onSwitch: (rootPath: string) => void | Promise<void>
}

/**
 * Dropdown that lists recent workspaces and switches the active one. Switching
 * goes through the server's `POST /v1/workspace/open` (see `workspaceOpen`),
 * which atomically interrupts + snapshots the originating workspace's agent
 * threads before switching the active root.
 *
 * Presentational (props-driven) so it is straightforward to test; the default
 * export {@link ProjectSwitcher} wires it to the workspace UI store + the
 * server-side open.
 */
export function ProjectSwitcherView({
  activeName,
  disabled,
  labels,
  recentWorkspaces,
  onSwitch,
}: ProjectSwitcherProps) {
  const [open, setOpen] = useState(false)
  const containerRef = useRef<HTMLDivElement | null>(null)

  useEffect(() => {
    if (!open) {
      return
    }
    const handler = (event: MouseEvent) => {
      if (!containerRef.current?.contains(event.target as Node)) {
        setOpen(false)
      }
    }
    document.addEventListener("mousedown", handler)
    return () => document.removeEventListener("mousedown", handler)
  }, [open])

  return (
    <div ref={containerRef} className="relative inline-block" data-testid="project-switcher">
      <button
        type="button"
        className="inline-flex items-center gap-1 rounded px-2 py-1 text-sm hover:bg-muted disabled:opacity-50"
        disabled={disabled}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-label={labels.toggle}
        onClick={() => setOpen((value) => !value)}
      >
        <Folder className="h-4 w-4" />
        <span className="max-w-[12rem] truncate">{activeName ?? labels.noActive}</span>
        <ChevronDown className="h-3 w-3" />
      </button>
      {open && recentWorkspaces.length > 0 && (
        <ul
          data-testid="project-switcher-list"
          className="absolute z-50 mt-1 max-h-80 w-72 overflow-auto rounded border bg-background shadow-lg"
        >
          {recentWorkspaces.map((workspace) => (
            <li key={workspace.rootPath}>
              <button
                type="button"
                disabled={disabled}
                aria-label={workspace.name}
                data-testid={`project-switcher-item-${workspace.rootPath}`}
                className="flex w-full flex-col items-start gap-0.5 px-3 py-2 text-left hover:bg-muted disabled:opacity-50"
                onClick={() => {
                  onSwitch(workspace.rootPath)
                  setOpen(false)
                }}
              >
                <span className="truncate text-sm font-medium">{workspace.name}</span>
                <span className="truncate text-xs opacity-60">{workspace.rootPath}</span>
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}

/** Wired ProjectSwitcher: reads recent workspaces + switches via the server open. */
export function ProjectSwitcher({ activeName }: { activeName?: string }) {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const recentWorkspaces = useWorkspaceUiStore((state) => state.recentWorkspaces)
  const [switching, setSwitching] = useState(false)

  const handleSwitch = useCallback(async (rootPath: string) => {
    setSwitching(true)
    try {
      // Server-side open atomically interrupts + snapshots the originating
      // workspace's agent threads, then switches the active root. The response
      // is the fresh workspace state, used to update the cache directly.
      const result = await workspaceOpen(rootPath)
      try {
        queryClient.setQueryData(WORKSPACE_STATE_QUERY_KEY, result)
        await queryClient.invalidateQueries({ queryKey: WORKSPACE_STATE_QUERY_KEY })
      } catch (error) {
        console.warn("workspace state refresh failed after switch", error)
        await queryClient.invalidateQueries({ queryKey: WORKSPACE_STATE_QUERY_KEY }).catch((refreshError) => {
          console.warn("workspace state invalidation failed after switch", refreshError)
        })
      }
      const description = result.migrated
        ? t("pages.workspace.projectSwitcher.suspended", { count: result.migrated.suspendedCount })
        : undefined
      toast.success(t("pages.workspace.projectSwitcher.switched"), { description })
    } catch (error) {
      // Surfaced in the UI by the workspace state subscription; keep switching.
      console.warn("workspace switch failed", error)
    } finally {
      setSwitching(false)
    }
  }, [queryClient, t])

  return (
    <ProjectSwitcherView
      activeName={activeName}
      disabled={switching}
      labels={{
        toggle: t("pages.workspace.projectSwitcher.toggle"),
        noActive: t("pages.workspace.projectSwitcher.noActive"),
      }}
      recentWorkspaces={recentWorkspaces}
      onSwitch={handleSwitch}
    />
  )
}
