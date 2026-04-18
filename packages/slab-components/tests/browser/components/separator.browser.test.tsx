import { page } from "vitest/browser"
import { describe, expect, it } from "vitest"
import { Separator } from "@/separator"
import { renderComponentScene } from "../test-utils"

function SeparatorGallery() {
  return (
    <div data-testid="separator-gallery" className="flex flex-col gap-4 rounded-[28px] border border-border/60 bg-surface-1 p-6 shadow-sm">
      <div className="space-y-2">
        <p>Content above horizontal separator</p>
        <Separator orientation="horizontal" data-testid="separator" />
        <p>Content below horizontal separator</p>
      </div>
      <div className="flex items-center gap-4">
        <p>Left</p>
        <Separator orientation="vertical" className="h-8" />
        <p>Right</p>
      </div>
    </div>
  )
}

describe("Separator browser coverage", () => {
  it("matches the shared separator gallery screenshot", async () => {
    await renderComponentScene(<SeparatorGallery />)
    const separator = page.getByTestId("separator")
    await expect.element(separator).toBeVisible()
    await expect(page.getByTestId("separator-gallery")).toMatchScreenshot("separator-gallery.png")
  })

  it("keeps interactive separator orientations covered", async () => {
    await renderComponentScene(
      <div className="space-y-4">
        <Separator orientation="horizontal" data-testid="horizontal-separator" />
        <div className="flex h-8 items-center gap-4">
          <Separator orientation="vertical" className="h-8" data-testid="vertical-separator" />
        </div>
      </div>
    )
    const horizontal = page.getByTestId("horizontal-separator")
    await expect.element(horizontal).toBeVisible()
    await expect.element(horizontal).toHaveAttribute("data-orientation", "horizontal")

    const vertical = page.getByTestId("vertical-separator")
    await expect.element(vertical).toBeVisible()
    await expect.element(vertical).toHaveAttribute("data-orientation", "vertical")
  })
})
