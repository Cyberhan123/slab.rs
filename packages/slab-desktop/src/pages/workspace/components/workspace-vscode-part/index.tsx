import { useEffect, useRef, useState } from "react"
import { Loader2 } from "lucide-react"
import * as Monaco from "monaco-editor"

import { cn } from "@/lib/utils"
import type { WorkspaceEditorSettings } from "@/store/useWorkspaceUiStore"
import {
  applySlabMonacoTheme,
  type WorkspaceThemeMode,
} from "../../lib/monaco-theme"

import "./index.css"

type WorkspaceVscodePartProps = {
  className?: string
  editorSettings?: WorkspaceEditorSettings
  part: "editor" | "explorer"
  themeMode?: WorkspaceThemeMode
  workspaceRoot: string
}

type MountState = "failed" | "pending" | "ready"

export function WorkspaceVscodePart({
  className,
  editorSettings,
  part,
  themeMode = "light",
  workspaceRoot,
}: WorkspaceVscodePartProps) {
  const containerRef = useRef<HTMLDivElement | null>(null)
  const themeModeRef = useRef(themeMode)
  const wrapperRef = useRef<HTMLDivElement | null>(null)
  const [mountState, setMountState] = useState<MountState>("pending")

  useEffect(() => {
    themeModeRef.current = themeMode
  }, [themeMode])

  useEffect(() => {
    let cancelled = false
    let disposable: { dispose(): void } | null = null
    let stage = "initialize"

    setMountState("pending")

    void (async () => {
      const {
        applyWorkspaceEditorSettings,
        ensureWorkspaceLspServices,
        setWorkspaceLspFileServiceRoot,
      } = await import("../../lib/workspace-lsp")
      setWorkspaceLspFileServiceRoot(workspaceRoot)
      await ensureWorkspaceLspServices(workspaceRoot)
      applySlabMonacoTheme(Monaco, themeModeRef.current)
      if (editorSettings) {
        await applyWorkspaceEditorSettings(editorSettings, workspaceRoot)
      }
      stage = "load-views"
      const views = await import("@codingame/monaco-vscode-views-service-override")
      if (cancelled || !containerRef.current) {
        return
      }


      stage = part === "explorer" ? "render-explorer" : "render-editor"
      if (part === "explorer") {
        disposable = views.renderSidebarPart(containerRef.current)
      } else {
        disposable = views.renderEditorPart(containerRef.current)
      }

      setMountState("ready")
    })().catch((error) => {
      if (!cancelled) {
        setMountState("failed")
      }
      console.debug("workspace VS Code part unavailable", {
        error: error instanceof Error
          ? { message: error.message, name: error.name, stack: error.stack }
          : String(error),
        part,
        stage,
        workspaceRoot,
      })
    })

    return () => {
      cancelled = true
      disposable?.dispose()
    }
  }, [editorSettings, part, workspaceRoot])

  useEffect(() => {
    if (mountState === "ready") {
      applySlabMonacoTheme(Monaco, themeMode)
    }
  }, [mountState, themeMode])

  return (
    <div
      ref={wrapperRef}
      className={cn("slab-vscode-part relative h-full min-h-0 w-full overflow-hidden", className)}
      data-testid={part === "explorer" ? "workspace-vscode-explorer" : "workspace-vscode-editor"}
    >
      <div ref={containerRef} className="h-full min-h-0 w-full overflow-hidden" />
      {mountState === "pending" ? (
        <div className="absolute inset-0 flex items-center justify-center bg-background/40">
          <Loader2 className="size-4 animate-spin text-muted-foreground" />
        </div>
      ) : null}
    </div>
  )
}
