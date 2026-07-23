import { render, screen } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import type { ReactNode } from "react"
import { describe, expect, it, vi } from "vitest"

import { setupSlabI18nMock } from "@slab/test-utils/mocks"

import { AssistantSessionSheet } from "../assistant-session-sheet"

vi.mock("@slab/i18n", () => setupSlabI18nMock())

vi.mock("@slab/components/sheet", () => ({
  Sheet: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  SheetContent: ({
    children,
    ...rest
  }: { children: ReactNode } & Record<string, unknown>) => (
    <div data-testid={rest["data-testid"] ?? "sheet-content"}>{children}</div>
  ),
  SheetHeader: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  SheetDescription: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  SheetTitle: ({ children }: { children: ReactNode }) => <h2>{children}</h2>,
}))

vi.mock("@slab/components/scroll-area", () => ({
  ScrollArea: ({ children }: { children: ReactNode }) => <div>{children}</div>,
}))

vi.mock("@slab/components/badge", () => ({
  Badge: ({ children }: { children: ReactNode }) => <span>{children}</span>,
}))

vi.mock("@slab/components/button", () => ({
  Button: ({
    children,
    onClick,
    disabled,
    ...rest
  }: {
    children: ReactNode
    onClick?: () => void
    disabled?: boolean
  } & Record<string, unknown>) => (
    <button type="button" onClick={onClick} disabled={disabled} {...rest}>
      {children}
    </button>
  ),
}))

vi.mock("@slab/components/dropdown-menu", () => ({
  DropdownMenu: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  DropdownMenuTrigger: ({ children }: { children: ReactNode }) => <>{children}</>,
  DropdownMenuContent: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  DropdownMenuItem: ({
    children,
    onClick,
    disabled,
    ...rest
  }: {
    children: ReactNode
    onClick?: () => void
    disabled?: boolean
  } & Record<string, unknown>) => (
    <button type="button" onClick={onClick} disabled={disabled} {...rest}>
      {children}
    </button>
  ),
}))

const conversations = [
  { key: "k1", label: "First", group: "workspace-a" },
  { key: "k2", label: "Second", group: "workspace-b" },
  { key: "k3" },
]

describe("AssistantSessionSheet", () => {
  it("renders a row per conversation", () => {
    render(
      <AssistantSessionSheet
        open
        onOpenChange={vi.fn<(open: boolean) => void>()}
        conversations={conversations}
        currentConversation="k1"
        onSelect={vi.fn<(key: string) => void>()}
        onDelete={vi.fn<(key: string) => void>()}
      />,
    )

    expect(screen.getByTestId("assistant-session-row-k1")).toBeInTheDocument()
    expect(screen.getByTestId("assistant-session-row-k2")).toBeInTheDocument()
    expect(screen.getByTestId("assistant-session-row-k3")).toBeInTheDocument()
  })

  it("marks only the current conversation with the current badge", () => {
    render(
      <AssistantSessionSheet
        open
        onOpenChange={vi.fn<(open: boolean) => void>()}
        conversations={conversations}
        currentConversation="k2"
        onSelect={vi.fn<(key: string) => void>()}
        onDelete={vi.fn<(key: string) => void>()}
      />,
    )

    const currentRow = screen.getByTestId("assistant-session-row-k2")
    const otherRow = screen.getByTestId("assistant-session-row-k1")
    expect(currentRow.textContent).toContain("pages.assistant.sessionSheet.current")
    expect(otherRow.textContent).not.toContain("pages.assistant.sessionSheet.current")
  })

  it("selects a conversation when its row is clicked", async () => {
    const user = userEvent.setup()
    const onSelect = vi.fn<(key: string) => void>()
    render(
      <AssistantSessionSheet
        open
        onOpenChange={vi.fn<(open: boolean) => void>()}
        conversations={conversations}
        currentConversation="k1"
        onSelect={onSelect}
        onDelete={vi.fn<(key: string) => void>()}
      />,
    )

    await user.click(screen.getByTestId("assistant-session-select-k2"))

    expect(onSelect).toHaveBeenCalledExactlyOnceWith("k2")
  })

  it("deletes a conversation from the actions menu", async () => {
    const user = userEvent.setup()
    const onDelete = vi.fn<(key: string) => void>()
    render(
      <AssistantSessionSheet
        open
        onOpenChange={vi.fn<(open: boolean) => void>()}
        conversations={conversations}
        currentConversation="k1"
        onSelect={vi.fn<(key: string) => void>()}
        onDelete={onDelete}
      />,
    )

    await user.click(screen.getByTestId("assistant-session-delete-k3"))

    expect(onDelete).toHaveBeenCalledExactlyOnceWith("k3")
  })

  it("disables every trigger while busy", () => {
    render(
      <AssistantSessionSheet
        open
        onOpenChange={vi.fn<(open: boolean) => void>()}
        conversations={conversations}
        currentConversation="k1"
        busy
        onSelect={vi.fn<(key: string) => void>()}
        onDelete={vi.fn<(key: string) => void>()}
      />,
    )

    expect(screen.getByTestId("assistant-session-select-k1")).toBeDisabled()
    expect(screen.getByTestId("assistant-session-actions-k1")).toBeDisabled()
  })
})
