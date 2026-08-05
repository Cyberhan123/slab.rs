import { fireEvent, render, screen } from "@testing-library/react"
import type { ReactNode } from "react"
import { describe, expect, it, vi } from "vitest"

import { setupSlabI18nMock } from "@slab/test-utils/mocks"

import { MessageInteractionContext } from "../message-interaction-context"
import { MessageItem, type TMessage } from "../message/message-item"

vi.mock("@slab/i18n", () => setupSlabI18nMock())

vi.mock("motion/react", () => ({
  motion: { create: <C,>(component: C): C => component },
  useReducedMotion: () => null,
}))

vi.mock("@mantine/hooks", () => ({
  useClipboard: () => ({ copy: vi.fn<() => void>(), copied: false }),
}))

vi.mock("@/pages/assistant/lib/message-animations", () => ({
  MESSAGE_ANIMATIONS: { "slide-up": { variants: {} } },
}))

vi.mock("@slab/components/message", () => ({
  Message: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  MessageContent: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  MessageHeader: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  MessageAvatar: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  MessageFooter: ({ children }: { children: ReactNode }) => <div>{children}</div>,
}))

vi.mock("@slab/components/message-scroller", () => ({
  MessageScrollerItem: ({ children }: { children: ReactNode }) => (
    <div data-testid="scroller-item">{children}</div>
  ),
}))

vi.mock("@slab/components/bubble", () => ({
  Bubble: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  BubbleContent: ({ children }: { children: ReactNode }) => <div>{children}</div>,
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

vi.mock("@/pages/assistant/components/agent-avatar", () => ({
  default: ({ name }: { name: string }) => <div data-testid="agent-avatar">{name}</div>,
}))
vi.mock("@/pages/assistant/components/user-avatar", () => ({
  default: ({ name }: { name: string }) => <div data-testid="user-avatar">{name}</div>,
}))

vi.mock("../message/message-text-part", () => ({
  default: ({ part }: { part?: { text?: string } }) => (
    <div data-testid="text-part">{part?.text}</div>
  ),
}))
vi.mock("../message/message-reasoning-part", () => ({
  default: () => <div data-testid="reasoning-part" />,
}))
vi.mock("../message/message-fallback-part", () => ({
  default: () => <div data-testid="fallback-part" />,
}))
vi.mock("../message/message-tool-part", () => ({
  default: ({ part }: { part?: { type?: string } }) => (
    <div data-testid="tool-part">{part?.type}</div>
  ),
}))
vi.mock("../message/message-tool-command-part", () => ({
  default: () => <div data-testid="tool-command-part" />,
}))

function message(overrides: Partial<TMessage> = {}): TMessage {
  return {
    id: "m1",
    role: "assistant",
    parts: [{ type: "text", text: "hello" }],
    ...overrides,
  } as TMessage
}

describe("MessageItem", () => {
  it("renders the text part for an assistant message and shows a copy button", () => {
    render(<MessageItem message={message()} />)

    expect(screen.getByTestId("agent-avatar")).toBeInTheDocument()
    expect(screen.getByTestId("text-part")).toHaveTextContent("hello")
    expect(screen.getByRole("button", { name: "Copy" })).toBeInTheDocument()
  })

  it("renders the user avatar for a user message", () => {
    render(<MessageItem message={message({ role: "user", id: "m2" })} />)

    expect(screen.getByTestId("user-avatar")).toBeInTheDocument()
    expect(screen.queryByTestId("agent-avatar")).not.toBeInTheDocument()
  })

  it("routes a tool part to the tool component and hides the copy button when there is no text", () => {
    render(
      <MessageItem
        message={message({ parts: [{ type: "tool-call", state: "input-available" }] })}
      />,
    )

    expect(screen.getByTestId("tool-part")).toHaveTextContent("tool-call")
    expect(screen.queryByRole("button", { name: "Copy" })).not.toBeInTheDocument()
  })

  it("shows a rollback button on a retracable user message and emits the message id", () => {
    const rollbackToMessage = vi.fn()
    render(
      <MessageInteractionContext.Provider
        value={{
          approvalStatusByItemId: new Map(),
          liveOutputByItemId: new Map(),
          livePatchByItemId: new Map(),
          userMessageTurnIndex: new Map([["mu1", 2]]),
          rollbackToMessage,
        }}
      >
        <MessageItem
          message={message({ id: "mu1", role: "user", parts: [{ type: "text", text: "hi" }] })}
        />
      </MessageInteractionContext.Provider>,
    )

    const btn = screen.getByTestId("assistant-message-rollback")
    fireEvent.click(btn)
    expect(rollbackToMessage).toHaveBeenCalledWith("mu1")
  })

  it("hides the rollback button on the first user message (turn 0)", () => {
    render(
      <MessageInteractionContext.Provider
        value={{
          approvalStatusByItemId: new Map(),
          liveOutputByItemId: new Map(),
          livePatchByItemId: new Map(),
          userMessageTurnIndex: new Map([["mu0", 0]]),
          rollbackToMessage: vi.fn(),
        }}
      >
        <MessageItem
          message={message({ id: "mu0", role: "user", parts: [{ type: "text", text: "hi" }] })}
        />
      </MessageInteractionContext.Provider>,
    )

    expect(screen.queryByTestId("assistant-message-rollback")).not.toBeInTheDocument()
  })

  it("hides the rollback button on assistant messages", () => {
    render(
      <MessageInteractionContext.Provider
        value={{
          approvalStatusByItemId: new Map(),
          liveOutputByItemId: new Map(),
          livePatchByItemId: new Map(),
          userMessageTurnIndex: new Map([["ma1", 2]]),
          rollbackToMessage: vi.fn(),
        }}
      >
        <MessageItem message={message({ id: "ma1", role: "assistant" })} />
      </MessageInteractionContext.Provider>,
    )

    expect(screen.queryByTestId("assistant-message-rollback")).not.toBeInTheDocument()
  })
})
