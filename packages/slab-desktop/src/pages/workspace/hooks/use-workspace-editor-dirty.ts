import { useEffect, useState } from "react"

import type { WorkspaceFileContent } from "@/lib/workspace-bridge"

type UseWorkspaceEditorDirtyOptions = {
  workspaceRoot: string | null
  selectedFile: WorkspaceFileContent | null
}

/**
 * Single source of truth for "the active file has unsaved changes".
 *
 * The editor surface edits through the VS Code working-copy service, which owns
 * the dirty signal. We subscribe to it via the workspace-lsp watcher. When no
 * workspace is open (no watcher) there is no editor, so the file is never dirty.
 */
export function useWorkspaceEditorDirty({
  workspaceRoot,
  selectedFile,
}: UseWorkspaceEditorDirtyOptions) {
  const [monacoDirty, setMonacoDirty] = useState(false)

  useEffect(() => {
    if (!workspaceRoot) {
      setMonacoDirty(false)
      return
    }

    let disposed = false
    let disposable: { dispose(): void } | null = null

    void import("../lib/workspace-lsp")
      .then(({ watchWorkspaceVscodeEditorDirty }) =>
        watchWorkspaceVscodeEditorDirty(workspaceRoot, (dirty) => {
          if (!disposed) {
            setMonacoDirty(dirty)
          }
        }),
      )
      .then((next) => {
        if (disposed) {
          next.dispose()
          return
        }
        disposable = next
      })
      .catch((error) => {
        console.debug("workspace VS Code dirty watch unavailable", { workspaceRoot, error })
      })

    return () => {
      disposed = true
      disposable?.dispose()
    }
  }, [workspaceRoot])

  // Reset Monaco dirty when there is no active file so a stale signal cannot leak
  // into the next file's guard.
  useEffect(() => {
    if (!selectedFile) {
      setMonacoDirty(false)
    }
  }, [selectedFile])

  return monacoDirty
}
