import { page } from "vitest/browser"
import { describe, expect, it } from "vitest"

import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/tabs"

import { renderComponentScene } from "../test-utils"

function TabsShowcase() {
  return (
    <div
      data-testid="tabs-showcase"
      className="w-full max-w-3xl rounded-[28px] border border-border/60 bg-surface-1 p-6 shadow-sm"
    >
      <Tabs defaultValue="overview">
        <TabsList variant="line" aria-label="Model workspace sections">
          <TabsTrigger value="overview">Overview</TabsTrigger>
          <TabsTrigger value="downloads">Downloads</TabsTrigger>
          <TabsTrigger value="history">History</TabsTrigger>
        </TabsList>
        <TabsContent value="overview" className="pt-4 text-sm text-muted-foreground">
          Track the current runtime summary and installed payload state.
        </TabsContent>
        <TabsContent value="downloads" className="pt-4 text-sm text-muted-foreground">
          Review background downloads and verify their current progress.
        </TabsContent>
        <TabsContent value="history" className="pt-4 text-sm text-muted-foreground">
          Inspect recent actions before reopening a workspace.
        </TabsContent>
      </Tabs>
    </div>
  )
}

describe("Tabs browser coverage", () => {
  it("switches the active tab and captures a stable visual baseline", async () => {
    await renderComponentScene(<TabsShowcase />)

    const downloadsTab = page.getByRole("tab", { name: "Downloads" })
    await downloadsTab.click()

    await expect.element(
      page.getByText("Review background downloads and verify their current progress.")
    ).toBeVisible()
    await expect(page.getByTestId("tabs-showcase")).toMatchScreenshot("tabs-downloads.png")
  })
})
