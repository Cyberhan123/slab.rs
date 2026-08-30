import { useCallback, useState } from "react"
import { useQueryClient } from "@tanstack/react-query"
import { toast } from "sonner"

import { useTranslation } from "@slab/i18n"
import {
  WORKSPACE_STATE_QUERY_KEY,
  workspaceClose,
  workspaceOpen,
  type WorkspaceStateResponse,
} from "@slab/core/workspace/bridge"
import { useWorkspaceUiStore } from "@slab/ui/store/useWorkspaceUiStore"

/** Where a chat should run: a workspace root, or global (no workspace). */
export type WorkspaceSelection =
  | { kind: "root"; rootPath: string; name?: string }
  | { kind: "global" }

/** Loose root-path equality (case-insensitive, separator-normalized). */
export function isSameRoot(left: string | null | undefined, right: string | null | undefined) {
  if (!left || !right) return false
  const normalize = (value: string) => value.replace(/\\/g, "/").replace(/\/+$/, "").toLowerCase()
  return normalize(left) === normalize(right)
}

function getErrorMessage(error: unknown): string {
  if (error instanceof Error && error.message) return error.message
  return String(error)
}

/**
 * Apply a workspace selection to the server (open / close / no-op) and refresh
 * the workspace-state cache. Shared by the new-chat dialog submit and the
 * Sender's live workspace dropdown so both switch through the exact same path:
 *
 * - Opening a *different* root makes the server interrupt + snapshot the
 *   originating workspace's agent threads (the `migrated` summary is toasted).
 * - `global` closes the workspace — the honest "no workspace" state; the
 *   workspace stays in the client-side recent list for reopening.
 */
export function useWorkspaceSwitch() {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const rememberRecentWorkspace = useWorkspaceUiStore((state) => state.rememberRecentWorkspace)
  const [switching, setSwitching] = useState(false)

  const refreshCache = useCallback(
    async (result: WorkspaceStateResponse) => {
      try {
        queryClient.setQueryData(WORKSPACE_STATE_QUERY_KEY, result)
        await queryClient.invalidateQueries({ queryKey: WORKSPACE_STATE_QUERY_KEY })
      } catch (error) {
        console.warn("workspace state refresh failed after switch", error)
        await queryClient
          .invalidateQueries({ queryKey: WORKSPACE_STATE_QUERY_KEY })
          .catch((refreshError) => {
            console.warn("workspace state invalidation failed after switch", refreshError)
          })
      }
    },
    [queryClient],
  )

  /**
   * @returns `true` when the active workspace actually changed on the server.
   * @throws when the open/close request fails (caller decides how to surface).
   */
  const applyWorkspace = useCallback(
    async (selection: WorkspaceSelection, currentRoot: string | null | undefined) => {
      if (selection.kind === "root" && isSameRoot(currentRoot, selection.rootPath)) {
        return false
      }
      if (selection.kind === "global" && !currentRoot) {
        return false
      }

      setSwitching(true)
      try {
        const result =
          selection.kind === "root"
            ? await workspaceOpen(selection.rootPath)
            : await workspaceClose()
        if (result.current) {
          rememberRecentWorkspace({
            name: result.current.name,
            rootPath: result.current.rootPath,
          })
        }
        await refreshCache(result)

        const description = result.migrated
          ? t("pages.workspace.projectSwitcher.suspended", {
              count: result.migrated.suspendedCount,
            })
          : undefined
        if (selection.kind === "global") {
          toast.success(t("pages.assistant.newChat.switchedGlobal"), { description })
        } else {
          toast.success(t("pages.workspace.projectSwitcher.switched"), { description })
        }
        return true
      } catch (error) {
        const key =
          selection.kind === "global"
            ? "pages.workspace.toast.closeFailed"
            : "pages.workspace.toast.openFailed"
        toast.error(t(key), { description: getErrorMessage(error) })
        throw error
      } finally {
        setSwitching(false)
      }
    },
    [refreshCache, rememberRecentWorkspace, t],
  )

  return { applyWorkspace, switching }
}
