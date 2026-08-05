import type { ReactNode } from "react"
import { describe, expect, it, vi } from "vitest"
import { render } from "vitest-browser-react"

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
  // Real consumers (e.g. message-tool-file-change-part) import these named
  // exports; browser native ESM throws if a mock omits a consumed named export,
  // so expose stubs alongside the default.
  Tool: ({ children }: { children?: ReactNode }) => <div>{children}</div>,
  ToolHeader: ({ children }: { children?: ReactNode }) => <div>{children}</div>,
  ToolContent: ({ children }: { children?: ReactNode }) => <div>{children}</div>,
}))
vi.mock("../message/message-tool-command-part", () => ({
  default: () => <div data-testid="tool-command-part" />,
}))
vi.mock("../message/message-tool-file-change-part", () => ({
  default: () => <div data-testid="tool-file-change-part" />,
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
  it("renders the text part for an assistant message and shows a copy button", async () => {
    const screen = await render(<MessageItem message={message()} />)

    await expect.element(screen.getByTestId("agent-avatar")).toBeInTheDocument()
    await expect.element(screen.getByTestId("text-part")).toHaveTextContent("hello")
    await expect.element(screen.getByRole("button", { name: "Copy" })).toBeInTheDocument()
  })

  it("renders the user avatar for a user message", async () => {
    const screen = await render(<MessageItem message={message({ role: "user", id: "m2" })} />)

    await expect.element(screen.getByTestId("user-avatar")).toBeInTheDocument()
    await expect.element(screen.getByTestId("agent-avatar")).not.toBeInTheDocument()
  })

  it("routes a tool part to the tool component and hides the copy button when there is no text", async () => {
    const screen = await render(
      <MessageItem
        message={message({ parts: [{ type: "tool-call", state: "input-available" }] })}
      />,
    )

    await expect.element(screen.getByTestId("tool-part")).toHaveTextContent("tool-call")
    await expect.element(screen.getByRole("button", { name: "Copy" })).not.toBeInTheDocument()
  })

  it("shows a rollback button on a retracable user message and emits the message id", async () => {
    const rollbackToMessage = vi.fn()
    const screen = await render(
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
    await btn.click()
    expect(rollbackToMessage).toHaveBeenCalledWith("mu1")
  })

  it("hides the rollback button on the first user message (turn 0)", async () => {
    const screen = await render(
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

    await expect.element(screen.getByTestId("assistant-message-rollback")).not.toBeInTheDocument()
  })

  it("hides the rollback button on assistant messages", async () => {
    const screen = await render(
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

    await expect.element(screen.getByTestId("assistant-message-rollback")).not.toBeInTheDocument()
  })
})
