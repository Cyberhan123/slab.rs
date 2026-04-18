import { page } from "vitest/browser"
import { describe, expect, it } from "vitest"
import { Button } from "@/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/dialog"
import { renderComponentScene } from "../test-utils"

function DialogGallery() {
  return (
    <div data-testid="dialog-gallery" className="flex flex-wrap items-center gap-4 rounded-[28px] border border-border/60 bg-surface-1 p-6 shadow-sm">
      <Dialog>
        <DialogTrigger asChild>
          <Button data-testid="dialog-trigger">Open Dialog</Button>
        </DialogTrigger>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Are you sure?</DialogTitle>
            <DialogDescription>
              This action cannot be undone.
            </DialogDescription>
          </DialogHeader>
          <div className="py-4">
            <p>Dialog content goes here.</p>
          </div>
          <DialogFooter>
            <Button variant="outline">Cancel</Button>
            <Button>Confirm</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  )
}

describe("Dialog browser coverage", () => {
  it("matches the shared dialog gallery screenshot", async () => {
    await renderComponentScene(<DialogGallery />)
    const trigger = page.getByTestId("dialog-trigger")
    await expect.element(trigger).toBeVisible()
    await expect(page.getByTestId("dialog-gallery")).toMatchScreenshot("dialog-gallery.png")
  })

  it("keeps interactive dialog open behavior covered", async () => {
    await renderComponentScene(
      <Dialog>
        <DialogTrigger asChild>
          <Button data-testid="open-dialog">Open Dialog</Button>
        </DialogTrigger>
        <DialogContent>
          <DialogHeader>
            <DialogTitle data-testid="dialog-title">Test Dialog</DialogTitle>
            <DialogDescription>Test description</DialogDescription>
          </DialogHeader>
          <div className="py-4">
            <p>Dialog content</p>
          </div>
          <DialogFooter>
            <Button variant="outline">Cancel</Button>
            <Button>Confirm</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    )
    const trigger = page.getByTestId("open-dialog")
    await expect.element(trigger).toBeVisible()
    await trigger.click()

    const dialog = page.getByRole("dialog")
    await expect.element(dialog).toBeVisible()

    const title = page.getByTestId("dialog-title")
    await expect.element(title).toBeVisible()
    await expect.element(title).toHaveTextContent("Test Dialog")
  })
})
