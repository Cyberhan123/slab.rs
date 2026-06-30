import { page } from "vitest/browser"
import { describe, expect, it, vi } from "vitest"

import {
  A2uImageSurface,
  A2uPluginSurface,
  A2uReviewSurface,
  A2uWorkspaceSurface,
} from "@/a2u"
import { renderComponentScene } from "../test-utils"

describe("a2u surface browser coverage", () => {
  it("renders workspace and image payloads with action buttons", async () => {
    const openWorkspace = vi.fn<() => void>()
    const openImage = vi.fn<() => void>()

    await renderComponentScene(
      <div className="grid gap-4">
        <A2uWorkspaceSurface
          revealPath="src/main.rs"
          actions={[
            {
              label: "Open workspace",
              onClick: openWorkspace,
              testId: "open-workspace",
            },
          ]}
          labels={{
            description: "Workspace target ready.",
            emptyDescription: "Workspace target missing.",
            revealPath: "Path",
            title: "Workspace",
          }}
        />
        <A2uImageSurface
          prompt="A compact workbench preview"
          actions={[
            {
              label: "Open image",
              onClick: openImage,
              testId: "open-image",
            },
          ]}
          labels={{
            description: "Image target ready.",
            emptyDescription: "Image prompt missing.",
            prompt: "Prompt",
            title: "Image",
          }}
        />
      </div>
    )

    await expect.element(page.getByTestId("a2u-workspace-surface")).toHaveTextContent("src/main.rs")
    await page.getByTestId("open-workspace").click()
    expect(openWorkspace).toHaveBeenCalledOnce()

    await expect.element(page.getByTestId("a2u-image-surface")).toHaveTextContent("A compact workbench preview")
    await page.getByTestId("open-image").click()
    expect(openImage).toHaveBeenCalledOnce()
  })

  it("renders review and plugin metadata without host effects", async () => {
    await renderComponentScene(
      <div className="grid gap-4">
        <A2uReviewSurface
          path="src/lib.rs"
          diff="+ added line"
          labels={{
            description: "Review target ready.",
            diff: "Diff",
            emptyDescription: "Review target missing.",
            path: "Path",
            title: "Review",
          }}
        />
        <A2uPluginSurface
          pluginId="video-subtitle-translator"
          surface="editor"
          labels={{
            description: "Plugin target ready.",
            emptyDescription: "Plugin target missing.",
            pluginId: "Plugin",
            surface: "Surface",
            title: "Plugin",
          }}
        />
      </div>
    )

    await expect.element(page.getByTestId("a2u-review-surface")).toHaveTextContent("src/lib.rs")
    await expect.element(page.getByTestId("a2u-review-surface")).toHaveTextContent("+ added line")
    await expect.element(page.getByTestId("a2u-plugin-surface")).toHaveTextContent("video-subtitle-translator")
    await expect.element(page.getByTestId("a2u-plugin-surface")).toHaveTextContent("editor")
  })
})
