import { describe, expect, it, vi } from "vitest"
import { render } from "vitest-browser-react"
import { userEvent } from "vitest/browser"


import { isTauri } from "@slab/ui/hooks/use-tauri"

import { OpenWorkspaceButton } from "../open-workspace-button"

vi.mock("@slab/i18n", async () => {
  const { setupSlabI18nMock } = await import("@slab/test-utils/mocks")
  return setupSlabI18nMock()
})

vi.mock("@slab/ui/hooks/use-tauri", () => ({
  isTauri: vi.fn(),
}))

describe("OpenWorkspaceButton", () => {
  it("renders only the folder button in Tauri and opens the native dialog", async () => {
    vi.mocked(isTauri).mockReturnValue(true)
    const onOpenFolder = vi.fn()
    const screen = await render(
      <OpenWorkspaceButton onOpenFolder={onOpenFolder} onOpenWorkspacePath={vi.fn()} />,
    )

    // No duplicate always-visible path form in the Tauri shell.
    expect(screen.getByTestId("workspace-path-input").query()).toBeNull()
    expect(screen.getByTestId("workspace-open-path-button").query()).toBeNull()

    await userEvent.click(screen.getByTestId("workspace-open-folder-button"))
    expect(onOpenFolder).toHaveBeenCalledOnce()
  })

  it("reveals the path popover on click in the browser and opens the typed path", async () => {
    vi.mocked(isTauri).mockReturnValue(false)
    const onOpenWorkspacePath = vi.fn<(rootPath: string) => Promise<void>>()
    const screen = await render(
      <OpenWorkspaceButton onOpenFolder={vi.fn()} onOpenWorkspacePath={onOpenWorkspacePath} />,
    )

    // Path input is hidden until the single folder button is clicked.
    expect(screen.getByTestId("workspace-path-input").query()).toBeNull()

    await userEvent.click(screen.getByTestId("workspace-open-folder-button"))
    await userEvent.fill(screen.getByTestId("workspace-path-input"), "/repos/alpha")
    await userEvent.click(screen.getByTestId("workspace-open-path-button"))

    expect(onOpenWorkspacePath).toHaveBeenCalledExactlyOnceWith("/repos/alpha")
  })
})
