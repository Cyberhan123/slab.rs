import { page } from "vitest/browser"
import { describe, expect, it } from "vitest"

import { StateSurface } from "@/state-surface"
import { renderComponentScene } from "../test-utils"

describe("StateSurface browser coverage", () => {
  it("renders success and aborted states through the shared state contract", async () => {
    await renderComponentScene(
      <div className="grid gap-4">
        <StateSurface
          data-testid="state-success"
          variant="success"
          title="Task complete"
          description="The workspace artifact is ready."
        />
        <StateSurface
          data-testid="state-aborted"
          variant="aborted"
          title="Task interrupted"
          description="The run stopped before producing output."
        />
      </div>
    )

    await expect.element(page.getByTestId("state-success")).toHaveAttribute("data-variant", "success")
    await expect.element(page.getByText("Task complete")).toBeVisible()
    await expect.element(page.getByTestId("state-aborted")).toHaveAttribute("data-variant", "aborted")
    await expect.element(page.getByText("Task interrupted")).toBeVisible()
  })

  it("keeps loading as a status surface", async () => {
    await renderComponentScene(
      <StateSurface
        data-testid="state-loading"
        variant="loading"
        title="Loading"
        description="Preparing the surface."
      />
    )

    await expect.element(page.getByTestId("state-loading")).toHaveAttribute("role", "status")
  })
})
