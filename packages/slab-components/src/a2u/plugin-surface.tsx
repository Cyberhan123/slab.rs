import { Puzzle } from "lucide-react"

import { A2uSurfaceFrame, type A2uSurfaceAction } from "./surface-frame"

export type A2uPluginSurfaceLabels = {
  description: string
  emptyDescription: string
  pluginId: string
  surface: string
  title: string
}

type A2uPluginSurfaceProps = {
  actions?: A2uSurfaceAction[]
  labels: A2uPluginSurfaceLabels
  pluginId?: string
  surface?: string
}

export function A2uPluginSurface({
  actions,
  labels,
  pluginId,
  surface,
}: A2uPluginSurfaceProps) {
  return (
    <A2uSurfaceFrame
      actions={actions}
      data-testid="a2u-plugin-surface"
      icon={Puzzle}
      title={labels.title}
      description={
        <div className="space-y-2">
          <p>{pluginId ? labels.description : labels.emptyDescription}</p>
          {pluginId ? (
            <p className="rounded-[10px] bg-[var(--surface-1)] px-3 py-2 font-mono text-xs text-foreground">
              <span className="mr-2 font-sans text-muted-foreground">{labels.pluginId}</span>
              {pluginId}
            </p>
          ) : null}
          {surface ? (
            <p className="rounded-[10px] bg-[var(--surface-1)] px-3 py-2 font-mono text-xs text-foreground">
              <span className="mr-2 font-sans text-muted-foreground">{labels.surface}</span>
              {surface}
            </p>
          ) : null}
        </div>
      }
    />
  )
}
