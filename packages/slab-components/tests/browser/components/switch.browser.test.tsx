import { page } from "vitest/browser"
import { describe, expect, it, vi } from "vitest"
import { Switch } from "@/switch"
import { renderComponentScene } from "../test-utils"

function SwitchGallery() {
  return (
    <div data-testid="switch-gallery" className="flex flex-wrap items-center gap-4 rounded-[28px] border border-border/60 bg-surface-1 p-6 shadow-sm">
      <Switch size="sm" variant="default" data-testid="switch" />
      <Switch size="default" variant="default" />
      <Switch size="sm" variant="workspace" defaultChecked />
      <Switch size="default" variant="workspace" defaultChecked />
      <Switch disabled />
    </div>
  )
}

describe("Switch browser coverage", () => {
  it("matches the shared switch gallery screenshot", async () => {
    await renderComponentScene(<SwitchGallery />)
    const switchEl = page.getByTestId("switch")
    await expect.element(switchEl).toBeVisible()
    await expect(page.getByTestId("switch-gallery")).toMatchScreenshot("switch-gallery.png")
  })

  it("keeps interactive switch toggle behavior covered", async () => {
    const onChange = vi.fn<(checked: boolean) => void>()
    await renderComponentScene(<Switch variant="default" data-testid="switch" onCheckedChange={onChange} />)
    const switchEl = page.getByTestId("switch")
    await expect.element(switchEl).toBeVisible()
    await switchEl.click()
    expect(onChange).toHaveBeenCalledWith(true)
  })
})
