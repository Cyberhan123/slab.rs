import { render, screen } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import type { ComponentProps } from "react"
import { describe, expect, it, vi } from "vitest"

import { WorkspaceTreeRow } from "../workspace-tree-row"

type TreeRowProps = ComponentProps<typeof WorkspaceTreeRow>

function buildNode(overrides: Record<string, unknown> = {}) {
  return {
    data: { kind: "directory", relativePath: "src", name: "src", loaded: false },
    isOpen: false,
    select: vi.fn<() => void>(),
    toggle: vi.fn<() => void>(),
    ...overrides,
  }
}

function renderRow(node: ReturnType<typeof buildNode>, extra: Record<string, unknown> = {}) {
  return render(
    <WorkspaceTreeRow
      {...({
        node,
        style: {},
        selectedPath: null,
        loadingPaths: new Set<string>(),
        onOpenDirectory: vi.fn<(relativePath: string) => Promise<unknown>>(),
        onOpenFile: vi.fn<(relativePath: string) => Promise<unknown>>(),
        ...extra,
      } as unknown as TreeRowProps)}
    />,
  )
}

describe("WorkspaceTreeRow", () => {
  it("opens then toggles an unloaded directory", async () => {
    const user = userEvent.setup()
    const onOpenDirectory = vi.fn<(relativePath: string) => Promise<unknown>>()
    const node = buildNode({ data: { kind: "directory", relativePath: "src", name: "src", loaded: false } })
    renderRow(node, { onOpenDirectory })

    await user.click(screen.getByTestId("workspace-tree-row-src"))

    expect(onOpenDirectory).toHaveBeenCalledExactlyOnceWith("src")
    expect(node.toggle).toHaveBeenCalledOnce()
    expect(node.select).toHaveBeenCalledOnce()
  })

  it("only toggles an already-loaded directory", async () => {
    const user = userEvent.setup()
    const onOpenDirectory = vi.fn<(relativePath: string) => Promise<unknown>>()
    const node = buildNode({ data: { kind: "directory", relativePath: "src", name: "src", loaded: true } })
    renderRow(node, { onOpenDirectory })

    await user.click(screen.getByTestId("workspace-tree-row-src"))

    expect(onOpenDirectory).not.toHaveBeenCalled()
    expect(node.toggle).toHaveBeenCalledOnce()
  })

  it("opens a file when clicked", async () => {
    const user = userEvent.setup()
    const onOpenFile = vi.fn<(relativePath: string) => Promise<unknown>>()
    const node = buildNode({
      data: { kind: "file", relativePath: "src/a.ts", name: "a.ts", loaded: true },
    })
    renderRow(node, { onOpenFile })

    await user.click(screen.getByTestId("workspace-tree-row-src-a-ts"))

    expect(onOpenFile).toHaveBeenCalledExactlyOnceWith("src/a.ts")
  })

  it("derives a root test-id for an empty relative path", () => {
    const node = buildNode({ data: { kind: "directory", relativePath: "", name: "root", loaded: true } })
    renderRow(node)

    expect(screen.getByTestId("workspace-tree-row-root")).toBeInTheDocument()
  })
})
