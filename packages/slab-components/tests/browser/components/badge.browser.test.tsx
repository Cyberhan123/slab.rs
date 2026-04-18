import { page } from "vitest/browser"
import { describe, expect, it } from "vitest"
import { Badge } from "@/badge"
import { renderComponentScene } from "../test-utils"

function BadgeGallery() {
  return (
    <div data-testid="badge-gallery" className="flex flex-wrap items-center gap-3 rounded-[28px] border border-border/60 bg-surface-1 p-6 shadow-sm">
      <Badge variant="default" data-testid="badge">Default</Badge>
      <Badge variant="secondary">Secondary</Badge>
      <Badge variant="destructive">Destructive</Badge>
      <Badge variant="outline">Outline</Badge>
      <Badge variant="ghost">Ghost</Badge>
      <Badge variant="link">Link</Badge>
      <Badge variant="chip">Chip</Badge>
      <Badge variant="counter">Counter</Badge>
      <Badge variant="status" data-status="success">Success</Badge>
      <Badge variant="status" data-status="info">Info</Badge>
      <Badge variant="status" data-status="danger">Danger</Badge>
    </div>
  )
}

describe("Badge browser coverage", () => {
  it("matches the shared badge gallery screenshot", async () => {
    await renderComponentScene(<BadgeGallery />)
    const badge = page.getByTestId("badge")
    await expect.element(badge).toBeVisible()
    await expect(page.getByTestId("badge-gallery")).toMatchScreenshot("badge-gallery.png")
  })

  it("keeps interactive badge link behavior covered", async () => {
    await renderComponentScene(
      <Badge variant="link" asChild>
        <a href="/docs">
          Clickable link
        </a>
      </Badge>
    )
    const link = page.getByRole("link", { name: "Clickable link" })
    await expect.element(link).toBeVisible()
    await expect.element(link).toHaveAttribute("href", "/docs")
  })
})
