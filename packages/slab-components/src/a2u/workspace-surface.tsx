import { FolderOpen } from "lucide-react"

import { A2uSurfaceFrame, type A2uSurfaceAction } from "./surface-frame"

export type A2uWorkspaceSurfaceLabels = {
  description: string
  emptyDescription: string
  revealPath: string
  title: string
}

type A2uWorkspaceSurfaceProps = {
  actions?: A2uSurfaceAction[]
  labels: A2uWorkspaceSurfaceLabels
  revealPath?: string
}

export function A2uWorkspaceSurface({
  actions,
  labels,
  revealPath,
}: A2uWorkspaceSurfaceProps) {
  return (
    <A2uSurfaceFrame
      actions={actions}
      data-testid="a2u-workspace-surface"
      icon={FolderOpen}
      title={labels.title}
      description={
        <div className="space-y-2">
          <p>{revealPath ? labels.description : labels.emptyDescription}</p>
          {revealPath ? (
            <p className="rounded-[10px] bg-[var(--surface-1)] px-3 py-2 font-mono text-xs text-foreground">
              <span className="mr-2 font-sans text-muted-foreground">{labels.revealPath}</span>
              {revealPath}
            </p>
          ) : null}
        </div>
      }
    />
  )
}
