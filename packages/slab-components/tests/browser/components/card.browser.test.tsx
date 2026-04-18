import { page } from "vitest/browser"
import { describe, expect, it } from "vitest"
import {
  Card,
  CardHeader,
  CardTitle,
  CardDescription,
  CardContent,
  CardFooter
} from "@/card"
import { renderComponentScene } from "../test-utils"

function CardGallery() {
  return (
    <div data-testid="card-gallery" className="grid grid-cols-1 gap-4 md:grid-cols-2">
      <Card variant="default" data-testid="card">
        <CardHeader>
          <CardTitle>Default Card</CardTitle>
          <CardDescription>This is the default variant</CardDescription>
        </CardHeader>
        <CardContent>Card content goes here</CardContent>
      </Card>

      <Card variant="soft">
        <CardHeader>
          <CardTitle>Soft Card</CardTitle>
          <CardDescription>Soft variant with shadow</CardDescription>
        </CardHeader>
        <CardContent>Card content goes here</CardContent>
      </Card>

      <Card variant="elevated">
        <CardHeader>
          <CardTitle>Elevated Card</CardTitle>
          <CardDescription>Elevated with more shadow</CardDescription>
        </CardHeader>
        <CardContent>Card content goes here</CardContent>
      </Card>

      <Card variant="metric">
        <CardHeader>
          <CardTitle>Metric Card</CardTitle>
          <CardDescription>Gradient background</CardDescription>
        </CardHeader>
        <CardContent>Card content goes here</CardContent>
      </Card>

      <Card variant="hero" className="md:col-span-2">
        <CardHeader>
          <CardTitle>Hero Card</CardTitle>
          <CardDescription>Full width with gradient</CardDescription>
        </CardHeader>
        <CardContent>Card content goes here</CardContent>
      </Card>
    </div>
  )
}

describe("Card browser coverage", () => {
  it("matches the shared card gallery screenshot", async () => {
    await renderComponentScene(<CardGallery />)
    const card = page.getByTestId("card").first()
    await expect.element(card).toBeVisible()
    await expect(page.getByTestId("card-gallery")).toMatchScreenshot("card-gallery.png")
  })

  it("keeps interactive card structure covered", async () => {
    await renderComponentScene(
      <Card variant="default" data-testid="card">
        <CardHeader>
          <CardTitle data-testid="card-title">Test Card</CardTitle>
          <CardDescription>Test description</CardDescription>
        </CardHeader>
        <CardContent data-testid="card-content">Test content</CardContent>
        <CardFooter>Test footer</CardFooter>
      </Card>
    )
    const card = page.getByTestId("card")
    await expect.element(card).toBeVisible()
    await expect.element(card).toHaveAttribute("data-variant", "default")

    const title = page.getByTestId("card-title")
    await expect.element(title).toBeVisible()

    const content = page.getByTestId("card-content")
    await expect.element(content).toBeVisible()
  })
})
