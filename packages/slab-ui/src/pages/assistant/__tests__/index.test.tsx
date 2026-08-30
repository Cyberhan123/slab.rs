import type { UIMessage } from "ai"
import type { ReactNode } from "react"
import { render } from "vitest-browser-react"
import { userEvent } from "vitest/browser"
import { beforeEach, describe, expect, it, vi } from "vitest"

import { MemoryRouter } from "react-router-dom"

import { HeaderProvider } from "@slab/ui/layouts/header-provider"
import Header from "@slab/ui/layouts/header"
import { SlabProvider } from "@slab/ui/provider/slab-provider"
import { createTestSlabPorts } from "@slab/ui/provider/test-ports"

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
  const harnessConversation = {
    activeConversation: undefined as string | undefined,
    error: null as string | null,
    isHistoryLoading: false,
    restoredMessages: [] as UIMessage[],
    restoredThreadId: null as string | null,
    restoreVersion: 1,
    transport: {},
    approvals: [] as Array<Record<string, unknown>>,
    approvalStatusByItemId: new Map<string, "pending" | "approved" | "denied">(),
    liveOutputByItemId: new Map<string, string>(),
    commands: [] as Array<Record<string, unknown>>,
    resolveApproval: vi.fn<
      (itemId: string, approved: boolean, scope: "run_once" | "always_in_workspace" | "always" | "deny") => Promise<void>
    >(),
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
    sendMessage: vi.fn(),
    stop: vi.fn(),
    models,
    setCurrentSessionId: vi.fn(),
    setSelectedModelId: vi.fn(),
    toastInfo: vi.fn(),
    toastError: vi.fn(),
    translate,
    harnessConversation,
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
      stop: mocks.stop,
    }
  }),
}))

vi.mock("../hooks/use-harness-conversation", () => ({
  useHarnessConversation: vi.fn(() => mocks.harnessConversation),
}))

vi.mock("@slab/i18n", () => ({
  // `default` is required by @slab/ui's query-client (i18n.t for error toasts).
  default: { t: mocks.translate },
  DEFAULT_ASSISTANT_LABELS: ["pages.assistant.runtime.newChat"],
  LEGACY_DEFAULT_CHAT_LABELS: ["New Chat"],
  getResolvedAppLanguage: () => "en-US",
  Trans: ({ i18nKey, values }: { i18nKey: string; values?: Record<string, unknown> }) => (
    <span>
      {i18nKey}
      {values ? ` ${Object.values(values).join(" ")}` : ""}
    </span>
  ),
  translateServerField: (_i18n: unknown, _field: unknown, fallback: string) => fallback,
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

// The page's workspace switcher + new-chat dialog query the workspace state
// through React Query; the page test has no QueryClientProvider, so provide
// the two hooks the page uses with inert stand-ins (keeping the real module's
// other exports — e.g. MutationCache used by the provider — intact).
vi.mock("@tanstack/react-query", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@tanstack/react-query")>()
  return {
    ...actual,
    useQuery: () => ({ data: { current: null } }),
    useQueryClient: () => ({
      setQueryData: vi.fn(),
      invalidateQueries: async () => undefined,
    }),
  }
})

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

vi.mock("@slab/ui/hooks/use-ai-model", () => ({
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

vi.mock("@slab/ui/pages/assistant/components/message-list", () => ({
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

function restoredMessages(): UIMessage[] {
  return [
    { id: "message-user", parts: [{ text: "previous prompt", type: "text" }], role: "user" },
    { id: "message-assistant", parts: [{ text: "previous answer", type: "text" }], role: "assistant" },
  ]
}

async function renderAssistant() {
  return render(
    <SlabProvider deps={{ ports: createTestSlabPorts() }}>
      <HeaderProvider>
        <MemoryRouter>
          <Header />
          <Assistant />
        </MemoryRouter>
      </HeaderProvider>
    </SlabProvider>,
  )
}

describe("Assistant page session and model lifecycle", () => {
  beforeEach(() => {
    mocks.createSession.mockResolvedValue({ id: "session-new" })
    mocks.currentMessagesRef.value = []
    mocks.deleteSession.mockResolvedValue(true)
    mocks.ensureDownloaded.mockResolvedValue({ downloadedNow: false })
    mocks.ensureLoaded.mockResolvedValue({ runtimeStatus: null })
    mocks.harnessConversation.restoredMessages = restoredMessages()
    mocks.harnessConversation.restoredThreadId = "thread-a"
    mocks.harnessConversation.activeConversation = "session-a"
    mocks.harnessConversation.isHistoryLoading = false
    mocks.harnessConversation.error = null
    mocks.sendMessage.mockClear()
    mocks.stop.mockClear()
    mocks.setCurrentSessionId.mockClear()
    mocks.setSelectedModelId.mockClear()
    mocks.toastInfo.mockClear()
    mocks.toastError.mockClear()
    mocks.updateSessionLabel.mockResolvedValue(true)
  })

  it("restores the current session and renders restored messages", async () => {
    const screen = await renderAssistant()

    await expect.element(screen.getByText("previous prompt")).toBeInTheDocument()
    await expect.element(screen.getByText("previous answer")).toBeInTheDocument()
  })

  it("opens the session sheet from the header history button and selects sessions", async () => {
    const screen = await renderAssistant()

    await expect.element(screen.getByText("previous answer")).toBeInTheDocument()

    await screen.getByTestId("header-history-control").click()

    await expect.element(screen.getByTestId("assistant-session-sheet")).toBeInTheDocument()
    await screen.getByTestId("assistant-session-select-session-b").click()

    expect(mocks.setCurrentSessionId).toHaveBeenCalledWith("session-b")
  })

  it("prepares the model before sending in a restored session", async () => {
    const screen = await renderAssistant()

    await expect.element(screen.getByText("previous answer")).toBeInTheDocument()
    await userEvent.type(screen.getByLabelText("Message"), "continue restored")
    await userEvent.click(screen.getByRole("button", { name: "Send" }))

    await vi.waitFor(() => {
      expect(mocks.ensureDownloaded).not.toHaveBeenCalled()
    })
    expect(mocks.sendMessage).toHaveBeenCalledWith(
      expect.objectContaining({ text: "continue restored" }),
    )
  })

  it("switches models immediately when the current session has no messages", async () => {
    mocks.harnessConversation.restoredMessages = []

    const screen = await renderAssistant()

    await screen.getByText("Model B").click()

    expect(mocks.setSelectedModelId).toHaveBeenCalledWith("model-b")
    expect(screen.getByText("pages.assistant.dialog.title").query()).toBeNull()
  })

  it("asks how to switch models when the current session has messages", async () => {
    const screen = await renderAssistant()

    await expect.element(screen.getByText("previous answer")).toBeInTheDocument()
    await screen.getByText("Model B").click()

    await expect.element(screen.getByText("pages.assistant.dialog.title")).toBeInTheDocument()

    await screen.getByRole("button", { name: "pages.assistant.dialog.keepTitle" }).click()
    expect(mocks.setSelectedModelId).toHaveBeenCalledWith("model-b")
  })

  it("creates a new session before switching models from a populated session", async () => {
    const screen = await renderAssistant()

    await expect.element(screen.getByText("previous answer")).toBeInTheDocument()
    await screen.getByText("Model B").click()
    await screen.getByRole("button", { name: "pages.assistant.dialog.createTitle" }).click()

    await vi.waitFor(() => expect(mocks.createSession).toHaveBeenCalledWith({ select: true }))
    expect(mocks.setSelectedModelId).toHaveBeenCalledWith("model-b")
  })

  it("blocks model switching while session restore is still busy", async () => {
    mocks.harnessConversation.isHistoryLoading = true

    const screen = await renderAssistant()

    // While history is loading the model picker is disabled, so its options
    // (e.g. "Model B") stay hidden. Browser mode's actionability checks reject
    // clicking a disabled control, so assert the disabled state directly.
    await expect.element(screen.getByRole("button", { name: "model-select" })).toBeDisabled()
    expect(screen.getByText("Model B").query()).toBeNull()

    mocks.harnessConversation.isHistoryLoading = false
  })
})
