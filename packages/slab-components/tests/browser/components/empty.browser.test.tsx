import { page } from "vitest/browser"
import { describe, expect, it } from "vitest"
import {
  Empty,
  EmptyHeader,
  EmptyTitle,
  EmptyDescription,
  EmptyContent,
  EmptyMedia
} from "@/empty"
import { renderComponentScene } from "../test-utils"

function EmptyGallery() {
  return (
    <div data-testid="empty-gallery" className="flex flex-col gap-4">
      <Empty data-testid="empty">
        <EmptyHeader>
          <EmptyTitle>No items found</EmptyTitle>
          <EmptyDescription>Get started by creating a new item</EmptyDescription>
        </EmptyHeader>
      </Empty>

      <Empty>
        <EmptyHeader>
          <EmptyMedia variant="icon">
            <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M20 13V6a2 2 0 00-2-2H6a2 2 0 00-2 2v7m16 0v5a2 2 0 01-2 2H6a2 2 0 01-2-2v-5m16 0h-2.586a1 1 0 00-.707.293l-2.414 2.414a1 1 0 01-.707.293h-3.172a1 1 0 01-.707-.293l-2.414-2.414A1 1 0 006.586 13H4" />
            </svg>
          </EmptyMedia>
          <EmptyTitle>No files yet</EmptyTitle>
          <EmptyDescription>Upload your first file to get started</EmptyDescription>
        </EmptyHeader>
        <EmptyContent>
          <button type="button" className="inline-flex items-center justify-center rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90">
            Upload file
          </button>
        </EmptyContent>
      </Empty>
    </div>
  )
}

describe("Empty browser coverage", () => {
  it("matches the shared empty gallery screenshot", async () => {
    await renderComponentScene(<EmptyGallery />)
    const empty = page.getByTestId("empty")
    await expect.element(empty).toBeVisible()
    await expect(page.getByTestId("empty-gallery")).toMatchScreenshot("empty-gallery.png")
  })

  it("keeps interactive empty state structure covered", async () => {
    await renderComponentScene(
      <Empty data-testid="test-empty">
        <EmptyHeader>
          <EmptyTitle data-testid="empty-title">No results found</EmptyTitle>
          <EmptyDescription data-testid="empty-description">Try adjusting your search terms</EmptyDescription>
        </EmptyHeader>
        <EmptyContent data-testid="empty-content">
          <button type="button" className="rounded-md bg-primary px-4 py-2 text-sm text-primary-foreground">
            Clear filters
          </button>
        </EmptyContent>
      </Empty>
    )
    const empty = page.getByTestId("test-empty")
    await expect.element(empty).toBeVisible()

    const title = page.getByTestId("empty-title")
    await expect.element(title).toBeVisible()
    await expect.element(title).toHaveTextContent("No results found")

    const description = page.getByTestId("empty-description")
    await expect.element(description).toBeVisible()

    const content = page.getByTestId("empty-content")
    await expect.element(content).toBeVisible()

    const button = page.getByRole("button", { name: "Clear filters" })
    await expect.element(button).toBeVisible()
  })
})
