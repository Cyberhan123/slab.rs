import { page } from "vitest/browser"
import { describe, expect, it } from "vitest"
import { Button } from "@/button"
import {
  Drawer,
  DrawerClose,
  DrawerContent,
  DrawerDescription,
  DrawerFooter,
  DrawerHeader,
  DrawerTitle,
  DrawerTrigger,
} from "@/drawer"
import { renderComponentScene } from "../test-utils"

function DrawerGallery() {
  return (
    <div data-testid="drawer-gallery" className="flex flex-wrap items-center gap-4 rounded-[28px] border border-border/60 bg-surface-1 p-6 shadow-sm">
      <Drawer>
        <DrawerTrigger asChild>
          <Button data-testid="drawer-trigger">Open Drawer</Button>
        </DrawerTrigger>
        <DrawerContent>
          <DrawerHeader>
            <DrawerTitle>Drawer Title</DrawerTitle>
            <DrawerDescription>
              Drawer description goes here
            </DrawerDescription>
          </DrawerHeader>
          <div className="py-4">
            <p>Drawer content goes here</p>
          </div>
          <DrawerFooter>
            <DrawerClose asChild>
              <Button variant="outline">Cancel</Button>
            </DrawerClose>
            <Button>Confirm</Button>
          </DrawerFooter>
        </DrawerContent>
      </Drawer>
    </div>
  )
}

describe("Drawer browser coverage", () => {
  it("matches the shared drawer gallery screenshot", async () => {
    await renderComponentScene(<DrawerGallery />)
    const trigger = page.getByTestId("drawer-trigger")
    await expect.element(trigger).toBeVisible()
    await expect(page.getByTestId("drawer-gallery")).toMatchScreenshot("drawer-gallery.png")
  })

  it("keeps interactive drawer open behavior covered", async () => {
    await renderComponentScene(
      <Drawer>
        <DrawerTrigger asChild>
          <Button data-testid="open-drawer">Open Drawer</Button>
        </DrawerTrigger>
        <DrawerContent data-testid="drawer-content">
          <DrawerHeader>
            <DrawerTitle>Test Drawer</DrawerTitle>
            <DrawerDescription>Test description</DrawerDescription>
          </DrawerHeader>
          <div className="py-4">
            <p>Drawer content</p>
          </div>
          <DrawerFooter>
            <DrawerClose asChild>
              <Button variant="outline">Cancel</Button>
            </DrawerClose>
            <Button>Confirm</Button>
          </DrawerFooter>
        </DrawerContent>
      </Drawer>
    )
    const trigger = page.getByTestId("open-drawer")
    await expect.element(trigger).toBeVisible()
    await trigger.click()

    const content = page.getByTestId("drawer-content")
    await expect.element(content).toBeVisible()
  })
})
