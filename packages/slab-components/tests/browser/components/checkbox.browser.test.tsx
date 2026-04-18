import { page } from "vitest/browser"
import { describe, expect, it, vi } from "vitest"
import { Checkbox } from "@/checkbox"
import { renderComponentScene } from "../test-utils"

function CheckboxGallery() {
  return (
    <div data-testid="checkbox-gallery" className="flex flex-wrap items-center gap-4 rounded-[28px] border border-border/60 bg-surface-1 p-6 shadow-sm">
      <Checkbox defaultChecked data-testid="checkbox" />
      <Checkbox />
      <Checkbox disabled />
      <Checkbox disabled checked />
    </div>
  )
}

describe("Checkbox browser coverage", () => {
  it("matches the shared checkbox gallery screenshot", async () => {
    await renderComponentScene(<CheckboxGallery />)
    const checkbox = page.getByTestId("checkbox")
    await expect.element(checkbox).toBeVisible()
    await expect(page.getByTestId("checkbox-gallery")).toMatchScreenshot("checkbox-gallery.png")
  })

  it("keeps interactive checkbox toggle behavior covered", async () => {
    const onChange = vi.fn<(checked: boolean) => void>()
    await renderComponentScene(<Checkbox data-testid="checkbox" onCheckedChange={onChange} />)
    const checkbox = page.getByTestId("checkbox")
    await expect.element(checkbox).toBeVisible()
    await checkbox.click()
    expect(onChange).toHaveBeenCalledWith(true)
  })
})
