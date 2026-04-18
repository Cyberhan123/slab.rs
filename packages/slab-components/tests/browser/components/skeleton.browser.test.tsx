import { page } from "vitest/browser"
import { describe, expect, it } from "vitest"
import { Skeleton } from "@/skeleton"
import { renderComponentScene } from "../test-utils"

function SkeletonGallery() {
  return (
    <div data-testid="skeleton-gallery" className="flex flex-col gap-4 rounded-[28px] border border-border/60 bg-surface-1 p-6 shadow-sm">
      <div className="space-y-2">
        <Skeleton className="h-4 w-3/4" data-testid="skeleton" />
        <Skeleton className="h-4 w-full" />
        <Skeleton className="h-4 w-5/6" />
      </div>
      <div className="flex items-center gap-4">
        <Skeleton className="h-12 w-12 rounded-full" />
        <div className="space-y-2">
          <Skeleton className="h-4 w-32" />
          <Skeleton className="h-3 w-24" />
        </div>
      </div>
      <Skeleton className="h-32 w-full rounded-xl" />
    </div>
  )
}

describe("Skeleton browser coverage", () => {
  it("matches the shared skeleton gallery screenshot", async () => {
    await renderComponentScene(<SkeletonGallery />)
    const skeleton = page.getByTestId("skeleton")
    await expect.element(skeleton).toBeVisible()
    await expect(page.getByTestId("skeleton-gallery")).toMatchScreenshot("skeleton-gallery.png")
  })

  it("keeps interactive skeleton rendering covered", async () => {
    await renderComponentScene(
      <div className="space-y-2">
        <Skeleton className="h-4 w-3/4" data-testid="custom-skeleton" />
      </div>
    )
    const skeleton = page.getByTestId("custom-skeleton")
    await expect.element(skeleton).toBeVisible()
    await expect.element(skeleton).toHaveClass(/animate-pulse/)
  })
})
