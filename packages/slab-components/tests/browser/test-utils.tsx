import type { ReactNode } from "react"
import { render } from "vitest-browser-react"

type ComponentSceneOptions = {
  className?: string
}

export async function renderComponentScene(
  ui: ReactNode,
  { className = "" }: ComponentSceneOptions = {}
) {
  return render(
    <div
      data-testid="component-browser-scene"
      className={[
        "min-h-screen bg-background px-8 py-10 text-foreground",
        className,
      ]
        .filter(Boolean)
        .join(" ")}
    >
      {ui}
    </div>
  )
}
