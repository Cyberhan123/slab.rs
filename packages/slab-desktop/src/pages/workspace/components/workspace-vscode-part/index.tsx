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
const WORKSPACE_VSCODE_PART_MOUNT_TIMEOUT_MS = 45_000

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
  const [mountStage, setMountStage] = useState("initialize")
  const [mountError, setMountError] = useState<string | null>(null)

  useEffect(() => {
    themeModeRef.current = themeMode
  }, [themeMode])

  useEffect(() => {
    let cancelled = false
    let disposable: { dispose(): void } | null = null
    let stage = "initialize"
    let timedOut = false
    let timeoutId: number | null = null

    setMountState("pending")
    setMountStage(stage)
    setMountError(null)

    const markFailed = (error: unknown) => {
      if (cancelled) {
        return
      }
      setMountState("failed")
      setMountError(error instanceof Error ? `${error.name}: ${error.message}` : String(error))
      console.debug("workspace VS Code part unavailable", {
        error: error instanceof Error
          ? { message: error.message, name: error.name, stack: error.stack }
          : String(error),
        part,
        stage,
        workspaceRoot,
      })
    }

    timeoutId = window.setTimeout(() => {
      timedOut = true
      markFailed(new Error(`workspace VS Code part timed out during ${stage}`))
    }, WORKSPACE_VSCODE_PART_MOUNT_TIMEOUT_MS)

    void (async () => {
      const { ensureWorkspaceLspServices, setWorkspaceLspFileServiceRoot } = await import("../../lib/workspace-services")
      const { applyWorkspaceEditorSettings } = await import("../../lib/workspace-editor")
      if (cancelled || timedOut) {
        return
      }
      stage = "set-root"
      setMountStage(stage)
      setWorkspaceLspFileServiceRoot(workspaceRoot)
      stage = "ensure-services"
      setMountStage(stage)
      await ensureWorkspaceLspServices(workspaceRoot)
      if (cancelled || timedOut) {
        return
      }
      stage = "theme"
      setMountStage(stage)
      applySlabMonacoTheme(Monaco, themeModeRef.current)
      if (editorSettings) {
        stage = "editor-settings"
        setMountStage(stage)
        await applyWorkspaceEditorSettings(editorSettings, workspaceRoot)
        if (cancelled || timedOut) {
          return
        }
      }
      stage = "load-views"
      setMountStage(stage)
      const views = await import("@codingame/monaco-vscode-views-service-override")
      if (cancelled || timedOut || !containerRef.current) {
        return
      }

      stage = part === "explorer" ? "render-explorer" : "render-editor"
      setMountStage(stage)
      if (part === "explorer") {
        disposable = views.renderSidebarPart(containerRef.current)
      } else {
        disposable = views.renderEditorPart(containerRef.current)
      }

      stage = "ready"
      setMountStage(stage)
      if (timeoutId !== null) {
        window.clearTimeout(timeoutId)
        timeoutId = null
      }
      setMountState("ready")
    })().catch(markFailed)

    return () => {
      cancelled = true
      if (timeoutId !== null) {
        window.clearTimeout(timeoutId)
      }
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
      data-mount-stage={mountStage}
      data-mount-state={mountState}
      data-mount-error={mountError ?? undefined}
      data-part={part}
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
