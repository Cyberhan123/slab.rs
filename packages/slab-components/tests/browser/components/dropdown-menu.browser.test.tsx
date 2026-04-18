import { page } from "vitest/browser"
import { describe, expect, it, vi } from "vitest"
import { Button } from "@/button"
import {
  DropdownMenu,
  DropdownMenuTrigger,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuShortcut,
} from "@/dropdown-menu"
import { renderComponentScene } from "../test-utils"

function DropdownMenuGallery() {
  return (
    <div data-testid="dropdown-menu-gallery" className="flex flex-wrap items-center gap-4 rounded-[28px] border border-border/60 bg-surface-1 p-6 shadow-sm">
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button data-testid="dropdown-menu-trigger">Open Menu</Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent>
          <DropdownMenuLabel>My Account</DropdownMenuLabel>
          <DropdownMenuSeparator />
          <DropdownMenuItem>
            Profile <DropdownMenuShortcut>⌘P</DropdownMenuShortcut>
          </DropdownMenuItem>
          <DropdownMenuItem>
            Settings <DropdownMenuShortcut>⌘S</DropdownMenuShortcut>
          </DropdownMenuItem>
          <DropdownMenuSeparator />
          <DropdownMenuItem variant="destructive">
            Log out <DropdownMenuShortcut>⌘Q</DropdownMenuShortcut>
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    </div>
  )
}

describe("DropdownMenu browser coverage", () => {
  it("matches the shared dropdown menu gallery screenshot", async () => {
    await renderComponentScene(<DropdownMenuGallery />)
    const trigger = page.getByTestId("dropdown-menu-trigger")
    await expect.element(trigger).toBeVisible()
    await expect(page.getByTestId("dropdown-menu-gallery")).toMatchScreenshot("dropdown-menu-gallery.png")
  })

  it("keeps interactive dropdown menu behavior covered", async () => {
    const onSelect = vi.fn<() => void>()
    await renderComponentScene(
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button data-testid="menu-trigger">Actions</Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent data-testid="dropdown-menu-content">
          <DropdownMenuItem onSelect={onSelect}>Edit</DropdownMenuItem>
          <DropdownMenuItem onSelect={onSelect}>Delete</DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    )
    const trigger = page.getByTestId("menu-trigger")
    await expect.element(trigger).toBeVisible()
    await trigger.click()

    const content = page.getByTestId("dropdown-menu-content")
    await expect.element(content).toBeVisible()
  })
})
