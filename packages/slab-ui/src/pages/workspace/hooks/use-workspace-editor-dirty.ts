import { useEffect, useState } from "react"

import type { WorkspaceFileContent } from "@slab/core/workspace/bridge"

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

  // Reset the dirty flag while there is no workspace / no active file so a
  // stale signal cannot leak into the next file's guard (React-docs
  // adjust-state pattern instead of setState-in-effects).
  if ((!workspaceRoot || !selectedFile) && monacoDirty) {
    setMonacoDirty(false)
  }

  useEffect(() => {
    if (!workspaceRoot) {
      return
    }

    let disposed = false
    let disposable: { dispose(): void } | null = null

    void import("../lib/workspace-editor")
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

  return monacoDirty
}
