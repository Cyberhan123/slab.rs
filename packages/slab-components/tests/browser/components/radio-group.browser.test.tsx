import { page } from "vitest/browser"
import { describe, expect, it, vi } from "vitest"
import { RadioGroup, RadioGroupItem } from "@/radio-group"
import { renderComponentScene } from "../test-utils"

function RadioGroupGallery() {
  return (
    <div data-testid="radio-group-gallery" className="flex flex-col gap-4 rounded-[28px] border border-border/60 bg-surface-1 p-6 shadow-sm">
      <RadioGroup defaultValue="option1" data-testid="radio-group">
        <div className="flex items-center gap-2">
          <RadioGroupItem id="gallery-option-1" value="option1" />
          <label htmlFor="gallery-option-1">Option 1</label>
        </div>
        <div className="flex items-center gap-2">
          <RadioGroupItem id="gallery-option-2" value="option2" />
          <label htmlFor="gallery-option-2">Option 2</label>
        </div>
        <div className="flex items-center gap-2">
          <RadioGroupItem id="gallery-option-3" value="option3" disabled />
          <label htmlFor="gallery-option-3">Disabled Option</label>
        </div>
      </RadioGroup>
    </div>
  )
}

describe("RadioGroup browser coverage", () => {
  it("matches the shared radio group gallery screenshot", async () => {
    await renderComponentScene(<RadioGroupGallery />)
    const radioGroup = page.getByTestId("radio-group")
    await expect.element(radioGroup).toBeVisible()
    const radioItem = page.getByRole("radio").first()
    await expect.element(radioItem).toBeVisible()
    await expect(page.getByTestId("radio-group-gallery")).toMatchScreenshot("radio-group-gallery.png")
  })

  it("keeps interactive radio group selection behavior covered", async () => {
    const onChange = vi.fn<(value: string) => void>()
    await renderComponentScene(
      <RadioGroup defaultValue="option1" onValueChange={onChange} data-testid="radio-group">
        <div className="flex items-center gap-2">
          <RadioGroupItem id="interactive-option-1" value="option1" />
          <label htmlFor="interactive-option-1">Option 1</label>
        </div>
        <div className="flex items-center gap-2">
          <RadioGroupItem id="interactive-option-2" value="option2" />
          <label htmlFor="interactive-option-2">Option 2</label>
        </div>
      </RadioGroup>
    )
    const radioItems = page.getByRole("radio")
    await expect.element(radioItems.nth(0)).toBeVisible()
    await radioItems.nth(1).click()
    expect(onChange).toHaveBeenCalledWith("option2")
  })
})
