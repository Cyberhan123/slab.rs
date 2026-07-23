import { render, screen } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import type { ReactNode } from "react"
import { describe, expect, it, vi } from "vitest"

// `@slab/test-utils` factory import MUST precede the mocked module imports
// below (vitest hoists `vi.mock`; the factory runs when `@slab/i18n` is first
// imported by the SUT, so its binding must already be initialized).
import { setupSlabI18nMock } from "@slab/test-utils/mocks"

import { AssistantModelSwitchDialog } from "../assistant-model-switch-dialog"

vi.mock("@slab/i18n", () => setupSlabI18nMock())

vi.mock("@slab/components/dialog", () => ({
  Dialog: ({
    open,
    children,
  }: {
    open: boolean
    children: ReactNode
    onOpenChange: (open: boolean) => void
  }) => (open ? <div>{children}</div> : null),
  DialogContent: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  DialogHeader: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  DialogDescription: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  DialogFooter: ({ children }: { children: ReactNode }) => (
    <div data-testid="dialog-footer">{children}</div>
  ),
  DialogTitle: ({ children }: { children: ReactNode }) => <h2>{children}</h2>,
}))

vi.mock("@slab/components/button", () => ({
  Button: ({
    children,
    onClick,
    disabled,
    variant,
    ...rest
  }: {
    children: ReactNode
    onClick?: () => void
    disabled?: boolean
    variant?: string
  } & Record<string, unknown>) => (
    <button type="button" onClick={onClick} disabled={disabled} data-variant={variant} {...rest}>
      {children}
    </button>
  ),
}))

function baseProps(overrides: Record<string, unknown> = {}) {
  return {
    conversationLabel: "Draft",
    isCreatingSession: false,
    messageCount: 3,
    onCreateSession: vi.fn<() => void>(),
    onKeepSession: vi.fn<() => void>(),
    onOpenChange: vi.fn<(open: boolean) => void>(),
    pendingModelId: "model-a",
    pendingModelLabel: "Model A",
    selectedModelLabel: "Model B",
    ...overrides,
  }
}

describe("AssistantModelSwitchDialog", () => {
  it("renders nothing when there is no pending model", () => {
    render(<AssistantModelSwitchDialog {...baseProps({ pendingModelId: null })} />)

    expect(screen.queryByTestId("dialog-footer")).not.toBeInTheDocument()
  })

  it("fires onCreateSession when the create button is clicked", async () => {
    const user = userEvent.setup()
    const onCreateSession = vi.fn<() => void>()
    render(<AssistantModelSwitchDialog {...baseProps({ onCreateSession })} />)

    await user.click(screen.getByRole("button", { name: "pages.assistant.dialog.createTitle" }))

    expect(onCreateSession).toHaveBeenCalledOnce()
  })

  it("fires onKeepSession when the keep button is clicked", async () => {
    const user = userEvent.setup()
    const onKeepSession = vi.fn<() => void>()
    render(<AssistantModelSwitchDialog {...baseProps({ onKeepSession })} />)

    await user.click(screen.getByRole("button", { name: "pages.assistant.dialog.keepTitle" }))

    expect(onKeepSession).toHaveBeenCalledOnce()
  })

  it("closes (onOpenChange false) when the cancel button is clicked", async () => {
    const user = userEvent.setup()
    const onOpenChange = vi.fn<(open: boolean) => void>()
    render(<AssistantModelSwitchDialog {...baseProps({ onOpenChange })} />)

    await user.click(screen.getByRole("button", { name: "pages.assistant.dialog.cancel" }))

    expect(onOpenChange).toHaveBeenCalledExactlyOnceWith(false)
  })

  it("disables every action while a session is being created", () => {
    render(<AssistantModelSwitchDialog {...baseProps({ isCreatingSession: true })} />)

    const footer = screen.getByTestId("dialog-footer")
    const buttons = footer.querySelectorAll("button")
    expect(buttons).toHaveLength(3)
    buttons.forEach((button) => {
      expect(button).toBeDisabled()
    })
  })
})
