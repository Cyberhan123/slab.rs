import { useEffect, useRef, useState } from "react"
import { Loader2 } from "lucide-react"

import { cn } from "@/lib/utils"
import { ensureWorkspaceLspServices, setWorkspaceLspFileServiceRoot } from "../../lib/workspace-lsp"

import "./index.css"

type WorkspaceVscodePartProps = {
  className?: string
  part: "editor" | "explorer"
  workspaceRoot: string
}

type MountState = "failed" | "pending" | "ready"

export function WorkspaceVscodePart({ className, part, workspaceRoot }: WorkspaceVscodePartProps) {
  const containerRef = useRef<HTMLDivElement | null>(null)
  const wrapperRef = useRef<HTMLDivElement | null>(null)
  const [mountState, setMountState] = useState<MountState>("pending")

  useEffect(() => {
    let cancelled = false
    let disposable: { dispose(): void } | null = null
    let stage = "initialize"

    setMountState("pending")
    setWorkspaceLspFileServiceRoot(workspaceRoot)

    void (async () => {
      await ensureWorkspaceLspServices(workspaceRoot)
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
  }, [part, workspaceRoot])

  return (
    <div ref={wrapperRef} className={cn("slab-vscode-part relative h-full min-h-0 w-full overflow-hidden", className)}>
      <div ref={containerRef} className="h-full min-h-0 w-full overflow-hidden" />
      {mountState === "pending" ? (
        <div className="absolute inset-0 flex items-center justify-center bg-background/40">
          <Loader2 className="size-4 animate-spin text-muted-foreground" />
        </div>
      ) : null}
    </div>
  )
}
