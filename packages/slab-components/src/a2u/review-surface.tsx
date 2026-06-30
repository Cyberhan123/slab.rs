import { FileSearch } from "lucide-react"

import { A2uSurfaceFrame, type A2uSurfaceAction } from "./surface-frame"

export type A2uReviewSurfaceLabels = {
  description: string
  diff: string
  emptyDescription: string
  path: string
  title: string
}

type A2uReviewSurfaceProps = {
  actions?: A2uSurfaceAction[]
  diff?: string
  labels: A2uReviewSurfaceLabels
  path?: string
}

export function A2uReviewSurface({
  actions,
  diff,
  labels,
  path,
}: A2uReviewSurfaceProps) {
  const diffPreview = diff?.trim()
  const clippedDiff = diffPreview && diffPreview.length > 360
    ? `${diffPreview.slice(0, 360)}...`
    : diffPreview

  return (
    <A2uSurfaceFrame
      actions={actions}
      data-testid="a2u-review-surface"
      icon={FileSearch}
      title={labels.title}
      description={
        <div className="space-y-2 text-left">
          <p className="text-center">{path || clippedDiff ? labels.description : labels.emptyDescription}</p>
          {path ? (
            <p className="rounded-[10px] bg-[var(--surface-1)] px-3 py-2 font-mono text-xs text-foreground">
              <span className="mr-2 font-sans text-muted-foreground">{labels.path}</span>
              {path}
            </p>
          ) : null}
          {clippedDiff ? (
            <pre className="max-h-28 overflow-auto rounded-[10px] bg-[var(--surface-1)] px-3 py-2 text-xs leading-5 text-foreground">
              <span className="mb-1 block font-sans text-muted-foreground">{labels.diff}</span>
              {clippedDiff}
            </pre>
          ) : null}
        </div>
      }
    />
  )
}
