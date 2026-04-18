import { page } from "vitest/browser"
import { describe, expect, it } from "vitest"
import { Button } from "@/button"
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetFooter,
  SheetHeader,
  SheetTitle,
  SheetTrigger,
} from "@/sheet"
import { renderComponentScene } from "../test-utils"

function SheetGallery() {
  return (
    <div data-testid="sheet-gallery" className="flex flex-wrap items-center gap-4 rounded-[28px] border border-border/60 bg-surface-1 p-6 shadow-sm">
      <Sheet>
        <SheetTrigger asChild>
          <Button data-testid="sheet-trigger">Open Sheet</Button>
        </SheetTrigger>
        <SheetContent side="right">
          <SheetHeader>
            <SheetTitle>Sheet Title</SheetTitle>
            <SheetDescription>
              Sheet description goes here
            </SheetDescription>
          </SheetHeader>
          <div className="py-4">
            <p>Sheet content goes here</p>
          </div>
          <SheetFooter>
            <Button variant="outline">Cancel</Button>
            <Button>Save</Button>
          </SheetFooter>
        </SheetContent>
      </Sheet>
    </div>
  )
}

describe("Sheet browser coverage", () => {
  it("matches the shared sheet gallery screenshot", async () => {
    await renderComponentScene(<SheetGallery />)
    const trigger = page.getByTestId("sheet-trigger")
    await expect.element(trigger).toBeVisible()
    await expect(page.getByTestId("sheet-gallery")).toMatchScreenshot("sheet-gallery.png")
  })

  it("keeps interactive sheet open behavior covered", async () => {
    await renderComponentScene(
      <Sheet>
        <SheetTrigger asChild>
          <Button data-testid="open-sheet">Open Sheet</Button>
        </SheetTrigger>
        <SheetContent side="right" data-testid="sheet-content">
          <SheetHeader>
            <SheetTitle>Test Sheet</SheetTitle>
            <SheetDescription>Test description</SheetDescription>
          </SheetHeader>
          <div className="py-4">
            <p>Sheet content</p>
          </div>
          <SheetFooter>
            <Button variant="outline">Cancel</Button>
            <Button>Save</Button>
          </SheetFooter>
        </SheetContent>
      </Sheet>
    )
    const trigger = page.getByTestId("open-sheet")
    await expect.element(trigger).toBeVisible()
    await trigger.click()

    const content = page.getByTestId("sheet-content")
    await expect.element(content).toBeVisible()
  })
})
