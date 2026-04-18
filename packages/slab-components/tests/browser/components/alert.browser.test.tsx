import { page } from "vitest/browser"
import { describe, expect, it } from "vitest"
import { Alert, AlertTitle, AlertDescription } from "@/alert"
import { renderComponentScene } from "../test-utils"

function AlertGallery() {
  return (
    <div data-testid="alert-gallery" className="flex flex-col gap-4 rounded-[28px] border border-border/60 bg-surface-1 p-6 shadow-sm">
      <Alert variant="default" data-testid="alert">
        <AlertTitle>Default Alert</AlertTitle>
        <AlertDescription>This is a default alert message</AlertDescription>
      </Alert>

      <Alert variant="destructive">
        <AlertTitle>Destructive Alert</AlertTitle>
        <AlertDescription>This is a destructive alert message</AlertDescription>
      </Alert>
    </div>
  )
}

describe("Alert browser coverage", () => {
  it("matches the shared alert gallery screenshot", async () => {
    await renderComponentScene(<AlertGallery />)
    const alert = page.getByTestId("alert")
    await expect.element(alert).toBeVisible()
    await expect(page.getByTestId("alert-gallery")).toMatchScreenshot("alert-gallery.png")
  })

  it("keeps interactive alert structure covered", async () => {
    await renderComponentScene(
      <Alert variant="default">
        <AlertTitle data-testid="alert-title">Test Alert</AlertTitle>
        <AlertDescription data-testid="alert-description">Test description</AlertDescription>
      </Alert>
    )
    const alert = page.getByRole("alert")
    await expect.element(alert).toBeVisible()

    const title = page.getByTestId("alert-title")
    await expect.element(title).toBeVisible()
    await expect.element(title).toHaveTextContent("Test Alert")

    const description = page.getByTestId("alert-description")
    await expect.element(description).toBeVisible()
    await expect.element(description).toHaveTextContent("Test description")
  })
})
