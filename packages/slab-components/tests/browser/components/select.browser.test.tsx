import { page } from "vitest/browser"
import { describe, expect, it, vi } from "vitest"
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/select"
import { renderComponentScene } from "../test-utils"

function SelectGallery() {
  return (
    <div data-testid="select-gallery" className="flex flex-wrap items-center gap-4 rounded-[28px] border border-border/60 bg-surface-1 p-6 shadow-sm">
      <Select defaultValue="default">
        <SelectTrigger size="sm" variant="default" data-testid="select-trigger">
          <SelectValue placeholder="Small default" />
        </SelectTrigger>
        <SelectContent variant="default">
          <SelectItem value="default">Default</SelectItem>
          <SelectItem value="option">Option</SelectItem>
        </SelectContent>
      </Select>

      <Select defaultValue="soft">
        <SelectTrigger size="default" variant="soft">
          <SelectValue placeholder="Default soft" />
        </SelectTrigger>
        <SelectContent variant="soft">
          <SelectItem value="soft">Soft</SelectItem>
          <SelectItem value="option">Option</SelectItem>
        </SelectContent>
      </Select>

      <Select defaultValue="pill">
        <SelectTrigger size="pill" variant="pill">
          <SelectValue placeholder="Pill pill" />
        </SelectTrigger>
        <SelectContent variant="pill">
          <SelectItem value="pill">Pill</SelectItem>
          <SelectItem value="option">Option</SelectItem>
        </SelectContent>
      </Select>

      <Select disabled>
        <SelectTrigger variant="default">
          <SelectValue placeholder="Disabled" />
        </SelectTrigger>
        <SelectContent variant="default">
          <SelectItem value="disabled">Disabled</SelectItem>
        </SelectContent>
      </Select>
    </div>
  )
}

describe("Select browser coverage", () => {
  it("matches the shared select gallery screenshot", async () => {
    await renderComponentScene(<SelectGallery />)
    const trigger = page.getByTestId("select-trigger")
    await expect.element(trigger).toBeVisible()
    await expect(page.getByTestId("select-gallery")).toMatchScreenshot("select-gallery.png")
  })

  it("keeps interactive select trigger behavior covered", async () => {
    const onValueChange = vi.fn<(value: string) => void>()
    await renderComponentScene(
      <Select onValueChange={onValueChange}>
        <SelectTrigger variant="default" data-testid="select-trigger">
          <SelectValue placeholder="Choose an option" />
        </SelectTrigger>
        <SelectContent variant="default">
          <SelectItem value="option1">Option 1</SelectItem>
          <SelectItem value="option2">Option 2</SelectItem>
        </SelectContent>
      </Select>
    )
    const trigger = page.getByTestId("select-trigger")
    await expect.element(trigger).toBeVisible()
    await trigger.click()
    const option2 = page.getByText("Option 2")
    await option2.click()
    expect(onValueChange).toHaveBeenCalledWith("option2")
  })
})
