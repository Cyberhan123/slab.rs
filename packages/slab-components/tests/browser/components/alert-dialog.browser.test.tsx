import { page } from "vitest/browser"
import { describe, expect, it } from "vitest"
import { Button } from "@/button"
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from "@/alert-dialog"
import { renderComponentScene } from "../test-utils"

function AlertDialogGallery() {
  return (
    <div data-testid="alert-dialog-gallery" className="flex flex-wrap items-center gap-4 rounded-[28px] border border-border/60 bg-surface-1 p-6 shadow-sm">
      <AlertDialog>
        <AlertDialogTrigger asChild>
          <Button variant="destructive">Delete Account</Button>
        </AlertDialogTrigger>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Are you sure?</AlertDialogTitle>
            <AlertDialogDescription>
              This action cannot be undone. This will permanently delete your account.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction variant="destructive">Delete</AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  )
}

describe("AlertDialog browser coverage", () => {
  it("matches the shared alert dialog gallery screenshot", async () => {
    await renderComponentScene(<AlertDialogGallery />)
    const trigger = page.getByRole("button", { name: "Delete Account" })
    await expect.element(trigger).toBeVisible()
    await expect(page.getByTestId("alert-dialog-gallery")).toMatchScreenshot("alert-dialog-gallery.png")
  })

  it("keeps interactive alert dialog open behavior covered", async () => {
    await renderComponentScene(
      <AlertDialog>
        <AlertDialogTrigger asChild>
          <Button data-testid="open-alert">Delete Item</Button>
        </AlertDialogTrigger>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Confirm Deletion</AlertDialogTitle>
            <AlertDialogDescription>
              This will permanently delete the item.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction variant="destructive">Delete</AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    )
    const trigger = page.getByTestId("open-alert")
    await expect.element(trigger).toBeVisible()
    await trigger.click()

    const content = page.getByRole("alertdialog")
    await expect.element(content).toBeVisible()

    const title = page.getByRole("heading", { name: "Confirm Deletion" })
    await expect.element(title).toBeVisible()
    await expect.element(title).toHaveTextContent("Confirm Deletion")
  })
})
