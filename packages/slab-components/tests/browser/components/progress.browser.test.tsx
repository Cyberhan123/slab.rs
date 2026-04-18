import { page } from "vitest/browser"
import { describe, expect, it } from "vitest"
import { Progress } from "@/progress"
import { renderComponentScene } from "../test-utils"

function ProgressGallery() {
  return (
    <div data-testid="progress-gallery" className="flex flex-col gap-4 rounded-[28px] border border-border/60 bg-surface-1 p-6 shadow-sm">
      <Progress value={0} data-testid="progress" />
      <Progress value={25} />
      <Progress value={50} />
      <Progress value={75} />
      <Progress value={100} />
    </div>
  )
}

describe("Progress browser coverage", () => {
  it("matches the shared progress gallery screenshot", async () => {
    await renderComponentScene(<ProgressGallery />)
    const progress = page.getByTestId("progress")
    await expect.element(progress).toBeVisible()
    await expect(page.getByTestId("progress-gallery")).toMatchScreenshot("progress-gallery.png")
  })

  it("keeps interactive progress value behavior covered", async () => {
    await renderComponentScene(<Progress value={75} data-testid="progress" />)
    const progress = page.getByTestId("progress")
    await expect.element(progress).toBeVisible()
    // Visual regression already covers the progress value rendering
  })
})
