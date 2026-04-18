import { page } from "vitest/browser"
import { describe, expect, it } from "vitest"
import { Spinner } from "@/spinner"
import { renderComponentScene } from "../test-utils"

function SpinnerGallery() {
  return (
    <div data-testid="spinner-gallery" className="flex flex-wrap items-center gap-4 rounded-[28px] border border-border/60 bg-surface-1 p-6 shadow-sm">
      <Spinner />
      <Spinner className="size-6" />
      <Spinner className="size-8" />
      <Spinner className="size-12" />
    </div>
  )
}

describe("Spinner browser coverage", () => {
  it("matches the shared spinner gallery screenshot", async () => {
    await renderComponentScene(<SpinnerGallery />)
    const spinner = page.getByRole("status").first()
    await expect.element(spinner).toBeVisible()
    await expect(page.getByTestId("spinner-gallery")).toMatchScreenshot("spinner-gallery.png")
  })

  it("keeps interactive spinner rendering covered", async () => {
    await renderComponentScene(<Spinner data-testid="test-spinner" />)
    const spinner = page.getByTestId("test-spinner")
    await expect.element(spinner).toBeVisible()
    await expect.element(spinner).toHaveAttribute("role", "status")
    await expect.element(spinner).toHaveAttribute("aria-label", "Loading")
    await expect.element(spinner).toHaveClass(/animate-spin/)
  })
})
