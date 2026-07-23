import { render, screen } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { describe, expect, it, vi } from "vitest"

import { setupSlabI18nMock } from "@slab/test-utils/mocks"

import { WorkspaceSearchPanel } from "../workspace-search-panel"

vi.mock("@slab/i18n", () => setupSlabI18nMock())

function baseProps(overrides: Record<string, unknown> = {}) {
  return {
    activeFilePath: null,
    fileFetching: false,
    fileResults: [],
    fileTruncated: false,
    query: "",
    textFetching: false,
    textResults: [],
    textTruncated: false,
    onOpenFile: vi.fn<(relativePath: string) => Promise<unknown>>(),
    onOpenMatch: vi.fn<(relativePath: string, match: unknown) => Promise<void>>(),
    onQueryChange: vi.fn<(query: string) => void>(),
    ...overrides,
  }
}

describe("WorkspaceSearchPanel", () => {
  it("reports query changes as the user types", async () => {
    const user = userEvent.setup()
    const onQueryChange = vi.fn<(query: string) => void>()
    render(<WorkspaceSearchPanel {...baseProps({ onQueryChange })} />)

    // The input is controlled by the parent; each keystroke emits the new value.
    await user.type(screen.getByLabelText("pages.workspace.search.placeholder"), "x")

    expect(onQueryChange).toHaveBeenCalledExactlyOnceWith("x")
  })

  it("renders file results and opens a file on click", async () => {
    const user = userEvent.setup()
    const onOpenFile = vi.fn<(relativePath: string) => Promise<unknown>>()
    render(
      <WorkspaceSearchPanel
        {...baseProps({
          query: "a",
          fileResults: [{ kind: "file", relativePath: "src/a.ts", name: "a.ts" }] as never,
          onOpenFile,
        })}
      />,
    )

    await user.click(screen.getByRole("button", { name: /a\.ts/ }))

    expect(onOpenFile).toHaveBeenCalledExactlyOnceWith("src/a.ts")
  })

  it("highlights the matched substring and opens the match on click", async () => {
    const user = userEvent.setup()
    const onOpenMatch = vi.fn<(relativePath: string, match: unknown) => Promise<void>>()
    render(
      <WorkspaceSearchPanel
        {...baseProps({
          query: "world",
          textResults: [
            {
              relativePath: "src/b.ts",
              name: "b.ts",
              lineMatches: [{ lineNumber: 1, matchStart: 6, matchEnd: 11, lineText: "hello world" }],
            },
          ] as never,
          onOpenMatch,
        })}
      />,
    )

    expect(screen.getByText("world").tagName).toBe("MARK")

    // Clicking the highlight bubbles to the line-match button.
    await user.click(screen.getByText("world"))

    expect(onOpenMatch).toHaveBeenCalledOnce()
    expect(onOpenMatch.mock.calls[0]?.[0]).toBe("src/b.ts")
  })

  it("clears the query via the clear button", async () => {
    const user = userEvent.setup()
    const onQueryChange = vi.fn<(query: string) => void>()
    render(<WorkspaceSearchPanel {...baseProps({ query: "leftover", onQueryChange })} />)

    await user.click(screen.getByRole("button", { name: "pages.workspace.search.clear" }))

    expect(onQueryChange).toHaveBeenCalledExactlyOnceWith("")
  })

  it("does not render result sections without a query", () => {
    render(<WorkspaceSearchPanel {...baseProps()} />)

    expect(screen.queryByText("pages.workspace.commandPalette.files")).not.toBeInTheDocument()
    expect(screen.queryByText("pages.workspace.textSearch.results")).not.toBeInTheDocument()
  })
})
