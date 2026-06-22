import { useEffect, useState } from "react"

import type { WorkspaceFileContent } from "@/lib/workspace-bridge"

type UseWorkspaceEditorDirtyOptions = {
  workspaceRoot: string | null
  selectedFile: WorkspaceFileContent | null
  editorContent: string
}

/**
 * Single source of truth for "the active file has unsaved changes".
 *
 * The desktop editor surface edits through the VS Code working-copy service, which
 * is invisible to the React `editorContent` state. So we prefer the Monaco/VS Code
 * dirty signal and fall back to the React `editorContent !== selectedFile.content`
 * comparison whenever the VS Code services are unavailable (browser mode, Monaco
 * not yet initialized, or a watcher error). The OR means an unsaved edit on either
 * surface is enough to trigger the discard guards.
 */
export function useWorkspaceEditorDirty({
  workspaceRoot,
  selectedFile,
  editorContent,
}: UseWorkspaceEditorDirtyOptions) {
  const reactDirty = Boolean(selectedFile && editorContent !== selectedFile.content)
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
        // Monaco/VS Code services unavailable (e.g. browser fallback): the React
        // state comparison remains the sole dirty source.
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

  return monacoDirty || reactDirty
}
