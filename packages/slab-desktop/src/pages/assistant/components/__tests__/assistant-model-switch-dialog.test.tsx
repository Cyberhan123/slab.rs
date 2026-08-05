import type { ReactNode } from "react"
import { userEvent } from "vitest/browser"
import { describe, expect, it, vi } from "vitest"
import { render } from "vitest-browser-react"

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
  it("renders nothing when there is no pending model", async () => {
    const screen = await render(<AssistantModelSwitchDialog {...baseProps({ pendingModelId: null })} />)

    await expect.element(screen.getByTestId("dialog-footer")).not.toBeInTheDocument()
  })

  it("fires onCreateSession when the create button is clicked", async () => {
    const onCreateSession = vi.fn<() => void>()
    const screen = await render(<AssistantModelSwitchDialog {...baseProps({ onCreateSession })} />)

    await userEvent.click(screen.getByRole("button", { name: "pages.assistant.dialog.createTitle" }))

    expect(onCreateSession).toHaveBeenCalledOnce()
  })

  it("fires onKeepSession when the keep button is clicked", async () => {
    const onKeepSession = vi.fn<() => void>()
    const screen = await render(<AssistantModelSwitchDialog {...baseProps({ onKeepSession })} />)

    await userEvent.click(screen.getByRole("button", { name: "pages.assistant.dialog.keepTitle" }))

    expect(onKeepSession).toHaveBeenCalledOnce()
  })

  it("closes (onOpenChange false) when the cancel button is clicked", async () => {
    const onOpenChange = vi.fn<(open: boolean) => void>()
    const screen = await render(<AssistantModelSwitchDialog {...baseProps({ onOpenChange })} />)

    await userEvent.click(screen.getByRole("button", { name: "pages.assistant.dialog.cancel" }))

    expect(onOpenChange).toHaveBeenCalledExactlyOnceWith(false)
  })

  it("disables every action while a session is being created", async () => {
    const screen = await render(<AssistantModelSwitchDialog {...baseProps({ isCreatingSession: true })} />)

    const footer = screen.getByTestId("dialog-footer")
    const buttons = footer.getByRole("button")
    expect(buttons.length).toBe(3)
    expect(buttons.elements().every((b) => (b as HTMLButtonElement).disabled)).toBe(true)
  })
})
