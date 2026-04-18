import { page } from "vitest/browser"
import { describe, expect, it } from "vitest"
import { Textarea } from "@/textarea"
import { renderComponentScene } from "../test-utils"

function TextareaGallery() {
  return (
    <div data-testid="textarea-gallery" className="flex flex-col gap-4 rounded-[28px] border border-border/60 bg-surface-1 p-6 shadow-sm">
      <Textarea placeholder="Default textarea" variant="default" />
      <Textarea placeholder="Soft textarea" variant="soft" />
      <Textarea placeholder="Shell textarea" variant="shell" />
      <Textarea placeholder="Auto-resize textarea" variant="default" autoResize />
      <Textarea disabled placeholder="Disabled textarea" variant="soft" />
    </div>
  )
}

describe("Textarea browser coverage", () => {
  it("matches the shared textarea gallery screenshot", async () => {
    await renderComponentScene(<TextareaGallery />)
    const textarea = page.getByPlaceholder("Default textarea")
    await expect.element(textarea).toBeVisible()
    await expect(page.getByTestId("textarea-gallery")).toMatchScreenshot("textarea-gallery.png")
  })

  it("keeps interactive textarea behavior covered", async () => {
    await renderComponentScene(<Textarea placeholder="Type here" variant="default" />)
    const textarea = page.getByPlaceholder("Type here")
    await expect.element(textarea).toBeVisible()
    await textarea.fill("some text")
    await expect.element(textarea).toHaveValue("some text")
  })
})
