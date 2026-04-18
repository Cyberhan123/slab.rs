import { page } from "vitest/browser"
import { describe, expect, it, vi } from "vitest"

import { Button } from "@/button"

import { renderComponentScene } from "../test-utils"

function ButtonGallery() {
  return (
    <div
      data-testid="button-gallery"
      className="flex flex-wrap items-center gap-4 rounded-[28px] border border-border/60 bg-surface-1 p-6 shadow-sm"
    >
      <Button>Primary</Button>
      <Button variant="secondary">Secondary</Button>
      <Button variant="outline">Outline</Button>
      <Button variant="cta">Call to action</Button>
      <Button variant="pill" size="pill">Pill</Button>
    </div>
  )
}

describe("Button browser coverage", () => {
  it("matches the shared button gallery screenshot", async () => {
    await renderComponentScene(<ButtonGallery />)

    await expect.element(page.getByRole("button", { name: "Primary" })).toBeVisible()
    await expect(page.getByTestId("button-gallery")).toMatchScreenshot("button-gallery.png")
  })

  it("keeps interactive button clicks covered", async () => {
    const onClick = vi.fn<() => void>()

    await renderComponentScene(
      <Button variant="cta" onClick={onClick}>
        Launch desktop host
      </Button>
    )

    const button = page.getByRole("button", { name: "Launch desktop host" })
    await button.click()

    expect(onClick).toHaveBeenCalledOnce()
    await expect.element(button).toHaveAttribute("data-variant", "cta")
  })
})
