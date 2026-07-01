import { describe, expect, it, vi } from "vitest"
import { cleanup, fireEvent, render, screen } from "@testing-library/react"

import { ProjectSwitcherView } from "../project-switcher"

const labels = { toggle: "Switch workspace", noActive: "No workspace" }

describe("ProjectSwitcherView", () => {
  it("lists recent workspaces and fires onSwitch with the root path", () => {
    const onSwitch = vi.fn<(rootPath: string) => void>()
    render(
      <ProjectSwitcherView
        activeName="Slab"
        labels={labels}
        recentWorkspaces={[
          { rootPath: "repo-a", name: "Alpha" },
          { rootPath: "repo-b", name: "Beta" },
        ]}
        onSwitch={onSwitch}
      />
    )

    fireEvent.click(screen.getByLabelText("Switch workspace"))
    fireEvent.click(screen.getByTestId("project-switcher-item-repo-b"))

    expect(onSwitch).toHaveBeenCalledExactlyOnceWith("repo-b")
    cleanup()
  })

  it("toggles the listbox aria state and shows the active name", () => {
    render(
      <ProjectSwitcherView
        activeName="Active"
        labels={labels}
        recentWorkspaces={[]}
        onSwitch={() => {}}
      />
    )

    const toggle = screen.getByLabelText("Switch workspace")
    expect(toggle.getAttribute("aria-expanded")).toBe("false")
    expect(screen.getByText("Active")).toBeDefined()
    fireEvent.click(toggle)
    expect(toggle.getAttribute("aria-expanded")).toBe("true")
    // No recent workspaces ⇒ the listbox is not rendered.
    expect(screen.queryByTestId("project-switcher-list")).toBeNull()
    cleanup()
  })
})
