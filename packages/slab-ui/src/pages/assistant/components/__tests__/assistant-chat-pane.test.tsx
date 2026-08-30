import type { ReactNode } from "react"
import type { UIMessage } from "ai"
import { beforeEach, describe, expect, it, vi } from "vitest"
import { render } from "vitest-browser-react"

import { AssistantChatPane } from "../assistant-chat-pane"
import type { HarnessChatTransport } from "@slab/core/harness"
import type { ApprovalStatus } from "@slab/core/harness"

// Mutable stand-in for the `useChat` return so each test can set messages/status.
const chatState = vi.hoisted(() => ({
  messages: [] as UIMessage[],
  status: "ready" as string,
  sendMessage: () => undefined,
}))

vi.mock("@ai-sdk/react", () => ({
  useChat: () => ({
    messages: chatState.messages,
    sendMessage: chatState.sendMessage,
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

vi.mock("@slab/ui/pages/assistant/components/message-list", () => ({
  default: ({
    messages,
    queuedTexts,
  }: {
    messages: UIMessage[]
    queuedTexts?: readonly string[]
  }) => (
    <div data-testid="message-list" data-queued={queuedTexts?.length ?? 0}>
      {messages.length} messages
    </div>
  ),
}))

vi.mock("@slab/ui/pages/assistant/components/sender.tsx", () => ({
  default: ({
    approvals,
    loading,
    workspaceSlot,
  }: {
    approvals: unknown[]
    loading: boolean
    workspaceSlot?: ReactNode
  }) => (
    <div
      data-testid="sender"
      data-approvals={approvals.length}
      data-loading={loading ? "true" : "false"}
    >
      sender
      {workspaceSlot ? <div data-testid="sender-workspace-slot">{workspaceSlot}</div> : null}
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

vi.mock("@slab/components/tooltip", () => ({
  Tooltip: ({ children }: { children: ReactNode }) => <>{children}</>,
  TooltipTrigger: ({ children }: { children: ReactNode }) => <>{children}</>,
  TooltipContent: ({ children }: { children: ReactNode }) => <>{children}</>,
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
    livePatchByItemId: new Map<string, string[]>(),
    modelLoad: null,
    turnUsage: null,
    contextWindow: null,
    resolveApproval: vi.fn(),
    onCompact: vi.fn(),
    historyCreatedAt: null,
    commands: [],
    compactionMarkers: [],
    isCompacting: false,
    onFork: vi.fn(),
    isForking: false,
    userMessageTurnIndex: new Map<string, number>(),
    onRollbackFromTurn: vi.fn(),
    planMode: false,
    onPlanModeChange: vi.fn(),
    threadStatus: null,
    abortReason: null,
    queuedTexts: [],
    backgroundTasks: [],
    onSteerSubmit: vi.fn(),
    onInterrupt: vi.fn(),
    ...overrides,
  }
}

describe("AssistantChatPane", () => {
  beforeEach(() => {
    chatState.messages = []
    chatState.status = "ready"
    chatState.sendMessage = vi.fn()
  })

  it("shows the greeting empty state when there are no messages and not loading", async () => {
    const screen = await render(<AssistantChatPane {...baseProps()} />)
    expect(screen.getByTestId("assistant-empty-state").element().textContent).toContain("Hello-test")
    expect(screen.getByTestId("message-list").query()).toBeNull()
  })

  it("renders the new-chat CTA on the empty state when the handler is provided", async () => {
    const onStartNewChat = vi.fn()
    const screen = await render(<AssistantChatPane {...baseProps({ onStartNewChat })} />)
    await expect.element(screen.getByTestId("assistant-new-chat-cta")).toBeInTheDocument()
  })

  it("omits the new-chat CTA when no handler is provided", async () => {
    const screen = await render(<AssistantChatPane {...baseProps()} />)
    expect(screen.getByTestId("assistant-new-chat-cta").query()).toBeNull()
  })

  it("forwards the workspace slot into the Sender toolbar", async () => {
    const screen = await render(
      <AssistantChatPane
        {...baseProps({
          workspaceSlot: <div data-testid="live-workspace-selector" />,
        })}
      />,
    )
    await expect.element(screen.getByTestId("sender-workspace-slot")).toBeInTheDocument()
    expect(screen.getByTestId("live-workspace-selector").query()).not.toBeNull()
  })

  it("auto-sends a staged draft exactly once once the controller is ready", async () => {
    const onBeforeSubmit = vi.fn()
    const onAutoSendConsumed = vi.fn()
    const autoSend = {
      text: "kick off the build",
      files: [],
      metadata: { effort: "high", permissionMode: "default", agentType: undefined },
    }
    await render(
      <AssistantChatPane {...baseProps({ onBeforeSubmit, onAutoSendConsumed, autoSend })} />,
    )
    // Consumed BEFORE sending (claim-then-send), then through the manual path.
    expect(onAutoSendConsumed).toHaveBeenCalledTimes(1)
    expect(onBeforeSubmit).toHaveBeenCalledWith("kick off the build")
    expect(chatState.sendMessage).toHaveBeenCalledWith({
      text: "kick off the build",
      files: [],
      metadata: autoSend.metadata,
    })
  })

  it("does not auto-send while the session is loading or busy", async () => {
    const onAutoSendConsumed = vi.fn()
    const autoSend = { text: "hold", files: [], metadata: {} }
    await render(
      <AssistantChatPane
        {...baseProps({ isHistoryLoading: true, onAutoSendConsumed, autoSend })}
      />,
    )
    expect(onAutoSendConsumed).not.toHaveBeenCalled()
    expect(chatState.sendMessage).not.toHaveBeenCalled()
  })

  it("does not auto-send a busy (streaming) pane", async () => {
    chatState.status = "streaming"
    const onAutoSendConsumed = vi.fn()
    const autoSend = { text: "hold", files: [], metadata: {} }
    await render(<AssistantChatPane {...baseProps({ onAutoSendConsumed, autoSend })} />)
    expect(onAutoSendConsumed).not.toHaveBeenCalled()
    expect(chatState.sendMessage).not.toHaveBeenCalled()
  })

  it("renders the message list (session-load marker) while history is loading with no messages", async () => {
    const screen = await render(<AssistantChatPane {...baseProps({ isHistoryLoading: true })} />)
    // Loading no longer swaps in a full-page Empty; the MessageList renders so
    // the session-load Marker shows in-stream. The hero Empty is reserved for
    // the empty + idle case.
    await expect.element(screen.getByTestId("message-list")).toBeInTheDocument()
    expect(screen.getByTestId("assistant-loading-state").query()).toBeNull()
    expect(screen.getByTestId("assistant-empty-state").query()).toBeNull()
  })

  it("renders the message list once populated", async () => {
    chatState.messages = [
      { id: "m1", role: "user", parts: [{ type: "text", text: "hi" }] },
      { id: "m2", role: "assistant", parts: [{ type: "text", text: "hey" }] },
    ]
    const screen = await render(<AssistantChatPane {...baseProps()} />)
    expect(screen.getByTestId("message-list").element().textContent).toContain("2 messages")
  })

  it("forwards queued steering texts to the message list (ghost bubbles)", async () => {
    chatState.messages = [{ id: "m1", role: "user", parts: [{ type: "text", text: "hi" }] }]
    const screen = await render(
      <AssistantChatPane {...baseProps({ queuedTexts: ["also check the tests"] })} />,
    )
    expect(screen.getByTestId("message-list").element().getAttribute("data-queued")).toBe("1")
    // The footer count chip is gone — the in-stream bubbles replace it.
    expect(screen.getByTestId("assistant-queued-count").query()).toBeNull()
  })

  it("reports the busy state and message count via the effect callbacks", async () => {
    chatState.status = "streaming"
    chatState.messages = [
      { id: "m1", role: "user", parts: [{ type: "text", text: "hi" }] },
    ]
    const onBusyChange = vi.fn()
    const onMessageCountChange = vi.fn()
    const screen = await render(
      <AssistantChatPane
        {...baseProps({ onBusyChange, onMessageCountChange })}
      />,
    )
    expect(onBusyChange).toHaveBeenCalledWith(true)
    expect(onMessageCountChange).toHaveBeenCalledWith(1)
    // Sender reflects the busy flag too.
    expect(screen.getByTestId("sender").element().getAttribute("data-loading")).toBe("true")
  })

  it("forwards approvals + resolveApproval to the Sender", async () => {
    const approvals = [{ itemId: "call-1", status: "pending" }]
    const resolveApproval = vi.fn()
    const screen = await render(<AssistantChatPane {...baseProps({ approvals, resolveApproval })} />)
    const sender = screen.getByTestId("sender")
    expect(sender.element().getAttribute("data-approvals")).toBe("1")
  })

  it("does not render the token-usage indicator before a turn completes", async () => {
    const screen = await render(<AssistantChatPane {...baseProps({ turnUsage: null, contextWindow: 8192 })} />)
    expect(screen.getByTestId("assistant-token-usage").query()).toBeNull()
  })

  it("renders the token-usage percentage label once a turn reports usage", async () => {
    const screen = await render(
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
    await expect.element(indicator).toBeInTheDocument()
    // Percentage label rendered (i18n mock returns the key verbatim; 2048/8192 = 25%).
    expect(indicator.element().textContent).toContain("pages.assistant.usage.used")
    // The consumption bar has been removed.
    expect(screen.getByTestId("assistant-token-usage-bar").query()).toBeNull()
  })
})
