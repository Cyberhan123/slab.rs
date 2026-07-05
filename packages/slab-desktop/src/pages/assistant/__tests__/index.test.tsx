import type { UIMessage } from "ai"
import type { ReactNode } from "react"
import { fireEvent, render, screen, waitFor } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { beforeEach, describe, expect, it, vi } from "vitest"

import { HeaderProvider } from "@/layouts/header-provider"
import Header from "@/layouts/header"

const mocks = vi.hoisted(() => {
  let currentMessages: UIMessage[] = []
  const translate = (key: string, values?: Record<string, unknown>) =>
    values ? `${key} ${Object.values(values).join(" ")}` : key
  const conversationList = [
    { group: "Workspace", key: "session-a", label: "Session A" },
    { group: "Workspace", key: "session-b", label: "Session B" },
  ]
  const models = [
    {
      chat_capabilities: null,
      display_name: "Model A",
      id: "model-a",
      kind: "cloud",
      local_path: null,
      pending: false,
      runtime_presets: null,
      spec: { context_window: 4096 },
      status: "ready",
    },
    {
      chat_capabilities: null,
      display_name: "Model B",
      id: "model-b",
      kind: "cloud",
      local_path: null,
      pending: false,
      runtime_presets: null,
      spec: { context_window: 8192 },
      status: "ready",
    },
  ]
  const restoreMutation = {
    isPending: false,
    mutateAsync: vi.fn(),
  }

  return {
    conversationList,
    createSession: vi.fn<() => Promise<{ id: string } | null>>(),
    currentMessagesRef: {
      get value() {
        return currentMessages
      },
      set value(messages: UIMessage[]) {
        currentMessages = messages
      },
    },
    deleteSession: vi.fn<() => Promise<boolean>>(),
    ensureDownloaded: vi.fn<() => Promise<{ downloadedNow: boolean }>>(),
    ensureLoaded: vi.fn<() => Promise<{ runtimeStatus: null }>>(),
    mutateAsync: restoreMutation.mutateAsync,
    sendMessage: vi.fn(),
    models,
    setCurrentSessionId: vi.fn(),
    setSelectedModelId: vi.fn(),
    toastInfo: vi.fn(),
    toastError: vi.fn(),
    translate,
    restoreMutation,
    updateSessionLabel: vi.fn<() => Promise<boolean>>(),
  }
})

vi.mock("@ai-sdk/react", () => ({
  useChat: vi.fn(({ messages = [] }: { messages?: UIMessage[] }) => {
    mocks.currentMessagesRef.value = messages

    return {
      messages,
      sendMessage: mocks.sendMessage,
      status: "ready",
    }
  }),
}))

vi.mock("@slab/api", () => ({
  default: {
    useMutation: vi.fn(() => mocks.restoreMutation),
  },
}))

vi.mock("@slab/i18n", () => ({
  DEFAULT_ASSISTANT_LABELS: ["pages.assistant.runtime.newChat"],
  LEGACY_DEFAULT_CHAT_LABELS: ["New Chat"],
  getResolvedAppLanguage: () => "en-US",
  Trans: ({ i18nKey, values }: { i18nKey: string; values?: Record<string, unknown> }) => (
    <span>
      {i18nKey}
      {values ? ` ${Object.values(values).join(" ")}` : ""}
    </span>
  ),
  useTranslation: () => ({
    t: mocks.translate,
  }),
}))

vi.mock("sonner", () => ({
  toast: {
    error: mocks.toastError,
    info: mocks.toastInfo,
    message: vi.fn(),
    success: vi.fn(),
  },
}))

vi.mock("@slab/components/select", () => {
  let selectValueChange: ((value: string) => void) | undefined

  return {
    Select: ({
      children,
      disabled,
      onValueChange,
      value,
    }: {
      children: ReactNode
      disabled?: boolean
      onValueChange: (value: string) => void
      value: string
    }) => {
      selectValueChange = onValueChange

      return (
        <div data-disabled={disabled ? "true" : undefined} data-value={value}>
          <button aria-label="model-select" disabled={disabled} type="button">
            {value}
          </button>
          {disabled ? null : <div data-testid="mock-select-options">{children}</div>}
        </div>
      )
    },
    SelectContent: ({ children }: { children: ReactNode }) => <div>{children}</div>,
    SelectGroup: ({ children }: { children: ReactNode }) => <div>{children}</div>,
    SelectItem: ({
      children,
      disabled,
      value,
    }: {
      children: ReactNode
      disabled?: boolean
      value: string
    }) => (
      <button
        data-value={value}
        disabled={disabled}
        onClick={() => selectValueChange?.(value)}
        type="button"
      >
        {children}
      </button>
    ),
    SelectLabel: ({ children }: { children: ReactNode }) => <span>{children}</span>,
    SelectTrigger: ({ children }: { children: ReactNode }) => <div>{children}</div>,
    SelectValue: ({ placeholder }: { placeholder?: string }) => <span>{placeholder}</span>,
  }
})

vi.mock("@slab/components/dialog", () => ({
  Dialog: ({
    children,
    open,
  }: {
    children: ReactNode
    open?: boolean
    onOpenChange?: (open: boolean) => void
  }) => (open ? <div>{children}</div> : null),
  DialogContent: ({ children }: { children: ReactNode; showCloseButton?: boolean }) => (
    <div role="dialog">{children}</div>
  ),
  DialogDescription: ({ children }: { children: ReactNode }) => <p>{children}</p>,
  DialogFooter: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  DialogHeader: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  DialogTitle: ({ children }: { children: ReactNode }) => <h2>{children}</h2>,
}))

vi.mock("@slab/components/sheet", () => ({
  Sheet: ({
    children,
    open,
  }: {
    children: ReactNode
    open?: boolean
    onOpenChange?: (open: boolean) => void
  }) => (open ? <div>{children}</div> : null),
  SheetContent: ({ children, ...props }: { children: ReactNode }) => <div {...props}>{children}</div>,
  SheetDescription: ({ children }: { children: ReactNode }) => <p>{children}</p>,
  SheetHeader: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  SheetTitle: ({ children }: { children: ReactNode }) => <h2>{children}</h2>,
}))

vi.mock("@slab/components/scroll-area", () => ({
  ScrollArea: ({ children }: { children: ReactNode }) => <div>{children}</div>,
}))

vi.mock("@slab/components/dropdown-menu", () => ({
  DropdownMenu: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  DropdownMenuContent: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  DropdownMenuGroup: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  DropdownMenuItem: ({
    children,
    disabled,
    onClick,
  }: {
    children: ReactNode
    disabled?: boolean
    onClick?: () => void
  }) => (
    <button disabled={disabled} onClick={onClick} type="button">
      {children}
    </button>
  ),
  DropdownMenuLabel: ({ children }: { children: ReactNode }) => <span>{children}</span>,
  DropdownMenuSeparator: () => <hr />,
  DropdownMenuTrigger: ({ children }: { children: ReactNode }) => <>{children}</>,
}))

vi.mock("@/hooks/use-ai-model", () => ({
  useAiModel: vi.fn(() => ({
    ensureDownloaded: mocks.ensureDownloaded,
    ensureLoaded: mocks.ensureLoaded,
    loading: false,
    localModels: [],
    models: mocks.models,
    selectedId: "model-a",
    setSelectedId: mocks.setSelectedModelId,
    status: { busy: false },
  })),
}))

vi.mock("../hooks/use-assistant-sessions", () => ({
  useAssistantSessions: vi.fn(() => ({
    conversationList: mocks.conversationList,
    createSession: mocks.createSession,
    currentSessionId: "session-a",
    deleteSession: mocks.deleteSession,
    isCreatingSession: false,
    isDeletingSession: false,
    isSessionMutating: false,
    isSessionsLoading: false,
    setCurrentSessionId: mocks.setCurrentSessionId,
    updateSessionLabel: mocks.updateSessionLabel,
  })),
}))

vi.mock("@/pages/assistant/components/message/index.tsx", () => ({
  default: ({ messages }: { messages: UIMessage[] }) => (
    <div data-testid="assistant-message-list">
      {messages.map((message) => (
        <div key={message.id}>
          {message.parts
            .filter((part) => part.type === "text")
            .map((part) => part.text)
            .join("")}
        </div>
      ))}
    </div>
  ),
}))

import Assistant from "../index"

function restoredSessionResponse(sessionId: string, threadId: string) {
  return {
    messages: [
      {
        content: "previous prompt",
        created_at: "2026-07-05T00:00:00Z",
        id: "message-user",
        role: "user",
        sequence_number: 1,
        thread_id: threadId,
      },
      {
        content: "previous answer",
        created_at: "2026-07-05T00:00:01Z",
        id: "message-assistant",
        role: "assistant",
        sequence_number: 2,
        thread_id: threadId,
      },
    ],
    session_id: sessionId,
    thread: {
      id: threadId,
      session_id: sessionId,
      status: "completed",
    },
    type: "agent.session.restored",
  }
}

function renderAssistant() {
  return render(
    <HeaderProvider>
      <Header />
      <Assistant />
    </HeaderProvider>,
  )
}

describe("Assistant page session and model lifecycle", () => {
  beforeEach(() => {
    mocks.createSession.mockResolvedValue({ id: "session-new" })
    mocks.currentMessagesRef.value = []
    mocks.deleteSession.mockResolvedValue(true)
    mocks.ensureDownloaded.mockResolvedValue({ downloadedNow: false })
    mocks.ensureLoaded.mockResolvedValue({ runtimeStatus: null })
    mocks.mutateAsync.mockResolvedValue(restoredSessionResponse("session-a", "thread-a"))
    mocks.sendMessage.mockClear()
    mocks.setCurrentSessionId.mockClear()
    mocks.setSelectedModelId.mockClear()
    mocks.toastInfo.mockClear()
    mocks.toastError.mockClear()
    mocks.updateSessionLabel.mockResolvedValue(true)
  })

  it("restores the current session and renders restored messages", async () => {
    renderAssistant()

    await waitFor(() =>
      expect(mocks.mutateAsync).toHaveBeenCalledWith({
        body: expect.objectContaining({
          session_id: "session-a",
          type: "agent.session.restore",
        }),
      }),
    )

    expect(await screen.findByText("previous prompt")).toBeInTheDocument()
    expect(screen.getByText("previous answer")).toBeInTheDocument()
  })

  it("opens the session sheet from the header history button and selects sessions", async () => {
    renderAssistant()

    await screen.findByText("previous answer")

    fireEvent.click(screen.getByTestId("header-history-control"))

    expect(screen.getByTestId("assistant-session-sheet")).toBeInTheDocument()
    fireEvent.click(screen.getByTestId("assistant-session-select-session-b"))

    expect(mocks.setCurrentSessionId).toHaveBeenCalledWith("session-b")
  })

  it("prepares the model before sending in a restored session", async () => {
    const user = userEvent.setup()
    renderAssistant()

    await screen.findByText("previous answer")
    await user.type(screen.getByLabelText("Message"), "continue restored")
    await user.click(screen.getByRole("button", { name: "Send" }))

    await waitFor(() => expect(mocks.ensureDownloaded).not.toHaveBeenCalled())
    expect(mocks.sendMessage).toHaveBeenCalledWith({ text: "continue restored" })
  })

  it("switches models immediately when the current session has no messages", async () => {
    mocks.mutateAsync.mockResolvedValue({
      messages: [],
      session_id: "session-a",
      thread: null,
      type: "agent.session.restored",
    })

    renderAssistant()

    await waitFor(() => expect(mocks.mutateAsync).toHaveBeenCalled())
    fireEvent.click(screen.getByText("Model B"))

    expect(mocks.setSelectedModelId).toHaveBeenCalledWith("model-b")
    expect(screen.queryByText("pages.assistant.dialog.title")).not.toBeInTheDocument()
  })

  it("asks how to switch models when the current session has messages", async () => {
    renderAssistant()

    await screen.findByText("previous answer")
    fireEvent.click(screen.getByText("Model B"))

    expect(screen.getByText("pages.assistant.dialog.title")).toBeInTheDocument()

    fireEvent.click(screen.getByRole("button", { name: "pages.assistant.dialog.keepTitle" }))
    expect(mocks.setSelectedModelId).toHaveBeenCalledWith("model-b")
  })

  it("creates a new session before switching models from a populated session", async () => {
    renderAssistant()

    await screen.findByText("previous answer")
    fireEvent.click(screen.getByText("Model B"))
    fireEvent.click(screen.getByRole("button", { name: "pages.assistant.dialog.createTitle" }))

    await waitFor(() => expect(mocks.createSession).toHaveBeenCalledWith({ select: true }))
    expect(mocks.setSelectedModelId).toHaveBeenCalledWith("model-b")
  })

  it("blocks model switching while session restore is still busy", async () => {
    let resolveRestore: (value: unknown) => void = () => {}
    mocks.mutateAsync.mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveRestore = resolve
        }),
    )

    renderAssistant()

    fireEvent.click(screen.getByRole("button", { name: "model-select" }))

    expect(screen.queryByText("Model B")).not.toBeInTheDocument()

    resolveRestore(restoredSessionResponse("session-a", "thread-a"))
  })
})
