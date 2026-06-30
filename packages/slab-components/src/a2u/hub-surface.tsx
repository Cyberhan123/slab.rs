import { PackageSearch } from "lucide-react"

import { A2uSurfaceFrame, type A2uSurfaceAction } from "./surface-frame"

export type A2uHubSurfaceLabels = {
  description: string
  title: string
}

type A2uHubSurfaceProps = {
  actions?: A2uSurfaceAction[]
  labels: A2uHubSurfaceLabels
}

export function A2uHubSurface({
  actions,
  labels,
}: A2uHubSurfaceProps) {
  return (
    <A2uSurfaceFrame
      actions={actions}
      data-testid="a2u-hub-surface"
      icon={PackageSearch}
      title={labels.title}
      description={labels.description}
    />
  )
}
