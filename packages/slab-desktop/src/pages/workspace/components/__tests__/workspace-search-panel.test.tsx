import { userEvent } from "vitest/browser"
import { render } from "vitest-browser-react"
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
    const onQueryChange = vi.fn<(query: string) => void>()
    const screen = await render(<WorkspaceSearchPanel {...baseProps({ onQueryChange })} />)

    // The input is controlled by the parent; each keystroke emits the new value.
    await userEvent.type(screen.getByLabelText("pages.workspace.search.placeholder"), "x")

    expect(onQueryChange).toHaveBeenCalledExactlyOnceWith("x")
  })

  it("renders file results and opens a file on click", async () => {
    const onOpenFile = vi.fn<(relativePath: string) => Promise<unknown>>()
    const screen = await render(
      <WorkspaceSearchPanel
        {...baseProps({
          query: "a",
          fileResults: [{ kind: "file", relativePath: "src/a.ts", name: "a.ts" }] as never,
          onOpenFile,
        })}
      />,
    )

    await userEvent.click(screen.getByRole("button", { name: /a\.ts/ }))

    expect(onOpenFile).toHaveBeenCalledExactlyOnceWith("src/a.ts")
  })

  it("highlights the matched substring and opens the match on click", async () => {
    const onOpenMatch = vi.fn<(relativePath: string, match: unknown) => Promise<void>>()
    const screen = await render(
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

    expect(screen.getByText("world").element().tagName).toBe("MARK")

    // Clicking the highlight bubbled to the line-match button.
    await userEvent.click(screen.getByText("world"))

    expect(onOpenMatch).toHaveBeenCalledOnce()
    expect(onOpenMatch.mock.calls[0]?.[0]).toBe("src/b.ts")
  })

  it("clears the query via the clear button", async () => {
    const onQueryChange = vi.fn<(query: string) => void>()
    const screen = await render(<WorkspaceSearchPanel {...baseProps({ query: "leftover", onQueryChange })} />)

    await userEvent.click(screen.getByRole("button", { name: "pages.workspace.search.clear" }))

    expect(onQueryChange).toHaveBeenCalledExactlyOnceWith("")
  })

  it("does not render result sections without a query", async () => {
    const screen = await render(<WorkspaceSearchPanel {...baseProps()} />)

    await expect.element(
      screen.getByText("pages.workspace.commandPalette.files"),
    ).not.toBeInTheDocument()
    await expect.element(screen.getByText("pages.workspace.textSearch.results")).not.toBeInTheDocument()
  })
})
