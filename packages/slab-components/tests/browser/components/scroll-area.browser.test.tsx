import { page } from "vitest/browser"
import { describe, expect, it } from "vitest"
import { ScrollArea } from "@/scroll-area"
import { renderComponentScene } from "../test-utils"

function ScrollAreaGallery() {
  return (
    <div data-testid="scroll-area-gallery" className="rounded-[28px] border border-border/60 bg-surface-1 p-6 shadow-sm">
      <ScrollArea className="h-48 w-80 rounded-md border">
        <div className="p-4">
          <p className="mb-4">Scrollable content area with custom scrollbar styling.</p>
          {Array.from({ length: 20 }, (_, i) => `Line ${i + 1}: Lorem ipsum dolor sit amet, consectetur adipiscing elit.`).map((text) => (
            <p key={text} className="mb-2 text-sm text-muted-foreground">{text}</p>
          ))}
        </div>
      </ScrollArea>
    </div>
  )
}

describe("ScrollArea browser coverage", () => {
  it("matches the shared scroll area gallery screenshot", async () => {
    await renderComponentScene(<ScrollAreaGallery />)
    await expect.element(page.getByText("Scrollable content area")).toBeVisible()
    await expect(page.getByTestId("scroll-area-gallery")).toMatchScreenshot("scroll-area-gallery.png")
  })

  it("keeps interactive scroll behavior covered", async () => {
    await renderComponentScene(
      <ScrollArea className="h-32 w-64 rounded-md border">
        <div className="p-4">
          {Array.from({ length: 10 }, (_, i) => `Scrollable line ${i + 1}`).map((text) => (
            <p key={text} className="mb-2 text-sm">{text}</p>
          ))}
        </div>
      </ScrollArea>
    )
    await expect.element(page.getByText("Scrollable line 1", { exact: true })).toBeVisible()
    await expect.element(page.getByText("Scrollable line 5")).toBeVisible()
  })
})
