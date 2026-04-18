import { page } from "vitest/browser"
import { describe, expect, it } from "vitest"
import { Input } from "@/input"
import { renderComponentScene } from "../test-utils"

function InputGallery() {
  return (
    <div data-testid="input-gallery" className="flex flex-col gap-4 rounded-[28px] border border-border/60 bg-surface-1 p-6 shadow-sm">
      <Input placeholder="Default input" variant="default" />
      <Input placeholder="Soft input" variant="soft" />
      <Input placeholder="Shell input" variant="shell" />
      <Input type="email" placeholder="Email input" variant="default" />
      <Input type="password" placeholder="Password input" variant="soft" />
      <Input disabled placeholder="Disabled input" variant="default" />
    </div>
  )
}

describe("Input browser coverage", () => {
  it("matches the shared input gallery screenshot", async () => {
    await renderComponentScene(<InputGallery />)
    const input = page.getByPlaceholder("Default input")
    await expect.element(input).toBeVisible()
    await expect(page.getByTestId("input-gallery")).toMatchScreenshot("input-gallery.png")
  })

  it("keeps interactive input behavior covered", async () => {
    await renderComponentScene(<Input placeholder="Type here" variant="default" />)
    const input = page.getByPlaceholder("Type here")
    await expect.element(input).toBeVisible()
    await input.fill("hello world")
    await expect.element(input).toHaveValue("hello world")
  })
})
