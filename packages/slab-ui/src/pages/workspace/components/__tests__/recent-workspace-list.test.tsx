import { userEvent } from "vitest/browser"
import { render } from "vitest-browser-react"
import type { ReactNode } from "react"
import { describe, expect, it, vi } from "vitest"

import type { RecentWorkspace } from "@slab/ui/store/useWorkspaceUiStore"

import { RecentWorkspaceList } from "../recent-workspace-list"

vi.mock("@slab/components/workspace", () => ({
  SoftPanel: ({ children }: { children: ReactNode }) => <div>{children}</div>,
}))

vi.mock("@slab/components/button", () => ({
  Button: ({
    children,
    onClick,
    ...rest
  }: {
    children: ReactNode
    onClick?: () => void
  } & Record<string, unknown>) => (
    <button type="button" onClick={onClick} {...rest}>
      {children}
    </button>
  ),
}))

const workspaces: RecentWorkspace[] = [
  { rootPath: "/repos/alpha", name: "Alpha" },
  { rootPath: "/repos/beta", name: "Beta" },
] as RecentWorkspace[]

describe("RecentWorkspaceList", () => {
  it("shows the empty label when there are no recent workspaces", async () => {
    const screen = await render(
      <RecentWorkspaceList
        recentWorkspaces={[]}
        onOpen={vi.fn<(rootPath: string) => Promise<void>>()}
        title="Recent"
        emptyLabel="Nothing here"
        openLabel="Open"
      />,
    )

    await expect.element(screen.getByText("Nothing here")).toBeInTheDocument()
  })

  it("renders a row per recent workspace", async () => {
    const screen = await render(
      <RecentWorkspaceList
        recentWorkspaces={workspaces}
        onOpen={vi.fn<(rootPath: string) => Promise<void>>()}
        title="Recent"
        emptyLabel="Nothing here"
        openLabel="Open"
      />,
    )

    const rows = screen.getByTestId("recent-workspace-row").all()
    expect(rows).toHaveLength(2)
    await expect.element(rows[0]).toHaveAttribute("data-root-path", "/repos/alpha")
  })

  it("opens a workspace when its button is clicked", async () => {
    const onOpen = vi.fn<(rootPath: string) => Promise<void>>()
    const screen = await render(
      <RecentWorkspaceList
        recentWorkspaces={workspaces}
        onOpen={onOpen}
        title="Recent"
        emptyLabel="Nothing here"
        openLabel="Open"
      />,
    )

    await userEvent.click(screen.getByTestId("recent-workspace-open-button").all()[1])

    expect(onOpen).toHaveBeenCalledExactlyOnceWith("/repos/beta")
  })
})
