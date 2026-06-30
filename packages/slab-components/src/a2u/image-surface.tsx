import { ImageIcon } from "lucide-react"

import { A2uSurfaceFrame, type A2uSurfaceAction } from "./surface-frame"

export type A2uImageSurfaceLabels = {
  description: string
  emptyDescription: string
  prompt: string
  title: string
}

type A2uImageSurfaceProps = {
  actions?: A2uSurfaceAction[]
  labels: A2uImageSurfaceLabels
  prompt?: string
}

export function A2uImageSurface({
  actions,
  labels,
  prompt,
}: A2uImageSurfaceProps) {
  return (
    <A2uSurfaceFrame
      actions={actions}
      data-testid="a2u-image-surface"
      icon={ImageIcon}
      title={labels.title}
      description={
        <div className="space-y-2">
          <p>{prompt ? labels.description : labels.emptyDescription}</p>
          {prompt ? (
            <p className="rounded-[10px] bg-[var(--surface-1)] px-3 py-2 text-left text-xs text-foreground">
              <span className="mr-2 font-medium text-muted-foreground">{labels.prompt}</span>
              {prompt}
            </p>
          ) : null}
        </div>
      }
    />
  )
}
