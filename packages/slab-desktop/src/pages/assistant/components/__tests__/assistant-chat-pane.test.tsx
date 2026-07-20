import { cleanup, render, screen } from "@testing-library/react"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"
import type { ReactNode } from "react"
import type { UIMessage } from "ai"

import { AssistantChatPane } from "../assistant-chat-pane"
import type { HarnessChatTransport } from "../../lib/harness"
import type { ApprovalStatus } from "../../hooks/use-harness-conversation"

// Mutable stand-in for the `useChat` return so each test can set messages/status.
const chatState = vi.hoisted(() => ({
  messages: [] as UIMessage[],
  status: "ready" as string,
}))

vi.mock("@ai-sdk/react", () => ({
  useChat: () => ({
    messages: chatState.messages,
    sendMessage: vi.fn(),
    status: chatState.status,
    stop: vi.fn(),
  }),
}))

vi.mock("@slab/i18n", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}))

vi.mock("../../hooks/use-greeting", () => ({
  useGreeting: () => "Hello-test",
}))

vi.mock("@/pages/assistant/components/message/index.tsx", () => ({
  default: ({ messages }: { messages: UIMessage[] }) => (
    <div data-testid="message-list">{messages.length} messages</div>
  ),
}))

vi.mock("@/pages/assistant/components/sender.tsx", () => ({
  default: ({
    approvals,
    loading,
  }: {
    approvals: unknown[]
    loading: boolean
  }) => (
    <div
      data-testid="sender"
      data-approvals={approvals.length}
      data-loading={loading ? "true" : "false"}
    >
      sender
    </div>
  ),
}))

vi.mock("@slab/components/card", () => ({
  Card: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  CardContent: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  CardFooter: ({ children }: { children: ReactNode }) => <div>{children}</div>,
}))

vi.mock("@slab/components/empty", () => ({
  Empty: ({ children, ...rest }: { children: ReactNode } & Record<string, unknown>) => (
    <div data-testid={rest["data-testid"]}>{children}</div>
  ),
  EmptyHeader: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  EmptyMedia: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  EmptyTitle: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  EmptyDescription: ({ children }: { children: ReactNode }) => <div>{children}</div>,
}))

vi.mock("@slab/components/message-scroller", () => ({
  MessageScrollerProvider: ({ children }: { children: ReactNode }) => <div>{children}</div>,
}))

function baseProps(overrides: Record<string, unknown> = {}) {
  return {
    disabled: false,
    initialMessages: [],
    isHistoryLoading: false,
    modelStatusLabel: "model: ready",
    onBeforeSubmit: vi.fn(),
    onBusyChange: vi.fn(),
    onMessageCountChange: vi.fn(),
    transport: {} as unknown as HarnessChatTransport<UIMessage>,
    approvals: [],
    approvalStatusByItemId: new Map<string, ApprovalStatus>(),
    liveOutputByItemId: new Map<string, string>(),
    modelLoad: null,
    turnUsage: null,
    contextWindow: null,
    resolveApproval: vi.fn(),
    ...overrides,
  }
}

describe("AssistantChatPane", () => {
  beforeEach(() => {
    chatState.messages = []
    chatState.status = "ready"
  })
  afterEach(() => cleanup())

  it("shows the greeting empty state when there are no messages and not loading", () => {
    render(<AssistantChatPane {...baseProps()} />)
    expect(screen.getByTestId("assistant-empty-state").textContent).toContain("Hello-test")
    expect(screen.queryByTestId("message-list")).toBeNull()
  })

  it("shows the loading state while history is loading with no messages", () => {
    render(<AssistantChatPane {...baseProps({ isHistoryLoading: true })} />)
    expect(screen.getByTestId("assistant-loading-state")).toBeTruthy()
    expect(screen.queryByTestId("message-list")).toBeNull()
  })

  it("renders the message list once populated", () => {
    chatState.messages = [
      { id: "m1", role: "user", parts: [{ type: "text", text: "hi" }] },
      { id: "m2", role: "assistant", parts: [{ type: "text", text: "hey" }] },
    ]
    render(<AssistantChatPane {...baseProps()} />)
    expect(screen.getByTestId("message-list").textContent).toContain("2 messages")
  })

  it("reports the busy state and message count via the effect callbacks", () => {
    chatState.status = "streaming"
    chatState.messages = [
      { id: "m1", role: "user", parts: [{ type: "text", text: "hi" }] },
    ]
    const onBusyChange = vi.fn()
    const onMessageCountChange = vi.fn()
    render(
      <AssistantChatPane
        {...baseProps({ onBusyChange, onMessageCountChange })}
      />,
    )
    expect(onBusyChange).toHaveBeenCalledWith(true)
    expect(onMessageCountChange).toHaveBeenCalledWith(1)
    // Sender reflects the busy flag too.
    expect(screen.getByTestId("sender").getAttribute("data-loading")).toBe("true")
  })

  it("forwards approvals + resolveApproval to the Sender", () => {
    const approvals = [{ itemId: "call-1", status: "pending" }]
    const resolveApproval = vi.fn()
    render(<AssistantChatPane {...baseProps({ approvals, resolveApproval })} />)
    const sender = screen.getByTestId("sender")
    expect(sender.getAttribute("data-approvals")).toBe("1")
  })

  it("does not render the token-usage indicator before a turn completes", () => {
    render(<AssistantChatPane {...baseProps({ turnUsage: null, contextWindow: 8192 })} />)
    expect(screen.queryByTestId("assistant-token-usage")).toBeNull()
  })

  it("renders the token-usage indicator + context bar once a turn reports usage", () => {
    render(
      <AssistantChatPane
        {...baseProps({
          turnUsage: {
            promptTokens: 2048,
            completionTokens: 128,
            totalTokens: 2176,
            cachedTokens: 512,
          },
          contextWindow: 8192,
        })}
      />,
    )
    const indicator = screen.getByTestId("assistant-token-usage")
    expect(indicator).toBeTruthy()
    // Consumption bar present and proportionate (2048/8192 = 25%).
    const bar = screen.getByTestId("assistant-token-usage-bar")
    expect(bar.getAttribute("aria-valuenow")).toBe("25")
  })
})
